use js_sys::Array;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{window, Blob, BlobPropertyBag, MessageEvent, Url, Worker};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

fn worker_new(name: &str) -> Worker {
    let origin = window()
        .expect("window to be available")
        .location()
        .origin()
        .expect("origin to be available");

    let script = Array::new();
    script.push(&format!(r#"importScripts("{origin}/{name}.js");wasm_bindgen("{origin}/{name}_bg.wasm");"#).into());

    let blob = Blob::new_with_str_sequence_and_options(&script, BlobPropertyBag::new().type_("text/javascript")).expect("blob creation succeeds");

    let url = Url::create_object_url_with_blob(&blob).expect("url creation succeeds");

    Worker::new(&url).expect("failed to spawn worker")
}

fn main() {
    console_error_panic_hook::set_once();

    let worker = worker_new("worker");
    let worker_clone = worker.clone();

    let onmessage = Closure::wrap(Box::new(move |msg: MessageEvent| {
        let worker_clone = worker_clone.clone();
        let data = Array::from(&msg.data());

        if data.length() == 0 {
            let msg = Array::new();
            msg.push(&2.into());
            msg.push(&5.into());
            worker_clone.post_message(&msg.into()).expect("sending message to succeed");
        } else {
            let a = data.get(0).as_f64().expect("first array value to be a number") as u32;
            let b = data.get(1).as_f64().expect("second array value to be a number") as u32;
            let result = data.get(2).as_f64().expect("third array value to be a number") as u32;

            web_sys::console::log_1(&format!("{a} x {b} = {result}").into());
        }
    }) as Box<dyn Fn(MessageEvent)>);
    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}
