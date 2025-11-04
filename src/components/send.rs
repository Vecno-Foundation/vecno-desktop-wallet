use yew::prelude::*;
use crate::utils::{ve_to_veni, format_amount};
use crate::models::{SentTxInfo, Transaction};

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
}

#[function_component(Send)]
pub fn send(props: &SendProps) -> Html {
    let to_addr = use_state(String::new);
    let amount_ve = use_state(String::new);

    let payment_secret_words = use_state(|| vec![String::new(); 1]);
    let show_payment_secret = use_state(|| false);
    let has_extended_payment = use_state(|| false);

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
        Callback::from(move |e: InputEvent| {
            if let Some(i) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                a.set(i.value());
            }
        })
    };

    let on_amount = {
        let a = amount_ve.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(i) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                a.set(i.value());
            }
        })
    };

    let on_payment_word_change = {
        let words = payment_secret_words.clone();
        move |idx: usize| {
            let words = words.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                    let raw = input.value();
                    let value = raw.split_whitespace().next().unwrap_or("").trim().to_lowercase();
                    let mut current = (*words).clone();
                    if idx < current.len() {
                        current[idx] = value;
                        words.set(current);
                    }
                }
            })
        }
    };

    let add_payment_word = {
        let words = payment_secret_words.clone();
        Callback::from(move |_| {
            let mut current = (*words).clone();
            if current.len() < 24 {
                current.push(String::new());
                words.set(current);
            }
        })
    };

    let toggle_payment_secret = {
        let show = show_payment_secret.clone();
        let words = payment_secret_words.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let checked = input.checked();
                show.set(checked);
                if !checked {
                    words.set(vec![String::new(); 1]);
                }
            }
        })
    };

    let onsubmit = {
        let to = to_addr.clone();
        let amt = amount_ve.clone();
        let words = payment_secret_words.clone();
        let show_secret = *show_payment_secret;
        let cb = props.on_send.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let to = (*to).clone().trim().to_string();
            let amt = (*amt).clone();

            if to.is_empty() || amt.is_empty() {
                return;
            }

            let secret_opt = if show_secret {
                let filled: Vec<String> = (*words)
                    .iter()
                    .cloned()
                    .filter(|w| !w.is_empty())
                    .collect();
                if filled.is_empty() {
                    None
                } else {
                    Some(filled.join(" "))
                }
            } else {
                None
            };

            if let Some(veni) = ve_to_veni(&amt) {
                cb.emit((to, veni, secret_opt));
            }
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

            <form class="send-form" onsubmit={onsubmit}>
                <div class="row">
                    <input
                        placeholder="vecno:qrh6mye3..."
                        value={(*to_addr).clone()}
                        oninput={on_to}
                        disabled={props.is_loading || !props.wallet_created}
                        class="input"
                    />

                    <input
                        type="number"
                        inputmode="decimal"
                        placeholder="Amount (VE)"
                        step="any"
                        value={(*amount_ve).clone()}
                        oninput={on_amount}
                        disabled={props.is_loading || !props.wallet_created}
                        class="input"
                    />
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
                            {"Use Payment Secret (BIP39 Passphrase)"}

                            <span class="tooltip">
                                {"Optional – Only required if Payment Secret was set during wallet creation!"}
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
                                {"Custom Secret (BIP39 Passphrase)"}
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
                        }) }
                    </div>
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
                        <h3 class="send-recent-title">{"Recent Sent Transactions"}</h3>
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