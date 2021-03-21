use trunk_plugin::{trunk_plugin, Args, Output};

#[trunk_plugin]
pub fn main(args: Args) -> Output {
    let msg = format!("Hello from WASM\nYou passed the arguments: {:?}", args);
    Output { msg }
}
