use yew::UseStateHandle;
use yew::platform::spawn_local;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use log::error;
use js_sys::{Promise, Reflect};
use wasm_bindgen_futures::JsFuture;
use web_sys::window;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    pub async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

pub async fn safe_invoke(cmd: &str, args: JsValue) -> Result<JsValue, String> {
    let window = window().ok_or("No window")?;
    let tauri = Reflect::get(&window, &"__TAURI__".into())
        .map_err(|_| "Tauri not found".to_string())?;
    let core = Reflect::get(&tauri, &"core".into())
        .map_err(|_| "Tauri core not found".to_string())?;
    let invoke_fn = Reflect::get(&core, &"invoke".into())
        .map_err(|_| "invoke not found".to_string())?;

    let func = js_sys::Function::from(invoke_fn);
    let promise = func.call2(&JsValue::NULL, &cmd.into(), &args)
        .map_err(|_| "Call failed".to_string())?;

    let promise = Promise::from(promise);
    let result = JsFuture::from(promise).await
        .map_err(|js_err| get_error_message(js_err))?;

    Ok(result)
}

pub fn is_valid_filename(filename: &str) -> bool {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*', ','];
    !filename.is_empty()
        && !filename.chars().any(|c| invalid_chars.contains(&c))
        && filename.len() <= 255
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
        format!("{:.8} VE", ve)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
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
        let _ = JsFuture::from(Promise::new(&mut |resolve, _| {
            window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay_ms as i32)
                .unwrap();
        }))
        .await;
        status.set(String::new());
    });
}

pub fn get_error_message(res: JsValue) -> String {
    if let Ok(error_val) = Reflect::get(&res, &"error".into()) {
        if let Some(s) = error_val.as_string() {
            return s;
        }
    }

    if let Some(s) = res.as_string() {
        return s;
    }

    if let Ok(str_val) = js_sys::JSON::stringify(&res) {
        if let Some(s) = str_val.as_string() {
            return s;
        }
    }

    "Unknown error (failed to extract message)".to_string()
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

    let window = window().unwrap();
    let invoke_fn = match Reflect::get(&window, &"__TAURI__".into())
        .and_then(|tauri| Reflect::get(&tauri, &"core".into()))
        .and_then(|core| Reflect::get(&core, &"invoke".into()))
    {
        Ok(f) => f,
        Err(_) => return Err("Tauri not available".into()),
    };

    let promise = match js_sys::Function::from(invoke_fn)
        .call2(&JsValue::NULL, &"verify_wallet_password".into(), &args)
    {
        Ok(p) => p,
        Err(e) => {
            let msg = get_error_message(e);
            error!("Tauri invoke failed: {}", msg);
            return Err(msg);
        }
    };

    let result = match JsFuture::from(Promise::from(promise)).await {
        Ok(res) => res,
        Err(js_err) => {
            let msg = get_error_message(js_err);
            error!("Command failed: {}", msg);
            return Err(msg);
        }
    };

    let msg = get_error_message(result);

    if msg.contains("Incorrect password")
        || msg.contains("error")
        || msg.contains("not exist")
        || msg.contains("Invalid")
        || msg.contains("failed")
    {
        Err(msg)
    } else {
        Ok(())
    }
}

pub fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut result = String::new();

    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*ch);
    }
    result
}

pub fn format_hashrate(hashrate_mh: f64) -> String {
    let n = hashrate_mh;

    if n < 1_000.0 {
        format!("{:.2} MH/s", n)
    } else if n < 1_000_000.0 {
        format!("{:.2} GH/s", n / 1_000.0)
    } else if n < 1_000_000_000.0 {
        format!("{:.2} TH/s", n / 1_000_000.0)
    } else if n < 1_000_000_000_000.0 {
        format!("{:.2} PH/s", n / 1_000_000_000.0)
    } else {
        format!("{:.2} EH/s", n / 1_000_000_000_000.0)
    }
}

pub fn format_difficulty(diff: f64) -> String {
    if diff <= 0.0 {
        return "N/A".to_string();
    }

    let units = ["", "K", "M", "G", "T", "P", "E"];
    let mut value = diff;
    let mut index = 0;

    while value >= 1000.0 && index < units.len() - 1 {
        value /= 1000.0;
        index += 1;
    }

    let mut num_str = format!("{:.2}", value);
    num_str = num_str.trim_end_matches('0').to_string();
    num_str = num_str.trim_end_matches('.').to_string();

    let unit = units[index];
    if unit.is_empty() {
        num_str
    } else {
        format!("{} {}", num_str, unit)
    }
}