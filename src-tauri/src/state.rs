use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::async_runtime::Mutex;
use vecno_wallet_core::prelude::*;
use vecno_wrpc_client::prelude::Resolver;

/// Returned when a command fails – the frontend receives `{ "error": "…" }`
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Information about the currently resolved node
#[derive(Serialize, Deserialize)]
pub struct NodeInfo {
    pub url: String,
}

/// Simple in-memory cache for the node URL – shared between `is_node_connected`
/// and `get_node_info`.  A `None` value means “we have not succeeded yet (or the
/// last attempt failed)”.
#[derive(Default)]
pub struct NodeCache {
    pub url: Option<String>,
}

/// The global application state that Tauri injects into every command.
pub struct AppState {
    /// Open wallet (if any).  `Arc` makes it cheap to clone into async tasks.
    pub wallet: Mutex<Option<Arc<Wallet>>>,

    /// Resolver that can turn a `NetworkId` into a concrete wRPC endpoint.
    pub resolver: Mutex<Option<Resolver>>,

    /// The encryption password of the currently opened wallet – used to decrypt storage.
    pub wallet_secret: Mutex<Option<Secret>>,

    /// The mnemonic phrase (in-memory only while wallet is open).
    pub mnemonic: Mutex<Option<String>>,

    /// Cached node URL – avoids hitting the resolver on every `is_node_connected`
    /// call once we know a valid endpoint.
    pub node_cache: Mutex<NodeCache>,
}

/* -------------------------------------------------------------------------- */
/* Optional helper structs used by other commands (kept for completeness)     */
/* -------------------------------------------------------------------------- */

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

/* -------------------------------------------------------------------------- */
/* Error conversion: allow `?` to convert library errors into ErrorResponse   */
/* -------------------------------------------------------------------------- */

use vecno_wallet_core::error::Error as WalletError;
use std::io;

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