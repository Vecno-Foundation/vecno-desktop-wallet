use crate::state::{AppState, ErrorResponse};
use tauri::{command, State};

#[command]
pub async fn send_transaction(
    _to_address: String,
    _amount: u64,
    _state: State<'_, AppState>,
) -> Result<String, ErrorResponse> {
    // Example: Return˙
    Ok("Transactions not implemented".to_string())
}