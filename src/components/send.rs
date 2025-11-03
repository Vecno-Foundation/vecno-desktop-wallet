use yew::prelude::*;
use crate::utils::{ve_to_veni, format_amount};
use crate::models::{SentTxInfo, Transaction};

#[derive(Properties, PartialEq)]
pub struct SendProps {
    pub on_send: Callback<(String, u64)>,
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

    let onsubmit = {
        let to = to_addr.clone();
        let amt = amount_ve.clone();
        let cb = props.on_send.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let to = (*to).clone();
            let amt = (*amt).clone();
            if to.is_empty() || amt.is_empty() {
                return;
            }
            if let Some(veni) = ve_to_veni(&amt) {
                cb.emit((to, veni));
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

            <form class="row" {onsubmit}>
                <input
                    placeholder="vecno:qrh6mye3..."
                    oninput={on_to}
                    disabled={props.is_loading || !props.wallet_created}
                    class="input"
                />
                <input
                    type="number"
                    inputmode="decimal"
                    placeholder="Amount (VE)"
                    step="any"
                    oninput={on_amount}
                    disabled={props.is_loading || !props.wallet_created}
                    class="input"
                />
                <button
                    type="submit"
                    disabled={props.is_loading || !props.wallet_created}
                    class={classes!(
                        "btn",
                        "btn-primary",
                        if props.is_loading { "loading" } else { "" }
                    )}
                >
                    {"Send Transaction"}
                </button>
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