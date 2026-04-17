/// Cross-crate unit tests for the SilverOak shared, domain, and application crates.
///
/// These tests run entirely in memory — no database, no HTTP stack, no async runtime.
/// They verify:
///   - Type construction and field access for all public domain types
///   - JSON round-trip serialization for shared DTOs and domain entities
///   - Error display formatting for DomainError and AppError
///   - AppResult<T> Ok/Err propagation

// ── shared crate ────────────────────────────────────────────────────────────

#[test]
fn api_error_fields() {
    let e = shared::ApiError {
        code: "not_found".to_string(),
        message: "resource does not exist".to_string(),
    };
    assert_eq!(e.code, "not_found");
    assert_eq!(e.message, "resource does not exist");
}

#[test]
fn api_error_json_round_trip() {
    let original = shared::ApiError {
        code: "bad_request".to_string(),
        message: "year_month must be in YYYY-MM format".to_string(),
    };
    let json = serde_json::to_string(&original).expect("serialize");
    let decoded: shared::ApiError = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.code, original.code);
    assert_eq!(decoded.message, original.message);
}

#[test]
fn health_status_fields() {
    let h = shared::HealthStatus {
        status: "ok",
        version: "0.1.0",
    };
    assert_eq!(h.status, "ok");
    assert_eq!(h.version, "0.1.0");
}

#[test]
fn health_status_json_round_trip() {
    let original = shared::HealthStatus {
        status: "ok",
        version: "0.1.0",
    };
    let json = serde_json::to_string(&original).expect("serialize");
    // `HealthStatus` fields are `&'static str`, so the JSON buffer must outlive
    // the deserialised value for the full `'static` lifetime.  `Box::leak`
    // gives a `&'static str` at the cost of a tiny, one-off allocation — fine
    // in a unit test and avoids changing the public struct shape.
    let json_static: &'static str = Box::leak(json.into_boxed_str());
    let decoded: shared::HealthStatus =
        serde_json::from_str(json_static).expect("deserialize");
    assert_eq!(decoded.status, original.status);
    assert_eq!(decoded.version, original.version);
}

// ── domain::identity ────────────────────────────────────────────────────────

#[test]
fn user_id_wraps_uuid() {
    let id = uuid::Uuid::new_v4();
    let user_id = domain::identity::UserId(id);
    assert_eq!(user_id.0, id);
}

#[test]
fn user_fields() {
    let id = uuid::Uuid::new_v4();
    let user = domain::identity::User {
        id: domain::identity::UserId(id),
        email: "medical@silveroak.local".to_string(),
        display_name: "Morgan Medical".to_string(),
        is_active: true,
    };
    assert_eq!(user.email, "medical@silveroak.local");
    assert_eq!(user.display_name, "Morgan Medical");
    assert!(user.is_active);
    assert_eq!(user.id.0, id);
}

#[test]
fn user_json_round_trip() {
    let id = uuid::Uuid::new_v4();
    let user = domain::identity::User {
        id: domain::identity::UserId(id),
        email: "admin@silveroak.local".to_string(),
        display_name: "Avery Admin".to_string(),
        is_active: true,
    };
    let json = serde_json::to_string(&user).expect("serialize");
    let decoded: domain::identity::User = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.email, user.email);
    assert_eq!(decoded.display_name, user.display_name);
    assert_eq!(decoded.is_active, user.is_active);
    assert_eq!(decoded.id.0, id);
}

#[test]
fn role_fields() {
    let role = domain::identity::Role {
        id: uuid::Uuid::new_v4(),
        key: "medical".to_string(),
        label: "Medical Staff".to_string(),
    };
    assert_eq!(role.key, "medical");
    assert_eq!(role.label, "Medical Staff");
}

// ── domain::organisation ────────────────────────────────────────────────────

#[test]
fn institution_fields() {
    let inst = domain::organisation::Institution {
        id: uuid::Uuid::new_v4(),
        name: "SilverOak Demo".to_string(),
        code: "demo".to_string(),
    };
    assert_eq!(inst.name, "SilverOak Demo");
    assert_eq!(inst.code, "demo");
}

#[test]
fn site_links_to_institution() {
    let inst_id = uuid::Uuid::new_v4();
    let site = domain::organisation::Site {
        id: uuid::Uuid::new_v4(),
        institution_id: inst_id,
        name: "Main Campus".to_string(),
    };
    assert_eq!(site.institution_id, inst_id);
    assert_eq!(site.name, "Main Campus");
}

#[test]
fn department_links_to_site() {
    let site_id = uuid::Uuid::new_v4();
    let dept = domain::organisation::Department {
        id: uuid::Uuid::new_v4(),
        site_id,
        name: "Wing A".to_string(),
    };
    assert_eq!(dept.site_id, site_id);
    assert_eq!(dept.name, "Wing A");
}

#[test]
fn organisation_json_round_trip() {
    let inst_id = uuid::Uuid::new_v4();
    let site_id = uuid::Uuid::new_v4();
    let dept = domain::organisation::Department {
        id: uuid::Uuid::new_v4(),
        site_id,
        name: "Wing B".to_string(),
    };
    let json = serde_json::to_string(&dept).expect("serialize");
    let decoded: domain::organisation::Department =
        serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.site_id, site_id);
    assert_eq!(decoded.name, "Wing B");

    let _ = inst_id; // used for context only
}

// ── domain::DomainError ──────────────────────────────────────────────────────

#[test]
fn domain_error_display() {
    let e = domain::DomainError::Invariant("quantity must be > 0".to_string());
    assert_eq!(e.to_string(), "invariant violated: quantity must be > 0");
}

// ── application::AppError ───────────────────────────────────────────────────

#[test]
fn app_error_not_found_display() {
    let e = application::AppError::NotFound;
    assert_eq!(e.to_string(), "not found");
}

#[test]
fn app_error_repository_display() {
    let e = application::AppError::Repository("connection refused".to_string());
    assert_eq!(e.to_string(), "repository error: connection refused");
}

#[test]
fn app_result_ok_propagates() {
    let result: application::AppResult<u32> = Ok(42);
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn app_result_err_propagates() {
    let result: application::AppResult<u32> = Err(application::AppError::NotFound);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "not found");
}
