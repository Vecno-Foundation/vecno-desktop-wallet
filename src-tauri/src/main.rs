mod state;
mod wallet;
mod send_transactions;
mod get_transactions;
mod balance;
mod node;

use state::AppState;
use tauri::async_runtime::Mutex;
use vecno_wrpc_client::prelude::Resolver;
use vecno_wallet_core::settings::ensure_application_folder;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    if let Err(e) = ensure_application_folder().await {
        eprintln!("Failed to create application folder: {}", e);
    }

    let resolver = Resolver::default();

    tauri::Builder::default()
        .manage(AppState {
            wallet: Mutex::new(None),
            resolver: Mutex::new(Some(resolver)),
            wallet_secret: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            wallet::is_wallet_open,
            node::is_node_connected,
            node::get_node_info,
            wallet::create_wallet,
            wallet::import_wallets,
            wallet::generate_mnemonic,
            wallet::get_address,
            balance::get_balance,
            send_transactions::send_transaction,
            wallet::list_wallets,
            get_transactions::list_transactions,
            wallet::open_wallet
        ])
        .run(tauri::generate_context!())
        .expect("Error running Vecno Wallet App");
}