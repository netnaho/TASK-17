use anyhow::Result;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::error::ApiAppError;

/// Materialized authorization context for a request, computed once by the
/// auth middleware and read by route handlers via the `Principal` extractor.
#[derive(Debug, Clone, Serialize)]
pub struct Principal {
    pub user_id: Uuid,
    pub email: String,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
    pub scopes: Vec<Scope>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Scope {
    pub kind: ScopeKind,
    pub reference: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Institution,
    Site,
    Department,
    FamilyGroup,
}

impl ScopeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Institution => "institution",
            Self::Site => "site",
            Self::Department => "department",
            Self::FamilyGroup => "family_group",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "institution" => Self::Institution,
            "site" => Self::Site,
            "department" => Self::Department,
            "family_group" => Self::FamilyGroup,
            _ => return None,
        })
    }
}

impl Principal {
    pub fn require(&self, permission: &str) -> Result<(), ApiAppError> {
        if self.permissions.contains(permission) || self.permissions.contains("*") {
            Ok(())
        } else {
            Err(ApiAppError::Forbidden(format!("missing permission: {permission}")))
        }
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    /// Returns Ok if the principal has any scope of the given kind/reference,
    /// or holds the wildcard permission `scope:any`.
    pub fn require_scope(&self, kind: ScopeKind, reference: &str) -> Result<(), ApiAppError> {
        if self.permissions.contains("scope:any") {
            return Ok(());
        }
        let allowed = self.scopes.iter().any(|s| s.kind == kind && s.reference == reference);
        if allowed {
            Ok(())
        } else {
            Err(ApiAppError::Forbidden(format!(
                "scope denied: {}={}",
                kind.as_str(),
                reference
            )))
        }
    }
}

pub async fn load_principal(pool: &PgPool, user_id: Uuid) -> Result<Principal> {
    let user: (Uuid, String) = sqlx::query_as("SELECT id, email FROM users WHERE id = $1 AND is_active")
        .bind(user_id)
        .fetch_one(pool)
        .await?;

    let roles: Vec<(String,)> = sqlx::query_as(
        "SELECT r.key FROM roles r
         JOIN user_role_assignments ura ON ura.role_id = r.id
         WHERE ura.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let perms: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT p.key FROM permissions p
         JOIN role_permissions rp ON rp.permission_id = p.id
         JOIN user_role_assignments ura ON ura.role_id = rp.role_id
         WHERE ura.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let scopes: Vec<(String, String)> = sqlx::query_as(
        "SELECT scope_kind, scope_ref FROM user_scopes WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(Principal {
        user_id: user.0,
        email: user.1,
        roles: roles.into_iter().map(|(k,)| k).collect(),
        permissions: perms.into_iter().map(|(k,)| k).collect(),
        scopes: scopes
            .into_iter()
            .filter_map(|(k, r)| ScopeKind::parse(&k).map(|kind| Scope { kind, reference: r }))
            .collect(),
    })
}

pub async fn audit_permission_change(
    pool: &PgPool,
    actor: Option<Uuid>,
    target: Option<Uuid>,
    kind: &str,
    details: serde_json::Value,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO permission_change_audit (actor_user_id, target_user_id, change_kind, details)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(actor)
    .bind(target)
    .bind(kind)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}
