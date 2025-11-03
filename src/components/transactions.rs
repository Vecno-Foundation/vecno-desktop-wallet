use yew::prelude::*;
use crate::models::Transaction;
use crate::utils::format_amount;

#[derive(Properties, PartialEq)]
pub struct TransactionsProps {
    pub transactions: Vec<Transaction>,
    pub balance: String,
    pub is_loading: bool,
    pub our_receive_address: String,
}

#[function_component(Transactions)]
pub fn transactions(props: &TransactionsProps) -> Html {
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

            { if props.transactions.is_empty() && !props.is_loading {
                html! { <p class="info-text">{"No transactions yet."}</p> }
            } else {
                html! {
                    <>
                        <h3 class="section-title">{"Latest Activity"}</h3>
                        <div class="tx-grid">
                            { for props.transactions.iter().rev().take(4).collect::<Vec<_>>().chunks(2).map(|chunk| {
                                html! {
                                    <div class="tx-row">
                                        { for chunk.iter().map(|tx| {
                                            let is_outgoing = !tx.to_address.is_empty() && tx.to_address != props.our_receive_address;
                                            let amount_str = format_amount(tx.amount);
                                            let direction = if is_outgoing { "Sent" } else { "Received" };
                                            let amount_class = if is_outgoing { "amount-out" } else { "amount-in" };
                                            let icon_class = if is_outgoing { "outgoing" } else { "incoming" };

                                            html! {
                                                <div class="tx-card">
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