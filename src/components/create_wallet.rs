use crate::models::ToastKind;
use yew::prelude::*;
use crate::utils::{is_valid_password, is_valid_filename};

#[derive(Properties, PartialEq)]
pub struct CreateWalletProps {
    pub on_submit: Callback<(String, String, Option<String>)>,
    pub is_loading: bool,
    pub on_import: Callback<MouseEvent>,
    pub push_toast: Callback<(String, ToastKind)>,
}

#[function_component(CreateWallet)]
pub fn create_wallet(props: &CreateWalletProps) -> Html {
    let filename               = use_state(String::new);
    let password               = use_state(String::new);
    let payment_secret_words   = use_state(|| vec![String::new(); 1]);
    let show_payment_secret    = use_state(|| false);
    let has_extended_payment   = use_state(|| false);

    let filename_error         = use_state(String::new);
    let password_error         = use_state(String::new);
    let payment_secret_error   = use_state(String::new);

    {
        let words = payment_secret_words.clone();
        let has   = has_extended_payment.clone();
        use_effect_with(words.clone(), move |w| {
            let any = w.iter().any(|s| !s.is_empty()) && w.len() > 1;
            has.set(any);
            || ()
        });
    }

    let on_filename = {
        let f = filename.clone();
        let e = filename_error.clone();
        Callback::from(move |ev: InputEvent| {
            if let Some(i) = ev.target_dyn_into::<web_sys::HtmlInputElement>() {
                f.set(i.value());
                e.set(String::new());
            }
        })
    };

    let on_password = {
        let p = password.clone();
        let e = password_error.clone();
        Callback::from(move |ev: InputEvent| {
            if let Some(i) = ev.target_dyn_into::<web_sys::HtmlInputElement>() {
                p.set(i.value());
                e.set(String::new());
            }
        })
    };

    let on_payment_word_change = {
        let words = payment_secret_words.clone();
        let err   = payment_secret_error.clone();
        move |idx: usize| {
            let w = words.clone();
            let e = err.clone();
            Callback::from(move |ev: InputEvent| {
                if let Some(i) = ev.target_dyn_into::<web_sys::HtmlInputElement>() {
                    let raw   = i.value();
                    let word  = raw.split_whitespace().next().unwrap_or("").trim().to_lowercase();
                    let mut cur = (*w).clone();
                    if idx < cur.len() {
                        cur[idx] = word;
                        w.set(cur);
                        e.set(String::new());
                    }
                }
            })
        }
    };

    let add_payment_word = {
        let w = payment_secret_words.clone();
        Callback::from(move |_| {
            let mut cur = (*w).clone();
            if cur.len() < 24 {
                cur.push(String::new());
                w.set(cur);
            }
        })
    };

    let toggle_payment_secret = {
        let show = show_payment_secret.clone();
        let words = payment_secret_words.clone();
        let err   = payment_secret_error.clone();
        Callback::from(move |ev: InputEvent| {
            if let Some(i) = ev.target_dyn_into::<web_sys::HtmlInputElement>() {
                let checked = i.checked();
                show.set(checked);
                if !checked {
                    words.set(vec![String::new(); 1]);
                    err.set(String::new());
                }
            }
        })
    };


    let onsubmit = {
        let fnm   = filename.clone();
        let pwd   = password.clone();
        let words = payment_secret_words.clone();
        let show  = *show_payment_secret;

        let e_fn  = filename_error.clone();
        let e_pw  = password_error.clone();
        let e_ps  = payment_secret_error.clone();

        let cb    = props.on_submit.clone();
        let toast = props.push_toast.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            e_fn.set(String::new());
            e_pw.set(String::new());
            e_ps.set(String::new());

            let mut err = false;

            let name = (*fnm).trim();
            if name.is_empty() {
                toast.emit(("Filename is required".into(), ToastKind::Error));
                err = true;
            } else if !is_valid_filename(name) {
                toast.emit(("Invalid filename. Use letters, numbers, dashes.".into(), ToastKind::Error));
                err = true;
            }

            let pw = pwd.clone();
            if pw.is_empty() {
                toast.emit(("Password is required".into(), ToastKind::Error));
                err = true;
            } else if !is_valid_password(&pw) {
                toast.emit(("Password too weak. Use 8+ chars, mix types.".into(), ToastKind::Error));
                err = true;
            }

            let filled: Vec<String> = (*words)
                .iter()
                .cloned()
                .filter(|s| !s.is_empty())
                .collect();

            let pay_opt = if show && !filled.is_empty() {
                Some(filled.join(" "))
            } else {
                None
            };

            if show && filled.is_empty() {
                toast.emit(("Payment secret cannot be empty when enabled".into(), ToastKind::Error));
                err = true;
            }

            if err { return; }

            web_sys::console::log_1(&format!(
                "FRONTEND: CreateWallet → filename='{}', payment_secret={}",
                name,
                if pay_opt.is_some() { "provided" } else { "none" }
            ).into());

            cb.emit((name.to_string(), pw.to_string(), pay_opt));
        })
    };

    let has_payment_secret = !(*payment_secret_words).iter().all(|s| s.is_empty());

    html! {
        <div class="import-centered">
            <div class="import-inner">
                <h2 class="import-title">{"Create Wallet"}</h2>

                <form class="import-form" {onsubmit}>
                    <div class="row centered-row">
                        <div class="input-wrapper">
                            <input
                                type="text"
                                placeholder="Wallet filename"
                                class={classes!("input", if !(*filename_error).is_empty() { "error" } else { "" })}
                                value={(*filename).clone()}
                                oninput={on_filename}
                                disabled={props.is_loading}
                            />
                            if !(*filename_error).is_empty() {
                                <p class="status error">{ (*filename_error).clone() }</p>
                            }
                        </div>
                        <div class="input-wrapper">
                            <input
                                type="password"
                                placeholder="Password"
                                class={classes!("input", if !(*password_error).is_empty() { "error" } else { "" })}
                                value={(*password).clone()}
                                oninput={on_password}
                                disabled={props.is_loading}
                            />
                            if !(*password_error).is_empty() {
                                <p class="status error">{ (*password_error).clone() }</p>
                            }
                        </div>
                    </div>

                    <div class="row centered-row">
                        <div class="mnemonic-toggle">
                            <label class="checkbox-label tooltip-wrapper">
                                <input
                                    type="checkbox"
                                    checked={*show_payment_secret}
                                    oninput={toggle_payment_secret}
                                    disabled={props.is_loading}
                                />
                                {"Use custom secret (BIP39 passphrase)"}

                                <span class="tooltip">
                                    {"Optional – adds an extra layer of security. This key must be used when performing a wallet transfer!"}
                                </span>
                            </label>
                        </div>
                    </div>
                    <div class={classes!(
                        "create-payment-secret-section",
                        if *show_payment_secret { "visible" } else { "hidden" }
                    )}>
                        <div class="create-mnemonic-toggle">
                            <div style="display:flex;align-items:center;gap:0.5rem;width:100%;justify-content:space-between;">
                                <span class="section-title" style="font-size:1rem;margin:0;">
                                    {"Custom Secret (BIP39 Passphrase)"}
                                </span>
                                <button
                                    type="button"
                                    class="btn btn-small create-add-word-btn"
                                    onclick={add_payment_word}
                                    disabled={props.is_loading || (*payment_secret_words).len() >= 24}
                                >
                                    {"+ Add Word"}
                                </button>
                            </div>
                        </div>

                        <div class={classes!(
                            "create-mnemonic-grid",
                            if *has_extended_payment { "extended" } else { "" }
                        )}>
                            { for (0..(*payment_secret_words).len()).map(|i| {
                                let on_input = on_payment_word_change(i);
                                html! {
                                    <div class="create-word-slot" data-index={format!("{}", i+1)}>
                                        <input
                                            type="text"
                                            placeholder="word"
                                            value={(*payment_secret_words)[i].clone()}
                                            oninput={on_input}
                                            class={classes!(
                                                "create-word-input",
                                                if !(*payment_secret_error).is_empty() { "error" } else { "" }
                                            )}
                                            disabled={props.is_loading}
                                        />
                                    </div>
                                }
                            }) }
                        </div>

                        if !(*payment_secret_error).is_empty() {
                            <p class="status error centered-error">{ (*payment_secret_error).clone() }</p>
                        }
                    </div>

                    if has_payment_secret {
                        <div class="row">
                            <div>
                                <p class="status success">{"Custom secret will be used"}</p>
                            </div>
                        </div>
                    }

                    <div class="button-group">
                        <button
                            type="submit"
                            disabled={props.is_loading}
                            class={classes!("btn", "btn-prominent", if props.is_loading { "loading" } else { "" })}
                        >
                            { if props.is_loading { "Creating…" } else { "Create Wallet" } }
                        </button>
                    </div>
                </form>
                <p class="import-create-link">
                    {"Have a recovery phrase? "}
                    <a href="#" onclick={props.on_import.clone()}>{"Import Wallet"}</a>
                </p>
            </div>
        </div>
    }
}