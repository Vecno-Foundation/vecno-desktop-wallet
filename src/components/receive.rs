use yew::prelude::*;
use qrcode::QrCode;
use image::{Luma, ImageFormat};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use gloo::timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Navigator, Clipboard};
use crate::models::WalletAddress;

#[derive(Properties, PartialEq)]
pub struct ReceiveProps {
    pub addresses: Vec<WalletAddress>,
    pub is_loading: bool,
}

#[function_component(Receive)]
pub fn receive(props: &ReceiveProps) -> Html {
    let generate_qr_data_url = |text: &str| -> String {
        let qr_code = QrCode::new(text).unwrap_or_else(|_| QrCode::new("").unwrap());
        let qr_image = qr_code.render::<Luma<u8>>().min_dimensions(180, 180).build();

        let mut png_bytes: Vec<u8> = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        qr_image.write_to(&mut cursor, ImageFormat::Png).unwrap_or(());

        let b64 = BASE64.encode(&png_bytes);
        format!("data:image/png;base64,{}", b64)
    };

    let copied_state = use_state(|| ("".to_string(), "".to_string()));

    let address_display = {
        let copied_state = copied_state.clone();
        move |addr_type: &str, address: &str| {
            let address_str = address.to_string();
            let addr_type_str = addr_type.to_string();

            let is_copied = copied_state.0 == address_str && copied_state.1 == addr_type_str;
            let display_text = if is_copied { "Copied!" } else { address };

            let on_copy = {
                let address = address_str.clone();
                let copied_state = copied_state.clone();
                let addr_type = addr_type_str.clone();

                Callback::from(move |_| {
                    let address = address.clone();
                    let copied_state = copied_state.clone();
                    let addr_type = addr_type.clone();

                    let navigator: Navigator = window().unwrap().navigator();
                    let clipboard: Clipboard = navigator.clipboard();
                    let promise = clipboard.write_text(&address);
                    let future = wasm_bindgen_futures::JsFuture::from(promise);

                    spawn_local(async move {
                        let _ = future.await;
                        copied_state.set((address.clone(), addr_type));
                        TimeoutFuture::new(2000).await;
                        copied_state.set(("".to_string(), "".to_string()));
                    });
                })
            };

            html! {
                <div class="address-container">
                    <p class="receive-address">{ display_text }</p>
                    <button class="copy-button" onclick={on_copy} title="Copy to clipboard">
                        if is_copied {
                            <svg class="icon check" viewBox="0 0 24 24" fill="currentColor">
                                <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                            </svg>
                        } else {
                            <svg class="icon copy" viewBox="0 0 24 24" fill="currentColor">
                                <path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/>
                            </svg>
                        }
                    </button>
                </div>
            }
        }
    };

    html! {
        <div class="screen-container receive-centered">
            <div class="receive-inner">
                <h2 class="receive-title">{"Receive VE"}</h2>
                <p class="receive-subtitle">
                    {"Share one of your wallet addresses to receive Vecno. Each account has a unique receive address and change address."}
                </p>

                { if props.is_loading {
                    html! { <p class="receive-loading" aria-live="polite">{"Loading addresses..."}</p> }
                } else if props.addresses.is_empty() {
                    html! { <p class="receive-empty" aria-live="assertive">{"No addresses found."}</p> }
                } else {
                    html! {
                        <div class="receive-grid">
                            { for props.addresses.iter().enumerate().map(|(i, addr)| {
                                let receive_qr = generate_qr_data_url(&addr.receive_address);
                                let change_qr = generate_qr_data_url(&addr.change_address);

                                let receive_display = address_display("receive", &addr.receive_address);
                                let change_display = address_display("change", &addr.change_address);

                                html! {
                                    <div class="receive-card" key={i}>
                                        <div class="receive-card-header">
                                            <span class="receive-account-name">{ &addr.account_name }</span>
                                            <span class="receive-account-index">{ format!("Account #{}", addr.account_index) }</span>
                                        </div>

                                        // Receive Address Section
                                        <div class="address-section receive">
                                            <div class="address-header">
                                                <h3 class="address-type receive">
                                                    {"Receive Address"}
                                                    <div class="tooltip-wrapper">
                                                        <svg
                                                            class="tooltip-icon"
                                                            tabindex="0"
                                                            viewBox="0 0 20 20"
                                                            fill="currentColor"
                                                            aria-label="More info about receive address"
                                                        >
                                                            <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-8-5a1 1 0 100 2 1 1 0 000-2zm0 3a1 1 0 011 1v4a1 1 0 11-2 0v-4a1 1 0 011-1z" clip-rule="evenodd" />
                                                        </svg>
                                                        <div class="tooltip">
                                                            {"This is your public receive address. Share this to receive payments. Funds sent here appear in your wallet balance."}
                                                        </div>
                                                    </div>
                                                </h3>
                                            </div>
                                            { receive_display }
                                            <div class="qr-container">
                                                <img src={receive_qr} alt={format!("QR code for receive address {}", addr.receive_address)} class="qr-code" />
                                            </div>
                                        </div>

                                        // Change Address Section
                                        <div class="address-section change">
                                            <div class="address-header">
                                                <h3 class="address-type change">
                                                    {"Change Address"}
                                                    <div class="tooltip-wrapper">
                                                        <svg
                                                            class="tooltip-icon"
                                                            tabindex="0"
                                                            viewBox="0 0 20 20"
                                                            fill="currentColor"
                                                            aria-label="More info about change address"
                                                        >
                                                            <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-8-5a1 1 0 100 2 1 1 0 000-2zm0 3a1 1 0 011 1v4a1 1 0 11-2 0v-4a1 1 0 011-1z" clip-rule="evenodd" />
                                                        </svg>
                                                        <div class="tooltip">
                                                            {"This is your internal change address. Used automatically by the wallet for transaction change outputs. Do NOT share publicly unless required."}
                                                        </div>
                                                    </div>
                                                </h3>
                                            </div>
                                            { change_display }
                                            <div class="qr-container">
                                                <img src={change_qr} alt={format!("QR code for change address {}", addr.change_address)} class="qr-code" />
                                            </div>
                                        </div>
                                    </div>
                                }
                            })}
                        </div>
                    }
                }}
            </div>
        </div>
    }
}