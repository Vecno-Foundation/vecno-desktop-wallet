use yew::prelude::*;

#[function_component(Intro)]
pub fn intro() -> Html {
    html! {
        <div class="intro-screen">
            <div class="logo-wrapper logo-intro">
                <img src="public/vecnotest.png" class="logo vecno intro-logo" alt="Vecno"/>
            </div>
        </div>
    }
}