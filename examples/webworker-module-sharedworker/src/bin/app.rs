use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{MessageEvent, SharedWorker, WorkerOptions, WorkerType};

fn worker_new(url: &str) -> SharedWorker {
    let mut options = WorkerOptions::new();
    options.type_(WorkerType::Module);
    options.name("example shared worker");
    SharedWorker::new_with_worker_options(url, &options).expect("failed to spawn worker")
}

fn main() {
    console_error_panic_hook::set_once();

    let worker = worker_new("./worker_loader.js");

    // FIX: When creating the first worker connection withn SharedWorker(...)
    //      the spec described connect event in worker processing model
    //      does not reach the worker's onconnect handler.
    //      > 13. If is shared is true, then queue a global task on the DOM manipulation
    //      > task source given worker global scope to fire an event named connect at
    //      > worker global scope, using MessageEvent, with the data attribute
    //      > initialized to the empty string, the ports attribute initialized to
    //      > a new frozen array containing inside port, and the source attribute
    //      > initialized to inside port.
    //      (https://html.spec.whatwg.org/multipage/workers.html#worker-processing-model)
    //      This means that only subsequent worker connections reach that handler
    //      because SharedWorkerGlobalScope is already initialized
    //      (https://html.spec.whatwg.org/multipage/workers.html#shared-workers-and-the-sharedworker-interface)

    let onmessage = Closure::wrap(Box::new(move |msg: MessageEvent| {
        web_sys::console::log_2(
            &"webpage received message from worker".into(),
            &JsValue::from(msg),
        );
    }) as Box<dyn FnMut(MessageEvent)>);

    let port = worker.port();
    port.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    port.post_message(&"Hello from webpage".into())
        .expect("Successfully queing a message to be handled by worker");
}
