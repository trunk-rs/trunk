#![recursion_limit = "1024"]

use console_error_panic_hook::set_once as set_panic_hook;
use wasm_bindgen::prelude::*;
use ybc::TileCtx::{Ancestor, Child, Parent};
use yew::prelude::*;
use yew::services::ConsoleService;

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
        html! {
            <>
            <ybc::Navbar
                classes=classes!("is-success")
                padded=true
                navbrand=html!{
                    <ybc::NavbarItem>
                        <ybc::Title classes=classes!("has-text-white") size=ybc::HeaderSize::Is4>{"Trunk | Yew | YBC"}</ybc::Title>
                    </ybc::NavbarItem>
                }
                navstart=html!{}
                navend=html!{
                    <>
                    <ybc::NavbarItem>
                        <ybc::ButtonAnchor classes=classes!("is-black", "is-outlined") rel=String::from("noopener noreferrer") target=String::from("_blank") href="https://github.com/thedodd/trunk">
                            {"Trunk"}
                        </ybc::ButtonAnchor>
                    </ybc::NavbarItem>
                    <ybc::NavbarItem>
                        <ybc::ButtonAnchor classes=classes!("is-black", "is-outlined") rel=String::from("noopener noreferrer") target=String::from("_blank") href="https://yew.rs">
                            {"Yew"}
                        </ybc::ButtonAnchor>
                    </ybc::NavbarItem>
                    <ybc::NavbarItem>
                        <ybc::ButtonAnchor classes=classes!("is-black", "is-outlined") rel=String::from("noopener noreferrer") target=String::from("_blank") href="https://github.com/thedodd/ybc">
                            {"YBC"}
                        </ybc::ButtonAnchor>
                    </ybc::NavbarItem>
                    </>
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
                                    <ybc::Subtitle size=ybc::HeaderSize::Is3 classes=classes!("has-text-white")>{"Trunk"}</ybc::Subtitle>
                                    <p>{"Trunk is a WASM web application bundler for Rust."}</p>
                                </ybc::Tile>
                            </ybc::Tile>
                            <ybc::Tile ctx=Parent>
                                <ybc::Tile ctx=Child classes=classes!("notification", "is-success")>
                                    <ybc::Icon size=ybc::Size::Large classes=classes!("is-pulled-right")><img src="yew.svg"/></ybc::Icon>
                                    <ybc::Subtitle size=ybc::HeaderSize::Is3 classes=classes!("has-text-white")>
                                        {"Yew"}
                                    </ybc::Subtitle>
                                    <p>{"Yew is a modern Rust framework for creating multi-threaded front-end web apps with WebAssembly."}</p>
                                </ybc::Tile>
                            </ybc::Tile>
                            <ybc::Tile ctx=Parent>
                                <ybc::Tile ctx=Child classes=classes!("notification", "is-success")>
                                    <ybc::Subtitle size=ybc::HeaderSize::Is3 classes=classes!("has-text-white")>{"YBC"}</ybc::Subtitle>
                                    <p>{"A Yew component library based on the Bulma CSS framework."}</p>
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
