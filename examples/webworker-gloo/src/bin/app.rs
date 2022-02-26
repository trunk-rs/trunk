use std::rc::Rc;

use gloo_worker::Bridged;
use webworker_gloo::Multiplier;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

fn main() {
    console_error_panic_hook::set_once();

    let bridge = Multiplier::bridge(Rc::new(Box::new(|((a, b), result)| {
        web_sys::console::log_1(&format!("{a} x {b} = {result}").into());
    })));
    let bridge = Box::leak(bridge);

    bridge.send((2, 5));
    bridge.send((3, 3));
    bridge.send((50, 5));
}
