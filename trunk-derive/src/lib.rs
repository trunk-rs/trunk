use plugin_fn::PluginFn;
use proc_macro::TokenStream;

mod plugin_fn;

#[proc_macro_attribute]
pub fn trunk_plugin(_args: TokenStream, input: TokenStream) -> TokenStream {
    let plugin_fn = syn::parse_macro_input!(input as PluginFn);
    let plugin_main = plugin_fn.into_plugin_main();
    TokenStream::from(plugin_main)
}

#[proc_macro_attribute]
pub fn trunk_extern(_args: TokenStream, _input: TokenStream) -> TokenStream {
    todo!("Parse extern blocks and create safe wrappers (like wasm_bindgen)")
}
