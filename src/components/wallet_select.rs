use yew::prelude::*;
use crate::models::WalletFile;
use web_sys::{MouseEvent, Element, InputEvent};
use wasm_bindgen::JsCast;

#[derive(Properties, PartialEq)]
pub struct WalletSelectProps {
    pub wallets: Vec<WalletFile>,
    pub selected: String,
    pub on_select: Callback<String>,
}

#[function_component(WalletSelect)]
pub fn wallet_select(props: &WalletSelectProps) -> Html {
    let is_open = use_state(|| false);
    let search_query = use_state(String::new);
    let wrapper_ref = use_node_ref();
    let selected_wallet = props.wallets.iter().find(|w| w.path == props.selected);

    let toggle_dropdown = {
        let is_open = is_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            is_open.set(!*is_open);
        })
    };

    let select_wallet = {
        let is_open = is_open.clone();
        let search_query = search_query.clone();
        let on_select = props.on_select.clone();
        Callback::from(move |path: String| {
            on_select.emit(path);
            is_open.set(false);
            search_query.set(String::new());
        })
    };

    let on_search_input = {
        let search_query = search_query.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                search_query.set(input.value());
            }
        })
    };

    {
        let search_query = search_query.clone();
        use_effect_with(is_open.clone(), move |open| {
            if !**open {
                search_query.set(String::new());
            }
            || {}
        });
    }

    let filtered_wallets: Vec<&WalletFile> = if search_query.is_empty() {
        props.wallets.iter().collect()
    } else {
        let query_lower = search_query.to_lowercase();
        props.wallets.iter()
            .filter(|w| w.name.to_lowercase().contains(&query_lower))
            .collect()
    };

    {
        let is_open = is_open.clone();
        let wrapper_ref = wrapper_ref.clone();
        use_effect_with(is_open.clone(), move |open| {
            let mut closure_opt: Option<wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>> = None;
            
            if **open {
                let is_open_clone = is_open.clone();
                let wrapper_ref_clone = wrapper_ref.clone();
                
                let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |e: web_sys::Event| {
                    if let Some(target) = e.target() {
                        if let Ok(target_el) = target.dyn_into::<Element>() {
                            if let Some(wrapper_node) = wrapper_ref_clone.get() {
                                if let Some(wrapper_el) = wrapper_node.dyn_ref::<Element>() {
                                    if !wrapper_el.contains(Some(&target_el)) {
                                        is_open_clone.set(false);
                                    }
                                }
                            }
                        }
                    }
                }) as Box<dyn FnMut(_)>);

                let document = web_sys::window()
                    .and_then(|w| w.document());

                if let Some(doc) = document {
                    let _ = doc.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
                }

                closure_opt = Some(closure);
            }

            move || {
                if let Some(closure) = closure_opt {
                    closure.forget();
                }
            }
        });
    }

    html! {
        <div 
            ref={wrapper_ref.clone()}
            class="wallet-select-wrapper" 
            onclick={Callback::from(move |e: MouseEvent| e.stop_propagation())}
        >
            <button
                type="button"
                class="wallet-select-button"
                onclick={toggle_dropdown}
            >
                <span class="wallet-select-text">
                    { if let Some(wallet) = selected_wallet {
                        &wallet.name
                    } else {
                        "Select a wallet"
                    }}
                </span>
                <span class={classes!("wallet-select-arrow", if *is_open { "open" } else { "" })} aria-hidden="true"></span>
            </button>
            { if *is_open {
                html! {
                    <div class="wallet-select-dropdown">
                        <div class="wallet-select-search">
                            <input
                                type="text"
                                placeholder="Search wallets..."
                                class="wallet-select-search-input"
                                value={(*search_query).clone()}
                                oninput={on_search_input}
                                onclick={Callback::from(move |e: MouseEvent| e.stop_propagation())}
                            />
                        </div>
                        <div class="wallet-select-options">
                            { if filtered_wallets.is_empty() {
                                html! {
                                    <div class="wallet-select-empty">
                                        {"No wallets found"}
                                    </div>
                                }
                            } else {
                                html! {
                                    { for filtered_wallets.iter().map(|w| {
                                        let path = w.path.clone();
                                        let name = w.name.clone();
                                        let is_selected = w.path == props.selected;
                                        html! {
                                            <button
                                                type="button"
                                                class={classes!(
                                                    "wallet-select-option",
                                                    if is_selected { "selected" } else { "" }
                                                )}
                                                onclick={select_wallet.reform(move |_| path.clone())}
                                            >
                                                { &name }
                                            </button>
                                        }
                                    })}
                                }
                            }}
                        </div>
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
