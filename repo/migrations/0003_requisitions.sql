-- Phase 3: requisitions, configurable approval engine, immutable audit,
-- transactional inventory issue records.
--
-- Currency is stored as cents (BIGINT) to avoid floating point.
-- Quantities are stored as whole units (INTEGER) — assisted-living
-- catalogs are box/bottle/pack based; fractional units are out of scope.

-- =========================================================================
-- Inventory
-- =========================================================================
CREATE TABLE IF NOT EXISTS inventory_categories (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key         TEXT NOT NULL UNIQUE,
    label       TEXT NOT NULL,
    controlled  BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS inventory_items (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sku             TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    category_id     UUID NOT NULL REFERENCES inventory_categories(id),
    unit            TEXT NOT NULL DEFAULT 'each',
    unit_price_cents BIGINT NOT NULL CHECK (unit_price_cents >= 0),
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS inventory_stock_by_site (
    item_id     UUID NOT NULL REFERENCES inventory_items(id) ON DELETE CASCADE,
    site_id     UUID NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    on_hand     INTEGER NOT NULL DEFAULT 0 CHECK (on_hand >= 0),
    PRIMARY KEY (item_id, site_id)
);

-- =========================================================================
-- Approval engine (configurable, NOT hardcoded)
-- =========================================================================
CREATE TABLE IF NOT EXISTS approval_workflows (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key         TEXT NOT NULL UNIQUE,
    label       TEXT NOT NULL,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS approval_nodes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id     UUID NOT NULL REFERENCES approval_workflows(id) ON DELETE CASCADE,
    sequence        INTEGER NOT NULL,
    label           TEXT NOT NULL,
    required_role   TEXT NOT NULL,
    UNIQUE (workflow_id, sequence)
);

-- One row per (node, condition). A node is included for a given
-- requisition if ANY of its conditions match (OR semantics).
CREATE TABLE IF NOT EXISTS approval_rule_conditions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    node_id         UUID NOT NULL REFERENCES approval_nodes(id) ON DELETE CASCADE,
    condition_kind  TEXT NOT NULL CHECK (condition_kind IN
        ('always','amount_gt_cents','contains_controlled')),
    threshold_cents BIGINT,
    metadata        JSONB
);

CREATE TABLE IF NOT EXISTS approval_role_bindings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id     UUID NOT NULL REFERENCES approval_workflows(id) ON DELETE CASCADE,
    role_key        TEXT NOT NULL,
    UNIQUE (workflow_id, role_key)
);

-- =========================================================================
-- Requisitions
-- =========================================================================
CREATE TABLE IF NOT EXISTS requisitions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ref_code            TEXT NOT NULL UNIQUE,
    requester_id        UUID NOT NULL REFERENCES users(id),
    site_id             UUID NOT NULL REFERENCES sites(id),
    department_id       UUID NOT NULL REFERENCES departments(id),
    status              TEXT NOT NULL CHECK (status IN
        ('draft','pending_approval','sent_back','approved_final','rejected','withdrawn','issued')),
    needed_by           DATE NOT NULL,
    justification       TEXT NOT NULL,
    total_amount_cents  BIGINT NOT NULL DEFAULT 0,
    contains_controlled BOOLEAN NOT NULL DEFAULT FALSE,
    workflow_id         UUID REFERENCES approval_workflows(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    submitted_at        TIMESTAMPTZ,
    finalized_at        TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_req_requester ON requisitions(requester_id);
CREATE INDEX IF NOT EXISTS idx_req_status ON requisitions(status);
CREATE INDEX IF NOT EXISTS idx_req_dept ON requisitions(department_id);

CREATE TABLE IF NOT EXISTS requisition_lines (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requisition_id      UUID NOT NULL REFERENCES requisitions(id) ON DELETE CASCADE,
    item_id             UUID NOT NULL REFERENCES inventory_items(id),
    quantity            INTEGER NOT NULL CHECK (quantity > 0),
    unit_price_cents    BIGINT NOT NULL CHECK (unit_price_cents >= 0),
    line_total_cents    BIGINT NOT NULL CHECK (line_total_cents >= 0)
);
CREATE INDEX IF NOT EXISTS idx_req_lines_req ON requisition_lines(requisition_id);

-- Approval execution: instance per submission, one row per node it routed through.
CREATE TABLE IF NOT EXISTS requisition_approval_instances (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requisition_id  UUID NOT NULL REFERENCES requisitions(id) ON DELETE CASCADE,
    workflow_id     UUID NOT NULL REFERENCES approval_workflows(id),
    current_step    INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'in_progress'
        CHECK (status IN ('in_progress','approved','rejected','sent_back','withdrawn')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS requisition_approval_steps (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id     UUID NOT NULL REFERENCES requisition_approval_instances(id) ON DELETE CASCADE,
    sequence        INTEGER NOT NULL,
    node_id         UUID NOT NULL REFERENCES approval_nodes(id),
    required_role   TEXT NOT NULL,
    decision        TEXT CHECK (decision IN ('approved','rejected','sent_back')),
    decided_by      UUID REFERENCES users(id),
    decided_at      TIMESTAMPTZ,
    reason          TEXT,
    UNIQUE (instance_id, sequence)
);

-- =========================================================================
-- Immutable audit timeline
-- =========================================================================
CREATE TABLE IF NOT EXISTS requisition_audit (
    id              BIGSERIAL PRIMARY KEY,
    requisition_id  UUID NOT NULL REFERENCES requisitions(id) ON DELETE CASCADE,
    occurred_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_user_id   UUID REFERENCES users(id),
    event_kind      TEXT NOT NULL,
    comment         TEXT,
    payload         JSONB
);
CREATE INDEX IF NOT EXISTS idx_req_audit_req ON requisition_audit(requisition_id, occurred_at);

-- Append-only enforcement: a trigger blocks UPDATE and DELETE so the
-- audit timeline cannot be rewritten by application code or by an
-- accidental psql session.
CREATE OR REPLACE FUNCTION requisition_audit_immutable()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'requisition_audit is append-only (% rejected)', TG_OP;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS requisition_audit_no_update ON requisition_audit;
CREATE TRIGGER requisition_audit_no_update
    BEFORE UPDATE OR DELETE ON requisition_audit
    FOR EACH ROW EXECUTE FUNCTION requisition_audit_immutable();

-- =========================================================================
-- Issue records (created in the SAME tx as final approval + stock deduction)
-- =========================================================================
CREATE TABLE IF NOT EXISTS issue_records (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requisition_id  UUID NOT NULL UNIQUE REFERENCES requisitions(id),
    site_id         UUID NOT NULL REFERENCES sites(id),
    issued_by       UUID NOT NULL REFERENCES users(id),
    issued_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS issue_record_lines (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_record_id     UUID NOT NULL REFERENCES issue_records(id) ON DELETE CASCADE,
    item_id             UUID NOT NULL REFERENCES inventory_items(id),
    quantity            INTEGER NOT NULL CHECK (quantity > 0),
    unit_price_cents    BIGINT NOT NULL,
    line_total_cents    BIGINT NOT NULL
);
