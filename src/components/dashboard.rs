use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DashboardProps {
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
        </div>
    }
}