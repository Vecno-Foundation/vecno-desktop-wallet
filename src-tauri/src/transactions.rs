use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use vecno_wallet_core::prelude::*;

#[command]
pub async fn send_transaction(
    _to_address: String,
    _amount: u64,
    _state: State<'_, AppState>,
) -> Result<String, ErrorResponse> {
    // Example: Return˙
    Ok("Transactions not implemented".to_string())
}

#[command]
pub async fn list_transactions(_state: State<'_, AppState>) -> Result<Vec<TransactionId>, ErrorResponse> {
    Ok(vec![])
}