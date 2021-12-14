#![recursion_limit = "1024"]

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use console_error_panic_hook::set_once as set_panic_hook;
use web_sys::{window, Worker};

fn worker_new(file_name: &str) -> Worker {
    let origin = window().unwrap().location().origin().unwrap();
    let url = format!("{}/{}", origin, file_name);

    Worker::new(&url).expect("failed to spawn worker")
}

fn main() {
    set_panic_hook();
    let document = window().and_then(|win| win.document()).expect("Could not access document");
    let body = document.body().expect("Could not access document.body");
    let text_node = document.create_text_node("Hello, world from Rust!");
    body.append_child(text_node.as_ref()).expect("Failed to append text");

    let worker = worker_new("worker.js");
}
