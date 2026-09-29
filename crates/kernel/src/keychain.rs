use keyring::Entry;
use uji_native::native;

use crate::io::{self, Blocked};

#[native(keychain)]
async fn get(service: String, account: String) -> Result<Option<String>, Blocked<keyring::Error>> {
    io::blocking(move || {
        match Entry::new(&service, &account).and_then(|entry| entry.get_password()) {
            Err(keyring::Error::NoEntry) => Ok(None),
            secret => secret.map(Some),
        }
    })
    .await
}

#[native(keychain)]
async fn set(
    service: String,
    account: String,
    secret: String,
) -> Result<(), Blocked<keyring::Error>> {
    io::blocking(move || Entry::new(&service, &account)?.set_password(&secret)).await
}

#[native(keychain)]
async fn delete(service: String, account: String) -> Result<(), Blocked<keyring::Error>> {
    io::blocking(move || Entry::new(&service, &account)?.delete_credential()).await
}
