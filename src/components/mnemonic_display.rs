use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MnemonicDisplayProps {
    pub mnemonic: String,
    pub on_copy: Callback<String>,
    pub on_proceed: Callback<MouseEvent>,
    pub wallet_status: String,
}

#[function_component(MnemonicDisplay)]
pub fn mnemonic_display(props: &MnemonicDisplayProps) -> Html {
    let copy = {
        let m = props.mnemonic.clone();
        let cb = props.on_copy.clone();
        Callback::from(move |_| cb.emit(m.clone()))
    };

    html! {
        <div class="screen-container">
            <h2>{"Wallet Created Successfully"}</h2>
            <p class="instruction-text">
                {"Please save your 24-word mnemonic phrase securely. This is critical for recovering your wallet."}
            </p>
            <div class="mnemonic-container">
                <div class="mnemonic-box">
                    <code class="mnemonic-text">{ &props.mnemonic }</code>
                    <button onclick={copy} class="btn btn-sm btn-copy">{"Copy"}</button>
                </div>
            </div>
            <div class="button-group">
                <button onclick={props.on_proceed.clone()} class="btn btn-primary btn-prominent">
                    {"Proceed to Wallet"}
                </button>
            </div>
            { if !props.wallet_status.is_empty() {
                html! { <p class="status">{ &props.wallet_status }</p> }
            } else { html!{} }}
        </div>
    }
}