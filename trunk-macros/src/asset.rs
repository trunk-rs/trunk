use proc_macro::TokenStream;
use std::{fs, io::ErrorKind};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Error, LitStr, Result,
};

struct AssetParser {
    file_name_lit: LitStr,
}

impl Parse for AssetParser {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(AssetParser {
            file_name_lit: input.parse()?,
        })
    }
}

pub fn parse(input: TokenStream) -> TokenStream {
    let AssetParser { file_name_lit } = parse_macro_input!(input as AssetParser);

    let file_name = file_name_lit.value();

    let file_content = fs::read(&file_name);
    if let Err(error) = file_content {
        if error.kind() == ErrorKind::NotFound {
            return Error::new(file_name_lit.span(), format!("the file `{}` cannot be found", file_name))
                .to_compile_error()
                .into();
        }
        return Error::new(file_name_lit.span(), format!("error reading template file: {}", error.to_string()))
            .to_compile_error()
            .into();
    }
    let file_content = file_content.unwrap();
    let data_string = file_content.into_iter().map(|byte| byte.to_string()).collect::<Vec<_>>().join(", ");
    let byte_array = format!("[{}]", data_string);

    byte_array.parse().unwrap()
}
