use yew::prelude::*;
use crate::utils::{is_valid_password, is_valid_filename};

#[derive(Properties, PartialEq)]
pub struct ImportWalletProps {
    pub on_submit: Callback<(String, String, String)>,
    pub wallet_status: String,
    pub is_loading: bool,
    pub on_create: Callback<MouseEvent>,
}

#[function_component(ImportWallet)]
pub fn import_wallet(props: &ImportWalletProps) -> Html {
    let filename = use_state(String::new);
    let password = use_state(String::new);
    let mnemonic = use_state(String::new);

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
    let on_mnemonic = {
        let m = mnemonic.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(i) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                m.set(i.value());
            }
        })
    };

    let onsubmit = {
        let f = filename.clone();
        let p = password.clone();
        let m = mnemonic.clone();
        let cb = props.on_submit.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let words = (*m).split_whitespace().count();
            if (*f).is_empty() || (*p).is_empty() || (*m).is_empty()
                || (words != 12 && words != 24)
                || !is_valid_password(&p)
                || !is_valid_filename(&f) {
                return;
            }
            cb.emit(((*m).clone(), (*p).clone(), (*f).clone()));
        })
    };

    html! {
        <div class="screen-container import-centered">
            <div class="import-inner">
                <p class="import-title">{"Import an existing wallet using your 12 or 24-word mnemonic phrase."}</p>
                <form class="import-form" {onsubmit}>
                    <div class="row">
                        <input placeholder="Wallet filename" class="input" oninput={on_filename} />
                        <input type="password" placeholder="Enter new password" class="input" oninput={on_password} />
                    </div>
                    <div class="import-mnemonic-row">
                        <input placeholder="Enter 12 or 24-word mnemonic" class="input mnemonic-input" oninput={on_mnemonic} />
                    </div>
                    <button type="submit" disabled={props.is_loading}
                            class={classes!("btn","btn-primary", if props.is_loading {"loading"} else {""})}>
                        {"Import Wallet"}
                    </button>
                </form>

                { if !props.wallet_status.is_empty() {
                    html! { <p class="status">{ &props.wallet_status }</p> }
                } else { html!{} }}

                <p class="import-create-link">
                    {"Want to create a new wallet? "}
                    <a href="#" onclick={props.on_create.clone()}>{"Create New Wallet"}</a>
                </p>
            </div>
        </div>
    }
}