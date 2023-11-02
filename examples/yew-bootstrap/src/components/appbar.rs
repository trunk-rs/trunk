use yew::prelude::*;

#[function_component]
pub fn Appbar() -> Html {
    html!(
        <nav class="navbar navbar-dark bg-info">
            <div class="container-fluid">
                <span class="navbar-brand mb-0 h1">{"Trunk | Yew | Bootstrap"}</span>
            </div>
        </nav>
    )
}
