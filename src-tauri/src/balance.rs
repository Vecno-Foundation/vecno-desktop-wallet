use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use log::{info};
use workflow_core::abortable::Abortable;
use std::sync::Arc as StdArc;
use std::sync::Mutex;

#[command]
pub async fn get_balance(state: State<'_, AppState>) -> Result<String, ErrorResponse> {

    let wallet_guard = state.wallet.lock().await;
    let secret_guard = state.wallet_secret.lock().await;

    let wallet = wallet_guard.as_ref().ok_or_else(|| {
        ErrorResponse { error: "No wallet initialized".into() }
    })?;

    if !wallet.is_open() {
        return Err(ErrorResponse { error: "Wallet is not open".into() });
    }

    let wallet_secret = secret_guard.as_ref().cloned().ok_or_else(|| {
        ErrorResponse { error: "Wallet secret not set".into() }
    })?;

    let account = wallet.account().map_err(|e| ErrorResponse { error: e.to_string() })?;
    let derivation_account = account.clone().as_derivation_capable()
        .map_err(|_| ErrorResponse { error: "Account does not support derivation".into() })?;

    info!("Scanning entire account for UTXOs");

    let abortable = Abortable::new();
    let total = StdArc::new(Mutex::new(0u64));
    let total_clone = total.clone();

    let _ = derivation_account.derivation_scan(
        wallet_secret,
        None, 0, 1000, 128, false, None, &abortable, true,
        Some(StdArc::new(move |_, _, found, _| {
            let t = total_clone.clone();
            tauri::async_runtime::spawn(async move {
                *t.lock().unwrap() = found;
            });
        })),
    ).await;

    let balance = *total.lock().unwrap();
    info!("Account balance: {} VE", balance);
    Ok(balance.to_string())
}