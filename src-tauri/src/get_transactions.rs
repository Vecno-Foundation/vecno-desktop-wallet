use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use vecno_wallet_core::prelude::*;
use vecno_consensus_core::tx::{TransactionId, TransactionOutpoint};
use vecno_rpc_core::{RpcUtxosByAddressesEntry};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use chrono::{Local, TimeZone};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Transaction {
    pub txid: String,
    pub to_address: String,
    pub amount: u64,
    pub timestamp: String,
}

#[command]
pub async fn list_transactions(state: State<'_, AppState>) -> Result<Vec<Transaction>, ErrorResponse> {
    let wallet_guard = state.wallet.lock().await;
    let wallet = wallet_guard.as_ref().ok_or(ErrorResponse {
        error: "Wallet is not open".to_string(),
    })?.clone();

    if !wallet.is_open() {
        return Err(ErrorResponse {
            error: "Wallet is not open".to_string(),
        });
    }

    let account: Arc<dyn Account> = wallet.account().map_err(ErrorResponse::from)?;
    let receive_address = account.receive_address().map_err(ErrorResponse::from)?;

    // Fetch UTXOs for the receive address to get recent incoming transaction IDs
    // Note: This provides tx details for transactions that created UTXOs (incoming).
    // For full history (including outgoing), additional logic like scanning mempool or chain would be needed.
    let utxos: Vec<RpcUtxosByAddressesEntry> = wallet
        .rpc_api()
        .get_utxos_by_addresses(vec![receive_address.clone()])
        .await
        .map_err(|e| ErrorResponse {
            error: format!("Failed to fetch UTXOs: {}", e),
        })?;

    let mut tx_amounts: HashMap<TransactionId, u64> = HashMap::new();
    let mut tx_daa: HashMap<TransactionId, u64> = HashMap::new();
    let mut seen_txids: HashSet<TransactionId> = HashSet::new();

    for entry in &utxos {
        let outpoint: TransactionOutpoint = entry.outpoint.clone().into();
        let txid = outpoint.transaction_id.clone();

        if seen_txids.insert(txid.clone()) {
            let daa_score = entry.utxo_entry.block_daa_score;
            tx_daa.insert(txid.clone(), daa_score);
        }

        *tx_amounts.entry(txid).or_insert(0) += entry.utxo_entry.amount;
    }

    let unique_daas: Vec<u64> = tx_daa.values().cloned().collect::<Vec<_>>();
    let timestamps = wallet
        .rpc_api()
        .get_daa_score_timestamp_estimate(unique_daas.clone())
        .await
        .map_err(|e| ErrorResponse {
            error: format!("Failed to fetch timestamps for DAA scores: {}", e),
        })?;
    let daa_to_ts: HashMap<u64, u64> = unique_daas
        .into_iter()
        .zip(timestamps.into_iter())
        .collect();

    let mut transactions: Vec<(Transaction, u64)> = tx_amounts
        .iter()
        .filter_map(|(txid, amount)| {
            tx_daa.get(txid).map(|daa| {
                let timestamp = if let Some(&ts_ms) = daa_to_ts.get(daa) {
                    let ts_sec = ts_ms / 1000;
                    let ts_nsec = ((ts_ms % 1000) * 1_000_000) as u32;
                    Local
                        .timestamp_opt(ts_sec as i64, ts_nsec)
                        .single()
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| format!("DAA Score: {}", daa))
                } else {
                    format!("DAA Score: {}", daa)
                };
                let transaction = Transaction {
                    txid: txid.to_string(),
                    to_address: receive_address.to_string(),
                    amount: *amount,
                    timestamp,
                };
                (transaction, *daa)
            })
        })
        .collect();

    transactions.sort_by(|a, b| b.1.cmp(&a.1));
    let recent_transactions: Vec<Transaction> = transactions
        .into_iter()
        .take(20)
        .map(|(tx, _)| tx)
        .collect();

    Ok(recent_transactions)
}