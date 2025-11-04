
#[derive(serde::Deserialize, Debug)]
pub struct CreateWalletInput {
    pub secret: String,
    pub filename: String,
    pub payment_secret: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
pub struct OpenWalletInput {
    pub secret: String,
    pub filename: String,
    pub payment_secret: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
pub struct ImportWalletInput {
    pub mnemonic: String,
    pub secret: String,
    pub payment_secret: Option<String>,
    pub filename: String,
}

#[derive(serde::Deserialize, Debug)]
pub struct SendTransactionInput {
    pub to_address: String,
    pub amount: u64,
    #[serde(default)]
    pub payment_secret: Option<String>,
}