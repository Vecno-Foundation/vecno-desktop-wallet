use crate::models::ToastKind;
use yew::prelude::*;
use web_sys::{ClipboardEvent, HtmlInputElement};
use gloo::events::{EventListener, EventListenerOptions};
use gloo::utils::document;
use wasm_bindgen::JsCast;
use crate::utils::{is_valid_password, is_valid_filename};

#[derive(Properties, PartialEq)]
pub struct ImportWalletProps {
    pub on_submit: Callback<(String, String, Option<String>, String)>,
    pub is_loading: bool,
    pub on_create: Callback<MouseEvent>,
    pub push_toast: Callback<(String, ToastKind)>,
}

#[function_component(ImportWallet)]
pub fn import_wallet(props: &ImportWalletProps) -> Html {
    let filename = use_state(String::new);
    let password = use_state(String::new);
    let payment_secret_words = use_state(|| vec![String::new(); 1]);
    let show_payment_secret = use_state(|| false);
    let mnemonic_words = use_state(|| vec![String::new(); 24]);
    let is_12_word = use_state(|| false);
    let filename_error = use_state(String::new);
    let password_error = use_state(String::new);
    let payment_secret_error = use_state(String::new);
    let mnemonic_error = use_state(String::new);

    let has_extended_mnemonic = use_state(|| false);
    let has_extended_payment = use_state(|| false);

    {
        let mnemonic_words = mnemonic_words.clone();
        let has_extended_mnemonic = has_extended_mnemonic.clone();
        use_effect_with(mnemonic_words.clone(), move |words| {
            let any_extended = (12..24).any(|i| !(*words)[i].is_empty());
            has_extended_mnemonic.set(any_extended);
            || ()
        });
    }

    {
        let payment_secret_words = payment_secret_words.clone();
        let has_extended_payment = has_extended_payment.clone();
        use_effect_with(payment_secret_words.clone(), move |words| {
            let any_filled = words.iter().any(|w| !w.is_empty());
            has_extended_payment.set(any_filled && words.len() > 1);
            || ()
        });
    }

    let on_filename = {
        let filename = filename.clone();
        let filename_error = filename_error.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
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
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                let value = input.value();
                password.set(value.clone());
                password_error.set(String::new());
            }
        })
    };

    let on_payment_word_change = {
        let words = payment_secret_words.clone();
        let payment_secret_error = payment_secret_error.clone();
        move |idx: usize| {
            let words = words.clone();
            let payment_secret_error = payment_secret_error.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                    let raw = input.value();
                    let value = raw.split_whitespace().next().unwrap_or("").trim().to_lowercase();
                    let mut current = (*words).clone();
                    if idx < current.len() {
                        current[idx] = value;
                        words.set(current);
                        payment_secret_error.set(String::new());
                    }
                }
            })
        }
    };

    let add_payment_word = {
        let words = payment_secret_words.clone();
        Callback::from(move |_| {
            let mut current = (*words).clone();
            if current.len() < 24 {
                current.push(String::new());
                words.set(current);
            }
        })
    };

    let toggle_payment_secret = {
        let show = show_payment_secret.clone();
        let words = payment_secret_words.clone();
        let error = payment_secret_error.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                let checked = input.checked();
                show.set(checked);
                if !checked {
                    words.set(vec![String::new(); 1]);
                    error.set(String::new());
                }
            }
        })
    };

    let on_mnemonic_word_change = {
        let words = mnemonic_words.clone();
        let mnemonic_error = mnemonic_error.clone();
        move |idx: usize| {
            let words = words.clone();
            let mnemonic_error = mnemonic_error.clone();
            Callback::from(move |e: InputEvent| {
                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                    let raw = input.value();
                    let value = raw.split_whitespace().next().unwrap_or("").trim().to_lowercase();
                    let mut current = (*words).clone();
                    if idx < current.len() {
                        current[idx] = value;
                        words.set(current);
                        mnemonic_error.set(String::new());
                    }
                }
            })
        }
    };

    {
        let words = mnemonic_words.clone();
        let is_12_word = is_12_word.clone();
        let mnemonic_error = mnemonic_error.clone();
        let push_toast = props.push_toast.clone();

        use_effect(move || {
            let options = EventListenerOptions::enable_prevent_default();

            let listener = EventListener::new_with_options(
                &document(),
                "paste",
                options,
                move |e| {
                    e.stop_propagation();
                    if let Some(clip_event) = e.dyn_ref::<ClipboardEvent>() {
                        clip_event.prevent_default();

                        if let Some(data) = clip_event.clipboard_data() {
                            if let Ok(text) = data.get_data("text") {
                                let cleaned = text.trim().to_lowercase();
                                let pasted_words: Vec<String> = cleaned
                                    .split_whitespace()
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();

                                let expected = if pasted_words.len() == 12 {
                                    12
                                } else if pasted_words.len() == 24 {
                                    24
                                } else {
                                    push_toast.emit((
                                        format!(
                                            "Pasted {} words – exactly 12 or 24 required",
                                            pasted_words.len()
                                        ),
                                        ToastKind::Error,
                                    ));
                                    return;
                                };

                                let mut new_words = vec![String::new(); 24];
                                for (i, word) in pasted_words.iter().enumerate() {
                                    new_words[i] = word.clone();
                                }

                                words.set(new_words);
                                is_12_word.set(expected == 12);
                                mnemonic_error.set(String::new());

                                push_toast.emit(("Mnemonic pasted successfully".into(), ToastKind::Success));
                            }
                        }
                    }
                },
            );

            || drop(listener)
        });
    }

    let toggle_12_word = {
        let is_12_word = is_12_word.clone();
        let words = mnemonic_words.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                let checked = input.checked();
                is_12_word.set(checked);
                if checked {
                    let mut current = (*words).clone();
                    for i in 12..24 {
                        current[i].clear();
                    }
                    words.set(current);
                }
            }
        })
    };

    let onsubmit = {
        let filename = filename.clone();
        let password = password.clone();
        let payment_secret_words = payment_secret_words.clone();
        let show_payment_secret = *show_payment_secret;
        let mnemonic_words = mnemonic_words.clone();
        let is_12_word = is_12_word.clone();
        let filename_error = filename_error.clone();
        let password_error = password_error.clone();
        let payment_secret_error = payment_secret_error.clone();
        let mnemonic_error = mnemonic_error.clone();
        let cb = props.on_submit.clone();
        let push_toast = props.push_toast.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            filename_error.set(String::new());
            password_error.set(String::new());
            payment_secret_error.set(String::new());
            mnemonic_error.set(String::new());

            let mut has_error = false;
            let expected_mnemonic = if *is_12_word { 12 } else { 24 };
            let filled_mnemonic: Vec<String> = (*mnemonic_words)
                .iter()
                .take(expected_mnemonic)
                .cloned()
                .filter(|w| !w.is_empty())
                .collect();

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

            if show_payment_secret {
                let filled_payment: Vec<String> = (*payment_secret_words)
                    .iter()
                    .cloned()
                    .filter(|w| !w.is_empty())
                    .collect();
                if filled_payment.is_empty() {
                    push_toast.emit(("Payment secret cannot be empty when enabled".into(), ToastKind::Error));
                    has_error = true;
                }
            }

            if filled_mnemonic.len() != expected_mnemonic {
                push_toast.emit((format!("Exactly {} words required", expected_mnemonic), ToastKind::Error));
                has_error = true;
            }

            if has_error {
                return;
            }

            let mnemonic = filled_mnemonic.join(" ");
            let pay_secret = if show_payment_secret {
                let filled: Vec<String> = (*payment_secret_words)
                    .iter()
                    .cloned()
                    .filter(|w| !w.is_empty())
                    .collect();
                if !filled.is_empty() {
                    Some(filled.join(" "))
                } else {
                    None
                }
            } else {
                None
            };

            web_sys::console::log_1(&format!(
                "FRONTEND: ImportWallet submit → filename='{}', payment_secret={:?}",
                *filename, pay_secret
            ).into());

            cb.emit((mnemonic, (*password).clone(), pay_secret, (*filename).clone()));
        })
    };

    html! {
        <div class="screen-container import-centered">
            <div class="import-inner centered-inner">
                <h2 class="import-title">{"Import Wallet"}</h2>
                <form class="import-form" {onsubmit}>
                    <div class="row centered-row">
                        <div class="input-wrapper">
                            <input
                                type="text"
                                placeholder="Wallet filename"
                                class={classes!("input", if !(*filename_error).is_empty() { "error" } else { "" })}
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
                                placeholder="New password"
                                class={classes!("input", if !(*password_error).is_empty() { "error" } else { "" })}
                                oninput={on_password}
                                disabled={props.is_loading}
                            />
                            if !(*password_error).is_empty() {
                                <p class="status error">{ (*password_error).clone() }</p>
                            }
                        </div>
                    </div>

                    <div class="mnemonic-section">
                        <div class="mnemonic-toggle">
                            <label class="checkbox-label">
                                <input
                                    type="checkbox"
                                    checked={*is_12_word}
                                    oninput={toggle_12_word.clone()}
                                    disabled={props.is_loading}
                                />
                                {"Use 12-word mnemonic"}
                            </label>
                            <label class="checkbox-label" style="margin-left: 1rem;">
                                <input
                                    type="checkbox"
                                    checked={*show_payment_secret}
                                    oninput={toggle_payment_secret}
                                    disabled={props.is_loading}
                                />
                                {"Use payment secret"}
                            </label>
                        </div>

                        <div class={classes!(
                            "mnemonic-grid",
                            if *is_12_word { "mode-12" } else { "mode-24" },
                            if *has_extended_mnemonic { "extended" } else { "" }
                        )}>
                            { for (0..24).map(|i| {
                                let on_input = on_mnemonic_word_change(i);
                                let is_faded = *has_extended_mnemonic && i < 12;
                                let is_disabled_slot = *is_12_word && i >= 12;
                                html! {
                                    <div class="word-slot" data-index={format!("{}", i + 1)}>
                                        <input
                                            type="text"
                                            placeholder="word"
                                            value={(*mnemonic_words)[i].clone()}
                                            oninput={on_input}
                                            class={classes!(
                                                "word-input",
                                                if !(*mnemonic_error).is_empty() { "error" } else { "" },
                                                if is_faded { "faded" } else { "" },
                                                if is_disabled_slot { "disabled-slot" } else { "" }
                                            )}
                                            disabled={props.is_loading || is_disabled_slot}
                                            onpaste={Callback::from(|e: Event| e.prevent_default())}
                                        />
                                    </div>
                                }
                            }) }
                        </div>

                        if !(*mnemonic_error).is_empty() {
                            <p class="status error centered-error">{ (*mnemonic_error).clone() }</p>
                        }
                    </div>

                    <div class={classes!(
                        "mnemonic-section",
                        if *show_payment_secret { "visible" } else { "hidden" }
                    )}>
                        <div class="mnemonic-toggle">
                            <div style="display: flex; align-items: center; gap: 0.5rem; width: 100%; justify-content: space-between;">
                                <span class="section-title" style="font-size: 1rem; margin: 0;">{"Payment Secret (BIP39 Passphrase)"}</span>
                                <button
                                    type="button"
                                    class="btn btn-small"
                                    onclick={add_payment_word}
                                    disabled={props.is_loading || (*payment_secret_words).len() >= 24}
                                >
                                    {"+ Add Word"}
                                </button>
                            </div>
                        </div>

                        <div class="mnemonic-grid">
                            { for (0..(*payment_secret_words).len()).map(|i| {
                                let on_input = on_payment_word_change(i);
                                html! {
                                    <div class="word-slot" data-index={format!("{}", i + 1)}>
                                        <input
                                            type="text"
                                            placeholder="word"
                                            value={(*payment_secret_words)[i].clone()}
                                            oninput={on_input}
                                            class={classes!(
                                                "word-input",
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

                    <div class="button-group">
                        <button
                            type="submit"
                            disabled={props.is_loading}
                            class={classes!("btn", "btn-prominent", if props.is_loading { "loading" } else { "" })}
                        >
                            { if props.is_loading { "Importing..." } else { "Import Wallet" } }
                        </button>
                    </div>
                </form>

                <p class="import-create-link">
                    {"No phrase? "}
                    <a href="#" onclick={props.on_create.clone()}>{"Create New Wallet"}</a>
                </p>
            </div>
        </div>
    }
}