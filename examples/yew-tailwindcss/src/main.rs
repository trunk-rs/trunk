#![recursion_limit = "1024"]

use console_error_panic_hook::set_once as set_panic_hook;
use yew::prelude::*;

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
        let link_classes =
            "block px-4 py-2 hover:bg-black hover:text-white rounded border-black border";
        let links = [
            ("Trunk", "https://github.com/trunk-rs/trunk"),
            ("Yew", "https://yew.rs/"),
            ("Tailwind", "https://tailwindcss.com"),
        ];
        html! {
            <div class="flex flex-col h-screen">
                <nav class="bg-green-400 h-16 px-8 py-2">
                    <div class="container flex mx-auto gap-6 items-center h-full">
                        <h1 class="font-bold text-2xl text-white">{"Trunk | Yew | Tailwind"}</h1>
                        <div class="flex-1"></div>
                        {for links.iter().map(|(label, href)| html! {
                            <a class=link_classes href={*href}>{label}</a>
                        })}
                    </div>
                </nav>
                <div class="bg-gray-50 flex-1 flex py-16 px-8 items-center gap-6 justify-center">
                    {view_card("Trunk", None, html! {
                        <p>{"Trunk is a WASM web application bundler for Rust."}</p>
                    })}
                    {view_card("Yew", Some("yew.svg"), html! {
                        <p>{"Yew is a modern Rust framework for creating multi-threaded front-end web apps with WebAssembly."}</p>
                    })}
                    {view_card("Tailwind CSS", None, html! {
                        <p>{"Tailwind CSS is a library for styling markup using a comprehensive set of utility classes, no CSS required."}</p>
                    })}
                </div>
            </div>
        }
    }
}

fn view_card(title: &'static str, img_url: Option<&'static str>, content: Html) -> Html {
    html! {
        <div class="w-96 h-48 rounded bg-green-400 text-white p-6">
            {for img_url.map(|url| html! {
                <img class="float-right w-12" src={url} alt="Logo" />
            })}
            <h1 class="text-4xl mb-8">{title}</h1>
            {content}
        </div>
    }
}

fn main() {
    set_panic_hook();

    yew::start_app::<App>();
}
