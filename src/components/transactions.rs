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
    // Clone all owned data upfront – these are 'static
    let transactions = props.transactions.clone();
    let our_receive_address = props.our_receive_address.clone();
    let on_tx_click = props.on_tx_click.clone();

    // Pre-process into chunks of owned Transaction to avoid temporaries
    let mut recent: Vec<Transaction> = transactions.clone().into_iter().rev().take(4).collect();
    recent.reverse(); // Now oldest to newest, but doesn't matter for display
    let chunks: Vec<Vec<Transaction>> = recent.chunks(2).map(|c| c.to_vec()).collect();

    html! {
        <div class="screen-container" role="main" aria-label="Transactions">
            <div class="balance-container" aria-live="assertive">
                <h2>{"Wallet Balance"}</h2>
                <p class={classes!("balance", if props.is_loading && props.balance.is_empty() { "loading" } else { "" })}>
                    { if props.is_loading && props.balance.is_empty() {
                        "Fetching balance..."
                    } else {
                        &props.balance
                    }}
                </p>
            </div>

            <p>{"View your transaction history."}</p>

            { if transactions.is_empty() && !props.is_loading {
                html! { <p class="info-text">{"No transactions yet."}</p> }
            } else {
                html! {
                    <>
                        <h3 class="section-title">{"Latest Activity"}</h3>
                        <div class="tx-grid">
                            { for chunks.iter().map(move |chunk| {
                                let our_addr = our_receive_address.clone();
                                let cb = on_tx_click.clone();
                                html! {
                                    <div class="tx-row">
                                        { for chunk.iter().map(move |tx| {
                                            let tx_owned = tx.clone();
                                            let cb_inner = cb.clone();

                                            let on_click = Callback::from(move |_| {
                                                cb_inner.emit(tx_owned.clone());
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
                                }
                            })}
                        </div>
                    </>
                }
            }}
        </div>
    }
}