use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WalletAddress {
    pub account_name: String,
    pub account_index: u32,
    pub receive_address: String,
    pub change_address: String,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct CreateWalletArgs {
    pub secret: String,
    pub filename: String,
}

#[derive(Serialize, Debug, Deserialize)]
pub struct ImportWalletArgs {
    pub mnemonic: String,
    pub secret: String,
    pub payment_secret: Option<String>,
    pub filename: String,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct BalanceResponse {
    pub balance: u64,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize)]
pub struct GetBalanceArgs {
    pub address: String,
}

#[derive(Serialize, Deserialize)]
pub struct SendTransactionArgs {
    #[serde(rename = "toAddress")]
    pub to_address: String,
    pub amount: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct WalletFile {
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Transaction {
    pub txid: String,
    pub to_address: String,
    pub amount: u64,
    pub timestamp: String,
}

pub type SentTxInfo = Transaction;

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct NodeInfo {
    pub url: String,
}

#[derive(Clone, PartialEq)]
pub enum Screen {
    Intro,
    Home,
    CreateWallet,
    ImportWallet,
    MnemonicDisplay(String),
    Wallet,
    Receive,
    Transactions,
    Send,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub enum ToastKind {
    #[default]
    Error,
    Success,
    Warning,
    Info,
}

impl ToastKind {
    pub fn class(&self) -> &'static str {
        match self {
            ToastKind::Error => "toast-error",
            ToastKind::Success => "toast-success",
            ToastKind::Warning => "toast-warning",
            ToastKind::Info => "toast-info",
        }
    }
    pub fn icon_mask(&self) -> &'static str {
        match self {
            ToastKind::Error => "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23f87171' stroke-width='2'%3E%3Ccircle cx='12' cy='12' r='10'%3E%3C/circle%3E%3Cline x1='15' y1='9' x2='9' y2='15'%3E%3C/line%3E%3Cline x1='9' y1='9' x2='15' y2='15'%3E%3C/line%3E%3C/svg%3E",
            ToastKind::Success => "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%232dd4bf' stroke-width='2'%3E%3Cpolyline points='20 6 9 17 4 12'%3E%3C/polyline%3E%3C/svg%3E",
            ToastKind::Warning => "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23fbbf24' stroke-width='2'%3E%3Cpath d='M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z'%3E%3C/path%3E%3C/svg%3E",
            ToastKind::Info => "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%236b7280' stroke-width='2'%3E%3Ccircle cx='12' cy='12' r='10'%3E%3C/circle%3E%3Cpath d='M12 16v-4m0-4h.01'%3E%3C/path%3E%3C/svg%3E",
        }
    }
}