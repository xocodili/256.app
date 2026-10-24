//! Password **key derivation** via Argon2id. General-purpose digests use the
//! [`crypto::sha3`](crate::crypto::sha3) module (SHA3-512); passwords intentionally do **not**
//! use SHA3-512 alone.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::rngs::OsRng;

pub const MIN_PASSWORD_LEN: usize = 12;

/// Memory cost ~19 MiB, 3 iterations, 4 lanes — conservative default; tune under load testing.
fn argon2id() -> Argon2<'static> {
    let params = Params::new(19456, 3, 4, None).expect("valid argon2 params");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("password must be at least {MIN_PASSWORD_LEN} characters")]
    TooShort,
    #[error("invalid password hash")]
    InvalidHash,
    #[error("hashing failed")]
    HashFailed(argon2::password_hash::Error),
}

pub fn hash_password(plain: &str) -> Result<String, PasswordError> {
    if plain.len() < MIN_PASSWORD_LEN {
        return Err(PasswordError::TooShort);
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2id()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(PasswordError::HashFailed)?;
    Ok(hash.to_string())
}

pub fn verify_password(plain: &str, phc_string: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(phc_string).map_err(|_| PasswordError::InvalidHash)?;
    Ok(argon2id()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}
