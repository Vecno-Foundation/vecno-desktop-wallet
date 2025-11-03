use yew::prelude::*;
use qrcode::QrCode;
use image::{Luma, ImageFormat};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use crate::models::WalletAddress;

#[derive(Properties, PartialEq)]
pub struct ReceiveProps {
    pub addresses: Vec<WalletAddress>,
    pub is_loading: bool,
}

#[function_component(Receive)]
pub fn receive(props: &ReceiveProps) -> Html {
    html! {
        <div class="screen-container receive-centered">
            <div class="receive-inner">
                <h2 class="receive-title">{"Receive VE"}</h2>
                <p class="receive-subtitle">
                    {"Share one of your wallet addresses to receive Vecno. Each account has a unique receive address."}
                </p>

                { if props.is_loading {
                    html! { <p class="receive-loading" aria-live="polite">{"Loading addresses..."}</p> }
                } else if props.addresses.is_empty() {
                    html! { <p class="receive-empty" aria-live="assertive">{"No addresses found."}</p> }
                } else {
                    html! {
                        <div class="receive-grid">
                            { for props.addresses.iter().enumerate().map(|(i, addr)| {
                                let qr_code = QrCode::new(&addr.receive_address).unwrap_or_else(|_| QrCode::new("").unwrap());
                                let qr_image = qr_code.render::<Luma<u8>>()
                                    .min_dimensions(160, 160)
                                    .build();

                                let mut png_bytes: Vec<u8> = Vec::new();
                                let mut cursor = std::io::Cursor::new(&mut png_bytes);
                                qr_image.write_to(&mut cursor, ImageFormat::Png).unwrap_or(());

                                let b64 = BASE64.encode(&png_bytes);
                                let data_url = format!("data:image/png;base64,{}", b64);

                                html! {
                                    <div class="receive-card" key={i}>
                                        <div class="receive-card-header">
                                            <span class="receive-account-name">{ &addr.account_name }</span>
                                            <span class="receive-account-index">{ format!("Receive Address #{}", addr.account_index) }</span>
                                        </div>

                                        <div class="receive-address-container">
                                            <div class="receive-address">{ &addr.receive_address }</div>
                                        </div>

                                        <img src={data_url} alt={format!("QR code for {}", addr.receive_address)} class="qr-code" />
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