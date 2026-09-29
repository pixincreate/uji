use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ItemFn, Visibility};

use crate::signature::{Arguments, Returns, key};

pub(crate) fn register(attribute: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let parent: Option<Ident> = syn::parse2(attribute)?;
    let function: ItemFn = syn::parse2(item)?;
    let call = Arguments::of(&function.sig).call(&quote!(build), false);
    let value = if matches!(Returns::of(&function.sig.output), Returns::Value) {
        call
    } else {
        quote!(#call?)
    };
    Ok(export(&function, parent.as_ref(), &value))
}

pub(crate) fn function(attribute: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let parent: Option<Ident> = syn::parse2(attribute)?;
    let function: ItemFn = syn::parse2(item)?;
    let sig = &function.sig;
    let args = Arguments::of(sig);
    let returns = Returns::of(&sig.output);
    let (lua, binding) = (args.closure_lua(returns), args.binding());
    let value = if sig.asyncness.is_none() {
        let body = returns.finish(&args.call(&quote!(build), false), &quote!(lua));
        quote!(lua.create_function(|#lua, #binding| #body)?)
    } else if matches!(returns, Returns::Raises) && !args.borrows() {
        let future = args.call(&quote!(build), false);
        quote!(lua.create_async_function(|#lua, #binding| #future)?)
    } else {
        let body = args.future(returns, &args.call(&quote!(build), true));
        quote!(lua.create_async_function(|#lua, #binding| #body)?)
    };
    Ok(export(&function, parent.as_ref(), &value))
}

fn export(function: &ItemFn, parent: Option<&Ident>, value: &TokenStream) -> TokenStream {
    let ident = &function.sig.ident;
    let name = key(ident);
    let parents: Vec<String> = parent.map(key).into_iter().collect();
    let mut inner = function.clone();
    inner.sig.ident = format_ident!("build");
    inner.vis = Visibility::Inherited;
    quote! {
        pub(crate) fn #ident(lua: &mlua::Lua, uji: &mlua::Table) -> mlua::Result<()> {
            #inner
            crate::vm::table(lua, uji, &[#(#parents),*])?.set(#name, #value)
        }

        #[allow(unsafe_code)]
        const _: () = {
            #[linkme::distributed_slice(crate::REGISTERED)]
            static REGISTRATION: crate::Register = #ident;
        };
    }
}
