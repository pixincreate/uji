use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::{FnArg, GenericArgument, Ident, PathArguments, ReturnType, Signature, Type, parse_quote};

pub(crate) fn key(ident: &Ident) -> String {
    ident.unraw().to_string()
}

#[derive(Clone, Copy)]
pub(crate) enum Returns {
    Value,
    Raises,
    Settles,
    RaisesOrSettles,
}

impl Returns {
    pub(crate) fn of(output: &ReturnType) -> Self {
        let ReturnType::Type(_, ty) = output else {
            return Self::Value;
        };
        match generics(ty, "Result").as_deref() {
            None => Self::Value,
            Some([_, _]) => Self::Settles,
            Some([inner]) if generics(inner, "Result").is_some_and(|args| args.len() == 2) => {
                Self::RaisesOrSettles
            }
            Some(_) => Self::Raises,
        }
    }

    fn settles(self) -> bool {
        matches!(self, Self::Settles | Self::RaisesOrSettles)
    }

    pub(crate) fn finish(self, call: &TokenStream, lua: &TokenStream) -> TokenStream {
        match self {
            Self::Value => quote!(Ok(#call)),
            Self::Raises => quote!(#call),
            Self::Settles => quote!(crate::io::settle(#lua, #call)),
            Self::RaisesOrSettles => quote!(crate::io::settle(#lua, #call?)),
        }
    }
}

enum Passing {
    Owned,
    Borrowed,
    AsRef,
    AsDeref,
}

enum Parameter {
    Lua,
    Value(Box<Type>, Passing),
}

pub(crate) struct Arguments {
    parameters: Vec<Parameter>,
}

impl Arguments {
    pub(crate) fn of(sig: &Signature) -> Self {
        let parameters = sig
            .inputs
            .iter()
            .filter_map(|input| match input {
                FnArg::Typed(typed) if is_lua(&typed.ty) => Some(Parameter::Lua),
                FnArg::Typed(typed) => {
                    let (ty, passing) = owned(&typed.ty);
                    Some(Parameter::Value(Box::new(ty), passing))
                }
                FnArg::Receiver(_) => None,
            })
            .collect();
        Self { parameters }
    }

    fn values(&self) -> impl Iterator<Item = (&Type, &Passing)> {
        self.parameters
            .iter()
            .filter_map(|parameter| match parameter {
                Parameter::Value(ty, passing) => Some((ty.as_ref(), passing)),
                Parameter::Lua => None,
            })
    }

    fn takes_lua(&self) -> bool {
        self.parameters
            .iter()
            .any(|parameter| matches!(parameter, Parameter::Lua))
    }

    pub(crate) fn borrows(&self) -> bool {
        self.values()
            .any(|(_, passing)| !matches!(passing, Passing::Owned))
    }

    pub(crate) fn closure_lua(&self, returns: Returns) -> TokenStream {
        if self.takes_lua() || returns.settles() {
            quote!(lua)
        } else {
            quote!(_)
        }
    }

    pub(crate) fn binding(&self) -> TokenStream {
        let (names, types): (Vec<Ident>, Vec<&Type>) = self
            .values()
            .enumerate()
            .map(|(index, (ty, _))| (argument(index), ty))
            .unzip();
        if let ([name], [ty]) = (names.as_slice(), types.as_slice()) {
            quote!(#name: #ty)
        } else {
            quote!((#(#names),*): (#(#types),*))
        }
    }

    pub(crate) fn call(&self, target: &TokenStream, asynchronous: bool) -> TokenStream {
        let mut index = 0;
        let args = self.parameters.iter().map(|parameter| {
            let Parameter::Value(_, passing) = parameter else {
                return quote!(lua);
            };
            let name = argument(index);
            index += 1;
            match passing {
                Passing::Owned => quote!(#name),
                Passing::Borrowed => quote!(&#name),
                Passing::AsRef => quote!(#name.as_ref()),
                Passing::AsDeref => quote!(#name.as_deref()),
            }
        });
        let call = quote!(#target(#(#args),*));
        if asynchronous {
            quote!(#call.await)
        } else {
            call
        }
    }

    pub(crate) fn future(&self, returns: Returns, call: &TokenStream) -> TokenStream {
        if returns.settles() && self.takes_lua() {
            let body = returns.finish(call, &quote!(&owner));
            quote!({
                let owner = lua.clone();
                async move { #body }
            })
        } else {
            let body = returns.finish(call, &quote!(&lua));
            quote!(async move { #body })
        }
    }
}

fn argument(index: usize) -> Ident {
    format_ident!("arg{index}")
}

fn owned(ty: &Type) -> (Type, Passing) {
    if let Type::Reference(reference) = ty {
        return (target(&reference.elem).0, Passing::Borrowed);
    }
    if let Some(Type::Reference(reference)) =
        generics(ty, "Option").and_then(|args| args.into_iter().next())
    {
        let (inner, dereferenced) = target(&reference.elem);
        let passing = if dereferenced {
            Passing::AsDeref
        } else {
            Passing::AsRef
        };
        return (parse_quote!(Option<#inner>), passing);
    }
    (ty.clone(), Passing::Owned)
}

fn target(elem: &Type) -> (Type, bool) {
    match elem {
        Type::Path(path) if path.path.is_ident("str") => (parse_quote!(String), true),
        Type::Slice(slice) => {
            let item = &slice.elem;
            (parse_quote!(Vec<#item>), true)
        }
        _ => (elem.clone(), false),
    }
}

fn generics(ty: &Type, name: &str) -> Option<Vec<Type>> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != name {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Some(Vec::new());
    };
    Some(
        arguments
            .args
            .iter()
            .filter_map(|argument| match argument {
                GenericArgument::Type(ty) => Some(ty.clone()),
                _ => None,
            })
            .collect(),
    )
}

fn is_lua(ty: &Type) -> bool {
    match ty {
        Type::Reference(reference) => is_lua(&reference.elem),
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Lua"),
        _ => false,
    }
}
