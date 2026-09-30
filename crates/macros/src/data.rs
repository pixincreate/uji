use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub(crate) fn options(item: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(item)?;
    let name = &input.ident;
    Ok(quote! {
        #[derive(serde::Deserialize)]
        #input

        impl mlua::FromLua for #name {
            fn from_lua(value: mlua::Value, lua: &mlua::Lua) -> mlua::Result<Self> {
                let value = match value {
                    mlua::Value::Nil => mlua::Value::Table(lua.create_table()?),
                    value => value,
                };
                mlua::LuaSerdeExt::from_value(lua, value)
            }
        }
    })
}

pub(crate) fn value(item: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = syn::parse2(item)?;
    let name = &input.ident;
    Ok(quote! {
        #[derive(serde::Serialize)]
        #input

        impl mlua::IntoLua for #name {
            fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
                crate::io::to_lua(lua, &self)
            }
        }
    })
}
