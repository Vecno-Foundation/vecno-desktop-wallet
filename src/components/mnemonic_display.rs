use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MnemonicDisplayProps {
    pub mnemonic: String,
    pub on_copy: Callback<String>,
    pub on_proceed: Callback<MouseEvent>,
}

#[function_component(MnemonicDisplay)]
pub fn mnemonic_display(props: &MnemonicDisplayProps) -> Html {
    let words: Vec<&str> = props.mnemonic.split_whitespace().collect();

    let copy = {
        let m = props.mnemonic.clone();
        let cb = props.on_copy.clone();
        Callback::from(move |_| cb.emit(m.clone()))
    };

    html! {
        <div class="screen-container mnemonic-centered">
            <div class="mnemonic-inner">
                <h2 class="mnemonic-title">{"Wallet Created Successfully"}</h2>
                <p class="mnemonic-instruction">
                    {"Please save your 24-word mnemonic phrase securely. This is the ONLY way to recover your wallet if you lose access. Store it offline and never share it."}
                </p>

                <div class="mnemonic-container">
                    <div class="mnemonic-box">
                        <div class="mnemonic-text">
                            { for words.iter().enumerate().map(|(i, word)| {
                                html! {
                                    <span>
                                        <strong>{ format!("{}.", i + 1) }</strong>
                                        { *word }
                                    </span>
                                }
                            }) }
                        </div>
                        <button onclick={copy} class="btn btn-copy">{"Copy Mnemonic"}</button>
                    </div>
                </div>

                <div class="button-group mnemonic-button-group">
                    <button onclick={props.on_proceed.clone()} class="btn btn-prominent">
                        {"Proceed to Wallet"}
                    </button>
                </div>
            </div>
        </div>
    }
}