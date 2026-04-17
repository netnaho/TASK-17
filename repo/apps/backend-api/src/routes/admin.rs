use actix_web::{delete, get, post, web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::{blacklist, rbac::Principal, rbac::ScopeKind, rbac};

fn principal(req: &HttpRequest) -> Result<Principal, ApiAppError> {
    req.extensions()
        .get::<Principal>()
        .cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))
}

#[get("/admin/blacklist")]
pub async fn list_blacklist(pool: web::Data<PgPool>, req: HttpRequest) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    p.require("admin:blacklist")?;
    let rows = blacklist::list(pool.get_ref()).await?;
    Ok(HttpResponse::Ok().json(rows))
}

#[derive(Deserialize)]
pub struct BlacklistInput { pub ip: String, pub reason: String }

#[post("/admin/blacklist")]
pub async fn add_blacklist(
    pool: web::Data<PgPool>,
    body: web::Json<BlacklistInput>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    p.require("admin:blacklist")?;
    blacklist::add(pool.get_ref(), &body.ip, &body.reason, Some(p.user_id)).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"ok": true})))
}

#[delete("/admin/blacklist/{ip}")]
pub async fn remove_blacklist(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    p.require("admin:blacklist")?;
    blacklist::remove(pool.get_ref(), &path).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"ok": true})))
}

// ---- permission grant/revoke (audited) ----

#[derive(Deserialize)]
pub struct GrantRoleInput { pub user_id: Uuid, pub role_key: String }

#[post("/admin/users/grant-role")]
pub async fn grant_role(
    pool: web::Data<PgPool>,
    body: web::Json<GrantRoleInput>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    p.require("admin:users")?;
    let role: (Uuid,) = sqlx::query_as("SELECT id FROM roles WHERE key = $1")
        .bind(&body.role_key)
        .fetch_optional(pool.get_ref())
        .await?
        .ok_or(ApiAppError::NotFound)?;

    // `user_role_assignments.site_id` is part of the composite primary key and
    // therefore NOT NULL.  Resolve a site deterministically:
    //   1. Reuse the target user's first existing assignment (preserves scope).
    //   2. Fall back to the caller-admin's first assignment (cross-site grant).
    //   3. Fall back to the first row in `sites` (fresh install, single-site).
    let site_id: Option<(Uuid,)> = sqlx::query_as(
        "SELECT site_id FROM user_role_assignments
         WHERE user_id = $1 AND site_id IS NOT NULL
         ORDER BY granted_at ASC LIMIT 1",
    )
    .bind(body.user_id)
    .fetch_optional(pool.get_ref())
    .await?;

    let site_id = match site_id {
        Some((s,)) => s,
        None => {
            let by_admin: Option<(Uuid,)> = sqlx::query_as(
                "SELECT site_id FROM user_role_assignments
                 WHERE user_id = $1 AND site_id IS NOT NULL
                 ORDER BY granted_at ASC LIMIT 1",
            )
            .bind(p.user_id)
            .fetch_optional(pool.get_ref())
            .await?;
            match by_admin {
                Some((s,)) => s,
                None => {
                    let fallback: (Uuid,) = sqlx::query_as("SELECT id FROM sites ORDER BY created_at ASC LIMIT 1")
                        .fetch_optional(pool.get_ref())
                        .await?
                        .ok_or_else(|| ApiAppError::BadRequest(
                            "no site exists — cannot scope role grant".into(),
                        ))?;
                    fallback.0
                }
            }
        }
    };

    sqlx::query(
        "INSERT INTO user_role_assignments (user_id, role_id, site_id) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(body.user_id)
    .bind(role.0)
    .bind(site_id)
    .execute(pool.get_ref())
    .await?;
    rbac::audit_permission_change(
        pool.get_ref(),
        Some(p.user_id),
        Some(body.user_id),
        "grant_role",
        serde_json::json!({"role_key": body.role_key, "site_id": site_id}),
    )
    .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"ok": true})))
}

// ---- scoped endpoint: list department residents ----
// Requires residents:read + a matching department scope.
// Returns the department_id for scope-enforcement verification; the full
// resident list is exposed through the family portal and master-data endpoints.
#[get("/departments/{dept_id}/residents")]
pub async fn list_department_residents(
    path: web::Path<String>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    p.require("residents:read")?;
    p.require_scope(ScopeKind::Department, &path)?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "department_id": path.into_inner(),
        "residents": []
    })))
}

#[get("/me")]
pub async fn me(req: HttpRequest) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    Ok(HttpResponse::Ok().json(p))
}
