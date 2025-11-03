use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::async_runtime::Mutex;
use vecno_wallet_core::prelude::*;
use vecno_wrpc_client::prelude::Resolver;
use vecno_wallet_core::error::Error as WalletError;
use std::io;

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInfo {
    pub url: String,
}

#[derive(Default)]
pub struct NodeCache {
    pub url: Option<String>,
}

pub struct AppState {
    pub wallet: Mutex<Option<Arc<Wallet>>>,
    pub resolver: Mutex<Option<Resolver>>,
    pub wallet_secret: Mutex<Option<Secret>>,
    pub mnemonic: Mutex<Option<String>>,
    pub node_cache: Mutex<NodeCache>,
}

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

impl From<WalletError> for ErrorResponse {
    fn from(err: WalletError) -> Self {
        ErrorResponse { error: err.to_string() }
    }
}

impl From<io::Error> for ErrorResponse {
    fn from(err: io::Error) -> Self {
        ErrorResponse { error: err.to_string() }
    }
}