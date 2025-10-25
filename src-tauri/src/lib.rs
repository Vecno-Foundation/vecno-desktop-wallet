use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde_json::json;

#[function_component(App)]
fn app() -> Html {
    let balance = use_state(|| 0u64);
    let status = use_state(|| String::new());
    let address = use_state(|| String::new());

    let on_create_wallet = {
        let balance = balance.clone();
        let status = status.clone();
        let address = address.clone();
        Callback::from(move |_| {
            let status = status.clone();
            let balance = balance.clone();
            let address = address.clone();
            spawn_local(async move {
                // Create wallet
                let result: Result<String, JsValue> = tauri_sys::tauri::invoke("create_wallet", &json!({"mnemonic": null}))
                    .await
                    .map_err(|e| JsValue::from_str(&format!("Invoke error: {}", e)));
                status.set(match result {
                    Ok(msg) => msg,
                    Err(e) => format!("Error: {:?}", e),
                });

                // Get address
                let addr_result: Result<String, JsValue> = tauri_sys::tauri::invoke("get_address", &json!({}))
                    .await
                    .map_err(|e| JsValue::from_str(&format!("Invoke error: {}", e)));
                if let Ok(addr) = addr_result {
                    address.set(addr.clone());

                    // Fetch balance
                    let balance_result: Result<u64, JsValue> = tauri_sys::tauri::invoke("get_balance", &json!({"address": addr}))
                        .await
                        .map_err(|e| JsValue::from_str(&format!("Invoke error: {}", e)));
                    if let Ok(b) = balance_result {
                        balance.set(b);
                    }
                }
            });
        })
    };

    html! {
        <div style="padding: 20px;">
            <h1>{ "Vecno Wallet" }</h1>
            <button onclick={on_create_wallet}>{ "Create Wallet" }</button>
            <p>{ format!("Address: {}", *address) }</p>
            <p>{ format!("Balance: {} VE", *balance) }</p>
            <p>{ format!("Status: {}", *status) }</p>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}