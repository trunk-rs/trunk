use proc_macro::TokenStream;

mod asset;

pub fn asset(input: TokenStream) -> TokenStream {
    asset::parse(input)
}
