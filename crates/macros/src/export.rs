use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, Signature, Visibility};

use crate::signature::{Arguments, Placement, Returns, key};

pub(crate) fn constant(attribute: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let placement: Placement = syn::parse2(attribute)?;
    let function: ItemFn = syn::parse2(item)?;
    let returns = Returns::of(&function.sig.output, placement.raise);
    let value = returns.finish(
        &Arguments::of(&function.sig)?.call(&quote!(build), false),
        &quote!(lua),
    );
    Ok(export(&function, &placement, &value))
}

pub(crate) fn function(attribute: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let placement: Placement = syn::parse2(attribute)?;
    let function: ItemFn = syn::parse2(item)?;
    let value = callable(&function.sig, &quote!(build), placement.raise)?;
    Ok(export(&function, &placement, &value))
}

pub(crate) fn callable(
    sig: &Signature,
    target: &TokenStream,
    raise: bool,
) -> syn::Result<TokenStream> {
    let args = Arguments::of(sig)?;
    if sig.asyncness.is_some() && args.uses_state() {
        return Err(syn::Error::new_spanned(
            sig,
            "an async function cannot hold the state across a wait",
        ));
    }
    let returns = Returns::of(&sig.output, raise);
    let (lua, binding) = (args.closure_lua(returns), args.binding());
    Ok(if sig.asyncness.is_none() {
        let body = returns.finish(&args.call(target, false), &quote!(lua));
        quote!(lua.create_function(|#lua, #binding| #body))
    } else {
        let body = args.future(returns, &args.call(target, true));
        quote!(lua.create_async_function(|#lua, #binding| #body))
    })
}

fn export(function: &ItemFn, placement: &Placement, value: &TokenStream) -> TokenStream {
    let name = key(&function.sig.ident);
    let (module, field) = match &placement.module {
        Some(module) => (key(module), quote!(Some(#name))),
        None => (name, quote!(None)),
    };
    let mut inner = function.clone();
    inner.sig.ident = format_ident!("build");
    inner.vis = Visibility::Inherited;
    quote! {
        #[allow(unsafe_code)]
        const _: () = {
            fn export(lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
                #inner
                mlua::IntoLua::into_lua(#value?, lua)
            }

            #[linkme::distributed_slice(crate::vm::EXPORTS)]
            static EXPORT: crate::vm::Export = crate::vm::Export {
                module: #module,
                name: #field,
                build: export,
            };
        };
    }
}
