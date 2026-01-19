use yew::prelude::*;
use crate::models::WalletFile;
use crate::components::wallet_select::WalletSelect;

#[derive(Properties, PartialEq)]
pub struct HomeProps {
    pub available_wallets: Vec<WalletFile>,
    pub is_loading: bool,
    pub on_open_wallet: Callback<(String, String)>,
    pub on_create: Callback<MouseEvent>,
    pub on_import: Callback<MouseEvent>,
}

#[function_component(Home)]
pub fn home(props: &HomeProps) -> Html {
    let selected = use_state(String::new);
    let password = use_state(String::new);

    let on_wallet_select = {
        let selected = selected.clone();
        Callback::from(move |path: String| {
            selected.set(path);
        })
    };

    let on_password_change = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                password.set(input.value());
            }
        })
    };

    let onsubmit = {
        let sel = selected.clone();
        let pwd = password.clone();
        let cb = props.on_open_wallet.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            cb.emit(((*sel).clone(), (*pwd).clone()));
        })
    };

    html! {
        <div class="screen-container home-centered" role="main">
            <div class="home-inner">
                <p class="home-title">{"Your gateway to secure and decentralized wallet management."}</p>

                { if props.available_wallets.is_empty() && props.is_loading {
                    html! { <p class="home-loading">{"Scanning for wallets..."}</p> }
                } else if !props.available_wallets.is_empty() {
                    html! {
                        <form class="open-wallet-form" {onsubmit}>
                            <div class="row">
                                <WalletSelect
                                    wallets={props.available_wallets.clone()}
                                    selected={(*selected).clone()}
                                    on_select={on_wallet_select}
                                />
                                <input
                                    type="password"
                                    placeholder="Enter wallet password"
                                    class="input"
                                    oninput={on_password_change}
                                />
                            </div>
                            <button
                                type="submit"
                                disabled={props.is_loading}
                                class={classes!("btn","btn-primary", if props.is_loading {"loading"} else {""})}
                            >
                                {"Open Wallet"}
                            </button>
                        </form>
                    }
                } else { html!{} }}

                <div class="home-actions">
                    <button onclick={props.on_create.clone()} class="btn btn-primary">
                        {"Create New Wallet"}
                    </button>
                    <p class="home-import-link">
                        {"Have a mnemonic? "}
                        <a href="#" onclick={props.on_import.clone()}>{"Import Wallet"}</a>
                    </p>
                </div>
            </div>
        </div>
    }
}