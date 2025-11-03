use yew::prelude::*;
use crate::models::Transaction;
use crate::utils::format_amount;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::prelude::*;
use js_sys::Reflect;
use web_sys::window;

#[derive(Properties, PartialEq)]
pub struct TxDetailProps {
    pub tx: Transaction,
    pub our_address: String,
    pub on_close: Callback<()>,
}

#[function_component(TxDetailModal)]
pub fn tx_detail_modal(props: &TxDetailProps) -> Html {
    let is_out = !props.tx.to_address.is_empty() && props.tx.to_address != props.our_address;
    let direction = if is_out { "Sent" } else { "Received" };
    let sign = if is_out { "-" } else { "+" };
    let amount_class = if is_out { "amount-out" } else { "amount-in" };

    let explorer_url = format!("https://vecnoscan.org/txs/{}", props.tx.txid);

    let on_explorer_click = {
        let url = explorer_url.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            e.stop_propagation();

            let url = url.clone();
            let window = window().expect("window should exist");

            spawn_local(async move {
                let global = js_sys::global();
                if let Ok(tauri_obj) = Reflect::get(&global, &JsValue::from("__TAURI__")) {
                    if let Ok(opener_obj) = Reflect::get(&tauri_obj, &JsValue::from("opener")) {
                        if let Ok(open_fn) = Reflect::get(&opener_obj, &JsValue::from("openUrl")) {
                            let fn_obj = js_sys::Function::from(open_fn);
                            let _ = fn_obj.call1(&opener_obj, &JsValue::from(&url));
                            return;
                        }
                    }
                }
                let _ = window.open_with_url_and_target(&url, "_blank");
            });
        })
    };

    html! {
        <div class="modal-overlay" onclick={props.on_close.reform(|_| ())}>
            <div class="modal" onclick={|e: MouseEvent| e.stop_propagation()}>
                <div class="modal-header">
                    <h3>{ direction }{ " Transaction" }</h3>
                    <button class="close-btn" onclick={props.on_close.reform(|_| ())}>{"×"}</button>
                </div>
                <div class="modal-body">
                    <p><strong>{"Amount:"}</strong>
                        <span class={classes!("tx-amt", amount_class)}>
                            { sign }{ format_amount(props.tx.amount) }
                        </span>
                    </p>
                    <p><strong>{"Date:"}</strong> { &props.tx.timestamp }</p>
                    <p><strong>{"Address:"}</strong>
                        <span class="tx-addr">{ if is_out { &props.tx.to_address } else { &props.our_address } }</span>
                    </p>
                    <p><strong>{"TXID:"}</strong></p>
                    <div class="txid-box">
                        <code class="tx-addr">{ &props.tx.txid }</code>
                        <button onclick={on_explorer_click} class="btn btn-sm btn-explorer">
                            {"Open in Vecnoscan"}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}