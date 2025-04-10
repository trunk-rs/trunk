#![recursion_limit = "1024"]

use console_error_panic_hook::set_once as set_panic_hook;
use yew::prelude::*;

#[function_component]
fn App() -> Html {
    let link_classes = "block px-4 py-2 text-white hover:bg-white hover:text-black rounded border-white border transition-color duration-150";
    let links = [
        ("Trunk", "https://github.com/trunk-rs/trunk"),
        ("Yew", "https://yew.rs/"),
        ("Tailwind v4", "https://tailwindcss.com"),
    ];
    html! {
        <div class={"flex flex-col h-screen"}>
            <nav class={"bg-blue-400 h-16 px-8 py-2"}>
                <div class={"container flex mx-auto gap-6 items-center h-full"}>
                    <h1 class={"font-bold text-2xl text-white"}>{"Trunk | Yew | Tailwind V4"}</h1>
                    <div class={"flex-1"}></div>
                    {for links.iter().map(|(label, href)| html! {
                        <a class={link_classes} href={*href}>{label}</a>
                    })}
                </div>
            </nav>
            <div class={"bg-gray-50 flex-1 flex py-16 px-8 items-center gap-6 justify-center"}>
                <ViewCard title={"Trunk"} img_url={Some("trunk.png".to_string())}>
                    <p>{"Trunk is a WASM web application bundler for Rust."}</p>
                </ViewCard>
                <ViewCard title={"Yew"} img_url={Some("yew.svg".to_string())}>
                    <p>{"Yew is a modern Rust framework for creating multi-threaded front-end web apps with WebAssembly."}</p>
                </ViewCard>
                <ViewCard title={"Tailwind CSS V4"} img_url={Some("tailwindcss.png".to_string())}>
                    <p>{"Tailwind CSS is a library for styling markup using a comprehensive set of utility classes, no CSS required."}</p>
                </ViewCard>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct ViewCardProps {
    pub title: String,
    pub img_url: Option<String>,
    pub children: Html,
}

#[function_component]
fn ViewCard(props: &ViewCardProps) -> Html {
    let ViewCardProps {
        title,
        img_url,
        children,
    } = props;
    html! {
        <div class={"w-96 h-48 rounded bg-blue-400 text-white p-6"}>
            {for img_url.clone().map(|url| html! {
                <img class={"float-right w-12"} src={url} alt="Logo" />
            })}
            <h1 class={"text-4xl mb-8"}>{title}</h1>
            {children.clone()}
        </div>
    }
}

fn main() {
    set_panic_hook();

    yew::Renderer::<App>::new().render();
}
