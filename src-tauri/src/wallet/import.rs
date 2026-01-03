use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use vecno_wallet_core::prelude::*;
use crate::models::ImportWalletInput;
use vecno_wallet_core::storage::local::{Storage, Payload};
use vecno_wallet_core::storage::interface::CreateArgs;
use vecno_wallet_core::wallet::args::{AccountCreateArgsBip32, PrvKeyDataCreateArgs};
use vecno_wallet_core::storage::local::WalletStorage;
use vecno_wallet_core::storage::keydata::PrvKeyDataVariantKind;
use bip39::Mnemonic;
use log::{error, info};
use std::sync::Arc;
use vecno_wrpc_client::prelude::{Resolver, WrpcEncoding, ConnectOptions, ConnectStrategy};
use vecno_consensus_core::network::{NetworkId, NetworkType};
use vecno_wallet_core::settings::application_folder;

#[command]
pub async fn import_wallets(
    input: ImportWalletInput,
    state: State<'_, AppState>,
) -> Result<String, ErrorResponse> {
    let mnemonic = input.mnemonic.trim().to_string();
    let secret = input.secret;
    let payment_secret = input.payment_secret;
    let filename = input.filename;

    if secret.is_empty() {
        return Err(ErrorResponse { error: "Wallet password is required".into() });
    }
    if mnemonic.is_empty() {
        return Err(ErrorResponse { error: "Mnemonic is required".into() });
    }
    if filename.is_empty() {
        return Err(ErrorResponse { error: "Wallet filename is required".into() });
    }

    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    if words.len() != 12 && words.len() != 24 {
        return Err(ErrorResponse { error: "Mnemonic must be exactly 12 or 24 words".into() });
    }

    let mnemonic = Mnemonic::parse(&mnemonic).map_err(|e| {
        error!("Invalid mnemonic: {}", e);
        ErrorResponse { error: "Invalid mnemonic format".into() }
    })?;

    let passphrase = payment_secret.as_deref().unwrap_or("").trim();
    let _ = mnemonic.to_seed(passphrase);

    let network_id = NetworkId::new(NetworkType::Mainnet);
    let wallet_dir = application_folder().map_err(|e| ErrorResponse { error: e.to_string() })?;
    let storage_path = wallet_dir.join(&filename);

    let storage = Storage::try_new(
        storage_path.to_str().ok_or_else(|| ErrorResponse { error: "Invalid path".into() })?
    ).map_err(|e| ErrorResponse { error: e.to_string() })?;

    let store = Wallet::local_store().map_err(|e| ErrorResponse { error: e.to_string() })?;
    let wallet_secret = Secret::new(secret.as_bytes().to_vec());

    let create_args = CreateArgs {
        title: Some("Imported Wallet".into()),
        filename: Some(storage_path.to_str().ok_or_else(|| ErrorResponse { error: "Invalid path".into() })?.to_string()),
        encryption_kind: EncryptionKind::XChaCha20Poly1305,
        user_hint: None,
        overwrite_wallet: true,
    };

    store.create(&wallet_secret, create_args).await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    let resolver = Resolver::default();
    let wallet = Arc::new(
        Wallet::try_new(store.clone(), Some(resolver.clone()), Some(network_id))
            .map_err(|e| ErrorResponse { error: format!("Wallet init failed: {}", e) })?
    );

    if let Some(wrpc_client) = wallet.try_wrpc_client().as_ref() {
        let url = resolver.get_url(WrpcEncoding::Borsh, network_id).await
            .map_err(|e| ErrorResponse { error: format!("Node resolve failed: {}", e) })?;

        let options = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(url),
            ..Default::default()
        };
        wrpc_client.connect(Some(options)).await
            .map_err(|e| {
                error!("Node connection failed: {}", e);
                ErrorResponse {
                    error: "Failed to connect to Vecno node. Check your internet connection or try again later.".into()
                }
            })?;
    } else {
        return Err(ErrorResponse { error: "No wRPC client".into() });
    }

    if !wallet.is_open() {
        return Err(ErrorResponse { error: "Wallet not open after init".into() });
    }

    let stored_payment_secret = (!passphrase.is_empty()).then(|| Secret::new(passphrase.as_bytes().to_vec()));

    let prv_key_data = PrvKeyDataCreateArgs {
        name: None,
        payment_secret: stored_payment_secret.clone(),
        secret: Secret::new(mnemonic.to_string().into_bytes()),
        kind: PrvKeyDataVariantKind::Mnemonic,
    };

    let key_id = wallet.create_prv_key_data(&wallet_secret, prv_key_data).await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    let account_args = AccountCreateArgsBip32 {
        account_name: Some("default-account".into()),
        account_index: None,
    };

    let account = wallet
        .create_account_bip32(&wallet_secret, key_id, stored_payment_secret.as_ref(), account_args)
        .await
        .map_err(|e| ErrorResponse { error: format!("Account creation failed: {}", e) })?;

    wallet.select(Some(&account)).await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    account.start().await
        .map_err(|e| ErrorResponse { error: format!("Account start failed: {}", e) })?;

    let payload = Payload::new(vec![], vec![], vec![]);
    let wallet_storage = WalletStorage::try_new(
        Some("Imported Wallet".into()),
        None,
        &wallet_secret,
        EncryptionKind::XChaCha20Poly1305,
        payload,
        vec![],
    ).map_err(|e| ErrorResponse { error: e.to_string() })?;

    wallet_storage.try_store(&storage).await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    {
        let mut wallet_state = state.wallet.lock().await;
        let mut resolver_state = state.resolver.lock().await;
        let mut secret_state = state.wallet_secret.lock().await;
        let mut mnemonic_state = state.mnemonic.lock().await;

        *wallet_state = Some(wallet.clone());
        *resolver_state = Some(resolver);
        *secret_state = Some(wallet_secret);
        *mnemonic_state = Some(mnemonic.to_string());
    }

    info!("Wallet imported successfully at {}", storage_path.display());
    Ok(format!("Success: Wallet imported at {}", storage_path.display()))
}