use anyhow::Result;
use chrono::{Duration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// Sessions are server-tracked: the cookie value is a high-entropy random
/// token, and only its SHA-256 hash is stored in `sessions`. This gives us
/// real revocation ("logout invalidates session") without trusting the
/// client, and avoids the foot-guns of pure JWT cookies.
pub struct SessionService;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

const SESSION_TTL_HOURS: i64 = 12;

impl SessionService {
    pub fn random_token() -> String {
        let mut buf = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        hex::encode(buf)
    }

    pub fn hash(token: &str) -> String {
        let mut h = Sha256::new();
        h.update(token.as_bytes());
        hex::encode(h.finalize())
    }

    pub async fn create(pool: &PgPool, user_id: Uuid) -> Result<String> {
        let token = Self::random_token();
        let token_hash = Self::hash(&token);
        let expires_at = Utc::now() + Duration::hours(SESSION_TTL_HOURS);
        sqlx::query(
            "INSERT INTO sessions (id, user_id, token_hash, expires_at)
             VALUES (gen_random_uuid(), $1, $2, $3)",
        )
        .bind(user_id)
        .bind(&token_hash)
        .bind(expires_at)
        .execute(pool)
        .await?;
        Ok(token)
    }

    pub async fn lookup(pool: &PgPool, token: &str) -> Result<Option<SessionInfo>> {
        let token_hash = Self::hash(token);
        let row: Option<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT id, user_id FROM sessions
             WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > NOW()",
        )
        .bind(&token_hash)
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|(id, user_id)| SessionInfo { user_id, session_id: id }))
    }

    pub async fn revoke(pool: &PgPool, token: &str) -> Result<()> {
        let token_hash = Self::hash(token);
        sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE token_hash = $1")
            .bind(&token_hash)
            .execute(pool)
            .await?;
        Ok(())
    }
}
