#![recursion_limit = "1024"]

use console_error_panic_hook::set_once as set_panic_hook;
use web_sys::window;

fn start_app() {
    let document = window()
        .and_then(|win| win.document())
        .expect("could not access document");
    let body = document.body().expect("could not access document.body");

    let button_text_node = document.create_element("span").unwrap();
    button_text_node
        .set_attribute("class", "pf-v6-c-button__text")
        .unwrap();
    button_text_node.set_text_content(Some("Primary"));

    let button_node = document.create_element("button").unwrap();
    button_node
        .set_attribute("class", "pf-v6-c-button pf-m-primary")
        .unwrap();
    button_node.set_attribute("type", "button").unwrap();
    button_node
        .append_child(button_text_node.as_ref())
        .expect("failed to append button text");

    body.append_child(button_node.as_ref())
        .expect("failed to append button");
}

fn main() {
    set_panic_hook();
    start_app();
}
