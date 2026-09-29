use proc_macro2::TokenStream;
use quote::quote;
use syn::meta::ParseNestedMeta;
use syn::{Attribute, Data, DeriveInput, Expr, Field, Fields, LitBool};

use crate::signature::key;

pub(crate) fn from_lua(item: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(item)?;
    let name = &input.ident;
    let type_name = name.to_string();
    let reads = named_fields(&input)?
        .into_iter()
        .map(read)
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! {
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
    })
}

pub(crate) fn into_lua(item: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(item)?;
    let name = &input.ident;
    let mut nulls = true;
    lua_attributes(&input.attrs, |meta| {
        if !meta.path.is_ident("nulls") {
            return Err(meta.error("expected `nulls`"));
        }
        nulls = meta.value()?.parse::<LitBool>()?.value;
        Ok(())
    })?;
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
    Ok(quote! {
        impl mlua::IntoLua for #name {
            fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
                #value
            }
        }
    })
}

fn lua_attributes(
    attrs: &[Attribute],
    mut each: impl FnMut(ParseNestedMeta<'_>) -> syn::Result<()>,
) -> syn::Result<()> {
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("lua")) {
        attr.parse_nested_meta(&mut each)?;
    }
    Ok(())
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

fn read(field: &Field) -> syn::Result<TokenStream> {
    let Some(ident) = &field.ident else {
        return Err(syn::Error::new_spanned(field, "the field needs a name"));
    };
    let (name, ty) = (key(ident), &field.ty);
    let mut default: Option<Option<Expr>> = None;
    lua_attributes(&field.attrs, |meta| {
        if !meta.path.is_ident("default") {
            return Err(meta.error("expected `default`"));
        }
        let value = if meta.input.peek(syn::Token![=]) {
            Some(meta.value()?.parse()?)
        } else {
            None
        };
        default = Some(value);
        Ok(())
    })?;
    let context = format!("option `{name}`");
    let optional = quote!(mlua::ErrorContext::context(table.get::<Option<#ty>>(#name), #context)?);
    Ok(match default {
        None => quote!(#ident: mlua::ErrorContext::context(table.get::<#ty>(#name), #context)?),
        Some(None) => quote!(#ident: #optional.unwrap_or_default()),
        Some(Some(value)) => quote!(#ident: #optional.unwrap_or(#value)),
    })
}
