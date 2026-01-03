use crate::models::OpenWalletInput;
use crate::state::{AppState, ErrorResponse};
use log::{info, error};
use std::path::Path;
use std::sync::Arc;
use tauri::{command, State};
use vecno_consensus_core::network::{NetworkId, NetworkType};
use vecno_wallet_core::prelude::*;
use vecno_wallet_core::storage::interface::OpenArgs;
use vecno_wallet_core::storage::local::{Storage, WalletStorage};
use vecno_wallet_core::storage::keydata::PrvKeyDataVariant;
use vecno_wrpc_client::prelude::{ConnectOptions, ConnectStrategy, Resolver, WrpcEncoding};
use futures_lite::stream::StreamExt;
use bip39::Mnemonic;

#[command]
pub async fn open_wallet(
    input: OpenWalletInput,
    state: State<'_, AppState>,
) -> Result<String, ErrorResponse> {
    let filename = input.filename.trim();
    let secret = input.secret.trim();
    let payment_secret_opt = input.payment_secret.as_deref().map(str::trim).filter(|s| !s.is_empty());

    info!(
        "open_wallet → file: '{}', secret_provided: {}, payment_secret_provided: {}",
        filename,
        !secret.is_empty(),
        payment_secret_opt.is_some()
    );

    if filename.is_empty() {
        return Err(ErrorResponse { error: "Wallet filename is required".into() });
    }
    if secret.is_empty() {
        return Err(ErrorResponse { error: "Wallet password is required".into() });
    }

    let storage_path = Path::new(filename);
    if !storage_path.exists() {
        return Err(ErrorResponse { error: "Wallet file does not exist".into() });
    }

    let filename_stem = storage_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ErrorResponse { error: "Invalid wallet filename".into() })?
        .to_string();

    let wallet_secret = Secret::new(secret.as_bytes().to_vec());

    {
        let path_str = storage_path
            .to_str()
            .ok_or_else(|| ErrorResponse { error: "Invalid path encoding".into() })?;

        let store = Storage::try_new(path_str)
            .map_err(|e| ErrorResponse { error: e.to_string() })?;

        let wallet_storage = WalletStorage::try_load(&store)
            .await
            .map_err(|e| ErrorResponse { error: e.to_string() })?;

        if wallet_storage.payload(&wallet_secret).is_err() {
            return Err(ErrorResponse { error: "Incorrect wallet password".into() });
        }
        info!("Password verification succeeded");
    }

    let store = Wallet::local_store().map_err(|e| ErrorResponse { error: e.to_string() })?;
    store
        .open(&wallet_secret, OpenArgs { filename: Some(filename_stem.clone()) })
        .await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    let network_id = NetworkId::new(NetworkType::Mainnet);
    let resolver = Resolver::default();

    let wallet = Arc::new(
        Wallet::try_new(store.clone(), Some(resolver.clone()), Some(network_id))
            .map_err(|e| ErrorResponse { error: format!("Wallet initialization failed: {}", e) })?,
    );

    if let Some(wrpc) = wallet.try_wrpc_client().as_ref() {
        let url = resolver
            .get_url(WrpcEncoding::Borsh, network_id)
            .await
            .map_err(|e| ErrorResponse { error: format!("Failed to resolve node URL: {}", e) })?;

        let opts = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(url),
            ..Default::default()
        };

        wrpc.connect(Some(opts)).await
            .map_err(|e| {
                error!("Node connection failed: {}", e);
                ErrorResponse {
                    error: "Failed to connect to Vecno node. Check your internet connection or try again later.".into()
                }
            })?;
        info!("Connected to node");
    } else {
        return Err(ErrorResponse { error: "No wRPC client available".into() });
    }

    if !wallet.is_open() {
        return Err(ErrorResponse { error: "Wallet failed to open".into() });
    }

    let mut mnemonic: Option<String> = None;
    let mut bip39_seed: Option<String> = None;

    let mut keys = wallet
        .store()
        .as_prv_key_data_store()
        .map_err(|e| ErrorResponse { error: e.to_string() })?
        .iter()
        .await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    while let Some(info) = keys.try_next().await.map_err(|e| ErrorResponse { error: e.to_string() })? {
        let encrypted = match store
            .as_prv_key_data_store()
            .map_err(|e| ErrorResponse { error: e.to_string() })?
            .load_key_data(&wallet_secret, &info.id)
            .await
        {
            Ok(Some(e)) => e,
            Ok(None) => continue,
            Err(e) => {
                info!("Failed to load key data (ID: {}): {}", info.id, e);
                continue;
            }
        };

        let decrypted = match encrypted.payload.decrypt(Some(&wallet_secret)) {
            Ok(d) => d,
            Err(e) => {
                info!("Failed to decrypt key data (ID: {}): {}", info.id, e);
                continue;
            }
        };

        match &*decrypted.as_variant() {
            PrvKeyDataVariant::Mnemonic(s) => {
                let cleaned = s.trim().to_owned();
                let word_count = cleaned.split_whitespace().count();
                info!("Mnemonic loaded: {} words", word_count);
                mnemonic = Some(cleaned.clone());

                if let Some(payment_secret) = payment_secret_opt {
                    if let Ok(mnemonic_obj) = Mnemonic::parse(&cleaned) {
                        let seed = mnemonic_obj.to_seed(payment_secret);
                        let seed_hex = hex::encode(seed);
                        info!("Derived Bip39Seed from mnemonic + payment_secret");
                        bip39_seed = Some(seed_hex);
                    }
                }
            }
            PrvKeyDataVariant::Bip39Seed(s) => {
                let cleaned = s.trim().to_owned();
                info!("Bip39Seed loaded directly");
                bip39_seed = Some(cleaned);
            }
            _ => {}
        }
    }

    let mut account_id: Option<AccountId> = None;
    let mut accounts = wallet
        .store()
        .as_account_store()
        .map_err(|e| ErrorResponse { error: e.to_string() })?
        .iter(None)
        .await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    while let Some((acc, _)) = accounts.try_next().await.map_err(|e| ErrorResponse { error: e.to_string() })? {
        account_id = Some(*acc.id());
        break;
    }

    let account = if let Some(id) = account_id {
        let guard_mutex = wallet.guard();
        let guard = guard_mutex.lock().await;

        wallet
            .get_account_by_id(&id, &guard)
            .await
            .map_err(|e| ErrorResponse { error: e.to_string() })?
            .ok_or_else(|| ErrorResponse { error: "Account not found".into() })?
    } else {
        return Err(ErrorResponse { error: "No accounts found in wallet".into() });
    };

    wallet
        .select(Some(&account))
        .await
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    account
        .start()
        .await
        .map_err(|e| ErrorResponse { error: format!("Failed to start account: {}", e) })?;

    {
        let mut w = state.wallet.lock().await;
        let mut r = state.resolver.lock().await;
        let mut s = state.wallet_secret.lock().await;
        let mut m = state.mnemonic.lock().await;
        let mut seed_state = state.bip39_seed.lock().await;

        *w = Some(wallet.clone());
        *r = Some(resolver);
        *s = Some(wallet_secret);
        *m = mnemonic;
        *seed_state = bip39_seed;

        info!("AppState updated: wallet opened, mnemonic/seed loaded if available");
    }

    let msg = format!("Success: Wallet opened from {}", storage_path.display());
    info!("{}", msg);
    Ok(msg)
}