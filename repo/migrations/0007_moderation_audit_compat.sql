-- Migration 0007: align moderation_audit_log with service contract.
--
-- Background
-- ----------
-- The service layer writes two kinds of audit records:
--
--   1. "flagged" events (check_and_flag): carry structured metadata —
--      matched_keywords[], detected severity, and content_type — that is
--      naturally JSONB, not a flat TEXT note.
--
--   2. "review" events (review_item): carry a human-readable reviewer note
--      (fits `note TEXT`) and a reviewer identity (fits `performed_by UUID`).
--
-- The original schema (0006) only defined `performed_by UUID` and `note TEXT`.
-- The service was inserting into the nonexistent columns `actor_user_id` and
-- `detail`, causing every audit INSERT to fail at runtime.
--
-- Fix (additive, non-breaking)
-- ----------------------------
-- Add an optional `detail JSONB` column.  Existing rows receive NULL, the
-- `note TEXT` and `performed_by UUID` columns are untouched, and no existing
-- data is migrated or removed.  Clients that query the table by the old
-- columns continue to work unchanged.

ALTER TABLE moderation_audit_log
    ADD COLUMN IF NOT EXISTS detail JSONB;
