-- Phase 2: authentication, RBAC, scopes, security audit.
-- Additive only — never edit prior migrations.

-- ---- Permissions catalog ----
CREATE TABLE IF NOT EXISTS permissions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key         TEXT NOT NULL UNIQUE,
    label       TEXT NOT NULL,
    category    TEXT NOT NULL DEFAULT 'general'
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id         UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id   UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

-- ---- Data scopes ----
-- A user may be scoped to a specific institution/site/department or
-- (for family users) to a "family group" identified by a free-form key.
CREATE TABLE IF NOT EXISTS user_scopes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scope_kind      TEXT NOT NULL CHECK (scope_kind IN ('institution','site','department','family_group')),
    scope_ref       TEXT NOT NULL,
    granted_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, scope_kind, scope_ref)
);
CREATE INDEX IF NOT EXISTS idx_user_scopes_user ON user_scopes(user_id);

-- ---- Login attempts & lockout ----
CREATE TABLE IF NOT EXISTS login_attempts (
    id              BIGSERIAL PRIMARY KEY,
    email           TEXT NOT NULL,
    occurred_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    success         BOOLEAN NOT NULL,
    ip              TEXT
);
CREATE INDEX IF NOT EXISTS idx_login_attempts_email_time ON login_attempts(email, occurred_at DESC);

CREATE TABLE IF NOT EXISTS account_lockouts (
    user_id         UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    locked_until    TIMESTAMPTZ NOT NULL,
    reason          TEXT NOT NULL DEFAULT 'too_many_failed_logins',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---- API tokens (short-lived, server-tracked for revocation) ----
CREATE TABLE IF NOT EXISTS api_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash      TEXT NOT NULL UNIQUE,
    signing_key     TEXT NOT NULL,
    issued_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,
    revoked_at      TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_api_tokens_user ON api_tokens(user_id);

-- ---- Replay protection: persisted recent request signatures with TTL ----
CREATE TABLE IF NOT EXISTS used_signatures (
    signature       TEXT PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    seen_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_used_signatures_expires ON used_signatures(expires_at);

-- ---- IP blacklist ----
CREATE TABLE IF NOT EXISTS ip_blacklist (
    ip              TEXT PRIMARY KEY,
    reason          TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by      UUID REFERENCES users(id)
);

-- ---- Permission-change audit ----
CREATE TABLE IF NOT EXISTS permission_change_audit (
    id              BIGSERIAL PRIMARY KEY,
    occurred_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_user_id   UUID REFERENCES users(id),
    target_user_id  UUID REFERENCES users(id),
    change_kind     TEXT NOT NULL,
    details         JSONB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_perm_audit_target ON permission_change_audit(target_user_id, occurred_at DESC);

-- ---- Admin-mediated recovery requests ----
CREATE TABLE IF NOT EXISTS recovery_requests (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    requested_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status          TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','approved','rejected','consumed')),
    decided_by      UUID REFERENCES users(id),
    decided_at      TIMESTAMPTZ,
    notes           TEXT
);

-- ---- Consent baseline (used by family-portal visibility in later phases) ----
CREATE TABLE IF NOT EXISTS consent_flags (
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    flag_key    TEXT NOT NULL,
    granted     BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, flag_key)
);
