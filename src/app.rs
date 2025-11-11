use crate::components::*;
use crate::components::receive::Receive;
use crate::components::toast::*;
use crate::models::*;
use crate::utils::*;
use crate::utils::get_error_message;
use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::prelude::*;
use log::{error, info, debug};

async fn fetch_balance(
    addresses: UseStateHandle<Vec<WalletAddress>>,
    balance: UseStateHandle<String>,
    is_loading: UseStateHandle<bool>,
    push_toast: Callback<(String, ToastKind)>,
) {
    if (*addresses).is_empty() {
        error!("No valid address found for balance query");
        push_toast.emit(("No valid address for balance query".into(), ToastKind::Warning));
        balance.set("Balance: No valid address available".to_string());
        is_loading.set(false);
        return;
    }
    let address = (*addresses)
        .first()
        .map(|addr| addr.receive_address.clone())
        .unwrap_or_default();
    info!("Querying balance for address: {}", address);
    let args = serde_wasm_bindgen::to_value(&GetBalanceArgs { address: address.clone() })
        .unwrap_or(JsValue::NULL);
    let result = invoke("get_balance", args.clone()).await;
    let msg = get_error_message(result.clone());
    if msg.contains("error") || msg.contains("Error") || msg.contains("failed") || msg.contains("Failed") {
        push_toast.emit((msg, ToastKind::Error));
        balance.set("Balance: unavailable".into());
        is_loading.set(false);
        return;
    }
    if let Some(balance_str) = result.as_string() {
        debug!("Parsed balance response for {}: {}", address, balance_str);
        match balance_str.parse::<u64>() {
            Ok(v) => {
                info!("Balance for address {}: {} VE", address, v);
                balance.set(format_balance(v));
                is_loading.set(false);
                return;
            }
            Err(e) => {
                error!("Failed to parse balance: {}", e);
                push_toast.emit((format!("Balance parse error: {}", e), ToastKind::Error));
                balance.set(format!("Balance: Error - {}", e));
                is_loading.set(false);
                return;
            }
        }
    }
    push_toast.emit((msg, ToastKind::Error));
    balance.set("Balance: unavailable".into());
    is_loading.set(false);
}

