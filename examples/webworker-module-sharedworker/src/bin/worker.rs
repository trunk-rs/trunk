use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{MessageEvent, MessagePort, SharedWorkerGlobalScope};

fn main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"worker starting".into());

    let scope = SharedWorkerGlobalScope::from(JsValue::from(js_sys::global()));

    let onconnect = Closure::wrap(Box::new(move |msg: MessageEvent| {
        web_sys::console::log_2(&"got connect".into(), &JsValue::from(&msg));

        let port = msg.ports().get(0);
        let port = port.dyn_ref::<MessagePort>().unwrap();

        let callback = Closure::wrap(Box::new(|msg: MessageEvent| {
            web_sys::console::log_2(
                &"Received message from website".into(),
                &JsValue::from(&msg),
            );
            let target = msg.target().expect("message event should have a target");
            let port = target
                .dyn_ref::<MessagePort>()
                .expect("message event target should be the port which sent the message");
            port.post_message(&"thank you for the message".into())
                .expect("worker should be able to send a message back");
        }) as Box<dyn FnMut(MessageEvent)>);

        port.add_event_listener_with_callback("message", callback.as_ref().unchecked_ref())
            .unwrap_throw();
        callback.forget();
        port.start(); // this is requred when not using port.onmessage both for worker and webpage
    }) as Box<dyn FnMut(MessageEvent)>);

    scope.set_onconnect(Some(onconnect.as_ref().unchecked_ref()));
    onconnect.forget();
}
