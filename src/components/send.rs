use yew::prelude::*;
use crate::utils::{ve_to_veni, format_amount};
use crate::models::{SentTxInfo, Transaction, ToastKind};

#[derive(Properties, PartialEq)]
pub struct SendProps {
    pub on_send: Callback<(String, u64, Option<String>)>,
    pub transaction_status: String,
    pub last_sent: Option<SentTxInfo>,
    pub balance: String,
    pub is_loading: bool,
    pub wallet_created: bool,
    #[prop_or_default]
    pub sent_transactions: Vec<SentTxInfo>,
    pub on_tx_click: Callback<Transaction>,
    pub our_receive_address: String,
    pub push_toast: Callback<(String, ToastKind)>,
}

#[function_component(Send)]
pub fn send(props: &SendProps) -> Html {
    let to_addr = use_state(String::new);
    let amount_ve = use_state(String::new);
    let payment_secret_words = use_state(|| vec![String::new(); 1]);
    let show_payment_secret = use_state(|| false);
    let has_extended_payment = use_state(|| false);
    let to_addr_error = use_state(String::new);
    let amount_error = use_state(String::new);
    let payment_secret_error = use_state(String::new);
    let on_send = props.on_send.clone();
    let push_toast = props.push_toast.clone();
    let our_receive_address = props.our_receive_address.clone();

    {
        let words = payment_secret_words.clone();
        let has = has_extended_payment.clone();
        use_effect_with(words.clone(), move |w| {
            let any_filled = w.iter().skip(1).any(|s| !s.is_empty());
            has.set(any_filled);
            || ()
        });
    }

    let on_to = {
        let a = to_addr.clone();
        let e = to_addr_error.clone();
        Callback::from(move |ev: InputEvent| {
            if let Some(i) = ev.target_dyn_into::<web_sys::HtmlInputElement>() {
                let val = i.value();
                a.set(val.clone());
                if val.trim().is_empty() {
                    e.set(String::new());
                }
            }
        })
    };

    let on_amount = {
        let a = amount_ve.clone();
        let e = amount_error.clone();
        Callback::from(move |ev: InputEvent| {
            if let Some(i) = ev.target_dyn_into::<web_sys::HtmlInputElement>() {
                let val = i.value();
                a.set(val.clone());
                if val.trim().is_empty() {
                    e.set(String::new());
                } else if ve_to_veni(&val).is_none() {
                    e.set("Invalid or zero amount".into());
                } else {
                    e.set(String::new());
                }
            }
        })
    };

    let on_payment_word_change = {
        let words = payment_secret_words.clone();
        let err = payment_secret_error.clone();
        move |idx: usize| {
            let w = words.clone();
            let e = err.clone();
            Callback::from(move |ev: InputEvent| {
                if let Some(i) = ev.target_dyn_into::<web_sys::HtmlInputElement>() {
                    let raw = i.value();
                    let word = raw.split_whitespace().next().unwrap_or("").trim().to_lowercase();
                    let mut cur = (*w).clone();
                    if idx < cur.len() {
                        cur[idx] = word;
                        w.set(cur);
                        e.set(String::new());
                    }
                }
            })
        }
    };

    let add_payment_word = {
        let w = payment_secret_words.clone();
        Callback::from(move |_| {
            let mut cur = (*w).clone();
            if cur.len() < 24 {
                cur.push(String::new());
                w.set(cur);
            }
        })
    };

    let toggle_payment_secret = {
        let show = show_payment_secret.clone();
        let words = payment_secret_words.clone();
        let err = payment_secret_error.clone();
        Callback::from(move |ev: InputEvent| {
            if let Some(i) = ev.target_dyn_into::<web_sys::HtmlInputElement>() {
                let checked = i.checked();
                show.set(checked);
                if !checked {
                    words.set(vec![String::new(); 1]);
                    err.set(String::new());
                }
            }
        })
    };

    let onsubmit = {
        let to = to_addr.clone();
        let amt = amount_ve.clone();
        let words = payment_secret_words.clone();
        let show_secret = *show_payment_secret;

        let e_to = to_addr_error.clone();
        let e_amt = amount_error.clone();
        let e_ps = payment_secret_error.clone();

        let on_send = on_send.clone();
        let push_toast = push_toast.clone();
        let our_receive_address = our_receive_address.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            e_to.set(String::new());
            e_amt.set(String::new());
            e_ps.set(String::new());

            let mut has_error = false;

            let to_addr_str = (*to).trim().to_string();
            let amt_str = (*amt).trim();
            if to_addr_str.is_empty() {
                push_toast.emit(("Recipient address required".into(), ToastKind::Error));
                has_error = true;
            }

            let amount_veni = if amt_str.is_empty() {
                push_toast.emit(("Amount required".into(), ToastKind::Error));
                has_error = true;
                0
            } else {
                match ve_to_veni(amt_str) {
                    Some(v) if v > 0 => v,
                    _ => {
                        push_toast.emit(("Invalid amount".into(), ToastKind::Error));
                        has_error = true;
                        0
                    }
                }
            };

            let filled: Vec<String> = (*words)
                .iter()
                .cloned()
                .filter(|s| !s.is_empty())
                .collect();

            let pay_secret_opt = if show_secret && !filled.is_empty() {
                Some(filled.join(" "))
            } else {
                None
            };

            if show_secret && filled.is_empty() {
                push_toast.emit(("Payment secret enabled but empty".into(), ToastKind::Error));
                has_error = true;
            }

            if has_error {
                return;
            }

            if to_addr_str == our_receive_address {
                push_toast.emit(("Sending to your own wallet".into(), ToastKind::Warning));
            }

            push_toast.emit(("Sending transaction...".into(), ToastKind::Info));
            on_send.emit((to_addr_str, amount_veni, pay_secret_opt));
        })
    };

    let sent_to_tx = |sent: &SentTxInfo| Transaction {
        txid: sent.txid.clone(),
        to_address: sent.to_address.clone(),
        amount: sent.amount,
        timestamp: sent.timestamp.clone(),
    };

    let mut recent: Vec<SentTxInfo> = props.sent_transactions.clone();
    recent.reverse();
    let recent = recent.into_iter().take(4).collect::<Vec<_>>();
    let chunks: Vec<Vec<SentTxInfo>> = recent.chunks(2).map(|c| c.to_vec()).collect();
    let on_tx_click = props.on_tx_click.clone();

    html! {
        <div class="screen-container">
            <div class="balance-container">
                <h2>{"Wallet Balance"}</h2>
                <p class={classes!(
                    "balance",
                    if props.is_loading && props.balance.is_empty() { "loading" } else { "" }
                )}>
                    { if props.is_loading && props.balance.is_empty() {
                        "Fetching..."
                    } else {
                        &props.balance
                    }}
                </p>
            </div>

            <form class="send-form" {onsubmit}>
                <div class="row">
                    <div class="input-wrapper">
                        <input
                            placeholder="vecno:qrh6mye3..."
                            value={(*to_addr).clone()}
                            oninput={on_to}
                            disabled={props.is_loading || !props.wallet_created}
                            class={classes!("input", if !(*to_addr_error).is_empty() { "error" } else { "" })}
                        />
                        if !(*to_addr_error).is_empty() {
                            <p class="status error">{ (*to_addr_error).clone() }</p>
                        }
                    </div>

                    <div class="input-wrapper">
                        <input
                            type="text"
                            inputmode="decimal"
                            placeholder="Amount (VE)"
                            value={(*amount_ve).clone()}
                            oninput={on_amount}
                            disabled={props.is_loading || !props.wallet_created}
                            class={classes!("input", if !(*amount_error).is_empty() { "error" } else { "" })}
                        />
                        if !(*amount_error).is_empty() {
                            <p class="status error">{ (*amount_error).clone() }</p>
                        }
                    </div>
                </div>

                <div class="row centered-row">
                    <div class="mnemonic-toggle">
                        <label class="checkbox-label tooltip-wrapper">
                            <input
                                type="checkbox"
                                checked={*show_payment_secret}
                                oninput={toggle_payment_secret}
                                disabled={props.is_loading || !props.wallet_created}
                            />
                            {"Use Payment Secret"}
                            <span class="tooltip">
                                {"Only if set during wallet creation!"}
                            </span>
                        </label>
                    </div>
                </div>

                <div class={classes!(
                    "create-payment-secret-section",
                    if *show_payment_secret { "visible" } else { "hidden" }
                )}>
                    <div class="create-mnemonic-toggle">
                        <div style="display:flex;align-items:center;gap:0.5rem;width:100%;justify-content:space-between;">
                            <span class="section-title" style="font-size:1rem;margin:0;">
                                {"Custom Payment Secret"}
                            </span>
                            <button
                                type="button"
                                class="btn btn-small create-add-word-btn"
                                onclick={add_payment_word}
                                disabled={props.is_loading || !props.wallet_created || (*payment_secret_words).len() >= 24}
                            >
                                {"+ Add Word"}
                            </button>
                        </div>
                    </div>

                    <div class={classes!(
                        "create-mnemonic-grid",
                        if *has_extended_payment { "extended" } else { "" }
                    )}>
                        { for (0..(*payment_secret_words).len()).map(|i| {
                            let on_input = on_payment_word_change(i);
                            html! {
                                <div class="create-word-slot" data-index={format!("{}", i + 1)}>
                                    <input
                                        type="text"
                                        placeholder="word"
                                        value={(*payment_secret_words)[i].clone()}
                                        oninput={on_input}
                                        class="create-word-input"
                                        disabled={props.is_loading || !props.wallet_created}
                                    />
                                </div>
                            }
                        })}
                    </div>
                    if !(*payment_secret_error).is_empty() {
                        <p class="status error centered-error">{ (*payment_secret_error).clone() }</p>
                    }
                </div>

                <div class="button-group">
                    <button
                        type="submit"
                        disabled={props.is_loading || !props.wallet_created}
                        class={classes!("btn", "btn-prominent", if props.is_loading { "loading" } else { "" })}
                    >
                        { if props.is_loading { "Sending…" } else { "Send Transaction" } }
                    </button>
                </div>
            </form>

            { if !props.transaction_status.is_empty() {
                html! { <p class="status">{ &props.transaction_status }</p> }
            } else { html!{} }}

            { if !props.sent_transactions.is_empty() {
                html! {
                    <>
                        <h3 class="send-recent-title">{"Recent Sent"}</h3>
                        <div class="send-tx-grid">
                            { for chunks.iter().map(move |chunk| {
                                let on_tx_click = on_tx_click.clone();
                                html! {
                                    <>
                                        { for chunk.iter().map(move |sent| {
                                            let tx = sent_to_tx(sent);
                                            let on_click = {
                                                let tx = tx.clone();
                                                let cb = on_tx_click.clone();
                                                Callback::from(move |_| cb.emit(tx.clone()))
                                            };
                                            html! {
                                                <div class="send-tx-card" onclick={on_click}>
                                                    <div class="send-tx-header">
                                                        <span class="icon outgoing"></span>
                                                        <strong>{"Sent"}</strong>
                                                    </div>
                                                    <div class="send-tx-amt">
                                                        { "-" }{ format_amount(sent.amount) }
                                                    </div>
                                                    <div class="send-tx-time">
                                                        { &sent.timestamp }
                                                    </div>
                                                </div>
                                            }
                                        })}
                                    </>
                                }
                            })}
                        </div>
                    </>
                }
            } else { html!{} }}
        </div>
    }
}