#[function_component(App)]
pub fn app() -> Html {
    let screen = use_state(|| Screen::Intro);
    let intro_done = use_state(|| false);
    {
        let screen = screen.clone();
        let intro_done = intro_done.clone();
        use_effect_with(intro_done.clone(), move |_| {
            if *intro_done {
                screen.set(Screen::Home);
            }
            || {}
        });
    }
    {
        let intro_done = intro_done.clone();
        use_effect_with((), move |_| {
            let timeout = gloo_timers::callback::Timeout::new(6000, move || intro_done.set(true));
            || drop(timeout)
        });
    }
    let (_toast_state, push_toast, _clear_toast, toast_html) = use_toast();
    let wallet_created = use_state(|| false);
    let addresses = use_state(|| Vec::<WalletAddress>::new());
    let balance = use_state(|| String::new());
    let transaction_status = use_state(|| String::new());
    let is_loading = use_state(|| false);
    let available_wallets = use_state(|| Vec::<WalletFile>::new());
    let node_connected = use_state(|| false);
    let node_info = use_state(|| NodeInfo { url: String::new() });
    let transactions = use_state(|| Vec::<Transaction>::new());
    let last_txid = use_state(|| String::new());
    let selected_tx = use_state(|| Option::<Transaction>::None);
    let show_modal = use_state(|| false);
    let last_sent = use_state(|| Option::<SentTxInfo>::None);
    let sent_transactions = use_state(|| Vec::<SentTxInfo>::new());
    let payment_secret_required = use_state(|| false);
    const VERSION: &str = env!("CARGO_PKG_VERSION");

    {
        let last_txid = last_txid.clone();
        let screen = screen.clone();
        use_effect_with(screen.clone(), move |s| {
            if matches!(**s, Screen::Home) {
                last_txid.set(String::new());
            }
            || {}
        });
    }

    {
        let node_connected = node_connected.clone();
        let node_info = node_info.clone();
        let push_toast = push_toast.clone();
        use_effect_with(wallet_created.clone(), move |created| {
            if **created {
                let node_connected = node_connected.clone();
                let node_info = node_info.clone();
                let push_toast = push_toast.clone();
                spawn_local(async move {
                    let conn = invoke("is_node_connected", JsValue::NULL).await;
                    let msg = get_error_message(conn.clone());
                    if msg.contains("true") {
                        node_connected.set(true);
                        let info_res = invoke("get_node_info", JsValue::NULL).await;
                        let info_msg = get_error_message(info_res.clone());
                        if let Ok(info) = serde_wasm_bindgen::from_value::<NodeInfo>(info_res) {
                            node_info.set(info);
                        } else {
                            push_toast.emit((info_msg, ToastKind::Error));
                            node_info.set(NodeInfo { url: "Unknown".into() });
                        }
                    } else {
                        node_connected.set(false);
                        node_info.set(NodeInfo { url: "Not connected".into() });
                        push_toast.emit(("Warning: Not connected to Vecno node".into(), ToastKind::Warning));
                    }
                });
            } else {
                node_connected.set(false);
                node_info.set(NodeInfo { url: "".into() });
            }
            || {}
        });
    }

    {
        let screen = screen.clone();
        let available_wallets = available_wallets.clone();
        let is_loading = is_loading.clone();
        let push_toast = push_toast.clone();
        use_effect_with(screen.clone(), move |s| {
            if matches!(**s, Screen::Home) {
                let aw = available_wallets.clone();
                let loading = is_loading.clone();
                let push_toast = push_toast.clone();
                spawn_local(async move {
                    loading.set(true);
                    let res = invoke("list_wallets", JsValue::NULL).await;
                    let msg = get_error_message(res.clone());
                    if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<WalletFile>>(res) {
                        aw.set(list);
                    } else {
                        push_toast.emit((msg, ToastKind::Error));
                        aw.set(vec![]);
                    }
                    loading.set(false);
                });
            }
            || {}
        });
    }

    {
        let screen = screen.clone();
        let wallet_created = wallet_created.clone();
        let addresses = addresses.clone();
        let is_loading = is_loading.clone();
        let push_toast = push_toast.clone();
        let payment_secret_required = payment_secret_required.clone();

        use_effect_with((screen.clone(), wallet_created.clone()), move |(s, created)| {
            if **created && matches!(**s, Screen::Wallet | Screen::Receive | Screen::Send | Screen::Transactions) {
                let addr = addresses.clone();
                let loading = is_loading.clone();
                let push_toast = push_toast.clone();
                let scr = screen.clone();
                let wc = wallet_created.clone();
                let req = payment_secret_required.clone();

                spawn_local(async move {
                    loading.set(true);

                    let open_res = invoke("is_wallet_open", JsValue::NULL).await;
                    if !open_res.as_bool().unwrap_or(false) {
                        push_toast.emit(("Wallet not open".into(), ToastKind::Error));
                        scr.set(Screen::Home);
                        wc.set(false);
                        loading.set(false);
                        return;
                    }

                    let addr_res = invoke("get_address", JsValue::NULL).await;
                    let addr_msg = get_error_message(addr_res.clone());
                    if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<WalletAddress>>(addr_res) {
                        if list.is_empty() {
                            push_toast.emit(("No addresses loaded".into(), ToastKind::Error));
                            addr.set(vec![]);
                        } else {
                            addr.set(list);
                        }
                    } else {
                        push_toast.emit((addr_msg, ToastKind::Error));
                        addr.set(vec![]);
                        loading.set(false);
                        return;
                    }

                    let needs_res = invoke("wallet_needs_payment_secret", JsValue::NULL).await;
                    let needs = needs_res.as_bool().unwrap_or(false);
                    info!("Payment secret required: {}", needs);
                    req.set(needs);

                    loading.set(false);
                });
            } else if !**created {
                payment_secret_required.set(false);
                addresses.set(vec![]);
            }
            || {}
        });
    }

    {
        let addresses = addresses.clone();
        let balance = balance.clone();
        let is_loading = is_loading.clone();
        let push_toast = push_toast.clone();
        use_effect_with(addresses.clone(), move |addrs| {
            if !addrs.is_empty() {
                let a = addrs.clone();
                let b = balance.clone();
                let l = is_loading.clone();
                let pt = push_toast.clone();
                spawn_local(async move {
                    l.set(true);
                    fetch_balance(a, b, l, pt).await;
                });
            }
            || {}
        });
    }

    {
        let screen = screen.clone();
        let transactions = transactions.clone();
        let is_loading = is_loading.clone();
        let push_toast = push_toast.clone();
        use_effect_with(screen.clone(), move |s| {
            if matches!(**s, Screen::Transactions) {
                let txs = transactions.clone();
                let l = is_loading.clone();
                let pt = push_toast.clone();
                spawn_local(async move {
                    l.set(true);
                    let res = invoke("list_transactions", JsValue::NULL).await;
                    let msg = get_error_message(res.clone());
                    if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<Transaction>>(res) {
                        txs.set(list);
                    } else {
                        pt.emit((msg, ToastKind::Error));
                        txs.set(vec![]);
                    }
                    l.set(false);
                });
            }
            || {}
        });
    }

    let set_screen = |s: Screen| {
        let scr = screen.clone();
        Callback::from(move |_| scr.set(s.clone()))
    };
    let to_home = set_screen(Screen::Home);
    let to_wallet = {
        let scr = screen.clone();
        let wc = wallet_created.clone();
        Callback::from(move |_| if *wc { scr.set(Screen::Wallet) })
    };
    let to_receive = {
        let scr = screen.clone();
        let wc = wallet_created.clone();
        Callback::from(move |_| if *wc { scr.set(Screen::Receive) })
    };
    let to_transactions = {
        let scr = screen.clone();
        let wc = wallet_created.clone();
        Callback::from(move |_| if *wc { scr.set(Screen::Transactions) })
    };
    let to_send = {
        let scr = screen.clone();
        let wc = wallet_created.clone();
        Callback::from(move |_| if *wc { scr.set(Screen::Send) })
    };
    let navigate_to_intro = {
        let scr = screen.clone();
        let wc = wallet_created.clone();
        let nc = node_connected.clone();
        let ni = node_info.clone();
        let l = is_loading.clone();
        let req = payment_secret_required.clone();
        Callback::from(move |_| {
            scr.set(Screen::Home);
            wc.set(false);
            nc.set(false);
            ni.set(NodeInfo { url: "".into() });
            req.set(false);
            let l = l.clone();
            spawn_local(async move {
                l.set(true);
                let _ = invoke("close_wallet", JsValue::NULL).await;
                l.set(false);
            });
        })
    };

    let open_wallet = {
        let wc = wallet_created.clone();
        let scr = screen.clone();
        let l = is_loading.clone();
        let pt = push_toast.clone();
        Callback::from(move |(filename, secret): (String, String)| {
            if filename.is_empty() {
                pt.emit(("Select a Wallet".into(), ToastKind::Error));
                return;
            }
            if secret.is_empty() {
                pt.emit(("Wallet password is required".into(), ToastKind::Error));
                return;
            }
            if !is_valid_password(&secret) {
                pt.emit(("Password must be at least 8 characters".into(), ToastKind::Error));
                return;
            }
            let filename = filename.clone();
            let secret = secret.clone();
            let wc = wc.clone();
            let scr = scr.clone();
            let l = l.clone();
            let pt = pt.clone();
            spawn_local(async move {
                l.set(true);
                pt.emit(("Verifying password...".into(), ToastKind::Info));
                match verify_password(&filename, &secret).await {
                    Ok(()) => {
                        info!("Password correct. Opening wallet...");
                        let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                            "input": {
                                "filename": filename,
                                "secret": secret,
                                "payment_secret": null
                            }
                        }))
                        .unwrap_or(JsValue::NULL);

                        let res = invoke("open_wallet", args).await;
                        let msg = get_error_message(res.clone());
                        if let Some(s) = res.as_string() {
                            if s.contains("Success") {
                                pt.emit(("Wallet opened successfully!".into(), ToastKind::Success));
                                wc.set(true);
                                scr.set(Screen::Wallet);
                            } else {
                                pt.emit((s, ToastKind::Error));
                            }
                        } else {
                            pt.emit((msg, ToastKind::Error));
                        }
                    }
                    Err(e) => {
                        error!("Password verification failed: {}", e);
                        if e.contains("Incorrect password") {
                            pt.emit(("Incorrect password".into(), ToastKind::Error));
                        } else {
                            pt.emit((e, ToastKind::Error));
                        }
                    }
                }
                l.set(false);
            });
        })
    };

    let create_wallet = {
        let wc = wallet_created.clone();
        let scr = screen.clone();
        let l = is_loading.clone();
        let pt = push_toast.clone();
        Callback::from(move |(filename, secret, payment_secret): (String, String, Option<String>)| {
            if filename.is_empty() {
                pt.emit(("Wallet filename is required".into(), ToastKind::Error));
                return;
            }
            if !is_valid_filename(&filename) {
                pt.emit(("Filename contains invalid characters or is too long".into(), ToastKind::Error));
                return;
            }
            if secret.is_empty() {
                pt.emit(("Wallet password is required".into(), ToastKind::Error));
                return;
            }
            if !is_valid_password(&secret) {
                pt.emit(("Password must be at least 8 characters".into(), ToastKind::Error));
                return;
            }
            let wc = wc.clone();
            let scr = scr.clone();
            let l = l.clone();
            let pt = pt.clone();
            spawn_local(async move {
                l.set(true);
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                    "input": {
                        "filename": filename,
                        "secret": secret,
                        "payment_secret": payment_secret
                    }
                }))
                .unwrap_or(JsValue::NULL);

                let res = invoke("create_wallet", args).await;
                let msg = get_error_message(res.clone());
                if let Some(s) = res.as_string() {
                    if s.contains("Success") {
                        pt.emit(("Wallet created!".into(), ToastKind::Success));
                        wc.set(true);
                        if let Some(mnemonic) = s.split("with mnemonic: ").nth(1) {
                            scr.set(Screen::MnemonicDisplay(mnemonic.into()));
                        } else {
                            scr.set(Screen::Wallet);
                        }
                    } else {
                        pt.emit((s, ToastKind::Error));
                    }
                } else {
                    pt.emit((msg, ToastKind::Error));
                }
                l.set(false);
            });
        })
    };

    let import_wallets = {
        let wc = wallet_created.clone();
        let scr = screen.clone();
        let l = is_loading.clone();
        let pt = push_toast.clone();
        Callback::from(move |(mnemonic, secret, payment_secret, filename): (String, String, Option<String>, String)| {
            if mnemonic.is_empty() {
                pt.emit(("Mnemonic phrase is required".into(), ToastKind::Error));
                return;
            }
            let words = mnemonic.split_whitespace().count();
            if words != 12 && words != 24 {
                pt.emit(("Mnemonic must be 12 or 24 words".into(), ToastKind::Error));
                return;
            }
            if secret.is_empty() {
                pt.emit(("Wallet password is required".into(), ToastKind::Error));
                return;
            }
            if !is_valid_password(&secret) {
                pt.emit(("Password must be at least 8 characters".into(), ToastKind::Error));
                return;
            }
            if filename.is_empty() {
                pt.emit(("Wallet filename is required".into(), ToastKind::Error));
                return;
            }
            if !is_valid_filename(&filename) {
                pt.emit(("Filename contains invalid characters or is too long".into(), ToastKind::Error));
                return;
            }
            let wc = wc.clone();
            let scr = scr.clone();
            let l = l.clone();
            let pt = pt.clone();
            spawn_local(async move {
                l.set(true);
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                    "input": {
                        "mnemonic": mnemonic,
                        "secret": secret,
                        "payment_secret": payment_secret,
                        "filename": filename
                    }
                }))
                .unwrap_or(JsValue::NULL);

                web_sys::console::log_1(&format!("TAURI ARGS: {:?}", args).into());
                let res = invoke("import_wallets", args).await;
                let msg = get_error_message(res.clone());
                if let Some(s) = res.as_string() {
                    if s.contains("Success") {
                        pt.emit(("Wallet imported!".into(), ToastKind::Success));
                        wc.set(true);
                        scr.set(Screen::Wallet);
                    } else {
                        pt.emit((s, ToastKind::Error));
                    }
                } else {
                    pt.emit((msg, ToastKind::Error));
                }
                l.set(false);
            });
        })
    };

    let send_transaction = {
        let l = is_loading.clone();
        let txs = transactions.clone();
        let addrs = addresses.clone();
        let bal = balance.clone();
        let last = last_txid.clone();
        let wc = wallet_created.clone();
        let pt = push_toast.clone();
        let last_sent = last_sent.clone();
        let sent_transactions = sent_transactions.clone();
        Callback::from(move |(to_addr, amount_veni, payment_secret): (String, u64, Option<String>)| {
            if to_addr.is_empty() {
                pt.emit(("Recipient address is required".into(), ToastKind::Error));
                return;
            }
            if amount_veni == 0 {
                pt.emit(("Amount must be greater than 0".into(), ToastKind::Error));
                return;
            }
            if !*wc {
                pt.emit(("No wallet open".into(), ToastKind::Error));
                return;
            }

            let l = l.clone();
            let txs = txs.clone();
            let addrs = addrs.clone();
            let bal = bal.clone();
            let last = last.clone();
            let pt = pt.clone();
            let last_sent = last_sent.clone();
            let sent_transactions = sent_transactions.clone();

            spawn_local(async move {
                l.set(true);
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                    "input": {
                        "to_address": to_addr,
                        "amount": amount_veni,
                        "payment_secret": payment_secret
                    }
                })).unwrap_or(JsValue::NULL);

                let res = match safe_invoke("send_transaction", args).await {
                    Ok(r) => r,
                    Err(e) => {
                        pt.emit((e, ToastKind::Error));
                        l.set(false);
                        return;
                    }
                };
                let res_clone = res.clone();
                if let Ok(sent) = serde_wasm_bindgen::from_value::<SentTxInfo>(res) {
                    last.set(sent.txid.clone());
                    last_sent.set(Some(sent.clone()));
                    pt.emit(("Transaction sent!".into(), ToastKind::Success));

                    let mut current = (*sent_transactions).clone();
                    current.insert(0, sent.clone());
                    if current.len() > 2 {
                        current.truncate(2);
                    }
                    sent_transactions.set(current);

                    let mut current_txs = (*txs).clone();
                    let optimistic = Transaction {
                        txid: sent.txid.clone(),
                        to_address: sent.to_address.clone(),
                        amount: sent.amount,
                        timestamp: sent.timestamp.clone(),
                    };
                    current_txs.insert(0, optimistic);
                    txs.set(current_txs);
                } else {
                    let msg = get_error_message(res_clone);
                    pt.emit((msg, ToastKind::Error));
                }
                let list_res = invoke("list_transactions", JsValue::NULL).await;
                if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<Transaction>>(list_res) {
                    txs.set(list);
                }
                if !(*addrs).is_empty() {
                    let addrs = addrs.clone();
                    let bal = bal.clone();
                    let l = l.clone();
                    let pt = pt.clone();
                    
                    spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(3_000).await;
                        fetch_balance(addrs, bal, l, pt).await;
                    });
                }
                l.set(false);
            });
        })
    };

    let copy_mnemonic = {
        let pt = push_toast.clone();
        Callback::from(move |mnemonic: String| {
            let pt = pt.clone();
            spawn_local(async move {
                if let Some(nav) = web_sys::window().and_then(|w| Some(w.navigator())) {
                    if let Err(e) = wasm_bindgen_futures::JsFuture::from(nav.clipboard().write_text(&mnemonic)).await {
                        error!("Clipboard error: {:?}", e);
                        pt.emit(("Copy failed".into(), ToastKind::Error));
                    } else {
                        pt.emit(("Mnemonic copied!".into(), ToastKind::Success));
                    }
                }
            });
        })
    };

    let open_modal = {
        let selected = selected_tx.clone();
        let show = show_modal.clone();
        Callback::from(move |tx: Transaction| {
            selected.set(Some(tx));
            show.set(true);
        })
    };
    let close_modal = {
        let show = show_modal.clone();
        Callback::from(move |_| show.set(false))
    };

    html! {
        <div class="app-container">
            { toast_html }
            <div class="node-status node-status-fixed" aria-live="polite">
                <div class={classes!(
                    "node-indicator",
                    if *node_connected { "connected" } else { "disconnected" }
                )}></div>
                <span class="node-status-text">
                    { if *node_connected { "Connected" } else { "Disconnected" } }
                </span>
                { 
                    if !*node_connected {
                        html! {
                            <span class="node-tooltip">{"Open, import or create a wallet to connect!"}</span>
                        }
                    } else {
                        html! {
                            <span class="node-tooltip">{ &node_info.url }</span>
                        }
                    }
                }
            </div>
            <div class="app-title">{ format!("Vecno Wallet v{}", VERSION) }</div>
            <div class="layout">
                <aside class="sidebar">
                    <nav class="nav">
                        <button class={classes!("nav-item", if *screen == Screen::Home { "active" } else { "" })} onclick={to_home.clone()}>
                            <span aria-hidden="true"></span>
                            {"Home"}
                        </button>
                        <button class={classes!("nav-item", if *screen == Screen::Wallet { "active" } else { "" })} onclick={to_wallet} disabled={!*wallet_created}>
                            <span aria-hidden="true"></span>
                            {"Wallet"}
                        </button>
                        <button class={classes!("nav-item", if *screen == Screen::Transactions { "active" } else { "" })} onclick={to_transactions} disabled={!*wallet_created}>
                            <span aria-hidden="true"></span>
                            {"Transactions"}
                        </button>
                        <button class={classes!("nav-item", if *screen == Screen::Receive { "active" } else { "" })} onclick={to_receive} disabled={!*wallet_created}>
                            <span aria-hidden="true"></span>
                            {"Receive"}
                        </button>
                        <button class={classes!("nav-item", if *screen == Screen::Send { "active" } else { "" })} onclick={to_send} disabled={!*wallet_created}>
                            <span aria-hidden="true"></span>
                            {"Send"}
                        </button>
                    </nav>
                    <div class="sidebar-footer">
                        <button onclick={navigate_to_intro} class="exit-btn"><span aria-hidden="true"></span>{"Exit"}</button>
                    </div>
                </aside>
                <main class="main-content">
                    { match &*screen {
                        Screen::Intro => html! { <Intro /> },
                        Screen::Home => html! {
                            <Home
                                available_wallets={(*available_wallets).clone()}
                                is_loading={*is_loading}
                                on_open_wallet={open_wallet}
                                on_create={set_screen(Screen::CreateWallet)}
                                on_import={set_screen(Screen::ImportWallet)}
                            />
                        },
                        Screen::CreateWallet => html! {
                            <CreateWallet
                                on_submit={create_wallet}
                                is_loading={*is_loading}
                                on_import={set_screen(Screen::ImportWallet)}
                                push_toast={push_toast.clone()}
                            />
                        },
                        Screen::ImportWallet => html! {
                            <ImportWallet
                                on_submit={import_wallets}
                                is_loading={*is_loading}
                                on_create={set_screen(Screen::CreateWallet)}
                                push_toast={push_toast.clone()}
                            />
                        },
                        Screen::MnemonicDisplay(m) => html! {
                            <MnemonicDisplay
                                mnemonic={m.clone()}
                                on_copy={copy_mnemonic.clone()}
                                on_proceed={set_screen(Screen::Wallet)}
                            />
                        },
                        Screen::Wallet => html! {
                            <Dashboard
                                balance={(*balance).clone()}
                                is_loading={*is_loading}
                            />
                        },
                        Screen::Receive => html! {
                            <Receive
                                addresses={(*addresses).clone()}
                                is_loading={*is_loading}
                            />
                        },
                        Screen::Transactions => {
                            let recv = addresses.first().map(|a| a.receive_address.clone()).unwrap_or_default();
                            html! {
                                <Transactions
                                    transactions={(*transactions).clone()}
                                    balance={(*balance).clone()}
                                    is_loading={*is_loading}
                                    our_receive_address={recv.clone()}
                                    on_tx_click={open_modal.clone()}
                                />
                            }
                        },
                        Screen::Send => {
                            let recv = addresses.first().map(|a| a.receive_address.clone()).unwrap_or_default();
                            html! {
                                <Send
                                    on_send={send_transaction}
                                    transaction_status={(*transaction_status).clone()}
                                    last_sent={(*last_sent).clone()}
                                    balance={(*balance).clone()}
                                    is_loading={*is_loading}
                                    wallet_created={*wallet_created}
                                    sent_transactions={(*sent_transactions).clone()}
                                    on_tx_click={open_modal.clone()}
                                    our_receive_address={recv}
                                    push_toast={push_toast.clone()}
                                    payment_secret_required={*payment_secret_required}
                                />
                            }
                        },
                    }}
                    { if *show_modal {
                        if let Some(ref tx) = *selected_tx {
                            let recv = addresses.first().map(|a| a.receive_address.clone()).unwrap_or_default();
                            html! {
                                <TxDetailModal
                                    tx={tx.clone()}
                                    our_address={recv}
                                    on_close={close_modal}
                                />
                            }
                        } else { html!{} }
                    } else { html!{} }}
                </main>
            </div>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    wasm_logger::init(wasm_logger::Config::new(log::Level::Debug));

    spawn_local(async {
        let window = web_sys::window().expect("no global `window`");
        let document = window.document().expect("no `document`");

        let keydown = Closure::<dyn FnMut(_)>::new(move |e: web_sys::KeyboardEvent| {
            let key = e.key().to_ascii_lowercase();
            if key == "f5"
                || key == "f11"
                || (e.ctrl_key() && (key == "r" || key == "refresh"))
                || (e.meta_key() && key == "r")
            {
                e.prevent_default();
                e.stop_propagation();
            }
        });
        document
            .add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref())
            .unwrap();
        keydown.forget();

        let contextmenu = Closure::<dyn FnMut(_)>::new(move |e: web_sys::MouseEvent| {
            e.prevent_default();
            e.stop_propagation();
        });
        document
            .add_event_listener_with_callback("contextmenu", contextmenu.as_ref().unchecked_ref())
            .unwrap();
        contextmenu.forget();

        if let Ok(tauri) = js_sys::Reflect::get(&window, &"__TAURI__".into()) {
            if let Ok(window_obj) = js_sys::Reflect::get(&tauri, &"window".into()) {
                if let Ok(current_fn) = js_sys::Reflect::get(&window_obj, &"getCurrent".into()) {
                    if js_sys::Function::from(current_fn).is_function() {
                        let dummy = js_sys::Function::new_no_args("console.log('Reload blocked by Vecno Wallet')");
                        let _ = js_sys::Reflect::set(&window_obj, &"getCurrent".into(), &dummy);
                    }
                }
            }
        }

        let beforeunload = Closure::<dyn FnMut(_)>::new(move |e: web_sys::BeforeUnloadEvent| {
            e.prevent_default();
            e.set_return_value("");
        });
        window
            .add_event_listener_with_callback("beforeunload", beforeunload.as_ref().unchecked_ref())
            .unwrap();
        beforeunload.forget();
    });

    yew::Renderer::<App>::new().render();
}