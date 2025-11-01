use yew::UseStateHandle;
use yew::platform::spawn_local;

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

    // Use precise conversion: 1 VE = 100,000,000 VENI
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
