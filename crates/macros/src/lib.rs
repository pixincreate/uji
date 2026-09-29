mod derive;
mod export;
mod methods;
mod signature;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn register(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(export::register(attribute.into(), item.into()))
}

#[proc_macro_attribute]
pub fn function(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(export::function(attribute.into(), item.into()))
}

#[proc_macro_attribute]
pub fn methods(_: TokenStream, item: TokenStream) -> TokenStream {
    expand(methods::methods(item.into()))
}

#[proc_macro_derive(FromLua, attributes(lua))]
pub fn from_lua(item: TokenStream) -> TokenStream {
    expand(derive::from_lua(item.into()))
}

#[proc_macro_derive(IntoLua, attributes(lua))]
pub fn into_lua(item: TokenStream) -> TokenStream {
    expand(derive::into_lua(item.into()))
}

fn expand(result: syn::Result<proc_macro2::TokenStream>) -> TokenStream {
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}
