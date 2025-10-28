use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};
use vecno_wallet_core::prelude::*;

#[command]
pub async fn list_transactions(_state: State<'_, AppState>) -> Result<Vec<TransactionId>, ErrorResponse> {
    Ok(vec![])
}