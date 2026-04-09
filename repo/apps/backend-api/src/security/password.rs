use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use super::constants::MIN_PASSWORD_LEN;
use crate::error::ApiAppError;

/// Validate against the password policy. Phase 2 only enforces minimum
/// length per the Task Prompt; richer rules can be added without changing
/// callers.
pub fn validate_policy(password: &str) -> Result<(), ApiAppError> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(ApiAppError::BadRequest(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, ApiAppError> {
    validate_policy(password)?;
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ApiAppError::Internal(format!("hash error: {e}")))
}

pub fn verify_password(password: &str, encoded_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(encoded_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_passwords() {
        assert!(validate_policy("short").is_err());
        assert!(validate_policy("12345678901").is_err()); // 11
        assert!(validate_policy("123456789012").is_ok()); // 12
    }

    #[test]
    fn round_trips_valid_password() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password value", &hash));
    }

    #[test]
    fn exactly_minimum_length_accepted() {
        let exactly_12 = "123456789012"; // 12 chars
        assert!(validate_policy(exactly_12).is_ok());
        let eleven = "12345678901"; // 11 chars
        assert!(validate_policy(eleven).is_err());
    }

    #[test]
    fn wrong_password_does_not_verify() {
        let hash = hash_password("correct-horse-battery-staple-ok").unwrap();
        assert!(!verify_password("wrong-guess", &hash));
        assert!(!verify_password("", &hash));
    }

    #[test]
    fn malformed_hash_returns_false_not_panic() {
        assert!(!verify_password("any-password", "not-a-valid-hash"));
        assert!(!verify_password("any-password", ""));
        assert!(!verify_password("any-password", "$argon2id$invalid$data$here"));
    }
}
