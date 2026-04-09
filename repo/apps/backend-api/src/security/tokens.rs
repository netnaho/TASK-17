use anyhow::Result;
use chrono::Utc;
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use super::constants::API_TOKEN_TTL;

/// Short-lived API token issued after session login.
///
/// The token bearer string is stored only as a SHA-256 hash so a database
/// leak does not yield usable bearer credentials. The HMAC signing key is
/// kept server-side so the API can recompute and verify request signatures.
pub struct ApiTokenService;

#[derive(Debug, Clone)]
pub struct IssuedToken {
    pub token: String,
    pub signing_key: String,
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TokenContext {
    pub user_id: Uuid,
    pub signing_key: String,
    pub expires_at: chrono::DateTime<Utc>,
}

fn random_hex(len: usize) -> String {
    let mut buf = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

pub fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

impl ApiTokenService {
    pub async fn issue(pool: &PgPool, user_id: Uuid) -> Result<IssuedToken> {
        let token = random_hex(32);
        let signing_key = random_hex(32);
        let expires_at = Utc::now() + chrono::Duration::from_std(API_TOKEN_TTL).unwrap();
        sqlx::query(
            "INSERT INTO api_tokens (user_id, token_hash, signing_key, expires_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(user_id)
        .bind(sha256_hex(&token))
        .bind(&signing_key)
        .bind(expires_at)
        .execute(pool)
        .await?;
        Ok(IssuedToken { token, signing_key, expires_at })
    }

    pub async fn lookup(pool: &PgPool, token: &str) -> Result<Option<TokenContext>> {
        let token_hash = sha256_hex(token);
        let row: Option<(Uuid, String, chrono::DateTime<Utc>)> = sqlx::query_as(
            "SELECT user_id, signing_key, expires_at FROM api_tokens
             WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > NOW()",
        )
        .bind(&token_hash)
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|(user_id, signing_key, expires_at)| TokenContext {
            user_id,
            signing_key,
            expires_at,
        }))
    }

    pub async fn revoke_for_user(pool: &PgPool, user_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE api_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL")
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
