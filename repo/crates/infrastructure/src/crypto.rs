//! Field-level AES-256-GCM encryption for sensitive TEXT columns.
//!
//! Encrypted values are stored as `base64(nonce):base64(ciphertext)`.
//! The encryption key is loaded once from the `SILVEROAK_FIELD_KEY`
//! environment variable (base64-encoded 32 bytes) and cached for the
//! lifetime of the process.
//!
//! # Security note
//! NEVER log plaintext values. All log statements in this module are
//! restricted to error kinds and byte lengths only.

use std::sync::OnceLock;

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by the crypto module.
#[derive(Debug)]
pub enum CryptoError {
    /// The stored value is not in the expected `nonce:ciphertext` format.
    InvalidFormat,
    /// Base64 decoding failed.
    Base64Decode(base64::DecodeError),
    /// AES-GCM encryption or decryption failed.
    Aead(String),
    /// The encryption key could not be loaded or is the wrong length.
    KeyError(String),
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoError::InvalidFormat => {
                write!(f, "encrypted value is not in 'nonce:ciphertext' format")
            }
            CryptoError::Base64Decode(e) => write!(f, "base64 decode error: {e}"),
            CryptoError::Aead(msg) => write!(f, "AES-GCM error: {msg}"),
            CryptoError::KeyError(msg) => write!(f, "key error: {msg}"),
        }
    }
}

impl std::error::Error for CryptoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CryptoError::Base64Decode(e) => Some(e),
            _ => None,
        }
    }
}

impl From<base64::DecodeError> for CryptoError {
    fn from(e: base64::DecodeError) -> Self {
        CryptoError::Base64Decode(e)
    }
}

// ---------------------------------------------------------------------------
// Key management
// ---------------------------------------------------------------------------

/// 32-byte dev-only fallback key. Used only when `SILVEROAK_FIELD_KEY` is
/// absent. NOT safe for production.
const DEV_FALLBACK_KEY: &[u8; 32] = b"silveroak-dev-key-NOT-FOR-PROD!!";

// Stores Ok(cipher) on success or Err(message) on key-load failure.
// get_or_try_init is still unstable; we store Result directly and use get_or_init.
static CIPHER: OnceLock<std::result::Result<Aes256Gcm, String>> = OnceLock::new();

/// Returns (or initialises) the process-wide AES-256-GCM cipher.
///
/// On first call the key is read from `SILVEROAK_FIELD_KEY`. If the variable
/// is absent a loud warning is emitted and the dev fallback key is used.
fn cipher() -> Result<&'static Aes256Gcm, CryptoError> {
    CIPHER
        .get_or_init(|| {
            let key_bytes: Vec<u8> = match std::env::var("SILVEROAK_FIELD_KEY") {
                Ok(encoded) => {
                    let decoded = match B64.decode(encoded.trim()) {
                        Ok(d) => d,
                        Err(e) => {
                            return Err(format!(
                                "SILVEROAK_FIELD_KEY base64 decode failed: {e}"
                            ))
                        }
                    };
                    if decoded.len() != 32 {
                        return Err(format!(
                            "SILVEROAK_FIELD_KEY must decode to exactly 32 bytes, got {}",
                            decoded.len()
                        ));
                    }
                    decoded
                }
                Err(_) => {
                    // NEVER log the key value itself — only the warning.
                    tracing::warn!(
                        "SILVEROAK_FIELD_KEY is not set. \
                         Using the dev-only fallback encryption key. \
                         THIS IS INSECURE AND MUST NOT BE USED IN PRODUCTION."
                    );
                    DEV_FALLBACK_KEY.to_vec()
                }
            };

            let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
            Ok(Aes256Gcm::new(key))
        })
        .as_ref()
        .map_err(|e| CryptoError::KeyError(e.clone()))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Encrypts `plaintext` with AES-256-GCM using a freshly generated 12-byte
