use crate::models::CreateWalletInput;
use crate::state::{AppState, ErrorResponse};
use bip39::{Language, Mnemonic};
use log::info;
use rand::RngCore;
use std::sync::Arc;
use tauri::{command, State};
use vecno_consensus_core::network::{NetworkId, NetworkType};
use vecno_wallet_core::prelude::*;
use vecno_wallet_core::storage::interface::CreateArgs;
use vecno_wallet_core::wallet::args::{AccountCreateArgs, PrvKeyDataCreateArgs};
use vecno_wallet_core::storage::keydata::PrvKeyDataVariantKind;
use vecno_wrpc_client::prelude::{ConnectOptions, ConnectStrategy, Resolver, WrpcEncoding};
use vecno_wallet_core::settings::application_folder;

#[command]
pub async fn create_wallet(
    input: CreateWalletInput,
    state: State<'_, AppState>,
) -> Result<String, ErrorResponse> {
    let secret = input.secret.trim();
    let filename = input.filename.trim();
    let payment_passphrase = input.payment_secret.as_deref().map(str::trim);

    info!(
        "create_wallet → file: '{}', payment_passphrase_provided: {}",
        filename,
        payment_passphrase.is_some()
    );

    if secret.is_empty() {
        return Err(ErrorResponse { error: "Wallet password is required".into() });
    }
    if filename.is_empty() {
        return Err(ErrorResponse { error: "Wallet filename is required".into() });
    }

    let mut entropy = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
        .map_err(|e| ErrorResponse { error: e.to_string() })?
        .to_string();

    info!("Generated 24-word mnemonic");

    let payment_secret_opt = payment_passphrase
        .filter(|s| !s.is_empty())
        .map(|s| Secret::new(s.as_bytes().to_vec()));

    let wallet_dir = application_folder().map_err(|e| ErrorResponse { error: e.to_string() })?;
    let storage_path = wallet_dir.join(filename);
    let storage_path_str = storage_path
        .to_str()
        .ok_or_else(|| ErrorResponse { error: "Invalid path".into() })?
        .to_string();

    let store: Arc<dyn Interface> = Wallet::local_store()
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    let wallet_secret = Secret::new(secret.as_bytes().to_vec());

    let create_args = CreateArgs {
        title: Some("My Wallet".into()),
        filename: Some(storage_path_str.clone()),
        encryption_kind: EncryptionKind::XChaCha20Poly1305,
        user_hint: None,
        overwrite_wallet: true,
    };

    store
        .create(&wallet_secret, create_args)
        .await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    let network_id = NetworkId::new(NetworkType::Mainnet);
    let resolver = Resolver::default();

    let wallet = Arc::new(
        Wallet::try_new(store.clone(), Some(resolver.clone()), Some(network_id))
            .map_err(|e| ErrorResponse { error: format!("Wallet init failed: {}", e) })?,
    );

    if let Some(wrpc) = wallet.try_wrpc_client().as_ref() {
        let url = resolver
            .get_url(WrpcEncoding::Borsh, network_id)
            .await
            .map_err(|e| ErrorResponse { error: format!("Node resolve failed: {}", e) })?;

        let opts = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(url),
            ..Default::default()
        };

        wrpc.connect(Some(opts)).await
            .map_err(|e| ErrorResponse { error: format!("Node connect failed: {}", e) })?;
        info!("Connected to node");
    } else {
        return Err(ErrorResponse { error: "No wRPC client".into() });
    }

    if !wallet.is_open() {
        return Err(ErrorResponse { error: "Wallet failed to open".into() });
    }
    let prv_key_data_args = PrvKeyDataCreateArgs {
        name: None,
        payment_secret: payment_secret_opt.clone(),
        secret: Secret::from(mnemonic.clone()),
        kind: PrvKeyDataVariantKind::Mnemonic,
    };

    let key_id = wallet
        .create_prv_key_data(&wallet_secret, prv_key_data_args)
        .await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    let guard_mutex = wallet.guard();
    let guard = guard_mutex.lock().await;

    let account_args = AccountCreateArgs::new_bip32(
        key_id,
        payment_secret_opt,
        Some("default-account".into()),
        None,
    );

    let account = wallet
        .create_account(&wallet_secret, account_args, false, &guard)
        .await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    store.batch().await
        .map_err(|e| ErrorResponse { error: format!("Failed to start batch: {}", e) })?;

    wallet.select(Some(&account)).await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;
    account.start().await
        .map_err(|e| ErrorResponse { error: format!("Account start failed: {}", e) })?;

    store.flush(&wallet_secret).await
        .map_err(|e| ErrorResponse { error: format!("Flush failed: {}", e) })?;

    {
        let mut w = state.wallet.lock().await;
        let mut r = state.resolver.lock().await;
        let mut s = state.wallet_secret.lock().await;
        let mut m = state.mnemonic.lock().await;

        *w = Some(wallet.clone());
        *r = Some(resolver);
        *s = Some(wallet_secret);
        *m = Some(mnemonic.clone());
    }

    info!("Wallet successfully created at {}", storage_path.display());

    Ok(format!("Success: Wallet created at {} with mnemonic: {}", storage_path.display(), mnemonic))
}