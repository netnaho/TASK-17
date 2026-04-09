-- Phase 5: master data CRUD + secure import/export
-- Table creation order respects FK dependencies:
--   semesters → departments (existing) → classes → students, courses

-- Semesters ----------------------------------------------------------------
CREATE TABLE semesters (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    institution_id UUID NOT NULL REFERENCES institutions(id),
    code           TEXT NOT NULL,
    label          TEXT NOT NULL,
    starts_on      DATE NOT NULL,
    ends_on        DATE NOT NULL,
    is_active      BOOLEAN NOT NULL DEFAULT true,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (institution_id, code),
    CHECK (ends_on > starts_on)
);

-- Classes (cohort / study group) -------------------------------------------
CREATE TABLE classes (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    institution_id UUID NOT NULL REFERENCES institutions(id),
    code           TEXT NOT NULL,
    label          TEXT NOT NULL,
    semester_id    UUID REFERENCES semesters(id),
    department_id  UUID REFERENCES departments(id),
    is_active      BOOLEAN NOT NULL DEFAULT true,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (institution_id, code)
);

-- Courses ------------------------------------------------------------------
CREATE TABLE courses (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    institution_id UUID NOT NULL REFERENCES institutions(id),
    code           TEXT NOT NULL,
    title          TEXT NOT NULL,
    department_id  UUID REFERENCES departments(id),
    credits        INT NOT NULL DEFAULT 0 CHECK (credits >= 0),
    is_active      BOOLEAN NOT NULL DEFAULT true,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (institution_id, code)
);

-- Students -----------------------------------------------------------------
CREATE TABLE students (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    institution_id UUID NOT NULL REFERENCES institutions(id),
    student_number TEXT NOT NULL,
    first_name     TEXT NOT NULL,
    last_name      TEXT NOT NULL,
    email          TEXT,
    date_of_birth  DATE,
    class_id       UUID REFERENCES classes(id),
    is_active      BOOLEAN NOT NULL DEFAULT true,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (institution_id, student_number)
);

-- Import jobs (one per file upload) ----------------------------------------
CREATE TABLE import_jobs (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type      TEXT NOT NULL
                       CHECK (entity_type IN ('students','classes','courses','semesters','departments')),
    institution_id   UUID NOT NULL REFERENCES institutions(id),
    uploaded_by      UUID NOT NULL REFERENCES users(id),
    file_name        TEXT NOT NULL,
    file_size_bytes  BIGINT NOT NULL,
    file_fingerprint TEXT NOT NULL,          -- sha256 hex of raw bytes
    mime_type        TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'processing'
                       CHECK (status IN ('processing','done','failed')),
    total_rows       INT NOT NULL DEFAULT 0,
    accepted_rows    INT NOT NULL DEFAULT 0,
    rejected_rows    INT NOT NULL DEFAULT 0,
    error_message    TEXT,
    completed_at     TIMESTAMPTZ,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Per-row import results ---------------------------------------------------
CREATE TABLE import_job_rows (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id           UUID NOT NULL REFERENCES import_jobs(id) ON DELETE CASCADE,
    row_number       INT NOT NULL,
    status           TEXT NOT NULL CHECK (status IN ('accepted','rejected')),
    raw_data         JSONB NOT NULL,
    rejection_reason TEXT,
    entity_id        UUID            -- populated for accepted rows
);

-- File fingerprint registry: prevents re-ingestion of identical files -------
CREATE TABLE file_fingerprints (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fingerprint    TEXT NOT NULL,
    entity_type    TEXT NOT NULL,
    institution_id UUID NOT NULL REFERENCES institutions(id),
    import_job_id  UUID REFERENCES import_jobs(id),
    first_seen_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (fingerprint, entity_type, institution_id)
);

-- Export audit log ---------------------------------------------------------
CREATE TABLE export_logs (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type    TEXT NOT NULL,
    institution_id UUID NOT NULL REFERENCES institutions(id),
    exported_by    UUID NOT NULL REFERENCES users(id),
    format         TEXT NOT NULL CHECK (format IN ('csv','xlsx')),
    row_count      INT NOT NULL,
    file_name      TEXT NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes ------------------------------------------------------------------
CREATE INDEX idx_import_jobs_institution  ON import_jobs(institution_id, entity_type);
CREATE INDEX idx_import_job_rows_job      ON import_job_rows(job_id);
CREATE INDEX idx_fingerprints_lookup      ON file_fingerprints(fingerprint, entity_type, institution_id);
CREATE INDEX idx_students_institution     ON students(institution_id);
CREATE INDEX idx_classes_institution      ON classes(institution_id);
CREATE INDEX idx_courses_institution      ON courses(institution_id);
CREATE INDEX idx_semesters_institution    ON semesters(institution_id);
