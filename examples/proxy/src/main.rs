#![recursion_limit = "1024"]

use console_error_panic_hook::set_once as set_panic_hook;
use web_sys::window;

fn start_app() {
    let document = window().and_then(|win| win.document()).expect("could not access document");
    let body = document.body().expect("could not access document.body");
    let text_node = document.create_text_node("Trunk Proxy Demo App");
    body.append_child(text_node.as_ref()).expect("failed to append text");
}

fn main() {
    set_panic_hook();
    start_app();
}
