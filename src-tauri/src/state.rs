use serde::{Serialize, Deserialize};
use tauri::async_runtime::Mutex;
use vecno_wallet_core::prelude::*;
use vecno_wrpc_client::prelude::Resolver;
use std::sync::Arc;

#[derive(Serialize, Debug, Deserialize)]
pub struct WalletAddress {
    pub account_name: String,
    pub account_index: u32,
    pub receive_address: String,
    pub change_address: String,
}

#[derive(Serialize, Deserialize)]
pub struct WalletFile {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInfo {
    pub url: String,
}

pub struct AppState {
    pub wallet: Mutex<Option<Arc<Wallet>>>,
    pub resolver: Mutex<Option<Resolver>>,
    pub wallet_secret: Mutex<Option<Secret>>,
}