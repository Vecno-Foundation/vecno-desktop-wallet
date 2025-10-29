mod wallet;
mod state;
mod checks;
mod send_transactions;
mod get_transactions;
mod balance;
mod node;

use state::{AppState, NodeCache};
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
            mnemonic: Mutex::new(None),  
            node_cache: Mutex::new(NodeCache::default()),
        })
        .invoke_handler(tauri::generate_handler![
            checks::is_wallet_open,
            node::is_node_connected,
            node::get_node_info,
            wallet::create::create_wallet,
            wallet::import::import_wallets,
            checks::generate_mnemonic,
            checks::get_address,
            balance::get_balance,
            send_transactions::send_transaction,
            checks::list_wallets,
            get_transactions::list_transactions,
            wallet::open::open_wallet
        ])
        .run(tauri::generate_context!())
        .expect("Error running Vecno Wallet App");
}