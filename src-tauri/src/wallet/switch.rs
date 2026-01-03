use crate::state::{AppState, ErrorResponse, NodeCache};
use tauri::{command, State};
use log::{error, info};

#[command]
pub async fn switch_wallet(state: State<'_, AppState>) -> Result<(), ErrorResponse> {
    info!("Switching wallet — closing current session safely");

    {
        let wallet_guard = state.wallet.lock().await;
        if let Some(wallet) = wallet_guard.as_ref() {
            if let Ok(account) = wallet.account() {
                if let Err(e) = account.stop().await {
                    error!("Failed to stop account: {}", e);
                }
            }

            if let Some(wrpc_client) = wallet.try_wrpc_client() {
                if let Err(e) = wrpc_client.disconnect().await {
                    error!("Failed to disconnect wRPC client: {}", e);
                }
            }

            if let Err(e) = wallet.store().close().await {
                error!("Failed to close wallet store: {}", e);
            }
        }
    }

    // Clear all sensitive state
    let mut wallet_state = state.wallet.lock().await;
    *wallet_state = None;

    let mut secret_state = state.wallet_secret.lock().await;
    *secret_state = None;

    let mut mnemonic_state = state.mnemonic.lock().await;
    *mnemonic_state = None;

    let mut bip39_seed = state.bip39_seed.lock().await;
    *bip39_seed = None;

    let mut node_cache = state.node_cache.lock().await;
    *node_cache = NodeCache::default();

    info!("Wallet session cleared. Ready to open a new wallet.");

    Ok(())
}