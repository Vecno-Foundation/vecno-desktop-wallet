use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use log::info;
use vecno_rpc_core::RpcUtxosByAddressesEntry;
use serde::{Deserialize, Serialize};
use chrono::Utc;

#[derive(Serialize, Deserialize, Clone)]
pub struct BalanceResponse {
    pub balance: u64,
    pub timestamp: i64,
}

#[command]
pub async fn get_balance(state: State<'_, AppState>) -> Result<BalanceResponse, ErrorResponse> {
    info!("=== BALANCE REFRESH STARTED ===");

    let wallet_guard = state.wallet.lock().await;
    let wallet = wallet_guard
        .as_ref()
        .ok_or(ErrorResponse {
            error: "No wallet initialized".into(),
        })?
        .clone();

    if !wallet.is_open() {
        info!("=== BALANCE REFRESH FAILED: Wallet is not open ===");
        return Err(ErrorResponse {
            error: "Wallet is not open".into(),
        });
    }

    let account = wallet
        .account()
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    let receive_address = account
        .receive_address()
        .map_err(|e| ErrorResponse { error: e.to_string() })?;
    let change_address = account
        .change_address()
        .map_err(|e| ErrorResponse { error: e.to_string() })?;

    info!("Fetching UTXOs for receive: {} | change: {}", receive_address, change_address);

    let addresses = vec![receive_address.clone(), change_address.clone()];
    let utxos: Vec<RpcUtxosByAddressesEntry> = wallet
        .rpc_api()
        .get_utxos_by_addresses(addresses)
        .await
        .map_err(|e| ErrorResponse {
            error: format!("Failed to fetch UTXOs: {}", e),
        })?;

    let mut receive_utxos = Vec::new();
    let mut change_utxos = Vec::new();

    for entry in &utxos {
        if entry.address == Some(receive_address.clone()) {
            receive_utxos.push(entry);
        } else {
            change_utxos.push(entry);
        }
    }

    let receive_count = receive_utxos.len();
    let change_count = change_utxos.len();
    let total_count = receive_count + change_count;

    let receive_balance: u64 = receive_utxos.iter().map(|e| e.utxo_entry.amount).sum();
    let change_balance: u64 = change_utxos.iter().map(|e| e.utxo_entry.amount).sum();
    let total_balance = receive_balance + change_balance;

    info!("────────────────────────────────────────");
    info!("│ BALANCE REPORT (Receive + Change)");
    info!("├─ Receive Addr : {}", receive_address);
    info!("│   ├─ UTXOs     : {}", receive_count);
    info!("│   └─ Balance   : {} VE", receive_balance);
    info!("├─ Change Addr  : {}", change_address);
    info!("│   ├─ UTXOs     : {}", change_count);
    info!("│   └─ Balance   : {} VE", change_balance);
    info!("├─ TOTAL");
    info!("│   ├─ UTXOs     : {}", total_count);
    info!("│   └─ Balance   : {} VE", total_balance);
    info!("────────────────────────────────────────");

    if total_count > 0 {
        let all_amounts: Vec<u64> = utxos.iter().map(|e| e.utxo_entry.amount).collect();
        let min = all_amounts.iter().min().unwrap();
        let max = all_amounts.iter().max().unwrap();
        info!("   • Smallest UTXO : {} VE", min);
        info!("   • Largest UTXO  : {} VE", max);
    }

    let timestamp = Utc::now().timestamp();

    info!("=== BALANCE REFRESH COMPLETED: {} VE at {} ===", total_balance, timestamp);

    Ok(BalanceResponse {
        balance: total_balance,
        timestamp,
    })
}