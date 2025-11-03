
use crate::state::{AppState, ErrorResponse, WalletAddress, WalletFile};
use tauri::{command, State};
use log::{error, info};
use vecno_wallet_core::prelude::Secret;
use vecno_wallet_core::storage::local::{Storage, WalletStorage};
use std::path::Path;
use rand::Rng;
use bip39;

#[command]
pub async fn is_wallet_open(state: State<'_, AppState>) -> Result<bool, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| {
        let msg = "No wallet initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;
    let is_open = wallet.is_open();
    info!(
        "is_wallet_open: wallet exists: {}, is_open: {}",
        guard.is_some(),
        is_open
    );
    Ok(is_open)
}

#[command]
pub async fn generate_mnemonic() -> Result<String, ErrorResponse> {
    let entropy = rand::thread_rng().gen::<[u8; 32]>();
    let mnemonic = bip39::Mnemonic::from_entropy_in(bip39::Language::English, &entropy)
        .map_err(|e| ErrorResponse { error: e.to_string() })?;
    Ok(mnemonic.to_string())
}

#[command]
pub async fn get_address(state: State<'_, AppState>) -> Result<Vec<WalletAddress>, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| ErrorResponse {
        error: "No wallet initialized".to_string(),
    })?;

    if !wallet.is_open() {
        return Err(ErrorResponse {
            error: "Wallet is not open".to_string(),
        });
    }

    let account = wallet.account().map_err(|e| ErrorResponse {
        error: e.to_string(),
    })?;

    let receive = account
        .receive_address()
        .map_err(|e| ErrorResponse {
            error: e.to_string(),
        })?
        .to_string();
    let change = account
        .change_address()
        .map_err(|e| ErrorResponse {
            error: e.to_string(),
        })?
        .to_string();

    Ok(vec![WalletAddress {
        account_name: "default-account".to_string(),
        account_index: 0,
        receive_address: receive,
        change_address: change,
    }])
}

#[command]
pub async fn list_wallets() -> Result<Vec<WalletFile>, ErrorResponse> {
    use std::fs;
    use vecno_wallet_core::settings::application_folder;

    let wallet_dir = application_folder().map_err(|e| {
        error!("Failed to get application folder: {}", e);
        ErrorResponse {
            error: e.to_string(),
        }
    })?;

    let mut wallets = Vec::new();
    if let Ok(entries) = fs::read_dir(&wallet_dir) {
        for entry in entries.flatten() {
            if let Ok(file_name) = entry.file_name().into_string() {
                if file_name.ends_with(".wallet") {
                    let path = entry.path().to_string_lossy().into_owned();
                    let name = file_name
                        .strip_suffix(".wallet")
                        .unwrap_or(&file_name)
                        .to_string();
                    wallets.push(WalletFile { name, path });
                }
            }
        }
    }

    info!("Found {} wallet(s) in {}", wallets.len(), wallet_dir.display());
    Ok(wallets)
}

#[command]
pub async fn verify_wallet_password(
    filename: String,
    secret: String,
) -> Result<(), ErrorResponse> {
    info!("verify_wallet_password invoked for: {}", filename);

    if filename.is_empty() {
        return Err(ErrorResponse {
            error: "Wallet filename is required".into(),
        });
    }
    if secret.is_empty() {
        return Err(ErrorResponse {
            error: "Wallet password is required".into(),
        });
    }

    let storage_path = Path::new(&filename);

    if !storage_path.exists() {
        return Err(ErrorResponse {
            error: "Wallet file does not exist".into(),
        });
    }

    let path_str = storage_path.to_str().ok_or_else(|| ErrorResponse {
        error: "Invalid path encoding".into(),
    })?;

    let wallet_secret = Secret::new(secret.as_bytes().to_vec());

    info!("Loading wallet storage for password verification...");

    let store = Storage::try_new(path_str).map_err(|e| {
        error!("Storage initialization failed: {}", e);
        ErrorResponse {
            error: format!("Failed to initialize storage: {}", e),
        }
    })?;

    let wallet_storage = WalletStorage::try_load(&store).await.map_err(|e| {
        error!("Failed to load wallet file: {}", e);
        ErrorResponse {
            error: format!("Failed to load wallet: {}", e),
        }
    })?;

    if wallet_storage.payload(&wallet_secret).is_err() {
        info!("Password verification failed: incorrect password");
        return Err(ErrorResponse {
            error: "Incorrect password provided".into(),
        });
    }

    info!("Password verification successful for wallet: {}", filename);
    Ok(())
}