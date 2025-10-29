use crate::state::{AppState, ErrorResponse, NodeCache};
use tauri::{command, State};
use log::{error, info};

#[command]
pub async fn close_wallet(state: State<'_, AppState>) -> Result<String, ErrorResponse> {
    info!("Closing wallet");

    // Stop the selected account if present
    let wallet_guard = state.wallet.lock().await;
    if let Some(wallet) = wallet_guard.as_ref() {
        if let Ok(account) = wallet.account() {
            if let Err(e) = account.stop().await {
                error!("Failed to stop account: {}", e);
                // Continue with cleanup
            }
        }

        // Disconnect wRPC client if present
        if let Some(wrpc_client) = wallet.try_wrpc_client() {
            if let Err(e) = wrpc_client.disconnect().await {
                error!("Failed to disconnect wRPC client: {}", e);
                // Continue with cleanup
            }
        }

        // Close the wallet store (assuming WalletStore::close exists)
        if let Err(e) = wallet.store().close().await {
            error!("Failed to close wallet store: {}", e);
            // Continue with cleanup
        }
    }
    drop(wallet_guard); // Release lock before clearing other states

    // Clear wallet-related state
    let mut wallet_state = state.wallet.lock().await;
    *wallet_state = None;

    let mut secret_state = state.wallet_secret.lock().await;
    *secret_state = None;

    let mut mnemonic_state = state.mnemonic.lock().await;
    *mnemonic_state = None;

    // Clear node cache to ensure disconnected state
    let mut node_cache = state.node_cache.lock().await;
    *node_cache = NodeCache::default();

    // Resolver is left as-is (global/default)

    info!("Wallet closed successfully");
    Ok("Wallet closed successfully".to_string())
}