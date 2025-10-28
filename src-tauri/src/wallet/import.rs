use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use vecno_wallet_core::prelude::*;
use vecno_wallet_core::storage::local::{Storage, WalletStorage, Payload};
use vecno_wallet_core::storage::interface::CreateArgs;
use vecno_wallet_core::wallet::args::{AccountCreateArgsBip32, PrvKeyDataCreateArgs};
use vecno_wallet_core::storage::keydata::PrvKeyDataVariantKind;
use bip39::Mnemonic;
use log::{error, info};
use std::sync::Arc;
use vecno_wrpc_client::prelude::{Resolver, WrpcEncoding, ConnectOptions, ConnectStrategy};
use vecno_consensus_core::network::{NetworkId, NetworkType};
use vecno_wallet_core::settings::application_folder;

#[command]
pub async fn import_wallets(mnemonic: String, secret: String, filename: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    if secret.is_empty() {
        return Err(ErrorResponse { error: "Wallet password is required".to_string() });
    }
    if mnemonic.is_empty() {
        return Err(ErrorResponse { error: "Mnemonic is required".to_string() });
    }
    if filename.is_empty() {
        return Err(ErrorResponse { error: "Wallet filename is required".to_string() });
    }

    let mnemonic = Mnemonic::parse(&mnemonic).map_err(|e| {
        error!("Invalid mnemonic: {}", e);
        ErrorResponse { error: format!("Invalid mnemonic: Must be 12 or 24 words") }
    })?;
    if mnemonic.word_count() != 12 && mnemonic.word_count() != 24 {
        return Err(ErrorResponse { error: "Mnemonic must be exactly 12 or 24 words".to_string() });
    }

    let network_id = NetworkId::new(NetworkType::Mainnet);
    let wallet_dir = application_folder().map_err(|e| {
        error!("Failed to get application folder: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let storage_path = wallet_dir.join(&filename);

    let storage = Storage::try_new(storage_path.to_str().ok_or_else(|| ErrorResponse { error: "Invalid path".to_string() })?).map_err(|e| {
        error!("Storage initialization failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let store = Wallet::local_store().map_err(|e| {
        error!("Local store creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    let wallet_secret = Secret::new(secret.as_bytes().to_vec());
    let create_args = CreateArgs {
        title: Some("Imported Wallet".to_string()),
        filename: Some(storage_path.to_str().ok_or_else(|| ErrorResponse { error: "Invalid path".to_string() })?.to_string()),
        encryption_kind: EncryptionKind::XChaCha20Poly1305,
        user_hint: None,
        overwrite_wallet: true,
    };
    store.create(&wallet_secret, create_args).await.map_err(|e| {
        error!("Store creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let resolver = Resolver::default();
    info!("Initializing resolver for wallet import");
    let wallet = Arc::new(Wallet::try_new(store.clone(), Some(resolver.clone()), Some(network_id)).map_err(|e| {
        error!("Wallet creation failed: {}", e);
        ErrorResponse { error: format!("Wallet creation failed: {}", e) }
    })?);

    if let Some(wrpc_client) = wallet.try_wrpc_client().as_ref() {
        let url = resolver.get_url(WrpcEncoding::Borsh, network_id).await.map_err(|e| {
            error!("Failed to get resolver URL: {}", e);
            ErrorResponse { error: format!("Failed to resolve node URL: {}. Ensure seed.vecnoscan.org is reachable.", e) }
        })?;
        info!("Connecting to node: {}", url);
        let options = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(url),
            ..Default::default()
        };
        wrpc_client.connect(Some(options)).await.map_err(|e| {
            error!("Failed to connect to node: {}. Check Resolvers.toml for valid endpoints.", e);
            ErrorResponse { error: format!("Failed to connect to node: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) }
        })?;
    } else {
        error!("No wRPC client available for wallet");
        return Err(ErrorResponse { error: "No wRPC client available. Ensure wallet is properly initialized.".to_string() });
    }

    if !wallet.is_open() {
        error!("Wallet is not open after initialization");
        return Err(ErrorResponse { error: "Failed to open wallet: initialization error".to_string() });
    }

    let prv_key_data = PrvKeyDataCreateArgs {
        name: None,
        payment_secret: None,
        secret: Secret::new(mnemonic.to_string().into_bytes()),
        kind: PrvKeyDataVariantKind::Mnemonic,
    };

    let key_id = wallet
        .create_prv_key_data(&wallet_secret, prv_key_data)
        .await
        .map_err(|e| {
            error!("Private key data creation failed: {}", e);
            ErrorResponse { error: e.to_string() }
        })?;

    let account_args = AccountCreateArgsBip32 {
        account_name: Some("default-account".to_string()),
        account_index: None,
    };

    let _account = wallet
        .create_account_bip32(&wallet_secret, key_id, None, account_args)
        .await
        .map_err(|e| {
            error!("Account creation failed: {}", e);
            ErrorResponse { error: e.to_string() }
        })?;

    let payload = Payload::new(vec![], vec![], vec![]);
    let wallet_storage = WalletStorage::try_new(
        Some("Imported Wallet".to_string()),
        None,
        &wallet_secret,
        EncryptionKind::XChaCha20Poly1305,
        payload,
        vec![],
    ).map_err(|e| {
        error!("Wallet storage creation failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;
    wallet_storage.try_store(&storage).await.map_err(|e| {
        error!("Wallet storage failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    wallet.select(Some(&_account)).await.map_err(|e| {
        error!("Wallet selection failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    if let Err(e) = _account.start().await {
        error!("Account start failed: {}. Ensure Vecno node is running.", e);
        return Err(ErrorResponse { error: format!("Failed to start account: {}. Ensure seed.vecnoscan.org is reachable or run a local node.", e) });
    }

    let mut wallet_state = state.wallet.lock().await;
    let mut resolver_state = state.resolver.lock().await;
    let mut secret_state = state.wallet_secret.lock().await;
    *wallet_state = Some(wallet.clone());
    *resolver_state = Some(resolver);
    *secret_state = Some(wallet_secret);
    info!("Wallet successfully imported at {}", storage_path.display());
    Ok(format!("Success: Wallet imported at {}", storage_path.display()))
}