-- Migration 0006: compliance, family portal, analytics, wellness, ID mapping,
--                 content moderation, and anomaly detection tables.

-- ---------------------------------------------------------------------------
-- Family portal + consent
-- ---------------------------------------------------------------------------

-- family_groups: links guardian users to residents (students)
CREATE TABLE family_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    institution_id UUID NOT NULL REFERENCES institutions(id),
    label TEXT NOT NULL,  -- e.g., "Smith Family"
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- family_group_members: which users are guardians in this group
CREATE TABLE family_group_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    family_group_id UUID NOT NULL REFERENCES family_groups(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'guardian',  -- guardian | viewer
    UNIQUE(family_group_id, user_id)
);

-- family_group_residents: which students this family group can see
CREATE TABLE family_group_residents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    family_group_id UUID NOT NULL REFERENCES family_groups(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    UNIQUE(family_group_id, student_id)
);

-- consent_records: explicit per-family-group consent for each data category
CREATE TABLE consent_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    family_group_id UUID NOT NULL REFERENCES family_groups(id) ON DELETE CASCADE,
    data_category TEXT NOT NULL,  -- 'supply_usage' | 'wellness_summary' | 'activity_log'
    consented BOOLEAN NOT NULL DEFAULT false,
    consented_at TIMESTAMPTZ,
    consented_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(family_group_id, data_category)
);

-- ---------------------------------------------------------------------------
-- Analytics
-- ---------------------------------------------------------------------------

CREATE TABLE analytics_daily (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    institution_id UUID NOT NULL REFERENCES institutions(id),
    stat_date DATE NOT NULL,
    stat_key TEXT NOT NULL,  -- 'requisition_count'|'order_count'|'supply_spend_cents'|'approval_avg_hours'
    stat_value NUMERIC NOT NULL DEFAULT 0,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(institution_id, stat_date, stat_key)
);

CREATE TABLE analytics_weekly (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    institution_id UUID NOT NULL REFERENCES institutions(id),
    week_start DATE NOT NULL,  -- Monday of the week
    stat_key TEXT NOT NULL,
    stat_value NUMERIC NOT NULL DEFAULT 0,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(institution_id, week_start, stat_key)
);

CREATE TABLE analytics_monthly (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    institution_id UUID NOT NULL REFERENCES institutions(id),
    year_month TEXT NOT NULL,  -- 'YYYY-MM'
    stat_key TEXT NOT NULL,
    stat_value NUMERIC NOT NULL DEFAULT 0,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(institution_id, year_month, stat_key)
);

-- Wellness activity log (for aggregation, summary generation)
CREATE TABLE wellness_activities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    institution_id UUID NOT NULL REFERENCES institutions(id),
    student_id UUID REFERENCES students(id) ON DELETE SET NULL,
    activity_type TEXT NOT NULL,  -- 'exercise'|'therapy'|'social'|'nutrition'
    duration_minutes INTEGER NOT NULL DEFAULT 0,
    notes_encrypted TEXT,  -- AES-256-GCM encrypted; format: base64(nonce):base64(ciphertext)
    recorded_by UUID REFERENCES users(id),
    activity_date DATE NOT NULL DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Personal bests: best-ever duration per student per activity type
CREATE TABLE wellness_personal_bests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    activity_type TEXT NOT NULL,
    best_duration_minutes INTEGER NOT NULL,
    achieved_on DATE NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(student_id, activity_type)
);

-- Wellness goals
CREATE TABLE wellness_goals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    activity_type TEXT NOT NULL,
    target_minutes_per_week INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Unified ID mapping
-- ---------------------------------------------------------------------------

CREATE TABLE entity_id_map (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,   -- 'student'|'user'|'department'|'course'
    internal_id UUID NOT NULL,
    external_system TEXT NOT NULL,  -- 'sis'|'hrm'|'legacy'
    external_id TEXT NOT NULL,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_type, external_system, external_id)
);

