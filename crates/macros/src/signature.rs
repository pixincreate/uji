use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    FnArg, GenericArgument, Ident, PatType, PathArguments, ReturnType, Signature, Token, Type,
    parse_quote,
};

const RAISE: &str = "raise";

pub(crate) fn key(ident: &Ident) -> String {
    ident.unraw().to_string()
}

pub(crate) struct Placement {
    pub(crate) module: Option<Ident>,
    pub(crate) raise: bool,
}

impl Parse for Placement {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let words = Punctuated::<Ident, Token![,]>::parse_terminated(input)?;
        let (raises, modules): (Vec<Ident>, Vec<Ident>) =
            words.into_iter().partition(|word| word == RAISE);
        let mut modules = modules.into_iter();
        let module = modules.next();
        if let Some(extra) = modules.next() {
            return Err(syn::Error::new_spanned(extra, "a function has one module"));
        }
        Ok(Self {
            module,
            raise: !raises.is_empty(),
        })
    }
}

#[derive(Clone, Copy)]
enum Success {
    Unit,
    Value,
}

#[derive(Clone, Copy)]
pub(crate) struct Returns {
    wrapped: bool,
    result: Option<Success>,
    raise: bool,
}

impl Returns {
    pub(crate) fn of(output: &ReturnType, raise: bool) -> Self {
        let ReturnType::Type(_, ty) = output else {
            return Self {
                wrapped: false,
                result: None,
                raise,
            };
        };
        let wrapped = is_lua_result(ty);
        let inner = if wrapped {
            first_generic(ty, "Result")
        } else {
            Some(ty.as_ref().clone())
        };
        let result = inner
            .and_then(|inner| first_generic(&inner, "Result"))
            .map(|ok| match ok {
                Type::Tuple(tuple) if tuple.elems.is_empty() => Success::Unit,
                _ => Success::Value,
            });
        Self {
            wrapped,
            result,
            raise,
        }
    }

    pub(crate) fn settles(self) -> bool {
        self.result.is_some() && !self.raise
    }

    pub(crate) fn finish(self, call: &TokenStream, lua: &TokenStream) -> TokenStream {
        let Some(success) = self.result else {
            return if self.wrapped {
                call.clone()
            } else {
                quote!(mlua::Result::Ok(#call))
            };
        };
        let result = if self.wrapped {
            quote!(#call?)
        } else {
            call.clone()
        };
        match (self.raise, success) {
            (true, _) => quote!(#result.map_err(mlua::Error::external)),
            (false, Success::Unit) => quote!(crate::io::settle(#lua, #result.map(|()| true))),
            (false, Success::Value) => quote!(crate::io::settle(#lua, #result)),
        }
    }
}

enum Passing {
    Owned,
    Borrowed,
    Lossy,
    AsDeref,
}

enum Parameter {
    Lua,
    State {
        mutable: bool,
    },
    Value {
        name: Ident,
        ty: Box<Type>,
        passing: Passing,
    },
}

impl Parameter {
    fn of(index: usize, typed: &PatType) -> syn::Result<Self> {
        let ty = typed.ty.as_ref();
        if is_named(ty, "Lua") {
            return Ok(Self::Lua);
        }
        if is_named(ty, "State") {
            if index > 0 {
                return Err(syn::Error::new_spanned(
                    typed,
                    "the state is the first parameter",
                ));
            }
            return Ok(Self::State {
                mutable: matches!(ty, Type::Reference(reference) if reference.mutability.is_some()),
            });
        }
        let (ty, passing) = owned(ty);
        Ok(Self::Value {
            name: format_ident!("arg{index}"),
            ty: Box::new(ty),
            passing,
        })
    }
}

pub(crate) struct Arguments {
    parameters: Vec<Parameter>,
}

impl Arguments {
    pub(crate) fn of(sig: &Signature) -> syn::Result<Self> {
        let parameters = sig
            .inputs
            .iter()
            .filter_map(|input| match input {
                FnArg::Typed(typed) => Some(typed),
                FnArg::Receiver(_) => None,
            })
            .enumerate()
            .map(|(index, typed)| Parameter::of(index, typed))
            .collect::<syn::Result<_>>()?;
        Ok(Self { parameters })
    }

    fn takes_lua(&self) -> bool {
        self.parameters
            .iter()
            .any(|parameter| !matches!(parameter, Parameter::Value { .. }))
    }

    pub(crate) fn uses_state(&self) -> bool {
        self.parameters
            .iter()
            .any(|parameter| matches!(parameter, Parameter::State { .. }))
    }

    pub(crate) fn closure_lua(&self, returns: Returns) -> TokenStream {
        if self.takes_lua() || returns.settles() {
            quote!(lua)
        } else {
            quote!(_)
        }
    }

    pub(crate) fn binding(&self) -> TokenStream {
        let (names, types): (Vec<&Ident>, Vec<&Type>) = self
            .parameters
            .iter()
            .filter_map(|parameter| match parameter {
                Parameter::Value { name, ty, .. } => Some((name, ty.as_ref())),
                Parameter::Lua | Parameter::State { .. } => None,
            })
            .unzip();
        if let ([name], [ty]) = (names.as_slice(), types.as_slice()) {
            quote!(#name: #ty)
        } else {
            quote!((#(#names),*): (#(#types),*))
        }
    }

    pub(crate) fn call(&self, target: &TokenStream, asynchronous: bool) -> TokenStream {
        let args = self.parameters.iter().map(|parameter| match parameter {
            Parameter::Lua => quote!(lua),
            Parameter::State { mutable: false } => quote!(&*crate::kernel::State::of(lua)?),
            Parameter::State { mutable: true } => quote!(&mut *crate::kernel::State::of_mut(lua)?),
            Parameter::Value { name, passing, .. } => match passing {
                Passing::Owned => quote!(#name),
                Passing::Borrowed => quote!(&#name),
                Passing::Lossy => quote!(&String::from_utf8_lossy(&#name.as_bytes())),
                Passing::AsDeref => quote!(#name.as_deref()),
            },
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

fn owned(ty: &Type) -> (Type, Passing) {
    if let Type::Reference(reference) = ty {
        return match &*reference.elem {
            Type::Path(path) if path.path.is_ident("str") => {
                (parse_quote!(mlua::LuaString), Passing::Lossy)
            }
            Type::Slice(slice) if is_byte(&slice.elem) => {
                (parse_quote!(mlua::BString), Passing::Borrowed)
            }
            Type::Slice(slice) => {
                let item = &slice.elem;
                (parse_quote!(Vec<#item>), Passing::Borrowed)
            }
            elem => (elem.clone(), Passing::Borrowed),
        };
    }
    let optional_str = first_generic(ty, "Option").is_some_and(
        |inner| matches!(inner, Type::Reference(reference) if is_named(&reference.elem, "str")),
    );
    if optional_str {
        return (parse_quote!(Option<String>), Passing::AsDeref);
    }
    (ty.clone(), Passing::Owned)
}

fn is_byte(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.is_ident("u8"))
}

fn is_lua_result(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let segments: Vec<&Ident> = path.path.segments.iter().map(|part| &part.ident).collect();
    matches!(segments.as_slice(), [module, result] if *module == "mlua" && *result == "Result")
}

fn first_generic(ty: &Type, name: &str) -> Option<Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != name {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty.clone()),
        _ => None,
    })
}

fn is_named(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Reference(reference) => is_named(&reference.elem, name),
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name),
        _ => false,
    }
}
