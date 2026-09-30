use std::ffi::CString;

use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{
    Data, DeriveInput, Expr, FnArg, GenericArgument, Ident, ItemFn, Meta, Pat, PathArguments,
    ReturnType, Token, Type, Visibility,
};

use crate::ctypes::{c_field, c_type, key, snake, struct_name};
use crate::lua::{self, Output, Passing, Shape};

pub(crate) struct Argument {
    pub(crate) name: Ident,
    pub(crate) passing: Passing,
    c: Vec<String>,
    parameters: Vec<TokenStream>,
    convert: TokenStream,
}

struct Options {
    prefix: Option<Ident>,
    class: Option<Ident>,
    iterate: Option<Ident>,
    raise: bool,
}

fn named_value(value: Expr) -> syn::Result<Ident> {
    let Expr::Path(path) = value else {
        return Err(syn::Error::new_spanned(value, "this option takes a name"));
    };
    Ok(path.path.require_ident()?.clone())
}

impl Options {
    fn parse(attribute: TokenStream) -> syn::Result<Self> {
        let mut options = Self {
            prefix: None,
            class: None,
            iterate: None,
            raise: false,
        };
        let metas =
            syn::parse::Parser::parse2(Punctuated::<Meta, Token![,]>::parse_terminated, attribute)?;
        for meta in metas {
            match meta {
                Meta::Path(path) if path.is_ident("raise") => options.raise = true,
                Meta::Path(path) => options.prefix = Some(path.require_ident()?.clone()),
                Meta::NameValue(pair) if pair.path.is_ident("iterate") => {
                    options.iterate = Some(named_value(pair.value)?);
                }
                Meta::NameValue(pair) if pair.path.is_ident("class") => {
                    options.class = Some(named_value(pair.value)?);
                }
                other => return Err(syn::Error::new_spanned(other, "unknown native option")),
            }
        }
        Ok(options)
    }
}

fn named(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let PathArguments::AngleBracketed(arguments) = &path.path.segments.last()?.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn class(ty: &Type) -> Option<String> {
    let ty = match ty {
        Type::Reference(reference) => &reference.elem,
        other => other,
    };
    let ty = if named(ty).as_deref() == Some("Arc") {
        inner(ty)?
    } else {
        ty
    };
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| snake(&segment.ident))
}

fn slice_of_bytes(ty: &Type) -> bool {
    matches!(ty, Type::Slice(slice) if named(&slice.elem).as_deref() == Some("u8"))
}

