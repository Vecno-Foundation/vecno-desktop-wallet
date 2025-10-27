use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use log::{error, info};
use workflow_core::abortable::Abortable;
use std::sync::Arc as StdArc;
use std::sync::Mutex;

#[command]
pub async fn get_balance(address: String, state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    let guard = state.wallet.lock().await;
    let secret_guard = state.wallet_secret.lock().await;
    let wallet = guard.as_ref().ok_or_else(|| {
        let msg = "No wallet initialized";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;

    if !wallet.is_open() {
        let msg = "Wallet is not open";
        error!("{}", msg);
        return Err(ErrorResponse { error: msg.to_string() });
    }

    let wallet_secret = secret_guard.as_ref().cloned().ok_or_else(|| {
        let msg = "Wallet secret not set";
        error!("{}", msg);
        ErrorResponse { error: msg.to_string() }
    })?;

    let account = wallet.account().map_err(|e| {
        error!("Account retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?;

    if address != account.receive_address().map_err(|e| {
        error!("Receive address retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?.to_string() &&
       address != account.change_address().map_err(|e| {
        error!("Change address retrieval failed: {}", e);
        ErrorResponse { error: e.to_string() }
    })?.to_string() {
        error!("Invalid address for this account: {}", address);
        return Err(ErrorResponse { error: "Invalid address for this account".to_string() });
    }

    let derivation_account = account.clone()
        .as_derivation_capable()
        .map_err(|e| {
            error!("Account is not derivation capable: {}", e);
            ErrorResponse { error: "Account does not support address derivation".to_string() }
        })?;

    info!("Starting derivation scan for address: {}", address);
    let abortable = Abortable::new();
    let balance = StdArc::new(Mutex::new(0u64));
    let balance_clone = balance.clone();

    // Perform derivation scan with extended extent
    let _ = derivation_account
        .derivation_scan(
            wallet_secret,
            None, // payment_secret
            0,    // start
            1000, // extent (increased to cover more addresses)
            128,  // window
            false, // sweep
            None, // fee_rate
            &abortable,
            true, // verbose
            Some(StdArc::new(move |processed: usize, _, found_balance, txid| {
                let value = balance_clone.clone();
                tauri::async_runtime::spawn(async move {
                    let mut balance_guard = value.lock().unwrap(); // Use unwrap for simplicity; handle errors in production
                    *balance_guard = found_balance; // Dereference to update the u64 value
                    if let Some(txid) = txid {
                        info!("Scan detected {} VE at index {}; transfer txid: {}", found_balance, processed, txid);
                    } else if processed > 0 {
                        info!("Scanned {} derivations, found {} VE", processed, found_balance);
                    } else {
                        info!("Scanning for account UTXOs...");
                    }
                });
            })),
        )
        .await;

    // Retrieve the balance
    let balance_value = *balance.lock().unwrap(); // Use unwrap for simplicity; handle errors in production
    info!("Balance scan completed for address {}: {} VE", address, balance_value);
    Ok(balance_value.to_string())
}