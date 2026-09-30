mod data;
mod export;
mod methods;
mod signature;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn function(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(export::function(attribute.into(), item.into()))
}

#[proc_macro_attribute]
pub fn constant(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(export::constant(attribute.into(), item.into()))
}

#[proc_macro_attribute]
pub fn methods(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(methods::methods(attribute.into(), item.into()))
}

#[proc_macro_attribute]
pub fn options(_: TokenStream, item: TokenStream) -> TokenStream {
    expand(data::options(item.into()))
}

#[proc_macro_attribute]
pub fn value(_: TokenStream, item: TokenStream) -> TokenStream {
    expand(data::value(item.into()))
}

fn expand(result: syn::Result<proc_macro2::TokenStream>) -> TokenStream {
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}
