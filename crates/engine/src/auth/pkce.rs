use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};

const VERIFIER_BYTES: usize = 32;

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn generate() -> Self {
        let verifier = random_token();
        let digest = Sha256::digest(verifier.as_bytes());
        Self {
            challenge: URL_SAFE_NO_PAD.encode(digest),
            verifier,
        }
    }
}

pub fn random_token() -> String {
    let mut bytes = [0u8; VERIFIER_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
