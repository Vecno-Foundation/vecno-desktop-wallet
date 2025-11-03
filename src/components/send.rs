use yew::prelude::*;
use crate::utils::ve_to_veni;
use crate::models::SentTxInfo;
use crate::utils::format_amount;

#[derive(Properties, PartialEq)]
pub struct SendProps {
    pub on_send: Callback<(String, u64)>,
    pub transaction_status: String,
    pub last_sent: Option<SentTxInfo>,
    pub balance: String,
    pub is_loading: bool,
    pub wallet_created: bool,
    pub on_copy_txid: Callback<MouseEvent>,
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

    let copy_txid = props.on_copy_txid.clone();

    html! {
        <div class="screen-container">
            <div class="balance-container">
                <h2>{"Wallet Balance"}</h2>
                <p class={classes!("balance", if props.is_loading && props.balance.is_empty() {"loading"} else {""})}>
                    { if props.is_loading && props.balance.is_empty() { "Fetching..." } else { &props.balance }}
                </p>
            </div>

            <form class="row" {onsubmit}>
                <input placeholder="vecno:qrh6mye3..." oninput={on_to}
                       disabled={props.is_loading || !props.wallet_created} class="input" />
                <input type="number" inputmode="decimal" placeholder="Amount (VE)" step="any"
                       oninput={on_amount} disabled={props.is_loading || !props.wallet_created} class="input" />
                <button type="submit" disabled={props.is_loading || !props.wallet_created}
                        class={classes!("btn","btn-primary", if props.is_loading {"loading"} else {""})}>
                    {"Send Transaction"}
                </button>
            </form>

            { if !props.transaction_status.is_empty() {
                html! { <p class="status">{ &props.transaction_status }</p> }
            } else { html!{} }}

            { if let Some(ref sent) = props.last_sent {
                html! {
                    <div class="transaction-result">
                        <p><strong>{"Last sent transaction"}</strong></p>
                        <div class="tx-card sent-tx">
                            <div class="tx-header">
                                <span class="icon outgoing"></span>
                                <strong>{"Sent"}</strong>
                            </div>
                            <div class="tx-body">
                                <p class="tx-amt amount-out">
                                    { "-" }{ format_amount(sent.amount) }
                                </p>
                                <p class="tx-time">{ &sent.timestamp }</p>
                                <p class="tx-addr">{"to "}{ &sent.to_address }</p>
                                <div class="txid-box">
                                    <code class="txid-text">{ &sent.txid }</code>
                                    <button onclick={copy_txid.clone()} class="btn btn-sm btn-copy">{"Copy"}</button>
                                </div>
                            </div>
                        </div>
                    </div>
                }
            } else { html!{} }}
        </div>
    }
}