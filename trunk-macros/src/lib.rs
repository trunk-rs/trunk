use proc_macro::TokenStream;

mod asset;

#[proc_macro]
/// Takes a given asset and generates a `[u8; _]` out of it
pub fn asset(input: TokenStream) -> TokenStream {
    asset::parse(input)
}
