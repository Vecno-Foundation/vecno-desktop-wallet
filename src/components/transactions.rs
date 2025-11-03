use yew::prelude::*;
use crate::models::Transaction;
use crate::utils::format_amount;

#[derive(Properties, PartialEq)]
pub struct TransactionsProps {
    #[prop_or_default]
    pub transactions: Vec<Transaction>,
    pub balance: String,
    pub is_loading: bool,
    pub our_receive_address: String,
    pub on_tx_click: Callback<Transaction>,
}

#[function_component(Transactions)]
pub fn transactions(props: &TransactionsProps) -> Html {
    let transactions = props.transactions.clone();
    let our_receive_address = props.our_receive_address.clone();
    let on_tx_click = props.on_tx_click.clone();

    let mut recent: Vec<Transaction> = transactions
        .clone()
        .into_iter()
        .rev()
        .take(4)
        .collect();
    recent.reverse();

    html! {
        <div class="screen-container" role="main" aria-label="Transactions">
            <div class="balance-container" aria-live="assertive">
                <h2>{"Wallet Balance"}</h2>
                <p class={classes!(
                    "balance",
                    if props.is_loading && props.balance.is_empty() { "loading" } else { "" }
                )}>
                    { if props.is_loading && props.balance.is_empty() {
                        "Fetching balance..."
                    } else {
                        &props.balance
                    }}
                </p>
            </div>
            { if transactions.is_empty() && !props.is_loading {
                html! { <p class="info-text">{"No transactions yet."}</p> }
            } else {
                html! {
                    <>
                        <h3 class="tx-recent-title">{"Latest Activity"}</h3>
                        <div class="tx-grid">
                            { for recent.into_iter().map(move |tx| {
                                let tx_owned = tx.clone();
                                let cb = on_tx_click.clone();
                                let our_addr = our_receive_address.clone();

                                let on_click = Callback::from(move |_| {
                                    cb.emit(tx_owned.clone());
                                });

                                let is_outgoing = !tx.to_address.is_empty() && tx.to_address != our_addr;
                                let amount_str = format_amount(tx.amount);
                                let direction = if is_outgoing { "Sent" } else { "Received" };
                                let amount_class = if is_outgoing { "amount-out" } else { "amount-in" };
                                let icon_class = if is_outgoing { "outgoing" } else { "incoming" };

                                html! {
                                    <div class="tx-card clickable" onclick={on_click}>
                                        <div class="tx-header">
                                            <span class={classes!("icon", icon_class)}></span>
                                            <strong>{ direction }</strong>
                                        </div>
                                        <div class="tx-body">
                                            <p class={classes!("tx-amt", amount_class)}>
                                                { if is_outgoing { "-" } else { "+" } }{ amount_str }
                                            </p>
                                            <p class="tx-time">{ &tx.timestamp }</p>
                                        </div>
                                    </div>
                                }
                            })}
                        </div>
                    </>
                }
            }}
        </div>
    }
}