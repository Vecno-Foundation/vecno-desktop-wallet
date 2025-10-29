use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use web_sys::{HtmlInputElement, HtmlSelectElement, BeforeUnloadEvent, HtmlUListElement};
use log::{error, info, debug};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct WalletAddress {
    account_name: String,
    account_index: u32,
    receive_address: String,
    change_address: String,
}


#[derive(Serialize, Deserialize)]
struct CreateWalletArgs {
    secret: String,
    filename: String,
}

#[derive(Serialize, Deserialize)]
struct ImportWalletArgs {
    mnemonic: String,
    secret: String,
    filename: String,
}

#[derive(Serialize, Deserialize)]
struct GetBalanceArgs {
    address: String,
}

#[derive(Serialize, Deserialize)]
struct SendTransactionArgs {
    #[serde(rename = "toAddress")]
    to_address: String,
    /// Amount in **VENI** (1 VE = 100_000_000 VENI)
    amount: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct WalletFile {
    name: String,
    path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Transaction {
    pub txid: String,
    pub to_address: String,
    pub amount: u64,
    pub timestamp: String,
}

#[derive(Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct NodeInfo {
    url: String,
}

#[derive(Clone, PartialEq)]
enum Screen {
    Intro,
    CreateWallet,
    ImportWallet,
    MnemonicDisplay(String),
    Main,
    Transactions,
}

fn is_valid_filename(filename: &str) -> bool {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    !filename.is_empty() && !filename.contains(&invalid_chars[..]) && filename.len() <= 255
}

fn is_valid_password(secret: &str) -> bool {
    secret.len() >= 8
}

fn format_balance(balance: u64) -> String {
    if balance == 0 {
        "Balance: 0 VE".to_string()
    } else {
        let ve = balance as f64 / 100_000_000.0;
        format!("Balance: {:.8} VE", ve)
    }
}

fn format_amount(amount: u64) -> String {
    if amount == 0 {
        "0 VE".to_string()
    } else {
        let ve = amount as f64 / 100_000_000.0;
        format!("{:.8} VE", ve)
    }
}

/// Convert VE (as string) → VENI (u64)
fn ve_to_veni(ve_str: &str) -> Option<u64> {
    let ve = ve_str.trim().parse::<f64>().ok()?;
    if ve <= 0.0 {
        return None;
    }
    let veni = (ve * 100_000_000.0).round() as u64;
    if veni == 0 {
        None
    } else {
        Some(veni)
    }
}

fn clear_status_after_delay(status: UseStateHandle<String>, delay_ms: u64) {
    let status = status.clone();
    spawn_local(async move {
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _| {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay_ms as i32)
                .unwrap();
        }))
        .await
        .unwrap();
        status.set(String::new());
    });
}

async fn fetch_balance(
    addresses: UseStateHandle<Vec<WalletAddress>>,
    balance: UseStateHandle<String>,
    _wallet_status: UseStateHandle<String>,
    is_loading: UseStateHandle<bool>,
) {
    if (*addresses).is_empty() {
        error!("No valid address found for balance query");
        balance.set("Balance: No valid address available".to_string());
        clear_status_after_delay(balance.clone(), 5000);
        is_loading.set(false);
        return;
    }
    let address = (*addresses)
        .first()
        .map(|addr| addr.receive_address.clone())
        .unwrap_or_default();
    info!("Querying balance for address: {}", address);
    let args = serde_wasm_bindgen::to_value(&GetBalanceArgs { address: address.clone() })
        .map_err(|e| {
            error!("Failed to serialize GetBalanceArgs: {:?}", e);
            e
        })
        .unwrap_or(JsValue::NULL);
    let max_attempts = 3;
    let mut attempt = 1;
    let mut result = invoke("get_balance", args.clone()).await;
    while attempt <= max_attempts {
        match serde_wasm_bindgen::from_value::<String>(result.clone()) {
            Ok(balance_str) => {
                debug!("Parsed balance response for {}: {}", address, balance_str);
                match balance_str.parse::<u64>() {
                    Ok(balance_value) => {
                        info!("Balance for address {}: {} VE", address, balance_value);
                        balance.set(format_balance(balance_value));
                        is_loading.set(false);
                        return;
                    }
                    Err(e) => {
                        error!("Failed to parse balance for address {}: {}", address, e);
                        balance.set(format!("Balance: Error - Failed to parse balance: {}", e));
                        clear_status_after_delay(balance.clone(), 5000);
                        is_loading.set(false);
                        return;
                    }
                }
            }
            Err(e) => {
                error!("get_balance attempt {} failed for address {}: {:?}", attempt, address, e);
                match serde_wasm_bindgen::from_value::<ErrorResponse>(result.clone()) {
                    Ok(error_response) => {
                        if error_response.error.contains("Failed to scan for UTXOs")
                            || error_response.error.contains("Failed to connect to node")
                        {
                            attempt += 1;
                            if attempt > max_attempts {
                                error!("get_balance failed after {} attempts for address {}", max_attempts, address);
                                balance.set(format!("Balance: Error - {}", error_response.error));
                                clear_status_after_delay(balance.clone(), 5000);
                                is_loading.set(false);
                                return;
                            }
                            info!("Retrying get_balance (attempt {}/{})", attempt, max_attempts);
                            wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _| {
                                web_sys::window()
                                    .unwrap()
                                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 1000)
                                    .unwrap();
                            }))
                            .await
                            .unwrap();
                            result = invoke("get_balance", args.clone()).await;
                            continue;
                        } else {
                            error!("get_balance failed for address {}: {}", address, error_response.error);
                            balance.set(format!("Balance: Error - {}", error_response.error));
                            clear_status_after_delay(balance.clone(), 5000);
                            is_loading.set(false);
                            return;
                        }
                    }
                    Err(_) => {
                        balance.set(format!("Balance: Error - Failed to deserialize balance response: {:?}", e));
                        clear_status_after_delay(balance.clone(), 5000);
                        is_loading.set(false);
                        return;
                    }
                }
            }
        }
    }
}

