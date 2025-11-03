use yew::UseStateHandle;
use yew::platform::spawn_local;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use log::error;
use js_sys::{Promise, Reflect};
use wasm_bindgen_futures::JsFuture;

// Re-export invoke
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    pub async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

pub fn is_valid_filename(filename: &str) -> bool {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*', ','];
    !filename.is_empty() && !filename.contains(&invalid_chars[..]) && filename.len() <= 255
}

pub fn is_valid_password(secret: &str) -> bool {
    secret.len() >= 8
}

pub fn format_balance(balance: u64) -> String {
    if balance == 0 {
        "0 VE".to_string()
    } else {
        let ve = balance as f64 / 100_000_000.0;
        format!("{:.8} VE", ve)
    }
}

pub fn format_amount(amount: u64) -> String {
    if amount == 0 {
        "0 VE".to_string()
    } else {
        let ve = amount as f64 / 100_000_000.0;
        format!("{:.8} VE", ve).trim_end_matches('0').trim_end_matches('.').to_string() + ""
    }
}

pub fn ve_to_veni(ve_str: &str) -> Option<u64> {
    let ve_str = ve_str.trim();
    if ve_str.is_empty() || ve_str == "0" || ve_str == "0." || ve_str.ends_with('.') {
        return None;
    }

    let ve = ve_str.parse::<f64>().ok()?;
    if ve <= 0.0 {
        return None;
    }

    let veni = (ve * 100_000_000.0).round() as u64;

    if veni == 0 {
        None
    } else {
        Some(veni)
    }
}

pub fn clear_status_after_delay(status: UseStateHandle<String>, delay_ms: u64) {
    let status = status.clone();
    spawn_local(async move {
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _| {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay_ms as i32)
                .unwrap();
        }))
        .await
        .unwrap();
        status.set(String::new());
    });
}

// Add this function
pub fn get_error_message(res: JsValue) -> String {
    // 1. Try { error: "..." }
    if let Ok(error_val) = Reflect::get(&res, &"error".into()) {
        if let Some(s) = error_val.as_string() {
            return s;
        }
    }

    // 2. Try plain string
    if let Some(s) = res.as_string() {
        return s;
    }

    // 3. Fallback
    format!("{:?}", res)
}

pub async fn verify_password(filename: &str, secret: &str) -> Result<(), String> {
    if filename.is_empty() {
        return Err("Wallet filename is required".into());
    }
    if secret.is_empty() {
        return Err("Password is required".into());
    }

    let args = match serde_wasm_bindgen::to_value(&serde_json::json!({
        "filename": filename,
        "secret": secret
    })) {
        Ok(a) => a,
        Err(e) => {
            error!("Serialization error: {}", e);
            return Err(format!("Request error: {}", e));
        }
    };

    let promise = match js_sys::Reflect::get(&web_sys::window().unwrap(), &"__TAURI__".into())
        .and_then(|tauri| js_sys::Reflect::get(&tauri, &"core".into()))
        .and_then(|core| js_sys::Reflect::get(&core, &"invoke".into()))
        .ok()
    {
        Some(invoke_fn) => {
            match js_sys::Function::from(invoke_fn).call2(&JsValue::NULL, &"verify_wallet_password".into(), &args) {
                Ok(p) => p,
                Err(e) => {
                    let msg = get_error_message(e);
                    error!("Tauri invoke failed: {}", msg);
                    return Err(msg);
                }
            }
        }
        None => {
            return Err("Tauri not available".into());
        }
    };

    let promise = Promise::from(promise);

    let result = match JsFuture::from(promise).await {
        Ok(res) => res,
        Err(js_err) => {
            let msg = get_error_message(js_err);
            error!("Command failed: {}", msg);
            return Err(msg);
        }
    };

    let msg = get_error_message(result);

    if msg.contains("Incorrect password") ||
       msg.contains("error") ||
       msg.contains("not exist") ||
       msg.contains("Invalid") ||
       msg.contains("failed") {
        return Err(msg);
    }

    Ok(())
}