use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::{
    Attribute, Data, DeriveInput, Expr, Field, Fields, FnArg, GenericArgument, Ident, ImplItem,
    ImplItemFn, ItemFn, ItemImpl, LitBool, PathArguments, ReturnType, Signature, Type, Visibility,
    parse_macro_input, parse_quote,
};

#[proc_macro_attribute]
pub fn register(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let parent = parse_macro_input!(attribute as Option<Ident>);
    let function = parse_macro_input!(item as ItemFn);
    let call = Arguments::of(&function.sig).call(&quote!(build), false);
    let value = if matches!(Returns::of(&function.sig.output), Returns::Plain) {
        call
    } else {
        quote!(#call?)
    };
    export(&function, parent.as_ref(), &value)
}

#[proc_macro_attribute]
pub fn function(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let parent = parse_macro_input!(attribute as Option<Ident>);
    let function = parse_macro_input!(item as ItemFn);
    let sig = &function.sig;
    let args = Arguments::of(sig);
    let returns = Returns::of(&sig.output);
    let (lua, binding) = (args.closure_lua(returns), args.binding());
    let value = if sig.asyncness.is_none() {
        let body = returns.finish(&args.call(&quote!(build), false), &quote!(lua));
        quote!(lua.create_function(|#lua, #binding| #body)?)
    } else if matches!(returns, Returns::Hard) && !args.borrows() {
        let call = args.call(&quote!(build), false);
        quote!(lua.create_async_function(|#lua, #binding| #call)?)
    } else {
        let body = args.future(returns, &args.call(&quote!(build), true));
        quote!(lua.create_async_function(|#lua, #binding| #body)?)
    };
    export(&function, parent.as_ref(), &value)
}

#[proc_macro_attribute]
pub fn methods(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut block = parse_macro_input!(item as ItemImpl);
    let ty = &block.self_ty;
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    for item in &mut block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let getter = take(&mut method.attrs, "get").is_some();
        if let Some(attribute) = take(&mut method.attrs, "iterate") {
            match attribute.parse_args::<Ident>() {
                Ok(name) => methods.push(iterator(&name, &method.sig.ident)),
                Err(err) => return err.to_compile_error().into(),
            }
        }
        if getter {
            fields.push(field(method));
        } else {
            methods.push(method_binding(method));
        }
    }
    let fields = (!fields.is_empty()).then(|| {
        quote! {
            fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
                #(#fields)*
            }
        }
    });
    quote! {
        #block

        impl mlua::UserData for #ty {
            #fields

            fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
                #(#methods)*
            }
        }
    }
    .into()
}

#[proc_macro_derive(FromLua, attributes(lua))]
pub fn from_lua(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let fields = match named_fields(&input) {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error().into(),
    };
    let mut reads = Vec::new();
    for field in fields {
        match read(field) {
            Ok(tokens) => reads.push(tokens),
            Err(err) => return err.to_compile_error().into(),
        }
    }
    let type_name = name.to_string();
    quote! {
        impl mlua::FromLua for #name {
            fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
                let table = match value {
                    mlua::Value::Nil => lua.create_table()?,
                    mlua::Value::Table(table) => table,
                    other => {
                        return Err(mlua::Error::FromLuaConversionError {
                            from: other.type_name(),
                            to: #type_name.to_string(),
                            message: None,
                        });
                    }
                };
                Ok(Self { #(#reads),* })
            }
        }
    }
    .into()
}

#[proc_macro_derive(IntoLua, attributes(lua))]
pub fn into_lua(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let mut nulls = true;
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("lua"))
    {
        let parsed = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("nulls") {
                nulls = meta.value()?.parse::<LitBool>()?.value;
                Ok(())
            } else {
                Err(meta.error("expected `nulls`"))
            }
        });
        if let Err(err) = parsed {
            return err.to_compile_error().into();
        }
    }
    let value = if nulls {
        quote!(mlua::LuaSerdeExt::to_value(lua, &self))
    } else {
        quote! {
            mlua::LuaSerdeExt::to_value_with(
                lua,
                &self,
                mlua::serde::SerializeOptions::new().serialize_none_to_null(false),
            )
        }
    };
    quote! {
        impl mlua::IntoLua for #name {
            fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
                #value
            }
        }
    }
    .into()
}

fn named_fields(input: &DeriveInput) -> syn::Result<Vec<&Field>> {
    if let Data::Struct(data) = &input.data
        && let Fields::Named(fields) = &data.fields
    {
        return Ok(fields.named.iter().collect());
    }
    Err(syn::Error::new_spanned(
        &input.ident,
        "FromLua needs a struct with named fields",
    ))
}

