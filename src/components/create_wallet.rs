use yew::prelude::*;
use crate::utils::{is_valid_password, is_valid_filename};

#[derive(Properties, PartialEq)]
pub struct CreateWalletProps {
    pub on_submit: Callback<(String, String)>,
    pub wallet_status: String,
    pub is_loading: bool,
    pub on_import: Callback<MouseEvent>,
}

#[function_component(CreateWallet)]
pub fn create_wallet(props: &CreateWalletProps) -> Html {
    let filename = use_state(String::new);
    let password = use_state(String::new);

    let on_filename = {
        let f = filename.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(i) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                f.set(i.value());
            }
        })
    };
    let on_password = {
        let p = password.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(i) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                p.set(i.value());
            }
        })
    };

    let onsubmit = {
        let f = filename.clone();
        let p = password.clone();
        let cb = props.on_submit.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            if (*f).is_empty() || (*p).is_empty() { return; }
            if !is_valid_password(&p) || !is_valid_filename(&f) { return; }
            cb.emit(((*f).clone(), (*p).clone()));
        })
    };

    html! {
        <div class="screen-container create-centered">
            <div class="create-inner">
                <p class="create-title">{"Create a new wallet to start managing your Vecno assets."}</p>
                <form class="create-form" {onsubmit}>
                    <div class="row">
                        <input placeholder="Wallet filename (e.g., mywallet)" class="input" oninput={on_filename} />
                        <input type="password" placeholder="Enter wallet password" class="input" oninput={on_password} />
                    </div>
                    <button type="submit" disabled={props.is_loading}
                            class={classes!("btn","btn-primary", if props.is_loading {"loading"} else {""})}>
                        {"Create Wallet"}
                    </button>
                </form>

                { if !props.wallet_status.is_empty() {
                    html! { <p class="status">{ &props.wallet_status }</p> }
                } else { html!{} }}

                <p class="create-import-link">
                    {"Have a mnemonic? "}
                    <a href="#" onclick={props.on_import.clone()}>{"Import Wallet"}</a>
                </p>
            </div>
        </div>
    }
}