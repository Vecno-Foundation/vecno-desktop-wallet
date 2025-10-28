use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use log::{error, info};
use rand::Rng;

#[command]
pub async fn is_wallet_open(state: State<'_, AppState>) -> Result<bool, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| {
        let msg = "No wallet initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;
    let is_open = wallet.is_open();
    info!("is_wallet_open: wallet exists: {}, is_open: {}", guard.is_some(), is_open);
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
pub async fn get_address(state: State<'_, AppState>) -> Result<Vec<crate::state::WalletAddress>, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| ErrorResponse { error: "No wallet initialized".to_string() })?;
    if !wallet.is_open() {
        return Err(ErrorResponse { error: "Wallet is not open".to_string() });
    }

    let account = wallet.account()?;
    let receive = account.receive_address()?.to_string();
    let change = account.change_address()?.to_string();

    Ok(vec![crate::state::WalletAddress {
        account_name: "default-account".to_string(),
        account_index: 0,
        receive_address: receive,
        change_address: change,
    }])
}

#[command]
pub async fn list_wallets() -> Result<Vec<crate::state::WalletFile>, ErrorResponse> {
    use std::fs;
    use vecno_wallet_core::settings::application_folder;
    use log::{error, info};

    let wallet_dir = application_folder().map_err(|e| {
        error!("Failed to get application folder: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    let mut wallets = Vec::new();
    for entry in fs::read_dir(&wallet_dir)?.flatten() {
        if let Ok(file_name) = entry.file_name().into_string() {
            if file_name.ends_with(".wallet") {
                let path = entry.path().to_string_lossy().into_owned();
                let name = file_name.strip_suffix(".wallet").unwrap_or(&file_name).to_string();
                wallets.push(crate::state::WalletFile { name, path });
            }
        }
    }
    info!("Found {} wallets", wallets.len());
    Ok(wallets)
}