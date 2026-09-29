use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, FnArg, Ident, ImplItem, ItemImpl, Signature};

use crate::signature::{Arguments, Returns, key};

pub(crate) fn methods(item: TokenStream) -> syn::Result<TokenStream> {
    let mut block: ItemImpl = syn::parse2(item)?;
    let ty = block.self_ty.clone();
    let (mut fields, mut methods) = (Vec::new(), Vec::new());
    for item in &mut block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let getter = take(&mut method.attrs, "get").is_some();
        if let Some(attribute) = take(&mut method.attrs, "iterate") {
            methods.push(iterator(&attribute.parse_args()?, &method.sig.ident));
        }
        if getter {
            fields.push(field_binding(&method.sig));
        } else {
            methods.push(method_binding(&method.sig));
        }
    }
    let fields = (!fields.is_empty()).then(|| {
        quote! {
            fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
                #(#fields)*
            }
        }
    });
    Ok(quote! {
        #block

        impl mlua::UserData for #ty {
            #fields

            fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
                #(#methods)*
            }
        }
    })
}

fn take(attrs: &mut Vec<Attribute>, name: &str) -> Option<Attribute> {
    let index = attrs.iter().position(|attr| attr.path().is_ident(name))?;
    Some(attrs.remove(index))
}

fn field_binding(sig: &Signature) -> TokenStream {
    let (name, method) = (key(&sig.ident), &sig.ident);
    let args = Arguments::of(sig);
    let returns = Returns::of(&sig.output);
    let lua = args.closure_lua(returns);
    let body = returns.finish(&args.call(&quote!(this.#method), false), &quote!(lua));
    quote!(fields.add_field_method_get(#name, |#lua, this| #body);)
}

fn method_binding(sig: &Signature) -> TokenStream {
    let (name, method) = (key(&sig.ident), &sig.ident);
    let args = Arguments::of(sig);
    let returns = Returns::of(&sig.output);
    let (lua, binding) = (args.closure_lua(returns), args.binding());
    let asynchronous = sig.asyncness.is_some();
    let receiver = sig.inputs.iter().find_map(|input| match input {
        FnArg::Receiver(receiver) => Some(receiver.mutability.is_some()),
        FnArg::Typed(_) => None,
    });
    let target = if receiver.is_some() {
        quote!(this.#method)
    } else {
        quote!(Self::#method)
    };
    let call = args.call(&target, asynchronous);
    let body = if asynchronous {
        args.future(returns, &call)
    } else {
        returns.finish(&call, &quote!(lua))
    };
    match (receiver, asynchronous) {
        (None, false) => quote!(methods.add_function(#name, |#lua, #binding| #body);),
        (None, true) => quote!(methods.add_async_function(#name, |#lua, #binding| #body);),
        (Some(false), false) => quote!(methods.add_method(#name, |#lua, this, #binding| #body);),
        (Some(true), false) => {
            quote!(methods.add_method_mut(#name, |#lua, this, #binding| #body);)
        }
        (Some(false), true) => {
            quote!(methods.add_async_method(#name, |#lua, this, #binding| #body);)
        }
        (Some(true), true) => {
            quote!(methods.add_async_method_mut(#name, |#lua, mut this, #binding| #body);)
        }
    }
}

fn iterator(name: &Ident, step: &Ident) -> TokenStream {
    let (name, step) = (key(name), key(step));
    quote! {
        methods.add_function(#name, |_, this: mlua::AnyUserData| {
            Ok((mlua::ObjectLike::get::<mlua::Function>(&this, #step)?, this))
        });
    }
}