fn read(field: &Field) -> syn::Result<Tokens> {
    let Some(ident) = &field.ident else {
        return Err(syn::Error::new_spanned(field, "the field needs a name"));
    };
    let (key, ty) = (ident.unraw().to_string(), &field.ty);
    let mut default: Option<Option<Expr>> = None;
    for attr in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("lua"))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("default") {
                return Err(meta.error("expected `default`"));
            }
            default = Some(if meta.input.peek(syn::Token![=]) {
                Some(meta.value()?.parse()?)
            } else {
                None
            });
            Ok(())
        })?;
    }
    let context = format!("option `{key}`");
    Ok(match default {
        None => quote!(#ident: mlua::ErrorContext::context(table.get::<#ty>(#key), #context)?),
        Some(None) => quote! {
            #ident: mlua::ErrorContext::context(table.get::<Option<#ty>>(#key), #context)?
                .unwrap_or_default()
        },
        Some(Some(value)) => quote! {
            #ident: mlua::ErrorContext::context(table.get::<Option<#ty>>(#key), #context)?
                .unwrap_or(#value)
        },
    })
}

fn export(function: &ItemFn, parent: Option<&Ident>, value: &Tokens) -> TokenStream {
    let ident = &function.sig.ident;
    let key = ident.unraw().to_string();
    let parents: Vec<String> = parent.map(Ident::to_string).into_iter().collect();
    let mut inner = function.clone();
    inner.sig.ident = format_ident!("build");
    inner.vis = Visibility::Inherited;
    quote! {
        pub(crate) fn #ident(lua: &mlua::Lua, uji: &mlua::Table) -> mlua::Result<()> {
            #inner
            crate::vm::table(lua, uji, &[#(#parents),*])?.set(#key, #value)
        }

        #[allow(unsafe_code)]
        const _: () = {
            #[linkme::distributed_slice(crate::REGISTERED)]
            static REGISTRATION: crate::Register = #ident;
        };
    }
    .into()
}

fn take(attrs: &mut Vec<Attribute>, name: &str) -> Option<Attribute> {
    let index = attrs.iter().position(|attr| attr.path().is_ident(name))?;
    Some(attrs.remove(index))
}

fn field(method: &ImplItemFn) -> Tokens {
    let sig = &method.sig;
    let (key, name) = (sig.ident.unraw().to_string(), &sig.ident);
    let args = Arguments::of(sig);
    let returns = Returns::of(&sig.output);
    let lua = args.closure_lua(returns);
    let body = returns.finish(&args.call(&quote!(this.#name), false), &quote!(lua));
    quote!(fields.add_field_method_get(#key, |#lua, this| #body);)
}

fn method_binding(method: &ImplItemFn) -> Tokens {
    let sig = &method.sig;
    let (key, name) = (sig.ident.unraw().to_string(), &sig.ident);
    let args = Arguments::of(sig);
    let returns = Returns::of(&sig.output);
    let (lua, binding) = (args.closure_lua(returns), args.binding());
    let asynchronous = sig.asyncness.is_some();
    let receiver = sig.inputs.iter().find_map(|input| match input {
        FnArg::Receiver(receiver) => Some(receiver.mutability.is_some()),
        FnArg::Typed(_) => None,
    });
    let target = if receiver.is_some() {
        quote!(this.#name)
    } else {
        quote!(Self::#name)
    };
    let call = args.call(&target, asynchronous);
    let body = if asynchronous {
        args.future(returns, &call)
    } else {
        returns.finish(&call, &quote!(lua))
    };
    match (receiver, asynchronous) {
        (None, false) => quote!(methods.add_function(#key, |#lua, #binding| #body);),
        (None, true) => quote!(methods.add_async_function(#key, |#lua, #binding| #body);),
        (Some(false), false) => quote!(methods.add_method(#key, |#lua, this, #binding| #body);),
        (Some(true), false) => {
            quote!(methods.add_method_mut(#key, |#lua, this, #binding| #body);)
        }
        (Some(false), true) => {
            quote!(methods.add_async_method(#key, |#lua, this, #binding| #body);)
        }
        (Some(true), true) => {
            quote!(methods.add_async_method_mut(#key, |#lua, mut this, #binding| #body);)
        }
    }
}

fn iterator(name: &Ident, step: &Ident) -> Tokens {
    let (name, step) = (name.unraw().to_string(), step.unraw().to_string());
    quote! {
        methods.add_function(#name, |_, this: mlua::AnyUserData| {
            Ok((mlua::ObjectLike::get::<mlua::Function>(&this, #step)?, this))
        });
    }
}

#[derive(Clone, Copy)]
enum Returns {
    Plain,
    Hard,
    Soft,
    Settled,
}

impl Returns {
    fn of(output: &ReturnType) -> Self {
        let ReturnType::Type(_, ty) = output else {
            return Self::Plain;
        };
        match result_arguments(ty).as_deref() {
            None => Self::Plain,
            Some([_, _]) => Self::Soft,
            Some([inner]) if result_arguments(inner).is_some_and(|args| args.len() == 2) => {
                Self::Settled
            }
            Some(_) => Self::Hard,
        }
    }

    fn settles(self) -> bool {
        matches!(self, Self::Soft | Self::Settled)
    }

    fn finish(self, call: &Tokens, lua: &Tokens) -> Tokens {
        match self {
            Self::Plain => quote!(Ok(#call)),
            Self::Hard => quote!(#call),
            Self::Soft => quote!(crate::io::settle(#lua, #call)),
            Self::Settled => quote!(crate::io::settle(#lua, #call?)),
        }
    }
}

fn result_arguments(ty: &Type) -> Option<Vec<Type>> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Result" {
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

enum Pass {
    Owned,
    Borrowed,
    Optional,
    Dereferenced,
}

struct Arguments {
    lua: bool,
    values: Vec<Type>,
    passes: Vec<Pass>,
    order: Vec<Option<usize>>,
}

impl Arguments {
    fn of(sig: &Signature) -> Self {
        let mut arguments = Self {
            lua: false,
            values: Vec::new(),
            passes: Vec::new(),
            order: Vec::new(),
        };
        for input in &sig.inputs {
            let FnArg::Typed(typed) = input else {
                continue;
            };
            if is_lua(&typed.ty) {
                arguments.lua = true;
                arguments.order.push(None);
            } else {
                let (ty, pass) = owned(&typed.ty);
                arguments.order.push(Some(arguments.values.len()));
                arguments.values.push(ty);
                arguments.passes.push(pass);
            }
        }
        arguments
    }

    fn names(&self) -> Vec<Ident> {
        (0..self.values.len())
            .map(|index| format_ident!("arg{index}"))
            .collect()
    }

    fn borrows(&self) -> bool {
        self.passes.iter().any(|pass| !matches!(pass, Pass::Owned))
    }

    fn closure_lua(&self, returns: Returns) -> Tokens {
        if self.lua || returns.settles() {
            quote!(lua)
        } else {
            quote!(_)
        }
    }

    fn future(&self, returns: Returns, call: &Tokens) -> Tokens {
        if returns.settles() && self.lua {
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

    fn binding(&self) -> Tokens {
        let names = self.names();
        let types = &self.values;
        if let ([name], [ty]) = (names.as_slice(), types.as_slice()) {
            quote!(#name: #ty)
        } else {
            quote!((#(#names),*): (#(#types),*))
        }
    }

    fn call(&self, target: &Tokens, asynchronous: bool) -> Tokens {
        let names = self.names();
        let args = self.order.iter().map(|slot| {
            slot.map_or_else(
                || quote!(lua),
                |index| {
                    let name = &names[index];
                    match self.passes[index] {
                        Pass::Owned => quote!(#name),
                        Pass::Borrowed => quote!(&#name),
                        Pass::Optional => quote!(#name.as_ref()),
                        Pass::Dereferenced => quote!(#name.as_deref()),
                    }
                },
            )
        });
        if asynchronous {
            quote!(#target(#(#args),*).await)
        } else {
            quote!(#target(#(#args),*))
        }
    }
}

fn owned(ty: &Type) -> (Type, Pass) {
    if let Type::Reference(reference) = ty {
        return (target(&reference.elem).0, Pass::Borrowed);
    }
    if let Some(inner) = optional_reference(ty) {
        let (inner, dynamic) = target(&inner);
        let pass = if dynamic {
            Pass::Dereferenced
        } else {
            Pass::Optional
        };
        return (parse_quote!(Option<#inner>), pass);
    }
    (ty.clone(), Pass::Owned)
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

fn optional_reference(ty: &Type) -> Option<Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(Type::Reference(reference))) = arguments.args.first() else {
        return None;
    };
    Some((*reference.elem).clone())
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
