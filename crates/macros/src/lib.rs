mod ctypes;
mod lua;
mod native;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn native(attribute: TokenStream, item: TokenStream) -> TokenStream {
    expand(native::native(attribute.into(), item.into()))
}

#[proc_macro_derive(CStruct)]
pub fn c_struct(item: TokenStream) -> TokenStream {
    expand(native::c_struct(item.into()))
}

fn expand(result: syn::Result<proc_macro2::TokenStream>) -> TokenStream {
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}