fn out(name: &Ident, ty: &Type) -> syn::Result<Argument> {
    let missing = || syn::Error::new_spanned(ty, "an out needs a struct");
    let Some(Type::Path(path)) = inner(ty) else {
        return Err(missing());
    };
    let ident = &path.path.segments.last().ok_or_else(missing)?.ident;
    Ok(Argument {
        name: name.clone(),
        passing: Passing::Pointer,
        c: vec![format!("{} *{name}", struct_name(ident))],
        parameters: vec![quote!(#name: #ty)],
        convert: TokenStream::new(),
    })
}

fn optional(name: &Ident, ty: &Type) -> syn::Result<Argument> {
    let value = inner(ty).ok_or_else(|| syn::Error::new_spanned(ty, "an option needs a type"))?;
    let present = format_ident!("{name}_present");
    Ok(Argument {
        name: name.clone(),
        passing: Passing::Optional,
        c: vec![
            format!("{} {name}", c_type(value)?),
            format!("bool {present}"),
        ],
        parameters: vec![quote!(#name: #value), quote!(#present: bool)],
        convert: quote!(let #name = #present.then_some(#name);),
    })
}

fn argument(name: &Ident, ty: &Type) -> syn::Result<Argument> {
    let length = format_ident!("{name}_length");
    let fallback = quote!(return ::uji_native::abi::Fallback::fallback());
    let sized = |passing: Passing, convert: TokenStream| Argument {
        name: name.clone(),
        passing,
        c: vec![format!("const char *{name}"), format!("size_t {length}")],
        parameters: vec![quote!(#name: ::uji_native::abi::In), quote!(#length: usize)],
        convert,
    };
    let held = |convert: TokenStream| {
        Ok(Argument {
            name: name.clone(),
            passing: Passing::Handle(
                class(ty).ok_or_else(|| syn::Error::new_spanned(ty, "a handle needs a type"))?,
            ),
            c: vec![format!("void *{name}")],
            parameters: vec![quote!(#name: ::uji_native::abi::Handle)],
            convert,
        })
    };
    if let Some(text) = inner(ty).filter(|_| named(ty).as_deref() == Some("Option"))
        && let Type::Reference(reference) = text
        && named(&reference.elem).as_deref() == Some("str")
    {
        return Ok(sized(
            Passing::MaybeSized,
            quote! {
                let #name = #name.given(#length);
                let #name = #name.as_deref();
            },
        ));
    }
    if let Type::Reference(reference) = ty {
        let target = &reference.elem;
        if named(target).as_deref() == Some("str") {
            return Ok(sized(
                Passing::Sized,
                quote! {
                    let #name = #name.text(#length);
                    let #name = &*#name;
                },
            ));
        }
        if slice_of_bytes(target) {
            return Ok(sized(
                Passing::Sized,
                quote!(let #name = #name.bytes(#length);),
            ));
        }
        return held(quote! {
            let Some(#name) = #name.borrow::<#target>() else { #fallback };
        });
    }
    match named(ty).as_deref() {
        Some("String") => Ok(sized(
            Passing::Sized,
            quote!(let #name = #name.text(#length).into_owned();),
        )),
        Some("Vec") => Ok(sized(
            Passing::Sized,
            quote!(let #name = #name.bytes(#length).to_vec();),
        )),
        Some("Json") => Ok(sized(
            Passing::Encoded,
            quote! {
                let Some(#name) = #name.json(#length) else { #fallback };
            },
        )),
        Some("Arc") => held(quote! {
            let Some(#name) = #name.borrow::<#ty>().cloned() else { #fallback };
        }),
        Some("Out") => out(name, ty),
        Some("Option") => optional(name, ty),
        _ => Ok(Argument {
            name: name.clone(),
            passing: match ty {
                Type::Ptr(_) => Passing::Pointer,
                _ if named(ty).as_deref() == Some("bool") => Passing::Flag,
                _ => Passing::Plain,
            },
            c: vec![format!("{} {name}", c_type(ty)?)],
            parameters: vec![quote!(#name: #ty)],
            convert: TokenStream::new(),
        }),
    }
}

fn shape(ty: &Type) -> (bool, Shape) {
    match (named(ty).as_deref(), inner(ty)) {
        (Some("Result"), Some(ok)) => (true, shape(ok).1),
        (Some("Option"), Some(some)) => shape(some),
        (Some("Json"), _) => (false, Shape::Table),
        (Some("usize" | "u64" | "i64"), _) => (false, Shape::Number),
        (Some("Held"), Some(held)) => (false, class(held).map_or(Shape::Data, Shape::Object)),
        _ => match ty {
            Type::Tuple(tuple) if tuple.elems.is_empty() => (false, Shape::Done),
            Type::Tuple(_) => (false, Shape::Values),
            _ => (false, Shape::Data),
        },
    }
}

fn output(function: &ItemFn, raise: bool) -> Output {
    let ReturnType::Type(_, ty) = &function.sig.output else {
        return Output::Nothing;
    };
    if function.sig.asyncness.is_none()
        && let Ok(scalar) = c_type(ty)
    {
        return Output::Scalar {
            c: scalar,
            wide: matches!(
                named(ty).as_deref(),
                Some("i64" | "u64" | "usize" | "isize")
            ),
            pointer: matches!(**ty, Type::Ptr(_)),
        };
    }
    let (soft, shape) = shape(ty);
    Output::Answer {
        soft: soft && !raise,
        shape,
    }
}

fn arguments(function: &ItemFn) -> syn::Result<Vec<Argument>> {
    function
        .sig
        .inputs
        .iter()
        .map(|input| {
            let FnArg::Typed(typed) = input else {
                return Err(syn::Error::new_spanned(
                    input,
                    "a native function takes no self",
                ));
            };
            let Pat::Ident(pattern) = &*typed.pat else {
                return Err(syn::Error::new_spanned(&typed.pat, "name each parameter"));
            };
            argument(&pattern.ident, &typed.ty)
        })
        .collect()
}

struct Naming {
    name: String,
    place: String,
    receiver: bool,
}

fn naming(options: &Options, ident: &Ident, arguments: &[Argument]) -> Naming {
    let (owner, receiver) =
        if let Some(Passing::Handle(class)) = arguments.first().map(|argument| &argument.passing) {
            (Some(class.clone()), false)
        } else {
            let class = options.class.as_ref().map(key);
            let receiver = class.is_some();
            (class, receiver)
        };
    let (name, place) = match (owner, &options.prefix) {
        (Some(class), _) => (
            format!("{class}_{}", key(ident)),
            format!("{class}:{}", key(ident)),
        ),
        (None, Some(prefix)) => (
            format!("{}_{}", key(prefix), key(ident)),
            format!("{}.{}", key(prefix), key(ident)),
        ),
        (None, None) => (key(ident), key(ident)),
    };
    Naming {
        name,
        place,
        receiver,
    }
}

struct Returned {
    c: String,
    rust: TokenStream,
    body: TokenStream,
    finish: TokenStream,
}

fn returning(function: &ItemFn, output: &Output, call: TokenStream) -> Returned {
    if function.sig.asyncness.is_some() {
        return Returned {
            c: "uint64_t".to_string(),
            rust: quote!(-> u64),
            body: quote!(::uji_native::abi::start(async move {
                ::uji_native::abi::IntoReply::into_reply(#call.await)
            })),
            finish: quote!(::uji_native::abi::issued),
        };
    }
    match (output, &function.sig.output) {
        (Output::Scalar { c, .. }, ReturnType::Type(_, ty)) => Returned {
            c: c.clone(),
            rust: quote!(-> #ty),
            body: call,
            finish: TokenStream::new(),
        },
        (Output::Answer { .. }, _) => Returned {
            c: "uji_answer *".to_string(),
            rust: quote!(-> *mut ::uji_native::abi::Answer),
            body: quote!(::uji_native::abi::IntoReply::into_reply(#call).into_raw()),
            finish: quote!(::uji_native::abi::answered),
        },
        _ => Returned {
            c: "void".to_string(),
            rust: TokenStream::new(),
            body: call,
            finish: TokenStream::new(),
        },
    }
}

fn signature(returned: &str, arguments: &[Argument]) -> String {
    let c = arguments
        .iter()
        .flat_map(|argument| argument.c.iter().cloned())
        .collect::<Vec<_>>();
    let c = if c.is_empty() {
        "void".to_string()
    } else {
        c.join(", ")
    };
    format!("{returned} (*)({c})")
}

fn c_string(value: &str) -> syn::Result<Literal> {
    CString::new(value)
        .map(|text| Literal::c_string(&text))
        .map_err(|_| syn::Error::new(Span::call_site(), "a native string cannot hold a NUL byte"))
}

fn registration((place, source): (String, String)) -> syn::Result<TokenStream> {
    let (place, source) = (c_string(&place)?, c_string(&source)?);
    Ok(quote! {
        const _: () = {
            #[::uji_native::linkme::distributed_slice(::uji_native::WRAPPERS)]
            #[linkme(crate = ::uji_native::linkme)]
            static WRAPPER: ::uji_native::Wrapper = ::uji_native::Wrapper::new(#place, #source);
        };
    })
}

pub(crate) fn native(attribute: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let options = Options::parse(attribute)?;
    let function: ItemFn = syn::parse2(item)?;
    let arguments = arguments(&function)?;
    let naming = naming(&options, &function.sig.ident, &arguments);
    let output = output(&function, options.raise);
    let names = arguments.iter().map(|argument| &argument.name);
    let returned = returning(&function, &output, quote!(run(#(#names),*)));
    let signature = signature(&returned.c, &arguments);
    let wrappers = lua::wrappers(&lua::Native {
        name: &naming.name,
        place: &naming.place,
        arguments: &arguments,
        output: &output,
        asynchronous: function.sig.asyncness.is_some(),
        receiver: naming.receiver,
        iterate: options.iterate.as_ref().map(key),
    })
    .into_iter()
    .map(registration)
    .collect::<syn::Result<Vec<_>>>()?;
    let name = &naming.name;
    let external = format_ident!("{name}");
    let (name_c, signature_c) = (c_string(name)?, c_string(&signature)?);
    let parameters = arguments.iter().flat_map(|argument| &argument.parameters);
    let conversions = arguments.iter().map(|argument| &argument.convert);
    let Returned {
        rust, body, finish, ..
    } = returned;
    let mut inner = function;
    inner.sig.ident = format_ident!("run");
    inner.vis = Visibility::Inherited;
    Ok(quote! {
        pub(crate) extern "C" fn #external(#(#parameters),*) #rust {
            #inner
            #finish(::uji_native::abi::guard(|| {
                #(#conversions)*
                #body
            }))
        }

        #[allow(unsafe_code)]
        const _: () = {
            #[::uji_native::linkme::distributed_slice(::uji_native::NATIVES)]
            #[linkme(crate = ::uji_native::linkme)]
            static NATIVE: ::uji_native::Native = ::uji_native::Native::new(
                #name_c,
                #signature_c,
                #external as *const ::std::ffi::c_void,
            );

            #(#wrappers)*
        };
    })
}

pub(crate) fn c_struct(item: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(item)?;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "CStruct needs a struct",
        ));
    };
    let fields = data
        .fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().map_or_else(String::new, key);
            c_field(&name, &field.ty).map(|field| format!("{field};"))
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let name = struct_name(&input.ident);
    let fields = fields.join(" ");
    Ok(quote! {
        #[allow(unsafe_code)]
        const _: () = {
            #[::uji_native::linkme::distributed_slice(::uji_native::TYPES)]
            #[linkme(crate = ::uji_native::linkme)]
            static TYPE: ::uji_native::Type = ::uji_native::Type {
                name: #name,
                fields: #fields,
            };
        };
    })
}
