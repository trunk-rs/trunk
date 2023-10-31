use yew::prelude::*;

mod components;
mod pages;

#[function_component]
// the name of this function is passed to the renderer struct
fn AppRoot() -> Html {
    html!(
        <>
            // appbar section
            <components::appbar::Appbar />

            // Login section
            <div class="container-fluid">
                <div class="row min-vh-100 align-items-center justify-content-center">
                    <div class="col-sm-4">
                        <div class="card">
                            <div class="card-body">
                                <pages::login::Login />
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </>
    )
}

fn main() {
    yew::Renderer::<AppRoot>::new().render();
}
