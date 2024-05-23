use console_error_panic_hook::set_once as set_panic_hook;
use wasm_bindgen::prelude::*;
use web_sys::window;
use ybc::TileCtx::{Ancestor, Child, Parent};
use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    let location_service = LocationService {};
    let location = location_service.get_location().href().unwrap_or_default();
    html! {
        <>
        <ybc::Navbar
            classes={classes!("is-success")}
            padded=true
            navbrand={html!{
                <ybc::NavbarItem>
                    <ybc::Title classes={classes!("has-text-white")} size={ybc::HeaderSize::Is4}>{"Location Demo"}</ybc::Title>
                </ybc::NavbarItem>
            }}
        />

        <ybc::Hero
            classes={classes!("is-light")}
            size={ybc::HeroSize::FullheightWithNavbar}
            body={html!{
                <ybc::Container classes={classes!("is-centered")}>
                <ybc::Tile ctx={Ancestor}>
                    <ybc::Tile ctx={Parent} size={ybc::TileSize::Twelve}>
                        <ybc::Tile ctx={Parent}>
                            <ybc::Tile ctx={Child} classes={classes!("notification", "is-success")}>
                                <ybc::Subtitle size={ybc::HeaderSize::Is3} classes={classes!("has-text-white")}>{"Location"}</ybc::Subtitle>
                                <p>{"The current location is: "} {location}</p>
                            </ybc::Tile>
                        </ybc::Tile>
                    </ybc::Tile>
                </ybc::Tile>
                </ybc::Container>
            }}>
        </ybc::Hero>
        </>
    }
}

pub struct LocationService {}

impl LocationService {
    pub fn get_location(&self) -> web_sys::Location {
        window().unwrap().location()
    }
}

#[wasm_bindgen(inline_js = "export function snippetTest() { console.log('Hello from JS FFI!'); }")]
extern "C" {
    fn snippetTest();
}

fn main() {
    set_panic_hook();
    snippetTest();

    // Show off some feature flag enabling patterns.
    #[cfg(feature = "demo-abc")]
    {
        web_sys::console::log_1(&"feature `demo-abc` enabled".into());
    }
    #[cfg(feature = "demo-xyz")]
    {
        web_sys::console::log_1(&"feature `demo-xyz` enabled".into());
    }

    yew::Renderer::<App>::new().render();
}
