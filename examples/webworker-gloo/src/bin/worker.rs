use gloo_worker::PublicWorker;
use webworker_gloo::Multiplier;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

fn main() {
    console_error_panic_hook::set_once();

    Multiplier::register();
}
