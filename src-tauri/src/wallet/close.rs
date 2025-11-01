// src/commands.rs or wherever your commands are
use crate::state::{AppState, ErrorResponse, NodeCache};
use tauri::{command, AppHandle, State, Manager};
use log::{error, info};

#[command]
pub async fn close_wallet(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), ErrorResponse> {
    info!("Closing wallet and exiting app");

    // === Wallet Cleanup ===
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
    } // drop guard

    // === Clear App State ===
    let mut wallet_state = state.wallet.lock().await;
    *wallet_state = None;

    let mut secret_state = state.wallet_secret.lock().await;
    *secret_state = None;

    let mut mnemonic_state = state.mnemonic.lock().await;
    *mnemonic_state = None;

    let mut node_cache = state.node_cache.lock().await;
    *node_cache = NodeCache::default();

    info!("Wallet closed. Requesting graceful shutdown...");

    // === CORRECT METHOD: get_webview_window ===
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.close();
    } else {
        error!("Main window not found! Forcing exit.");
        app.exit(0);
    }

    Ok(())
}