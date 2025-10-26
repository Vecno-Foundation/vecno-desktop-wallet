use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use web_sys::{HtmlInputElement, HtmlSelectElement, BeforeUnloadEvent};
use log::{error, info};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
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
    amount: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct WalletFile {
    name: String,
    path: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Transaction {
    txid: String,
    from_address: String,
    to_address: String,
    amount: u64,
    timestamp: String,
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

// Helper function to validate filename
fn is_valid_filename(filename: &str) -> bool {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    !filename.is_empty() && !filename.contains(&invalid_chars[..]) && filename.len() <= 255
}

// Helper function to validate password for creation/import
fn is_valid_password(secret: &str) -> bool {
    secret.len() >= 8 // Minimum 8 characters for basic security
}

// Helper function to clear status messages after a delay
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
    let selected_address_ref = use_node_ref();
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
    let transactions = use_state(|| Vec::<Transaction>::new());

    // Fetch node connection status
    {
        let node_connected = node_connected.clone();
        let is_loading = is_loading.clone();
        let wallet_status = wallet_status.clone();
        use_effect_with((), move |_| {
            let node_connected = node_connected.clone();
            let is_loading = is_loading.clone();
            let wallet_status = wallet_status.clone();
            spawn_local(async move {
                is_loading.set(true);
                let result = invoke("is_node_connected", JsValue::NULL).await;
                match serde_wasm_bindgen::from_value::<bool>(result) {
                    Ok(connected) => {
                        node_connected.set(connected);
                        info!("Node connection status: {}", connected);
                        if !connected {
                            wallet_status.set("Warning: Not connected to Vecno node".to_string());
                            clear_status_after_delay(wallet_status, 5000);
                        }
                    }
                    Err(e) => {
                        error!("is_node_connected failed: {:?}", e);
                        node_connected.set(false);
                        wallet_status.set("Error: Failed to check node connection".to_string());
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
                            wallet_status.set("Error: Failed to list wallets".to_string());
                            clear_status_after_delay(wallet_status, 5000);
                        }
                    }
                    is_loading.set(false);
                });
            }
            || {}
        });
    }

    // Check wallet openness before fetching addresses
    {
        let screen = screen.clone();
        let wallet_created = wallet_created.clone();
        let addresses = addresses.clone();
        let is_loading = is_loading.clone();
        let wallet_status = wallet_status.clone();
        use_effect_with((screen.clone(), wallet_created.clone()), move |(screen, created)| {
            let wallet_status = wallet_status.clone();
            if **created && matches!(**screen, Screen::Main) && addresses.is_empty() {
                let addresses = addresses.clone();
                let is_loading = is_loading.clone();
                let screen = screen.clone();
                let wallet_created = wallet_created.clone();
                spawn_local(async move {
                    is_loading.set(true);
                    let is_open_result = invoke("is_wallet_open", JsValue::NULL).await;
                    match serde_wasm_bindgen::from_value::<bool>(is_open_result) {
                        Ok(is_open) if is_open => {
                            let result = invoke("get_address", JsValue::NULL).await;
                            match serde_wasm_bindgen::from_value::<Vec<WalletAddress>>(result) {
                                Ok(addrs) => {
                                    let addr_count = addrs.len();
                                    addresses.set(addrs);
                                    info!("Fetched {} addresses", addr_count);
                                }
                                Err(e) => {
                                    error!("get_address failed: {:?}", e);
                                    addresses.set(vec![]);
                                    wallet_status.set("Error: Failed to fetch addresses".to_string());
                                    clear_status_after_delay(wallet_status, 5000);
                                }
                            }
                        }
                        Ok(_) => {
                            error!("Wallet is not open, reverting to Intro screen");
                            wallet_status.set("Error: Wallet is not open, please open or create a wallet".to_string());
                            screen.set(Screen::Intro);
                            wallet_created.set(false);
                            clear_status_after_delay(wallet_status, 5000);
                        }
                        Err(e) => {
                            error!("is_wallet_open failed: {:?}", e);
                            wallet_status.set("Error: Failed to check wallet status".to_string());
                            screen.set(Screen::Intro);
                            wallet_created.set(false);
                            clear_status_after_delay(wallet_status, 5000);
                        }
                    }
                    is_loading.set(false);
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
                            wallet_status.set("Error: Failed to list transactions".to_string());
                            clear_status_after_delay(wallet_status, 5000);
                        }
                    }
                    is_loading.set(false);
                });
            }
            || {}
        });
    }

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
        Callback::from(move |_: MouseEvent| {
            screen.set(Screen::Main);
        })
    };

    let navigate_to_transactions = {
        let screen = screen.clone();
        Callback::from(move |_: MouseEvent| {
            screen.set(Screen::Transactions);
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
                wallet_status.set("Error: Please select a wallet".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if secret.is_empty() {
                wallet_status.set("Error: Wallet password is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            let wallet_status = wallet_status.clone();
            let wallet_created = wallet_created.clone();
            let screen = screen.clone();
            let is_loading = is_loading.clone();
            spawn_local(async move {
                is_loading.set(true);
                let args = serde_wasm_bindgen::to_value(&CreateWalletArgs {
                    secret,
                    filename,
                }).expect("Failed to serialize open_wallet args");
                let result = invoke("open_wallet", args).await;
                if js_sys::Reflect::get(&result, &JsValue::from_str("error")).is_ok() {
                    let error_msg = js_sys::Reflect::get(&result, &JsValue::from_str("error"))
                        .map(|v| v.as_string().unwrap_or_default())
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    if error_msg.contains("Wallet file does not exist") {
                        wallet_status.set("Error: Selected wallet file does not exist".to_string());
                    } else if error_msg.contains("No private key data found") {
                        wallet_status.set("Error: Wallet file is corrupted or empty".to_string());
                    } else {
                        wallet_status.set(format!("Error: {}", error_msg));
                    }
                    clear_status_after_delay(wallet_status.clone(), 5000);
                } else if let Some(msg) = result.as_string() {
                    if msg.contains("Success") {
                        wallet_status.set("Wallet opened successfully!".to_string());
                        wallet_created.set(true);
                        screen.set(Screen::Main);
                        clear_status_after_delay(wallet_status.clone(), 3000);
                    } else {
                        wallet_status.set(format!("Error: {}", msg));
                        clear_status_after_delay(wallet_status.clone(), 5000);
                    }
                } else {
                    error!("open_wallet failed with unexpected result: {:?}", result);
                    wallet_status.set("Error: Failed to open wallet (check console for details)".to_string());
                    clear_status_after_delay(wallet_status.clone(), 5000);
                }
                is_loading.set(false);
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
                            clear_status_after_delay(wallet_status.clone(), 3000);
                        }
                        Err(e) => {
                            error!("Clipboard write failed: {:?}", e);
                            wallet_status.set("Error: Failed to copy mnemonic".to_string());
                            clear_status_after_delay(wallet_status.clone(), 5000);
                        }
                    }
                } else {
                    wallet_status.set("Error: Clipboard not available".to_string());
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
                wallet_status.set("Error: Wallet password is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if filename.is_empty() {
                wallet_status.set("Error: Wallet filename is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if !is_valid_password(&secret) {
                wallet_status.set("Error: Password must be at least 8 characters long".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if !is_valid_filename(&filename) {
                wallet_status.set("Error: Invalid filename (use letters, numbers, or underscores only)".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            let wallet_status = wallet_status.clone();
            let wallet_created = wallet_created.clone();
            let screen = screen.clone();
            let is_loading = is_loading.clone();
            spawn_local(async move {
                is_loading.set(true);
                let args = serde_wasm_bindgen::to_value(&CreateWalletArgs { secret, filename }).expect("Failed to serialize create_wallet args");
                let result = invoke("create_wallet", args).await;
                if js_sys::Reflect::get(&result, &JsValue::from_str("error")).is_ok() {
                    let error_msg = js_sys::Reflect::get(&result, &JsValue::from_str("error"))
                        .map(|v| v.as_string().unwrap_or_default())
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    if error_msg.contains("Invalid path") {
                        wallet_status.set("Error: Invalid filename or path".to_string());
                    } else {
                        wallet_status.set(format!("Error: {}", error_msg));
                    }
                    clear_status_after_delay(wallet_status.clone(), 5000);
                } else if let Some(msg) = result.as_string() {
                    if msg.contains("Success") {
                        if let Some(mnemonic) = msg.split("with mnemonic: ").nth(1) {
                            wallet_status.set("Wallet created successfully!".to_string());
                            wallet_created.set(true);
                            screen.set(Screen::MnemonicDisplay(mnemonic.to_string()));
                            clear_status_after_delay(wallet_status.clone(), 3000);
                        } else {
                            wallet_status.set("Error: Mnemonic not found in response".to_string());
                            clear_status_after_delay(wallet_status.clone(), 5000);
                        }
                    } else {
                        wallet_status.set(format!("Error: {}", msg));
                        clear_status_after_delay(wallet_status.clone(), 5000);
                    }
                } else {
                    error!("create_wallet failed with unexpected result: {:?}", result);
                    wallet_status.set("Error: Failed to create wallet (check console for details)".to_string());
                    clear_status_after_delay(wallet_status.clone(), 5000);
                }
                is_loading.set(false);
            });
        })
    };

    let import_wallet = {
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
                wallet_status.set("Error: Mnemonic is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if secret.is_empty() {
                wallet_status.set("Error: Wallet password is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if filename.is_empty() {
                wallet_status.set("Error: Wallet filename is required".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            let word_count = mnemonic.split_whitespace().count();
            if word_count != 24 {
                wallet_status.set("Error: Mnemonic must be 24 words".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if !is_valid_password(&secret) {
                wallet_status.set("Error: Password must be at least 8 characters long".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            if !is_valid_filename(&filename) {
                wallet_status.set("Error: Invalid filename (use letters, numbers, or underscores only)".to_string());
                clear_status_after_delay(wallet_status.clone(), 5000);
                return;
            }
            let wallet_status = wallet_status.clone();
            let wallet_created = wallet_created.clone();
            let screen = screen.clone();
            let is_loading = is_loading.clone();
            spawn_local(async move {
                is_loading.set(true);
                let args = serde_wasm_bindgen::to_value(&ImportWalletArgs { mnemonic, secret, filename }).expect("Failed to serialize import_wallet args");
                let result = invoke("import_wallet", args).await;
                if js_sys::Reflect::get(&result, &JsValue::from_str("error")).is_ok() {
                    let error_msg = js_sys::Reflect::get(&result, &JsValue::from_str("error"))
                        .map(|v| v.as_string().unwrap_or_default())
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    if error_msg.contains("Invalid mnemonic") {
                        wallet_status.set("Error: Invalid mnemonic phrase".to_string());
                    } else if error_msg.contains("Invalid path") {
                        wallet_status.set("Error: Invalid filename or path".to_string());
                    } else {
                        wallet_status.set(format!("Error: {}", error_msg));
                    }
                    clear_status_after_delay(wallet_status.clone(), 5000);
                } else if let Some(msg) = result.as_string() {
                    if msg.contains("Success") {
                        wallet_status.set("Wallet imported successfully!".to_string());
                        wallet_created.set(true);
                        screen.set(Screen::Main);
                        clear_status_after_delay(wallet_status.clone(), 3000);
                    } else {
                        wallet_status.set(format!("Error: {}", msg));
                        clear_status_after_delay(wallet_status.clone(), 5000);
                    }
                } else {
                    error!("import_wallet failed with unexpected result: {:?}", result);
                    wallet_status.set("Error: Failed to import wallet (check console for details)".to_string());
                    clear_status_after_delay(wallet_status.clone(), 5000);
                }
                is_loading.set(false);
            });
        })
    };

    let get_balance = {
        let balance = balance.clone();
        let selected_address_ref = selected_address_ref.clone();
        let is_loading = is_loading.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let address = selected_address_ref
                .cast::<HtmlSelectElement>()
                .map(|select| select.value())
                .unwrap_or_default();
            if address.is_empty() {
                balance.set("Error: Please select an address".to_string());
                clear_status_after_delay(balance.clone(), 5000);
                return;
            }
            let balance = balance.clone();
            let is_loading = is_loading.clone();
            spawn_local(async move {
                is_loading.set(true);
                let args = serde_wasm_bindgen::to_value(&GetBalanceArgs { address }).expect("Failed to serialize get_balance args");
                let result = invoke("get_balance", args).await;
                if js_sys::Reflect::get(&result, &JsValue::from_str("error")).is_ok() {
                    let error_msg = js_sys::Reflect::get(&result, &JsValue::from_str("error"))
                        .map(|v| v.as_string().unwrap_or_default())
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    if error_msg.contains("Invalid address") {
                        balance.set("Error: Invalid address selected".to_string());
                    } else if error_msg.contains("No balance available") {
                        balance.set("Error: No balance data available".to_string());
                    } else {
                        balance.set(format!("Error: {}", error_msg));
                    }
                    clear_status_after_delay(balance.clone(), 5000);
                } else if let Some(msg) = result.as_string() {
                    balance.set(format!("Balance: {} VE", msg));
                    clear_status_after_delay(balance.clone(), 3000);
                } else {
                    error!("get_balance failed with unexpected result: {:?}", result);
                    balance.set("Error: Failed to get balance (check console for details)".to_string());
                    clear_status_after_delay(balance.clone(), 5000);
                }
                is_loading.set(false);
            });
        })
    };

    let send_transaction = {
        let transaction_status = transaction_status.clone();
        let to_address_input_ref = to_address_input_ref.clone();
        let amount_input_ref = amount_input_ref.clone();
        let selected_address_ref = selected_address_ref.clone();
        let is_loading = is_loading.clone();
        let transactions = transactions.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let to_address = to_address_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().to_string())
                .unwrap_or_default();
            let amount = amount_input_ref
                .cast::<HtmlInputElement>()
                .map(|input| input.value().trim().parse::<u64>().unwrap_or(0))
                .unwrap_or(0);
            let from_address = selected_address_ref
                .cast::<HtmlSelectElement>()
                .map(|select| select.value())
                .unwrap_or_default();
            if to_address.is_empty() || amount == 0 || from_address.is_empty() {
                transaction_status.set("Error: Recipient address, amount, and sender address are required".to_string());
                clear_status_after_delay(transaction_status.clone(), 5000);
                return;
            }
            if !to_address.starts_with("vecno:") {
                transaction_status.set("Error: Invalid recipient address (must start with vecno:)".to_string());
                clear_status_after_delay(transaction_status.clone(), 5000);
                return;
            }
            let transaction_status = transaction_status.clone();
            let is_loading = is_loading.clone();
            let transactions = transactions.clone();
            spawn_local(async move {
                is_loading.set(true);
                let args = serde_wasm_bindgen::to_value(&SendTransactionArgs { to_address, amount }).expect("Failed to serialize send_transaction args");
                let result = invoke("send_transaction", args).await;
                if js_sys::Reflect::get(&result, &JsValue::from_str("error")).is_ok() {
                    let error_msg = js_sys::Reflect::get(&result, &JsValue::from_str("error"))
                        .map(|v| v.as_string().unwrap_or_default())
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    if error_msg.contains("Insufficient") {
                        transaction_status.set("Error: Insufficient balance".to_string());
                    } else if error_msg.contains("Address parsing failed") {
                        transaction_status.set("Error: Invalid recipient address".to_string());
                    } else {
                        transaction_status.set(format!("Error: {}", error_msg));
                    }
                    clear_status_after_delay(transaction_status.clone(), 5000);
                } else if let Some(msg) = result.as_string() {
                    transaction_status.set(format!("Success: {}", msg));
                    clear_status_after_delay(transaction_status.clone(), 3000);
                    // Refresh transactions after successful send
                    let tx_result = invoke("list_transactions", JsValue::NULL).await;
                    match serde_wasm_bindgen::from_value::<Vec<Transaction>>(tx_result) {
                        Ok(txns) => {
                            transactions.set(txns);
                        }
                        Err(e) => {
                            error!("list_transactions failed after send: {:?}", e);
                            transaction_status.set("Error: Failed to refresh transactions".to_string());
                            clear_status_after_delay(transaction_status.clone(), 5000);
                        }
                    }
                } else {
                    error!("send_transaction failed with unexpected result: {:?}", result);
                    transaction_status.set("Error: Failed to send transaction (check console for details)".to_string());
                    clear_status_after_delay(transaction_status.clone(), 5000);
                }
                is_loading.set(false);
            });
        })
    };

    html! {
        <div class="app-container">
            <div class="node-status node-status-fixed">
                <div class={classes!("node-indicator", if *node_connected { "connected" } else { "disconnected" })}></div>
                <span class="node-status-text">{ if *node_connected { "Connected" } else { "Disconnected" } }</span>
            </div>
            { match &*screen {
                Screen::Intro => html! {
                    <main class="container">
                        <h1>{"Welcome to Vecno Wallet v0.0.1"}</h1>
                        <div class="row">
                            <img src="public/vecno.png" class="logo vecno" alt="Vecno logo"/>
                        </div>
                        <p>{"Your gateway to secure and decentralized wallet management."}</p>
                        { if available_wallets.is_empty() && *is_loading {
                            html! { <p>{"Scanning for wallets..."}</p> }
                        } else if available_wallets.is_empty() {
                            html! { <p>{""}</p> }
                        } else {
                            html! {
                                <form class="row" onsubmit={open_wallet}>
                                    <select ref={selected_wallet_ref} class="input">
                                        <option value="" selected=true disabled=true>{"Select a wallet"}</option>
                                        { for (*available_wallets).iter().map(|wallet| html! {
                                            <option value={wallet.path.clone()}>{ &wallet.name }</option>
                                        }) }
                                    </select>
                                    <input id="open-secret-input" ref={open_secret_input_ref} type="password" placeholder="Enter wallet password" class="input" />
                                    <button type="submit" disabled={*is_loading} class={classes!("btn", "btn-primary", if *is_loading { "loading" } else { "" })}>
                                        {"Open Wallet"}
                                    </button>
                                </form>
                            }
                        }}
                        <p class="status">{ &*wallet_status }</p>
                        <button onclick={proceed_to_create} class="btn btn-primary">{"Create New Wallet"}</button><br />
                        <p>{"Have a mnemonic? "}<a href="#" onclick={proceed_to_import}>{"Import Wallet"}</a></p>
                    </main>
                },
                Screen::CreateWallet => html! {
                    <main class="container">
                        <h1>{"Create New Wallet"}</h1>
                        <div class="row">
                            <img src="public/vecno.png" class="logo vecno" alt="Vecno logo"/>
                        </div>
                        <form class="row" onsubmit={create_wallet}>
                            <input id="filename-input" ref={filename_input_ref} placeholder="Wallet filename (e.g., mywallet)" class="input" />
                            <input id="secret-input" ref={secret_input_ref} type="password" placeholder="Enter wallet password (min 8 characters)" class="input" />
                            <button type="submit" disabled={*is_loading} class={classes!("btn", "btn-primary", if *is_loading { "loading" } else { "" })}>
                                {"Create Wallet"}
                            </button>
                        </form>
                        <p class="status">{ &*wallet_status }</p>
                        <p>{"Have a mnemonic? "}<a href="#" onclick={proceed_to_import}>{"Import Wallet"}</a></p>
                    </main>
                },
                Screen::ImportWallet => html! {
                    <main class="container">
                        <h1>{"Import Wallet"}</h1>
                        <div class="row">
                            <img src="public/vecno.png" class="logo vecno" alt="Vecno logo"/>
                        </div>
                        <form class="row" onsubmit={import_wallet}>
                            <input id="import-filename-input" ref={import_filename_input_ref} placeholder="Wallet filename (e.g., mywallet)" class="input" />
                            <input id="import-mnemonic-input" ref={import_mnemonic_input_ref} placeholder="Enter 24-word mnemonic" class="input" />
                            <input id="import-secret-input" ref={import_secret_input_ref} type="password" placeholder="Enter new password (min 8 characters)" class="input" />
                            <button type="submit" disabled={*is_loading} class={classes!("btn", "btn-primary", if *is_loading { "loading" } else { "" })}>
                                {"Import Wallet"}
                            </button>
                        </form>
                        <p class="status">{ &*wallet_status }</p>
                        <p>{"Want to create a new wallet? "}<a href="#" onclick={proceed_to_create}>{"Create New Wallet"}</a></p>
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
                        <main class="container">
                            <h1>{"Wallet Created"}</h1>
                            <div class="row">
                                <img src="public/vecno.png" class="logo vecno" alt="Vecno logo"/>
                            </div>
                            <p>{"Your wallet has been created successfully. Please save your 24-word mnemonic phrase securely."}</p>
                            <div class="mnemonic-box">
                                <p>{ &mnemonic_clone }</p>
                            </div>
                            <button onclick={move |_| copy.emit(mnemonic_clone.clone())} class="btn btn-secondary">{"Copy Mnemonic"}</button><br />
                            <button onclick={proceed} class="btn btn-primary">{"Proceed to Wallet"}</button>
                            <p class="status">{ &*wallet_status }</p>
                        </main>
                    }
                },
                Screen::Main => html! {
                    <main class="container">
                        <h1>{"Vecno Wallet"}</h1>
                        <div class="row">
                            <img src="public/vecno.png" class="logo vecno" alt="Vecno logo"/>
                        </div>
                        <p>{"Manage your Vecno wallet: check balance and view transactions."}</p>
                        <div class="row">
                            <button onclick={navigate_to_transactions} class="btn btn-primary">{"View Transactions"}</button>
                        </div>
                        <div>
                            <h3>{"Addresses"}</h3>
                            { if addresses.is_empty() && *is_loading {
                                html! { <p>{"Loading addresses..."}</p> }
                            } else if addresses.is_empty() {
                                html! { <p class="status">{"No addresses found. Try refreshing or check wallet setup."}</p> }
                            } else {
                                html! {
                                    <ul class="address-list">
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
                            <select ref={selected_address_ref.clone()} disabled={*is_loading || !*wallet_created || addresses.is_empty()} class="input">
                                <option value="" selected=true disabled=true>{"Select address for balance"}</option>
                                { for (*addresses).iter().flat_map(|addr| vec![
                                    html! { <option value={addr.receive_address.clone()}>{ format!("{} (Receive)", addr.account_name) }</option> },
                                    html! { <option value={addr.change_address.clone()}>{ format!("{} (Change)", addr.account_name) }</option> },
                                ]) }
                            </select>
                            <button onclick={get_balance} disabled={*is_loading || !*wallet_created || addresses.is_empty()} class={classes!("btn", "btn-primary", if *is_loading { "loading" } else { "" })}>
                                {"Get Balance"}
                            </button>
                        </div>
                        <p class="status">{ &*balance }</p>
                    </main>
                },
                Screen::Transactions => html! {
                    <main class="container">
                        <h1>{"Transactions"}</h1>
                        <div class="row">
                            <img src="public/vecno.png" class="logo vecno" alt="Vecno logo"/>
                        </div>
                        <p>{"Send transactions and view recent transaction history."}</p><br />
                        <button onclick={navigate_to_main} class="btn btn-secondary">{"Back to Wallet"}</button>
                        <form class="row" onsubmit={send_transaction}>
                            <select ref={selected_address_ref.clone()} disabled={*is_loading || !*wallet_created || addresses.is_empty()} class="input">
                                <option value="" selected=true disabled=true>{"Select sender address"}</option>
                                { for (*addresses).iter().flat_map(|addr| vec![
                                    html! { <option value={addr.receive_address.clone()}>{ format!("{} (Receive)", addr.account_name) }</option> },
                                    html! { <option value={addr.change_address.clone()}>{ format!("{} (Change)", addr.account_name) }</option> },
                                ]) }
                            </select>
                            <input id="to-address-input" ref={to_address_input_ref} placeholder="Recipient address (e.g., vecno:...)" disabled={*is_loading || !*wallet_created || addresses.is_empty()} class="input" />
                            <input id="amount-input" ref={amount_input_ref} type="number" placeholder="Amount (VE)" min="1" disabled={*is_loading || !*wallet_created || addresses.is_empty()} class="input" />
                            <button type="submit" disabled={*is_loading || !*wallet_created || addresses.is_empty()} class={classes!("btn", "btn-primary", if *is_loading { "loading" } else { "" })}>
                                {"Send Transaction"}
                            </button>
                        </form>
                        <p class="status">{ &*transaction_status }</p>
                        <h3>{"Recent Transactions"}</h3>
                        { if transactions.is_empty() && *is_loading {
                            html! { <p>{"Loading transactions..."}</p> }
                        } else if transactions.is_empty() {
                            html! { <p>{"No recent transactions found."}</p> }
                        } else {
                            html! {
                                <ul class="transaction-list">
                                    { for (*transactions).iter().map(|tx| {
                                        html! {
                                            <li>
                                                <strong>{ format!("TXID: {}", tx.txid) }</strong><br />
                                                { "From: " }{ &tx.from_address }<br />
                                                { "To: " }{ &tx.to_address }<br />
                                                { "Amount: " }{ format!("{} VE", tx.amount) }<br />
                                                { "Time: " }{ &tx.timestamp }
                                            </li>
                                        }
                                    }) }
                                </ul>
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
    // Initialize logging
    wasm_bindgen_futures::spawn_local(async {
        // Prevent refresh via Ctrl+R, Cmd+R, or F5
        let window = web_sys::window().expect("window not available");
        let document = window.document().expect("document not available");
        
        let closure = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            if event.key() == "F5" || 
               (event.ctrl_key() && event.key() == "r") || 
               (event.meta_key() && event.key() == "r") {
                event.prevent_default();
            }
        }) as Box<dyn FnMut(_)>);
        
        document
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
            .expect("failed to add keydown listener");
        closure.forget(); // Prevent closure from being dropped

        // Prevent refresh via beforeunload
        let beforeunload_closure = Closure::wrap(Box::new(move |event: BeforeUnloadEvent| {
            event.prevent_default();
            // Set return_value to block refresh (empty string for no prompt in Tauri WebView)
            event.set_return_value("");
        }) as Box<dyn FnMut(_)>);
        
        window
            .add_event_listener_with_callback("beforeunload", beforeunload_closure.as_ref().unchecked_ref())
            .expect("failed to add beforeunload listener");
        beforeunload_closure.forget(); // Prevent closure from being dropped
    });

    // Render the Yew app
    yew::Renderer::<App>::new().render();
}