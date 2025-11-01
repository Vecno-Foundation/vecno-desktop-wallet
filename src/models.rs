use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WalletAddress {
    pub account_name: String,
    pub account_index: u32,
    pub receive_address: String,
    pub change_address: String,
}


#[derive(Serialize, Deserialize)]
pub struct CreateWalletArgs {
    pub secret: String,
    pub filename: String,
}

#[derive(Serialize, Deserialize)]
pub struct ImportWalletArgs {
    pub mnemonic: String,
    pub secret: String,
    pub filename: String,
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

#[derive(Serialize, Deserialize)]
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
    Transactions,
    Send,
}