#[function_component(App)]
pub fn app() -> Html {
    let screen = use_state(|| Screen::Intro);
    let secret_input_ref = use_node_ref();
    let filename_input_ref = use_node_ref();
    let import_mnemonic_input_ref = use_node_ref();
    let import_secret_input_ref = use_node_ref();
    let import_filename_input_ref = use_node_ref();
    let to_address_input_ref = use_node_ref();
    let amount_input_ref = use_node_ref();
    let selected_wallet_ref = use_node_ref();
    let open_secret_input_ref = use_node_ref();
    let wallet_status = use_state(|| String::new());
    let wallet_created = use_state(|| false);
    let addresses = use_state(|| Vec::<WalletAddress>::new());
    let balance = use_state(|| String::new());
    let transaction_status = use_state(|| String::new());
    let is_loading = use_state(|| false);
    let available_wallets = use_state(|| Vec::<WalletFile>::new());
    let node_connected = use_state(|| false);
    let node_info = use_state(|| NodeInfo { url: String::new() });
    let transactions = use_state(|| Vec::<Transaction>::new());
    let transaction_list_ref = use_node_ref();

    const VERSION: &str = env!("CARGO_PKG_VERSION");

    // Node connection check
    {
        let node_connected = node_connected.clone();
        let node_info = node_info.clone();
        let is_loading = is_loading.clone();
        let wallet_status = wallet_status.clone();
        use_effect_with((), move |_| {
            let node_connected = node_connected.clone();
            let node_info = node_info.clone();
            let is_loading = is_loading.clone();
            let wallet_status = wallet_status.clone();
            spawn_local(async move {
                is_loading.set(true);
                info!("Checking node connection status");
                let conn_result = invoke("is_node_connected", JsValue::NULL).await;
                match serde_wasm_bindgen::from_value::<bool>(conn_result) {
                    Ok(connected) => {
                        node_connected.set(connected);
                        info!("Node connection status: {}", connected);
                        if !connected {
                            wallet_status.set("Warning: Not connected to Vecno node".to_string());
                            node_info.set(NodeInfo { url: "Not connected".to_string() });
                            clear_status_after_delay(wallet_status.clone(), 5000);
                        } else {
                            let info_result = invoke("get_node_info", JsValue::NULL).await;
                            match serde_wasm_bindgen::from_value::<NodeInfo>(info_result) {
                                Ok(info) => {
                                    node_info.set(info.clone());
                                    info!("Connected to node: {}", info.url);
                                }
                                Err(e) => {
                                    error!("get_node_info failed: {:?}", e);
                                    node_info.set(NodeInfo { url: "Unknown node".to_string() });
                                    wallet_status.set("Failed to fetch node info".to_string());
                                    clear_status_after_delay(wallet_status.clone(), 5000);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("is_node_connected failed: {:?}", e);
                        node_connected.set(false);
                        node_info.set(NodeInfo { url: "Not connected".to_string() });
                        wallet_status.set("Failed to check node connection".to_string());
                        clear_status_after_delay(wallet_status, 5000);
                    }
                }
                is_loading.set(false);
            });
            || {}
        });
    }

    // Fetch available wallets on Intro screen
    {
        let screen = screen.clone();
        let available_wallets = available_wallets.clone();
        let is_loading = is_loading.clone();
        let wallet_status = wallet_status.clone();
        use_effect_with(screen.clone(), move |screen| {
            let wallet_status = wallet_status.clone();
            if matches!(**screen, Screen::Intro) {
                let available_wallets = available_wallets.clone();
                let is_loading = is_loading.clone();
                spawn_local(async move {
                    is_loading.set(true);
                    info!("Fetching available wallets");
                    let result = invoke("list_wallets", JsValue::NULL).await;
                    match serde_wasm_bindgen::from_value::<Vec<WalletFile>>(result) {
                        Ok(wallets) => {
                            let wallet_count = wallets.len();
                            available_wallets.set(wallets);
                            info!("Fetched {} wallets", wallet_count);
                        }
                        Err(e) => {
                            error!("list_wallets failed: {:?}", e);
                            available_wallets.set(vec![]);
                            wallet_status.set("Failed to list wallets".to_string());
                            clear_status_after_delay(wallet_status, 5000);
                        }
                    }
                    is_loading.set(false);
                });
            }
            || {}
        });
    }

    // Check wallet openness and fetch addresses
    {
        let screen = screen.clone();
        let wallet_created = wallet_created.clone();
        let addresses = addresses.clone();
        let is_loading = is_loading.clone();
        let wallet_status = wallet_status.clone();
        use_effect_with((screen.clone(), wallet_created.clone()), move |(screen, created)| {
            let wallet_status = wallet_status.clone();
            let addresses = addresses.clone();
            let is_loading = is_loading.clone();
            let screen = screen.clone();
            let wallet_created = wallet_created.clone();
            if **created && matches!(*screen, Screen::Main) {
                spawn_local(async move {
                    is_loading.set(true);
                    info!("Checking if wallet is open");
                    let is_open_result = invoke("is_wallet_open", JsValue::NULL).await;
                    match serde_wasm_bindgen::from_value::<bool>(is_open_result) {
                        Ok(is_open) if is_open => {
                            info!("Wallet is open, fetching addresses");
                            let result = invoke("get_address", JsValue::NULL).await;
                            match serde_wasm_bindgen::from_value::<Vec<WalletAddress>>(result) {
                                Ok(addrs) => {
                                    let addr_count = addrs.len();
                                    debug!("Setting addresses state: {:?}", addrs);
                                    addresses.set(addrs);
                                    info!("Fetched {} addresses", addr_count);
                                    if addr_count == 0 {
                                        wallet_status.set("Warning: No addresses retrieved".to_string());
                                        clear_status_after_delay(wallet_status.clone(), 5000);
                                    }
                                }
                                Err(e) => {
                                    error!("get_address failed: {:?}", e);
                                    addresses.set(vec![]);
                                    wallet_status.set(format!("Failed to fetch addresses: {:?}", e));
                                    clear_status_after_delay(wallet_status, 5000);
                                }
                            }
                            is_loading.set(false);
                        }
                        Ok(_) => {
                            error!("Wallet is not open, reverting to Intro screen");
                            wallet_status.set("Wallet is not open, please open or create a wallet".to_string());
                            screen.set(Screen::Intro);
                            wallet_created.set(false);
                            clear_status_after_delay(wallet_status, 5000);
                        }
                        Err(e) => {
                            error!("is_wallet_open failed: {:?}", e);
                            wallet_status.set("Failed to check wallet status".to_string());
                            screen.set(Screen::Intro);
                            wallet_created.set(false);
                            clear_status_after_delay(wallet_status, 5000);
                        }
                    }
                });
            }
            || {}
        });
    }

    // Fetch balance when addresses change
    {
        let addresses = addresses.clone();
        let balance = balance.clone();
        let is_loading = is_loading.clone();
        let wallet_status = wallet_status.clone();
        use_effect_with(addresses.clone(), move |addresses| {
            let addresses = addresses.clone();
            let balance = balance.clone();
            let is_loading = is_loading.clone();
            let wallet_status = wallet_status.clone();
            if !addresses.is_empty() {
                spawn_local(async move {
                    is_loading.set(true);
                    fetch_balance(addresses.clone(), balance.clone(), wallet_status.clone(), is_loading.clone()).await;
                });
            }
            || {}
        });
    }

    // Fetch recent transactions on Transactions screen
    {
        let screen = screen.clone();
        let transactions = transactions.clone();
        let is_loading = is_loading.clone();
        let wallet_status = wallet_status.clone();
        use_effect_with(screen.clone(), move |screen| {
            let wallet_status = wallet_status.clone();
            if matches!(**screen, Screen::Transactions) {
                let transactions = transactions.clone();
                let is_loading = is_loading.clone();
                spawn_local(async move {
                    is_loading.set(true);
                    info!("Fetching recent transactions");
                    let result = invoke("list_transactions", JsValue::NULL).await;
                    match serde_wasm_bindgen::from_value::<Vec<Transaction>>(result) {
                        Ok(txns) => {
                            let tx_count = txns.len();
                            transactions.set(txns);
                            info!("Fetched {} transactions", tx_count);
                        }
                        Err(e) => {
                            error!("list_transactions failed: {:?}", e);
                            transactions.set(vec![]);
                            wallet_status.set("Failed to list transactions".to_string());
                            clear_status_after_delay(wallet_status, 5000);
                        }
                    }
                    is_loading.set(false);
                });
            }
            || {}
        });
    }

    // Transaction navigation callbacks
    let prev_transaction = {
        let list_ref = transaction_list_ref.clone();
        Callback::from(move |_| {
            if let Some(ul) = list_ref.cast::<HtmlUListElement>() {
                let width = ul.client_width() as f64;
                let current_scroll = ul.scroll_left() as f64;
                let new_scroll = (current_scroll - width).max(0.0) as i32;
                ul.set_scroll_left(new_scroll);
            }
        })
    };

    let next_transaction = {
        let list_ref = transaction_list_ref.clone();
        Callback::from(move |_| {
            if let Some(ul) = list_ref.cast::<HtmlUListElement>() {
                let width = ul.client_width() as f64;
                let current_scroll = ul.scroll_left() as f64;
                let new_scroll = (current_scroll + width) as i32;
                ul.set_scroll_left(new_scroll);
            }
        })
    };

    let proceed_to_create = {
        let screen = screen.clone();
        Callback::from(move |_: MouseEvent| {
            screen.set(Screen::CreateWallet);
        })
    };

    let proceed_to_import = {
        let screen = screen.clone();
        Callback::from(move |_: MouseEvent| {
            screen.set(Screen::ImportWallet);
        })
    };

    let navigate_to_main = {
        let screen = screen.clone();
        let addresses = addresses.clone();
        let balance = balance.clone();
        let wallet_status = wallet_status.clone();
        let is_loading = is_loading.clone();
        Callback::from(move |_: MouseEvent| {
            screen.set(Screen::Main);
            if !(*addresses).is_empty() {
                spawn_local(fetch_balance(addresses.clone(), balance.clone(), wallet_status.clone(), is_loading.clone()));
            }
        })
    };

    let navigate_to_transactions = {
        let screen = screen.clone();
        Callback::from(move |_: MouseEvent| {
            screen.set(Screen::Transactions);
        })
    };

    let navigate_to_intro = {
        let screen = screen.clone();
        let wallet_created = wallet_created.clone();
        Callback::from(move |_: MouseEvent| {
            screen.set(Screen::Intro);
            wallet_created.set(false);
        })
    };

    let open_wallet = {
        let wallet_status = wallet_status.clone();
        let wallet_created = wallet_created.clone();
        let screen = screen.clone();
        let selected_wallet_ref = selected_wallet_ref.clone();
        let open_secret_input_ref = open_secret_input_ref.clone();
        let is_loading = is_loading.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let filename = selected_wallet_ref
                .cast::<HtmlSelectElement>()
                .map(|select| select.value())
                .unwrap_or_default();
            let secret = open_secret_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().to_string())
                .unwrap_or_default();
            if filename.is_empty() {
                wallet_status.set("Please select a wallet".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if secret.is_empty() {
                wallet_status.set("Wallet password is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if !is_valid_password(&secret) {
                wallet_status.set("Password must be at least 8 characters long".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            let wallet_status = wallet_status.clone();
            let wallet_created = wallet_created.clone();
            let screen = screen.clone();
            let is_loading = is_loading.clone();
            let open_secret_input_ref = open_secret_input_ref.clone();
            spawn_local(async move {
                is_loading.set(true);
                info!("Attempting to open wallet: {}", filename);
                let args = serde_wasm_bindgen::to_value(&CreateWalletArgs {
                    secret,
                    filename,
                }).unwrap_or(JsValue::NULL);
                let result = invoke("open_wallet", args).await;
                match serde_wasm_bindgen::from_value::<ErrorResponse>(result.clone()) {
                    Ok(error_response) => {
                        if error_response.error == "Incorrect password provided" {
                            wallet_status.set("Incorrect password provided".to_string());
                            error!("Failed to open wallet: incorrect password");
                            if let Some(input) = open_secret_input_ref.cast::<HtmlInputElement>() {
                                input.set_value("");
                            }
                        } else {
                            wallet_status.set(format!("Error: {}", error_response.error));
                            error!("Failed to open wallet: {}", error_response.error);
                        }
                        clear_status_after_delay(wallet_status.clone(), 5000);
                        is_loading.set(false);
                    }
                    Err(_) => {
                        match result.as_string() {
                            Some(msg) if msg.contains("Success") => {
                                wallet_status.set("Wallet opened successfully!".to_string());
                                wallet_created.set(true);
                                screen.set(Screen::Main);
                                info!("Wallet opened successfully, navigating to Main screen");
                                clear_status_after_delay(wallet_status.clone(), 3000);
                            }
                            Some(msg) => {
                                wallet_status.set(format!("Error: {}", msg));
                                error!("Wallet open failed: {}", msg);
                                clear_status_after_delay(wallet_status.clone(), 5000);
                                is_loading.set(false);
                            }
                            None => {
                                error!("open_wallet returned unexpected result: {:?}", result);
                                wallet_status.set("Failed to open wallet (unexpected response)".to_string());
                                clear_status_after_delay(wallet_status.clone(), 5000);
                                is_loading.set(false);
                            }
                        }
                    }
                }
            });
        })
    };

    let copy_mnemonic = {
        let wallet_status = wallet_status.clone();
        Callback::from(move |mnemonic: String| {
            let wallet_status = wallet_status.clone();
            spawn_local(async move {
                if let Some(window) = web_sys::window() {
                    let navigator = window.navigator();
                    let clipboard = navigator.clipboard();
                    let promise = clipboard.write_text(&mnemonic);
                    match wasm_bindgen_futures::JsFuture::from(promise).await {
                        Ok(_) => {
                            wallet_status.set("Mnemonic copied to clipboard!".to_string());
                            info!("Mnemonic copied to clipboard");
                            clear_status_after_delay(wallet_status.clone(), 5000);
                        }
                        Err(e) => {
                            error!("Clipboard write failed: {:?}", e);
                            wallet_status.set("Failed to copy mnemonic".to_string());
                            clear_status_after_delay(wallet_status.clone(), 5000);
                        }
                    }
                } else {
                    error!("Clipboard not available");
                    wallet_status.set("Clipboard not available".to_string());
                    clear_status_after_delay(wallet_status.clone(), 5000);
                }
            });
        })
    };

    let create_wallet = {
        let wallet_status = wallet_status.clone();
        let wallet_created = wallet_created.clone();
        let secret_input_ref = secret_input_ref.clone();
        let filename_input_ref = filename_input_ref.clone();
        let screen = screen.clone();
        let is_loading = is_loading.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let secret = secret_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().to_string())
                .unwrap_or_default();
            let filename = filename_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().to_string())
                .unwrap_or_default();
            if secret.is_empty() {
                wallet_status.set("Wallet password is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if filename.is_empty() {
                wallet_status.set("Wallet filename is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if !is_valid_password(&secret) {
                wallet_status.set("Password must be at least 8 characters long".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if !is_valid_filename(&filename) {
                wallet_status.set("Invalid filename (use letters, numbers, or underscores only)".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            let wallet_status = wallet_status.clone();
            let wallet_created = wallet_created.clone();
            let screen = screen.clone();
            let is_loading = is_loading.clone();
            spawn_local(async move {
                is_loading.set(true);
                info!("Creating wallet with filename: {}", filename);
                let args = serde_wasm_bindgen::to_value(&CreateWalletArgs { secret, filename }).unwrap_or(JsValue::NULL);
                let result = invoke("create_wallet", args).await;
                if js_sys::Reflect::get(&result, &JsValue::from_str("error")).is_ok() {
                    let error_msg = js_sys::Reflect::get(&result, &JsValue::from_str("error"))
                        .map(|v| v.as_string().unwrap_or_default())
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    wallet_status.set(format!("Error: {}", error_msg));
                    error!("Failed to create wallet: {}", error_msg);
                    clear_status_after_delay(wallet_status.clone(), 5000);
                    is_loading.set(false);
                } else if let Some(msg) = result.as_string() {
                    if msg.contains("Success") {
                        if let Some(mnemonic) = msg.split("with mnemonic: ").nth(1) {
                            wallet_status.set("Wallet created successfully!".to_string());
                            wallet_created.set(true);
                            screen.set(Screen::MnemonicDisplay(mnemonic.to_string()));
                            info!("Wallet created successfully, displaying mnemonic");
                            clear_status_after_delay(wallet_status.clone(), 5000);
                        } else {
                            wallet_status.set("Mnemonic not found in response".to_string());
                            error!("Mnemonic not found in create_wallet response");
                            clear_status_after_delay(wallet_status.clone(), 5000);
                            is_loading.set(false);
                        }
                    } else {
                        wallet_status.set(format!("Error: {}", msg));
                        error!("Wallet creation failed: {}", msg);
                        clear_status_after_delay(wallet_status.clone(), 5000);
                        is_loading.set(false);
                    }
                } else {
                    error!("create_wallet failed with unexpected result: {:?}", result);
                    wallet_status.set("Failed to create wallet (check console for details)".to_string());
                    clear_status_after_delay(wallet_status.clone(), 5000);
                    is_loading.set(false);
                }
            });
        })
    };

    let import_wallets = {
        let wallet_status = wallet_status.clone();
        let wallet_created = wallet_created.clone();
        let import_mnemonic_input_ref = import_mnemonic_input_ref.clone();
        let import_secret_input_ref = import_secret_input_ref.clone();
        let import_filename_input_ref = import_filename_input_ref.clone();
        let screen = screen.clone();
        let is_loading = is_loading.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let mnemonic = import_mnemonic_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().to_string())
                .unwrap_or_default();
            let secret = import_secret_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().to_string())
                .unwrap_or_default();
            let filename = import_filename_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().to_string())
                .unwrap_or_default();
            if mnemonic.is_empty() {
                wallet_status.set("Mnemonic is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if secret.is_empty() {
                wallet_status.set("Wallet password is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if filename.is_empty() {
                wallet_status.set("Wallet filename is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            let word_count = mnemonic.split_whitespace().count();
            if word_count != 12 && word_count != 24 {
                wallet_status.set("Mnemonic must be 12 or 24 words".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if !is_valid_password(&secret) {
                wallet_status.set("Password must be at least 8 characters long".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if !is_valid_filename(&filename) {
                wallet_status.set("Invalid filename (use letters, numbers, or underscores only)".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            let wallet_status = wallet_status.clone();
            let wallet_created = wallet_created.clone();
            let screen = screen.clone();
            let is_loading = is_loading.clone();
            spawn_local(async move {
                is_loading.set(true);
                info!("Importing wallet with filename: {}", filename);
                let args = serde_wasm_bindgen::to_value(&ImportWalletArgs { mnemonic, secret, filename }).unwrap_or(JsValue::NULL);
                let result = invoke("import_wallets", args).await;
                if js_sys::Reflect::get(&result, &JsValue::from_str("error")).is_ok() {
                    let error_msg = js_sys::Reflect::get(&result, &JsValue::from_str("error"))
                        .map(|v| v.as_string().unwrap_or_default())
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    wallet_status.set(format!("Error: {}", error_msg));
                    error!("Failed to import wallet: {}", error_msg);
                    clear_status_after_delay(wallet_status.clone(), 5000);
                    is_loading.set(false);
                } else if let Some(msg) = result.as_string() {
                    if msg.contains("Success") {
                        wallet_status.set("Wallet imported successfully!".to_string());
                        wallet_created.set(true);
                        screen.set(Screen::Main);
                        info!("Wallet imported successfully, navigating to Main screen");
                        clear_status_after_delay(wallet_status.clone(), 3000);
                    } else {
                        wallet_status.set(format!("Error: {}", msg));
                        error!("Wallet import failed: {}", msg);
                        clear_status_after_delay(wallet_status.clone(), 5000);
                        is_loading.set(false);
                    }
                } else {
                    error!("import_wallets failed with unexpected result: {:?}", result);
                    wallet_status.set("Failed to import wallet".to_string());
                    clear_status_after_delay(wallet_status.clone(), 5000);
                    is_loading.set(false);
                }
            });
        })
    };

    let send_transaction = {
        let transaction_status = transaction_status.clone();
        let to_address_input_ref = to_address_input_ref.clone();
        let amount_input_ref = amount_input_ref.clone();
        let is_loading = is_loading.clone();
        let transactions = transactions.clone();
        let wallet_created = wallet_created.clone();
        let addresses = addresses.clone();
        let balance = balance.clone();
        let wallet_status = wallet_status.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let to_address = to_address_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().to_string())
                .unwrap_or_default();

            let amount_ve_str = amount_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().to_string())
                .unwrap_or_default();

            let amount_veni = match ve_to_veni(&amount_ve_str) {
                Some(s) => s,
                None => {
                    transaction_status.set("Invalid amount – must be > 0 VE".to_string());
                    clear_status_after_delay(transaction_status.clone(), 5000);
                    return;
                }
            };

            if to_address.is_empty() || amount_veni == 0 {
                transaction_status.set("Recipient address and amount are required".to_string());
                clear_status_after_delay(transaction_status.clone(), 5000);
                return;
            }
            if !to_address.starts_with("vecno:") {
                transaction_status.set("Invalid address (must start with vecno:)".to_string());
                clear_status_after_delay(transaction_status.clone(), 5000);
                return;
            }
            if !*wallet_created {
                transaction_status.set("No wallet is open".to_string());
                clear_status_after_delay(transaction_status.clone(), 5000);
                return;
            }

            let transaction_status = transaction_status.clone();
            let is_loading = is_loading.clone();
            let transactions = transactions.clone();
            let addresses = addresses.clone();
            let balance = balance.clone();
            let wallet_status = wallet_status.clone();

            spawn_local(async move {
                is_loading.set(true);
                info!("Sending {} VE ({} VENI) to {}", amount_ve_str, amount_veni, to_address);
                let args = serde_wasm_bindgen::to_value(&SendTransactionArgs {
                    to_address,
                    amount: amount_veni,
                }).unwrap_or(JsValue::NULL);
                let result = invoke("send_transaction", args).await;

                if js_sys::Reflect::get(&result, &JsValue::from_str("error")).is_ok() {
                    let error_msg = js_sys::Reflect::get(&result, &JsValue::from_str("error"))
                        .map(|v| v.as_string().unwrap_or_default())
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    transaction_status.set(format!("Error: {}", error_msg));
                    error!("Send failed: {}", error_msg);
                    clear_status_after_delay(transaction_status.clone(), 5000);
                } else if let Some(msg) = result.as_string() {
                    transaction_status.set(format!("Success: {}", msg));
                    info!("Transaction sent: {}", msg);
                    clear_status_after_delay(transaction_status.clone(), 3000);

                    let tx_result = invoke("list_transactions", JsValue::NULL).await;
                    if let Ok(txns) = serde_wasm_bindgen::from_value::<Vec<Transaction>>(tx_result) {
                        transactions.set(txns);
                    }
                    if !(*addresses).is_empty() {
                        fetch_balance(addresses.clone(), balance.clone(), wallet_status.clone(), is_loading.clone()).await;
                    }
                } else {
                    transaction_status.set("Failed to send transaction".to_string());
                    clear_status_after_delay(transaction_status.clone(), 5000);
                }
                is_loading.set(false);
            });
        })
    };

    html! {
        <div class="app-container">
            <div class="node-status node-status-fixed" aria-live="polite">
                <div class={classes!("node-indicator", if *node_connected { "connected" } else { "disconnected" })}></div>
                <span class="node-status-text">{ if *node_connected { "Connected" } else { "Disconnected" } }</span>
                <span class="node-tooltip">{ &node_info.url }</span>
            </div>
            <div class="app-title">{ format!("Vecno Wallet v{}", VERSION) }</div>
            { match &*screen {
                Screen::Intro => html! {
                    <>
                        <main class="container" role="main" aria-label="Welcome to Vecno Wallet">
                            <div class="row">
                                <img src="public/vecnotest.png" class="logo vecno" alt="Vecno logo"/>
                            </div>
                            <p>{"Your gateway to secure and decentralized wallet management."}</p>
                            { if available_wallets.is_empty() && *is_loading {
                                html! { <p aria-live="polite">{"Scanning for wallets..."}</p> }
                            } else if available_wallets.is_empty() {
                                html! { <p aria-live="polite">{""}</p> }
                            } else {
                                html! {
                                    <form class="row" onsubmit={open_wallet} aria-label="Open existing wallet">
                                        <label for="wallet-select" class="sr-only">{"Select a wallet"}</label>
                                        <select id="wallet-select" ref={selected_wallet_ref} class="input" aria-required="true">
                                            <option value="" selected=true disabled=true>{"Select a wallet"}</option>
                                            { for (*available_wallets).iter().map(|wallet| html! {
                                                <option value={wallet.path.clone()}>{ &wallet.name }</option>
                                            }) }
                                        </select>
                                        <label for="open-secret-input" class="sr-only">{"Wallet password"}</label>
                                        <input
                                            id="open-secret-input"
                                            ref={open_secret_input_ref}
                                            type="password"
                                            placeholder="Enter wallet password"
                                            class="input"
                                            aria-required="true"
                                        />
                                        <button
                                            type="submit"
                                            disabled={*is_loading}
                                            class={classes!("btn", "btn-primary", if *is_loading { "loading" } else { "" })}
                                            aria-busy={is_loading.to_string()}
                                        >
                                            {"Open Wallet"}
                                        </button>
                                    </form>
                                }
                            }}
                            <p class="status" aria-live="assertive">{ &*wallet_status }</p>
                            <button onclick={proceed_to_create} class="btn btn-primary" aria-label="Create a new wallet">{"Create New Wallet"}</button><br /><br />
                            <p>{"Have a mnemonic? "}<a href="#" onclick={proceed_to_import} aria-label="Import a wallet using mnemonic">{"Import Wallet"}</a></p>
                        </main>
                    </>
                },
                Screen::CreateWallet => html! {
                    <main class="container" role="main" aria-label="Create New Wallet">
                        <p>{"Create a new wallet to start managing your Vecno assets."}</p>
                        <form class="row" onsubmit={create_wallet} aria-label="Create new wallet form">
                            <label for="filename-input" class="sr-only">{"Wallet filename"}</label>
                            <input
                                id="filename-input"
                                ref={filename_input_ref}
                                placeholder="Wallet filename (e.g., mywallet)"
                                class="input"
                                aria-required="true"
                            />
                            <label for="secret-input" class="sr-only">{"Wallet password"}</label>
                            <input
                                id="secret-input"
                                ref={secret_input_ref}
                                type="password"
                                placeholder="Enter wallet password"
                                class="input"
                                aria-required="true"
                            />
                            <button
                                type="submit"
                                disabled={*is_loading}
                                class={classes!("btn", "btn-primary", if *is_loading { "loading" } else { "" })}
                                aria-busy={is_loading.to_string()}
                            >
                                {"Create Wallet"}
                            </button>
                        </form>
                        <p class="status" aria-live="assertive">{ &*wallet_status }</p>
                        <button onclick={navigate_to_intro.clone()} class="btn btn-secondary" aria-label="Go back to main menu">{"Go Back to Menu"}</button><br /><br />
                        <p>{"Have a mnemonic? "}<a href="#" onclick={proceed_to_import} aria-label="Import a wallet using mnemonic">{"Import Wallet"}</a></p>
                    </main>
                },
                Screen::ImportWallet => html! {
                    <main class="container" role="main" aria-label="Import Wallet">
                        <p>{"Import an existing wallet using your 12 or 24-word mnemonic phrase."}</p>
                        <form class="row" onsubmit={import_wallets} aria-label="Import wallet form">
                            <label for="import-filename-input" class="sr-only">{"Wallet filename"}</label>
                            <input
                                id="import-filename-input"
                                ref={import_filename_input_ref}
                                placeholder="Wallet filename (e.g., mywallet)"
                                class="input"
                                aria-required="true"
                            />
                            <label for="import-mnemonic-input" class="sr-only">{"Mnemonic phrase"}</label>
                            <input
                                id="import-mnemonic-input"
                                ref={import_mnemonic_input_ref}
                                placeholder="Enter 12 or 24-word mnemonic"
                                class="input"
                                aria-required="true"
                            />
                            <label for="import-secret-input" class="sr-only">{"New wallet password"}</label>
                            <input
                                id="import-secret-input"
                                ref={import_secret_input_ref}
                                type="password"
                                placeholder="Enter new password"
                                class="input"
                                aria-required="true"
                            />
                            <button
                                type="submit"
                                disabled={*is_loading}
                                class={classes!("btn", "btn-primary", if *is_loading { "loading" } else { "" })}
                                aria-busy={is_loading.to_string()}
                            >
                                {"Import Wallet"}
                            </button>
                        </form>
                        <p class="status" aria-live="assertive">{ &*wallet_status }</p>
                        <button onclick={navigate_to_intro.clone()} class="btn btn-secondary" aria-label="Go back to main menu">{"Go Back to Menu"}</button><br /><br />
                        <p>{"Want to create a new wallet? "}<a href="#" onclick={proceed_to_create} aria-label="Create a new wallet">{"Create New Wallet"}</a></p>
                    </main>
                },
                Screen::MnemonicDisplay(mnemonic) => {
                    let mnemonic_clone = mnemonic.clone();
                    let copy = copy_mnemonic.clone();
                    let proceed = {
                        let screen = screen.clone();
                        Callback::from(move |_: MouseEvent| {
                            screen.set(Screen::Main);
                        })
                    };
                    html! {
                        <main class="container" role="main" aria-label="Wallet Created">
                            <h2>{"Wallet Created Successfully"}</h2>
                            <p class="instruction-text">
                                {"Please save your 12 or 24-word mnemonic phrase securely. This is critical for recovering your wallet."}
                            </p>
                            <div class="mnemonic-box" aria-label="Mnemonic phrase">
                                <p>{ &mnemonic_clone }</p>
                            </div>
                            <div class="row button-group">
                                <button
                                    onclick={move |_| copy.emit(mnemonic_clone.clone())}
                                    class="btn btn-secondary"
                                    aria-label="Copy mnemonic to clipboard"
                                >
                                    {"Copy Mnemonic"}
                                </button>
                                <button
                                    onclick={proceed}
                                    class="btn btn-primary btn-prominent"
                                    aria-label="Proceed to wallet"
                                >
                                    {"Proceed to Wallet"}
                                </button>
                                <button
                                    onclick={navigate_to_intro.clone()}
                                    class="btn btn-secondary"
                                    aria-label="Go back to main menu"
                                >
                                    {"Go Back to Menu"}
                                </button>
                            </div>
                            <p class="status" aria-live="assertive">{ &*wallet_status }</p>
                        </main>
                    }
                },
                Screen::Main => html! {
                    <main class="container" role="main" aria-label="Vecno Wallet Dashboard">
                        <div class="balance-container" aria-live="assertive">
                            <h2>{"Wallet Balance"}</h2>
                            <p class={classes!("balance", if *is_loading && (*balance).is_empty() { "loading" } else { "" })}>
                                { if *is_loading && (*balance).is_empty() {
                                    "Fetching balance..."
                                } else {
                                    &*balance
                                }}
                            </p>
                        </div>
                        <p>{"Manage your Vecno wallet: check balance and view transactions."}</p>
                        <div>
                            <h3>{"Addresses"}</h3>
                            { if addresses.is_empty() && *is_loading {
                                html! { <p aria-live="polite">{"Loading addresses..."}</p> }
                            } else if addresses.is_empty() {
                                html! { <p class="status" aria-live="assertive">{"No addresses found. Try refreshing or check wallet setup."}</p> }
                            } else {
                                html! {
                                    <ul class="address-list" aria-label="Wallet addresses">
                                        { for (*addresses).iter().map(|addr| html! {
                                            <li>
                                                <strong>{ format!("Account: {} (Index: {})", addr.account_name, addr.account_index) }</strong><br />
                                                { "Receive Address: " }{ &addr.receive_address }<br />
                                                { "Change Address: " }{ &addr.change_address }
                                            </li>
                                        }) }
                                    </ul>
                                }
                            }}
                        </div>
                        <div class="row">
                            <button onclick={navigate_to_transactions} class="btn btn-primary" aria-label="View transaction history">{"View Transactions"}</button>
                            <button onclick={navigate_to_intro.clone()} class="btn btn-secondary" aria-label="Go back to main menu">{"Go Back to Menu"}</button>
                        </div>
                    </main>
                },
                Screen::Transactions => html! {
                    <main class="container" role="main" aria-label="Transactions">
                        <div class="balance-container" aria-live="assertive">
                            <h2>{"Wallet Balance"}</h2>
                            <p class={classes!("balance", if *is_loading && (*balance).is_empty() { "loading" } else { "" })}>
                                { if *is_loading && (*balance).is_empty() {
                                    "Fetching balance..."
                                } else {
                                    &*balance
                                }}
                            </p>
                        </div>
                        <p>{"Send transactions and view recent transaction history."}</p>
                        <form class="row" onsubmit={send_transaction} aria-label="Send transaction form">
                            <label for="to-address-input" class="sr-only">{"Recipient address"}</label>
                            <input
                                id="to-address-input"
                                ref={to_address_input_ref}
                                placeholder="Recipient address (e.g., vecno:...)"
                                disabled={*is_loading || !*wallet_created}
                                class="input"
                                aria-required="true"
                            />
                            <label for="amount-input" class="sr-only">{"Amount in VE"}</label>
                            <input
                                id="amount-input"
                                ref={amount_input_ref}
                                type="text"
                                inputmode="decimal"
                                placeholder="Amount to send (VE)"
                                step="any"
                                min="0.00001"
                                disabled={*is_loading || !*wallet_created}
                                class="input"
                                aria-required="true"
                            />
                            <button
                                type="submit"
                                disabled={*is_loading || !*wallet_created}
                                class={classes!("btn", "btn-primary", if *is_loading { "loading" } else { "" })}
                                aria-busy={is_loading.to_string()}
                            >
                                {"Send Transaction"}
                            </button>
                        </form>
                        <p class="status" aria-live="assertive">{ &*transaction_status }</p>
                        <div class="row">
                            <button onclick={navigate_to_main} class="btn btn-secondary" aria-label="Back to wallet dashboard">{"Back to Wallet"}</button>
                            <button onclick={navigate_to_intro.clone()} class="btn btn-secondary" aria-label="Go back to main menu">{"Go Back to Menu"}</button>
                        </div>
                        <h3>{"Recent Transactions"}</h3>
                        { if transactions.is_empty() && *is_loading {
                            html! { <p aria-live="polite">{"Loading transactions..."}</p> }
                        } else if transactions.is_empty() {
                            html! { <p aria-live="polite">{"No recent transactions found."}</p> }
                        } else {
                            html! {
                                <div class="transaction-container">
                                    <button
                                        class="transaction-prev"
                                        onclick={prev_transaction.clone()}
                                        disabled={transactions.is_empty()}
                                        aria-label="Previous transaction"
                                    />
                                    <ul
                                        ref={&transaction_list_ref}
                                        class="transaction-list"
                                        aria-label="Recent transactions"
                                    >
                                        { for (*transactions).iter().enumerate().map(|(index, tx)| {
                                            let list_ref = transaction_list_ref.clone();
                                            let click_callback = {
                                                let list_ref = list_ref.clone();
                                                let index = index;
                                                Callback::from(move |_: MouseEvent| {
                                                    if let Some(ul) = list_ref.cast::<HtmlUListElement>() {
                                                        let width = ul.client_width() as f64;
                                                        let scroll_pos = (index as f64 * width).round() as i32;
                                                        ul.set_scroll_left(scroll_pos);
                                                    }
                                                })
                                            };
                                            html! {
                                                <li
                                                    onclick={click_callback}
                                                    tabindex="0"
                                                    role="button"
                                                    aria-label={format!("View transaction {}", tx.txid)}
                                                >
                                                    <strong>{ format!("TXID: {}", tx.txid) }</strong><br />
                                                    { "To: " }{ &tx.to_address }<br />
                                                    { "Amount: " }{ format_amount(tx.amount) }<br />
                                                    { "Timestamp: " }{ &tx.timestamp }
                                                </li>
                                            }
                                        }) }
                                    </ul>
                                    <button
                                        class="transaction-next"
                                        onclick={next_transaction.clone()}
                                        disabled={transactions.is_empty()}
                                        aria-label="Next transaction"
                                    />
                                </div>
                            }
                        }}
                    </main>
                },
            }}
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    wasm_logger::init(wasm_logger::Config::new(log::Level::Debug));
    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().expect("window not available");
        let document = window.document().expect("document not available");

        let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            let key_js: JsValue = event.key().into();
            if let Some(key_str) = key_js.as_string() {
                if key_str.is_empty() {
                    error!("Keydown event has empty key");
                    return;
                }

                if key_str == "F5" || 
                   (event.ctrl_key() && key_str == "r") || 
                   (event.meta_key() && key_str == "r") {
                    event.prevent_default();
                    info!("Prevented refresh action for key: {}", key_str);
                }
            } else {
                error!("Keydown event has invalid or non-string key: {:?}", key_js);
            }
        }) as Box<dyn FnMut(_)>);

        document
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
            .expect("failed to add keydown listener");
        closure.forget();

        let beforeunload_closure = Closure::wrap(Box::new(move |event: BeforeUnloadEvent| {
            event.prevent_default();
            event.set_return_value("");
        }) as Box<dyn FnMut(_)>);

        window
            .add_event_listener_with_callback("beforeunload", beforeunload_closure.as_ref().unchecked_ref())
            .expect("failed to add beforeunload listener");
        beforeunload_closure.forget();
    });

    yew::Renderer::<App>::new().render();
}