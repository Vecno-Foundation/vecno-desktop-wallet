use yew::prelude::*;
use crate::models::WalletAddress;

#[derive(Properties, PartialEq)]
pub struct DashboardProps {
    pub addresses: Vec<WalletAddress>,
    pub balance: String,
    pub is_loading: bool,
}

#[function_component(Dashboard)]
pub fn dashboard(props: &DashboardProps) -> Html {
    html! {
        <div class="screen-container" role="main" aria-label="Vecno Wallet Dashboard">
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
            <p>{"Manage your Vecno wallet: check balance and view addresses."}</p>
            <div>
                <h3>{"Addresses"}</h3>
                { if props.addresses.is_empty() && props.is_loading {
                    html! { <p aria-live="polite">{"Loading addresses..."}</p> }
                } else if props.addresses.is_empty() {
                    html! { <p class="status" aria-live="assertive">{"No addresses found."}</p> }
                } else {
                    html! {
                        <ul class="address-list" aria-label="Wallet addresses">
                            { for props.addresses.iter().map(|addr| html! {
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
        </div>
    }
}