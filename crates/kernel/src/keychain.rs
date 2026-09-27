use keyring::Entry;
use mlua::Lua;
use uji_macros::function;

use crate::io;

#[function(keychain)]
async fn get(
    lua: Lua,
    service: String,
    account: String,
) -> mlua::Result<Result<Option<String>, keyring::Error>> {
    let secret = io::blocking(&lua, move || {
        Entry::new(&service, &account).and_then(|entry| entry.get_password())
    })
    .await?;
    Ok(match secret {
        Err(keyring::Error::NoEntry) => Ok(None),
        secret => secret.map(Some),
    })
}

#[function(keychain)]
async fn set(
    lua: Lua,
    service: String,
    account: String,
    secret: String,
) -> mlua::Result<Result<bool, keyring::Error>> {
    let stored = io::blocking(&lua, move || {
        Entry::new(&service, &account).and_then(|entry| entry.set_password(&secret))
    })
    .await?;
    Ok(stored.map(|()| true))
}

#[function(keychain)]
async fn delete(
    lua: Lua,
    service: String,
    account: String,
) -> mlua::Result<Result<bool, keyring::Error>> {
    let deleted = io::blocking(&lua, move || {
        Entry::new(&service, &account).and_then(|entry| entry.delete_credential())
    })
    .await?;
    Ok(deleted.map(|()| true))
}
