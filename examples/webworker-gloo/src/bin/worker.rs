use gloo_worker::Registrable;
use webworker_gloo::Multiplier;

fn main() {
    console_error_panic_hook::set_once();

    Multiplier::registrar().register();
}
