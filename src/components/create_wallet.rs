use crate::models::ToastKind;
use yew::prelude::*;
use crate::utils::{is_valid_password, is_valid_filename};

#[derive(Properties, PartialEq)]
pub struct CreateWalletProps {
    pub on_submit: Callback<(String, String)>,
    pub is_loading: bool,
    pub on_import: Callback<MouseEvent>,
    pub push_toast: Callback<(String, ToastKind)>,
}

#[function_component(CreateWallet)]
pub fn create_wallet(props: &CreateWalletProps) -> Html {
    let filename = use_state(String::new);
    let password = use_state(String::new);
    let filename_error = use_state(String::new);
    let password_error = use_state(String::new);

    let on_filename = {
        let filename = filename.clone();
        let filename_error = filename_error.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let value = input.value();
                filename.set(value.clone());
                filename_error.set(String::new());
            }
        })
    };

    let on_password = {
        let password = password.clone();
        let password_error = password_error.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                let value = input.value();
                password.set(value.clone());
                password_error.set(String::new());
            }
        })
    };

    let onsubmit = {
        let filename = filename.clone();
        let password = password.clone();
        let filename_error = filename_error.clone();
        let password_error = password_error.clone();
        let cb = props.on_submit.clone();
        let push_toast = props.push_toast.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            filename_error.set(String::new());
            password_error.set(String::new());

            let mut has_error = false;

            if (*filename).is_empty() {
                push_toast.emit(("Filename is required".into(), ToastKind::Error));
                has_error = true;
            } else if !is_valid_filename(&filename) {
                push_toast.emit(("Filename contains invalid characters or is too long".into(), ToastKind::Error));
                has_error = true;
            }

            if (*password).is_empty() {
                push_toast.emit(("Password is required".into(), ToastKind::Error));
                has_error = true;
            } else if !is_valid_password(&password) {
                push_toast.emit(("Password must be at least 8 characters".into(), ToastKind::Error));
                has_error = true;
            }

            if has_error {
                return;
            }

            cb.emit(((*filename).clone(), (*password).clone()));
        })
    };

    html! {
        <div class="screen-container create-centered">
            <div class="create-inner">
                <p class="instruction-text">{"Choose a secure filename and strong password."}</p>

                <form class="create-form" {onsubmit}>
                    <div class="row">
                        <div>
                            <input
                                type="text"
                                placeholder="Wallet filename (e.g., mywallet)"
                                class={classes!("input", if !(*filename_error).is_empty() { "error" } else { "" })}
                                oninput={on_filename}
                                disabled={props.is_loading}
                            />
                            if !(*filename_error).is_empty() {
                                <p class="status error">{ (*filename_error).clone() }</p>
                            }
                        </div>
                        <div>
                            <input
                                type="password"
                                placeholder="Password"
                                class={classes!("input", if !(*password_error).is_empty() { "error" } else { "" })}
                                oninput={on_password}
                                disabled={props.is_loading}
                            />
                            if !(*password_error).is_empty() {
                                <p class="status error">{ (*password_error).clone() }</p>
                            }
                        </div>
                    </div>

                    <div class="button-group">
                        <button
                            type="submit"
                            disabled={props.is_loading}
                            class={classes!("btn", "btn-prominent", if props.is_loading { "loading" } else { "" })}
                        >
                            { if props.is_loading { "Creating..." } else { "Create Wallet" } }
                        </button>
                    </div>
                </form>

                <p class="create-import-link">
                    {"Have a recovery phrase? "}
                    <a href="#" onclick={props.on_import.clone()}>{"Import Wallet"}</a>
                </p>
            </div>
        </div>
    }
}