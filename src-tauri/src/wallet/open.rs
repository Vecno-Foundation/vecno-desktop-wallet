
use crate::state::{AppState, ErrorResponse};
use log::{error, info};
use tauri::{command, State};
use vecno_wallet_core::prelude::*;
use vecno_wallet_core::storage::interface::OpenArgs;
use vecno_wallet_core::storage::local::{Storage, WalletStorage};
use vecno_wrpc_client::prelude::{Resolver, WrpcEncoding, ConnectOptions, ConnectStrategy};
use vecno_consensus_core::network::{NetworkId, NetworkType};
use futures_lite::stream::StreamExt;
use std::path::Path;
use std::sync::Arc;
use vecno_wallet_core::settings::application_folder;
use vecno_wallet_core::storage::PrvKeyDataId;
use vecno_wallet_core::storage::keydata::PrvKeyDataVariant;

#[command]
pub async fn open_wallet(
    filename: String,
    secret: String,
    state: State<'_, AppState>,
) -> Result<String, ErrorResponse> {
    info!("open_wallet invoked with filename: {}", filename);

    if filename.is_empty() {
        let err = ErrorResponse { error: "Wallet filename is required".into() };
        info!("Validation failed: {}", err.error);
        return Err(err);
    }
    if secret.is_empty() {
        let err = ErrorResponse { error: "Wallet password is required".into() };
        info!("Validation failed: {}", err.error);
        return Err(err);
    }

    let storage_path = Path::new(&filename);
    let wallet_dir = application_folder().map_err(|e| {
        let err = ErrorResponse { error: e.to_string() };
        error!("Failed to get application folder: {}", err.error);
        err
    })?;
    info!("Wallet dir: {}", wallet_dir.display());

    if !storage_path.exists() {
        let err = ErrorResponse { error: "Wallet file does not exist".into() };
        info!("{}", err.error);
        return Err(err);
    }

    let filename_stem = storage_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            let err = ErrorResponse { error: "Invalid wallet filename".into() };
            info!("{}", err.error);
            err
        })?
        .to_string();

    let wallet_secret = Secret::new(secret.as_bytes().to_vec());

    info!("Attempting manual password check...");
    {
        let path_str = storage_path.to_str().ok_or_else(|| {
            let err = ErrorResponse { error: "Invalid path encoding".into() };
            error!("{}", err.error);
            err
        })?;

        let store = Storage::try_new(path_str).map_err(|e| {
            let err = ErrorResponse { error: e.to_string() };
            error!("Storage init failed during password check: {}", err.error);
            err
        })?;

        let wallet_storage = WalletStorage::try_load(&store).await.map_err(|e| {
            let err = ErrorResponse { error: e.to_string() };
            error!("Failed to load wallet file during password check: {}", err.error);
            err
        })?;

        if wallet_storage.payload(&wallet_secret).is_err() {
            let err = ErrorResponse { error: "Incorrect password provided".into() };
            info!("{}", err.error);
            return Err(err);
        }
    }
    info!("Password check: CORRECT");

    info!("Password correct – proceeding to open wallet...");

    let store = Wallet::local_store().map_err(|e| {
        let err = ErrorResponse { error: e.to_string() };
        error!("Local store creation failed: {}", err.error);
        err
    })?;

    let open_args = OpenArgs { filename: Some(filename_stem) };

    store
        .open(&wallet_secret, open_args)
        .await
        .map_err(|e| {
            let err = ErrorResponse { error: e.to_string() };
            error!("Failed to open wallet storage: {}", err.error);
            err
        })?;

    info!("Wallet storage opened successfully");

    let network_id = NetworkId::new(NetworkType::Mainnet);
    let resolver = Resolver::default();
    info!("Resolver initialized for network: {:?}", network_id);

    let wallet = Arc::new(
        Wallet::try_new(store.clone(), Some(resolver.clone()), Some(network_id))
            .map_err(|e| {
                let err = ErrorResponse { error: e.to_string() };
                error!("Wallet creation failed: {}", err.error);
                err
            })?,
    );

    if let Some(wrpc_client) = wallet.try_wrpc_client().as_ref() {
        info!("wRPC client found – resolving URL...");
        let url = resolver
            .get_url(WrpcEncoding::Borsh, network_id)
            .await
            .map_err(|e| {
                let err = ErrorResponse {
                    error: format!(
                        "Failed to resolve node URL: {}. Ensure seed.vecnoscan.org is reachable.",
                        e
                    ),
                };
                error!("{}", err.error);
                err
            })?;
        info!("Resolved node URL: {}", url);

        let options = ConnectOptions {
            block_async_connect: true,
            strategy: ConnectStrategy::Fallback,
            url: Some(url),
            ..Default::default()
        };
        wrpc_client
            .connect(Some(options))
            .await
            .map_err(|e| {
                let err = ErrorResponse {
                    error: format!(
                        "Failed to connect to node: {}. Ensure seed.vecnoscan.org is reachable.",
                        e
                    ),
                };
                error!("{}", err.error);
                err
            })?;
        info!("Connected to node successfully");
    } else {
        let err = ErrorResponse {
            error: "No wRPC client available. Ensure wallet is properly initialized.".into(),
        };
        error!("{}", err.error);
        return Err(err);
    }

    if !wallet.is_open() {
        let err = ErrorResponse { error: "Wallet failed to initialize".into() };
        error!("{}", err.error);
        return Err(err);
    }
    info!("Wallet is open");

    info!("Loading private key data...");
    let mut key_data_id: Option<PrvKeyDataId> = None;
    let mut keys = wallet
        .store()
        .as_prv_key_data_store()
        .map_err(|e| {
            let err = ErrorResponse { error: e.to_string() };
            error!("Failed to access private key data store: {}", err.error);
            err
        })?
        .iter()
        .await
        .map_err(|e| {
            let err = ErrorResponse { error: e.to_string() };
            error!("Failed to iterate private key data: {}", err.error);
            err
        })?;
    while let Some(key_info) = keys.try_next().await.map_err(|e| {
        let err = ErrorResponse { error: e.to_string() };
        error!("Failed to read private key info: {}", err.error);
        err
    })? {
        key_data_id = Some(key_info.id);
        info!("Found key data ID: {:?}", key_info.id);
        break;
    }

    let mnemonic_opt = if let Some(id) = key_data_id {
        info!("Loading encrypted key data for ID: {:?}", id);
        let encrypted = store
            .as_prv_key_data_store()
            .map_err(|e| {
                let err = ErrorResponse { error: e.to_string() };
                error!("Failed to access private key data store: {}", err.error);
                err
            })?
            .load_key_data(&wallet_secret, &id)
            .await
            .map_err(|e| {
                let err = ErrorResponse { error: e.to_string() };
                error!("Failed to load private key data: {}", err.error);
                err
            })?;

        let encrypted_payload = encrypted.ok_or_else(|| {
            let err = ErrorResponse { error: "Encrypted private key data is missing".into() };
            error!("{}", err.error);
            err
        })?;

        let payload = encrypted_payload
            .payload
            .decrypt(Some(&wallet_secret))
            .map_err(|e| {
                let err = ErrorResponse { error: e.to_string() };
                error!("Failed to decrypt payload: {}", err.error);
                err
            })?;

        match *payload.as_variant() {
            PrvKeyDataVariant::Mnemonic(ref s) => {
                let mn = s.trim_end_matches('\0').trim().to_owned();
                info!("Mnemonic loaded successfully ({} words)", mn.split_whitespace().count());
                Some(mn)
            }
            _ => {
                let err = ErrorResponse { error: "Wallet does not contain a mnemonic".into() };
                error!("{}", err.error);
                return Err(err);
            }
        }
    } else {
        let err = ErrorResponse { error: "No private key data found in wallet".into() };
        error!("{}", err.error);
        return Err(err);
    };

    info!("Selecting first account...");
    let mut account_id: Option<AccountId> = None;
    let mut accounts = wallet
        .store()
        .as_account_store()
        .map_err(|e| {
            let err = ErrorResponse { error: e.to_string() };
            error!("Failed to access account store: {}", err.error);
            err
        })?
        .iter(None)
        .await
        .map_err(|e| {
            let err = ErrorResponse { error: e.to_string() };
            error!("Failed to iterate accounts: {}", err.error);
            err
        })?;
    while let Some((acc, _)) = accounts.try_next().await.map_err(|e| {
        let err = ErrorResponse { error: e.to_string() };
        error!("Failed to read account: {}", err.error);
        err
    })? {
        account_id = Some(*acc.id());
        info!("Found account ID: {:?}", acc.id());
        break;
    }

    if let Some(id) = account_id {
        let guard_obj = wallet.guard();
        let guard = guard_obj.lock().await;

        let account = wallet
            .get_account_by_id(&id, &guard)
            .await
            .map_err(|e| {
                let err = ErrorResponse { error: e.to_string() };
                error!("Failed to get account by ID: {}", err.error);
                err
            })?
            .ok_or_else(|| {
                let err = ErrorResponse { error: format!("Account ID {:?} not found", id) };
                error!("{}", err.error);
                err
            })?;

        drop(guard);

        wallet.select(Some(&account)).await.map_err(|e| {
            let err = ErrorResponse { error: e.to_string() };
            error!("Failed to select account: {}", err.error);
            err
        })?;
        info!("Account selected: {:?}", id);
    } else {
        let err = ErrorResponse { error: "No accounts found in wallet".into() };
        error!("{}", err.error);
        return Err(err);
    }

    info!("Starting account...");
    let account = wallet.account().map_err(|e| {
        let err = ErrorResponse { error: e.to_string() };
        error!("Failed to get current account: {}", err.error);
        err
    })?;
    account.start().await.map_err(|e| {
        let err = ErrorResponse {
            error: format!("Failed to start account: {}. Ensure seed.vecnoscan.org is reachable.", e),
        };
        error!("{}", err.error);
        err
    })?;
    info!("Account started");

    info!("Persisting wallet state...");
    {
        let mut wallet_state = state.wallet.lock().await;
        let mut resolver_state = state.resolver.lock().await;
        let mut secret_state = state.wallet_secret.lock().await;
        let mut mnemonic_state = state.mnemonic.lock().await;

        *wallet_state = Some(wallet.clone());
        *resolver_state = Some(resolver);
        *secret_state = Some(wallet_secret);
        *mnemonic_state = mnemonic_opt;
    }
    info!("State persisted");

    let success_msg = format!("Success: Wallet opened from {}", storage_path.display());
    info!("{}", success_msg);
    Ok(success_msg)
}