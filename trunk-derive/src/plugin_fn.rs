use proc_macro2::{Ident, TokenStream};
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::FnArg;
use syn::Token;
use syn::{Abi, Generics, ReturnType, Variadic, Visibility};

pub struct PluginFn {
    generics: Generics,
    inputs: Punctuated<FnArg, Token![,]>,
    output: ReturnType,
    code: Box<syn::Block>,
}

impl PluginFn {
    pub fn into_plugin_main(self) -> TokenStream {
        let PluginFn {
            generics,
            inputs,
            output,
            code,
        } = self;

        let generic_params = generics.params;
        let where_clause = generics.where_clause;

        quote::quote! {
            #[no_mangle]
            pub fn main(ptr: u32, len: u32) -> (u32, u32) {
                fn plugin_main<#generic_params>(#inputs) #output #where_clause #code

                let args = unsafe {
                    let slice = ::trunk_plugin::export::core::slice::from_raw_parts(
                        ptr as *const u8,
                        len as usize
                    );
                    ::trunk_plugin::export::serde_cbor::from_slice::<::trunk_plugin::export::Args>(slice)
                        .unwrap_or_else(|_| ::trunk_plugin::export::core::panic!(
                            "Failed to deserialize the plugin arguments!\n\
                            Trunk passed the ptr={} and the len={}\n\
                            The resulting slice could not be deserialized into an Args instance by cbor!\n\
                            This is a hard bug! Please open an issue on GitHub.",
                            ptr, len
                        ))
                };

                let plugin_ret = plugin_main(args);
                let output = ::trunk_plugin::export::Output::from(plugin_ret);

                let buf = ::trunk_plugin::export::serde_cbor::to_vec(&output)
                    .expect("Serializing Output to a Vec will never fail");

                (
                    buf.as_ptr() as u32,
                    buf.len() as u32
                )
            }
        }
    }

    fn check_fn_vis(vis: Visibility) -> syn::Result<()> {
        match vis {
            Visibility::Public(_) => Ok(()),
            _ => Err(syn::Error::new(vis.span(), "The plugins main function must be public")),
        }
    }

    fn check_fn_ident(ident: Ident) -> syn::Result<()> {
        if ident == "main" {
            Ok(())
        } else {
            Err(syn::Error::new(ident.span(), "The plugins entry point must be the main function"))
        }
    }

    fn check_fn_abi(abi: Option<Abi>) -> syn::Result<()> {
        match abi {
            Some(abi) if abi.name.is_none() => Ok(()),
            Some(abi) if matches!(abi.name, Some(ref name) if name.value() == "C") => Ok(()),
            None => Ok(()),
            _ => Err(syn::Error::new(
                abi.span(),
                r#"The plugins main function may only use no abi, the `extern` abi, or the `extern "C"` abi"#,
            )),
        }
    }

    fn check_fn_constness(constness: Option<Token![const]>) -> syn::Result<()> {
        match constness {
            Some(_) => Err(syn::Error::new(constness.span(), "The plugins main function cannot be const")),
            None => Ok(()),
        }
    }

    fn check_fn_asyncness(asyncness: Option<Token![async]>) -> syn::Result<()> {
        // Maybe allow asyncness and automatically add an runtime attribute
        match asyncness {
            Some(_) => Err(syn::Error::new(asyncness.span(), "The plugins main function cannot be async")),
            None => Ok(()),
        }
    }

    fn check_fn_unsafety(unsafety: Option<Token![unsafe]>) -> syn::Result<()> {
        match unsafety {
            Some(_) => Err(syn::Error::new(unsafety.span(), "The plugins main function may not be unsafe")),
            None => Ok(()),
        }
    }

    fn check_fn_variadic(variadic: Option<Variadic>) -> syn::Result<()> {
        match variadic {
            Some(_) => Err(syn::Error::new(
                variadic.span(),
                "The plugins main function may not contain any variadic arguments",
            )),
            None => Ok(()),
        }
    }

    fn check_fn_inputs(inputs: Punctuated<FnArg, Token![,]>) -> syn::Result<Punctuated<FnArg, Token![,]>> {
        if inputs.len() == 1 {
            Ok(inputs)
        } else {
            return Err(syn::Error::new(inputs.span(), "The plugins main function may only take one argument"));
        }
    }
}

impl Parse for PluginFn {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let func = syn::ItemFn::parse(input)?;

        Self::check_fn_ident(func.sig.ident)?;
        Self::check_fn_vis(func.vis)?;
        Self::check_fn_abi(func.sig.abi)?;
        Self::check_fn_constness(func.sig.constness)?;
        Self::check_fn_asyncness(func.sig.asyncness)?;
        Self::check_fn_unsafety(func.sig.unsafety)?;
        Self::check_fn_variadic(func.sig.variadic)?;
        let inputs = Self::check_fn_inputs(func.sig.inputs)?;

        Ok(Self {
            generics: func.sig.generics,
            inputs,
            output: func.sig.output,
            code: func.block,
        })
    }
}
