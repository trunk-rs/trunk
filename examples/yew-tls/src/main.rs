#![recursion_limit = "1024"]

use console_error_panic_hook::set_once as set_panic_hook;
use wasm_bindgen::prelude::*;
use ybc::TileCtx::{Ancestor, Child, Parent};
use yew::prelude::*;
use yew::services::ConsoleService;
use yew::utils::window;

struct App;

impl Component for App {
    type Message = ();
    type Properties = ();

    fn create(_: Self::Properties, _: ComponentLink<Self>) -> Self {
        Self
    }

    fn update(&mut self, _: Self::Message) -> bool {
        false
    }

    fn change(&mut self, _: Self::Properties) -> bool {
        false
    }

    fn view(&self) -> Html {
        let location_service = LocationService {};
        let location = location_service.get_location().href().unwrap_or_default();
        html! {
            <>
            <ybc::Navbar
                classes=classes!("is-success")
                padded=true
                navbrand=html!{
                    <ybc::NavbarItem>
                        <ybc::Title classes=classes!("has-text-white") size=ybc::HeaderSize::Is4>{"Location Demo"}</ybc::Title>
                    </ybc::NavbarItem>
                }
            />

            <ybc::Hero
                classes=classes!("is-light")
                size=ybc::HeroSize::FullheightWithNavbar
                body=html!{
                    <ybc::Container classes=classes!("is-centered")>
                    <ybc::Tile ctx=Ancestor>
                        <ybc::Tile ctx=Parent size=ybc::TileSize::Twelve>
                            <ybc::Tile ctx=Parent>
                                <ybc::Tile ctx=Child classes=classes!("notification", "is-success")>
                                    <ybc::Subtitle size=ybc::HeaderSize::Is3 classes=classes!("has-text-white")>{"Location"}</ybc::Subtitle>
                                    <p>{"The current location is: "} {location}</p>
                                </ybc::Tile>
                            </ybc::Tile>
                        </ybc::Tile>
                    </ybc::Tile>
                    </ybc::Container>
                }>
            </ybc::Hero>
            </>
        }
    }
}

pub struct LocationService {}

impl LocationService {
    pub fn get_location(&self) -> web_sys::Location {
        window().location()
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
        ConsoleService::log("feature `demo-abc` enabled");
    }
    #[cfg(feature = "demo-xyz")]
    {
        ConsoleService::log("feature `demo-xyz` enabled");
    }

    yew::start_app::<App>();
}
