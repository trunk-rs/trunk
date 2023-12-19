use js_sys::Array;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{MessageEvent, Worker, WorkerOptions, WorkerType};

fn worker_new(url: &str) -> Worker {
    let mut options = WorkerOptions::new();
    options.type_(WorkerType::Module);
    Worker::new_with_options(&url, &options).expect("failed to spawn worker")
}

fn main() {
    console_error_panic_hook::set_once();

    let worker = worker_new("./worker_loader.js");
    let worker_clone = worker.clone();

    // NOTE: We must wait for the worker to report that it's ready to receive
    //       messages. Any message we send beforehand will be discarded / ignored.
    //       This is different from js-based workers, which can send messages
    //       before the worker is initialized.
    //       REASON: This is because javascript only starts processing MessageEvents
    //       once the worker's script first yields to the javascript event loop.
    //       For js workers this means that you can register the event listener
    //       as first thing in the worker and will receive all previously sent
    //       message events. However, loading wasm is an asynchronous operation
    //       which yields to the js event loop before the wasm is loaded and had
    //       a change to register the event listener. At that point js processes
    //       the message events, sees that there isn't any listener registered,
    //       and drops them.

    let onmessage = Closure::wrap(Box::new(move |msg: MessageEvent| {
        let worker_clone = worker_clone.clone();
        let data = Array::from(&msg.data());

        if data.length() == 0 {
            let msg = Array::new();
            msg.push(&2.into());
            msg.push(&5.into());
            worker_clone
                .post_message(&msg.into())
                .expect("sending message to succeed");
        } else {
            let a = data
                .get(0)
                .as_f64()
                .expect("first array value to be a number") as u32;
            let b = data
                .get(1)
                .as_f64()
                .expect("second array value to be a number") as u32;
            let result = data
                .get(2)
                .as_f64()
                .expect("third array value to be a number") as u32;

            web_sys::console::log_1(&format!("{a} x {b} = {result}").into());
        }
    }) as Box<dyn Fn(MessageEvent)>);
    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}
