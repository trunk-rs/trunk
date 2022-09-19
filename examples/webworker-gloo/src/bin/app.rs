use gloo_console::log;
use gloo_worker::Spawnable;
use webworker_gloo::Multiplier;

fn main() {
    console_error_panic_hook::set_once();

    let bridge = Multiplier::spawner()
        .callback(move |((a, b), result)| {
            log!(format!("{} * {} = {}", a, b, result));
        })
        .spawn("./worker.js");
    let bridge = Box::leak(Box::new(bridge));

    bridge.send((2, 5));
    bridge.send((3, 3));
    bridge.send((50, 5));
}