/// (96-bit) nonce.
///
/// Returns `base64(nonce):base64(ciphertext)`.
///
/// # Security
/// NEVER log the plaintext argument; log only lengths or error kinds.
pub fn encrypt(plaintext: &str) -> Result<String, CryptoError> {
    let cipher = cipher()?;

    // Generate a random 12-byte nonce (96 bits — standard for GCM).
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| CryptoError::Aead(e.to_string()))?;

    let encoded = format!("{}:{}", B64.encode(nonce_bytes), B64.encode(&ciphertext));
    Ok(encoded)
}

/// Decrypts a value produced by [`encrypt`].
///
/// Expects the format `base64(nonce):base64(ciphertext)`.
///
/// # Security
/// NEVER log the returned plaintext; log only lengths or error kinds.
pub fn decrypt(ciphertext: &str) -> Result<String, CryptoError> {
    let cipher = cipher()?;

    let (nonce_b64, ct_b64) = ciphertext
        .split_once(':')
        .ok_or(CryptoError::InvalidFormat)?;

    let nonce_bytes = B64.decode(nonce_b64)?;
    if nonce_bytes.len() != 12 {
        return Err(CryptoError::InvalidFormat);
    }
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ct_bytes = B64.decode(ct_b64)?;

    let plaintext_bytes = cipher
        .decrypt(nonce, ct_bytes.as_slice())
        .map_err(|e| CryptoError::Aead(e.to_string()))?;

    String::from_utf8(plaintext_bytes)
        .map_err(|e| CryptoError::Aead(format!("UTF-8 decode failed: {e}")))
}

/// Returns `true` when `value` looks like an encrypted blob (contains `:`).
///
/// This is a heuristic only — a plain string containing `:` would also match.
/// Use it as a quick guard before calling [`decrypt`].
pub fn is_encrypted(value: &str) -> bool {
    value.contains(':')
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        // NEVER assert or print the plaintext in log output during tests.
        let plaintext = "sensitive-data-12345";
        let encrypted = encrypt(plaintext).expect("encrypt should succeed");
        assert!(is_encrypted(&encrypted), "encrypted value must contain ':'");
        let decrypted = decrypt(&encrypted).expect("decrypt should succeed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn different_nonces_produce_different_ciphertexts() {
        let plaintext = "same input";
        let ct1 = encrypt(plaintext).unwrap();
        let ct2 = encrypt(plaintext).unwrap();
        // Nonces are random so ciphertexts must differ.
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn invalid_format_returns_error() {
        let result = decrypt("notvalidbase64withoutcolon");
        assert!(matches!(result, Err(CryptoError::InvalidFormat)));
    }

    #[test]
    fn is_encrypted_heuristic() {
        assert!(is_encrypted("abc:def"));
        assert!(!is_encrypted("nodivider"));
    }

    #[test]
    fn encrypted_format_has_two_parts() {
        let encrypted = encrypt("test").unwrap();
        let parts: Vec<&str> = encrypted.splitn(2, ':').collect();
        assert_eq!(parts.len(), 2, "must be nonce:ciphertext format");
        assert!(!parts[0].is_empty(), "nonce part must not be empty");
        assert!(!parts[1].is_empty(), "ciphertext part must not be empty");
    }

    #[test]
    fn tampered_ciphertext_fails_to_decrypt() {
        let encrypted = encrypt("sensitive-value").unwrap();
        let (nonce_part, _ct_part) = encrypted.split_once(':').unwrap();
        // Replace ciphertext with valid-looking but wrong base64
        let tampered = format!("{nonce_part}:dGFtcGVyZWQ="); // base64("tampered")
        assert!(decrypt(&tampered).is_err(), "tampered ciphertext should fail to decrypt");
    }

    #[test]
    fn empty_string_encrypts_and_decrypts() {
        let encrypted = encrypt("").unwrap();
        let decrypted = decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn unicode_string_round_trips() {
        let plaintext = "Resident Müller — discomfort noted";
        let encrypted = encrypt(plaintext).unwrap();
        let decrypted = decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn missing_colon_separator_is_invalid_format() {
        assert!(matches!(decrypt("notvalidbase64withoutcolon"), Err(CryptoError::InvalidFormat)));
    }
}
