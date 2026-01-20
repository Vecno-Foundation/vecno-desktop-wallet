use yew::prelude::*;
use gloo_timers::callback::Timeout;
use crate::models::ToastKind;

#[derive(Properties, PartialEq)]
pub struct ToastProps {
    pub message: String,
    #[prop_or_default]
    pub kind: ToastKind,
    pub on_close: Callback<()>,
}

#[function_component(Toast)]
pub fn toast(props: &ToastProps) -> Html {
    let kind_class = props.kind.class();
    let icon_mask = props.kind.icon_mask();

    let close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    html! {
        <div class={classes!("toast", kind_class)}>
            <div class="toast-content">
                <span class="toast-icon" style={format!("-webkit-mask-image: url(\"{}\"); mask-image: url(\"{}\");", icon_mask, icon_mask)}></span>
                <span class="toast-message">{ &props.message }</span>
                <button class="toast-close" onclick={close} aria-label="Close">{ "×" }</button>
            </div>
            <div class="toast-progress-container">
                <div class="toast-progress-bar"></div>
            </div>
        </div>
    }
}

#[hook]
pub fn use_toast() -> (
    UseStateHandle<Option<(String, ToastKind)>>,
    Callback<(String, ToastKind)>,
    Callback<()>,
    Html,
) {
    let toast = use_state(|| None::<(String, ToastKind)>);

    {
        let toast = toast.clone();
        use_effect_with(toast.clone(), move |t| {
            if t.is_some() {
                let toast = toast.clone();
                let handle = Timeout::new(5_000, move || toast.set(None));
                handle.forget();
            }
            || ()
        });
    }

    let clear_toast = {
        let toast = toast.clone();
        Callback::from(move |_| toast.set(None))
    };

    let push_toast = {
        let toast = toast.clone();
        Callback::from(move |(msg, kind)| {
            web_sys::console::log_1(&format!("PUSH TOAST: {} ({:?})", msg, kind).into());
            toast.set(None);
            let toast_clone = toast.clone();
            let handle = Timeout::new(5_000, move || {
                toast_clone.set(Some((msg, kind)));
            });
            handle.forget();
        })
    };

    let render_toast = {
        let toast = toast.clone();
        let clear = clear_toast.clone();
        html! {
            <div class="toast-container">
                if let Some((msg, kind)) = &*toast {
                    <Toast message={msg.clone()} kind={kind.clone()} on_close={clear.reform(|_| ())} />
                }
            </div>
        }
    };

    (toast, push_toast, clear_toast, render_toast)
}