CREATE TABLE id_map_conflicts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,
    external_system TEXT NOT NULL,
    external_id TEXT NOT NULL,
    conflicting_internal_ids UUID[] NOT NULL,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved BOOLEAN NOT NULL DEFAULT false,
    resolved_at TIMESTAMPTZ,
    resolution_note TEXT
);

CREATE TABLE consistency_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    report_type TEXT NOT NULL,  -- 'orphan_check'|'id_map_gaps'|'referential_integrity'
    institution_id UUID REFERENCES institutions(id),
    findings JSONB NOT NULL DEFAULT '[]'::jsonb,
    finding_count INTEGER NOT NULL DEFAULT 0,
    run_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Content moderation
-- ---------------------------------------------------------------------------

CREATE TABLE keyword_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    keyword TEXT NOT NULL UNIQUE,
    severity TEXT NOT NULL DEFAULT 'medium',  -- 'low'|'medium'|'high'
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE moderation_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content_type TEXT NOT NULL,   -- 'requisition_note'|'wellness_note'|'import_comment'
    content_ref_id UUID,          -- references the source record
    content_snippet TEXT NOT NULL, -- sanitized excerpt (no full text stored here)
    matched_keywords TEXT[] NOT NULL DEFAULT '{}',
    severity TEXT NOT NULL DEFAULT 'medium',
    status TEXT NOT NULL DEFAULT 'pending',  -- 'pending'|'approved'|'rejected'|'escalated'
    flagged_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_by UUID REFERENCES users(id),
    reviewed_at TIMESTAMPTZ
);

CREATE TABLE moderation_audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    queue_item_id UUID NOT NULL REFERENCES moderation_queue(id),
    action TEXT NOT NULL,  -- 'flagged'|'approved'|'rejected'|'escalated'
    performed_by UUID REFERENCES users(id),
    note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Anomaly detection
-- ---------------------------------------------------------------------------

CREATE TABLE anomaly_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_key TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    description TEXT NOT NULL,
    threshold_count INTEGER NOT NULL DEFAULT 10,
    window_seconds INTEGER NOT NULL DEFAULT 60,
    severity TEXT NOT NULL DEFAULT 'medium',  -- 'low'|'medium'|'high'|'critical'
    is_active BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE anomaly_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_key TEXT NOT NULL,
    severity TEXT NOT NULL,
    subject_type TEXT NOT NULL,  -- 'ip'|'user'|'session'
    subject_value TEXT NOT NULL,
    detail JSONB NOT NULL DEFAULT '{}'::jsonb,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged BOOLEAN NOT NULL DEFAULT false,
    acknowledged_by UUID REFERENCES users(id),
    acknowledged_at TIMESTAMPTZ
);

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------

CREATE INDEX idx_family_groups_institution ON family_groups(institution_id);
CREATE INDEX idx_family_group_members_user ON family_group_members(user_id);
CREATE INDEX idx_consent_records_group ON consent_records(family_group_id);
CREATE INDEX idx_analytics_daily_inst_date ON analytics_daily(institution_id, stat_date);
CREATE INDEX idx_analytics_weekly_inst ON analytics_weekly(institution_id, week_start);
CREATE INDEX idx_analytics_monthly_inst ON analytics_monthly(institution_id, year_month);
CREATE INDEX idx_wellness_activities_student ON wellness_activities(student_id, activity_date);
CREATE INDEX idx_wellness_activities_inst ON wellness_activities(institution_id, activity_date);
CREATE INDEX idx_entity_id_map_internal ON entity_id_map(entity_type, internal_id);
CREATE INDEX idx_moderation_queue_status ON moderation_queue(status, flagged_at);
CREATE INDEX idx_anomaly_events_rule ON anomaly_events(rule_key, detected_at);
CREATE INDEX idx_anomaly_events_subject ON anomaly_events(subject_type, subject_value);
