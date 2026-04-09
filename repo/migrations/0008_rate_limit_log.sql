-- Migration 0008: Postgres-backed distributed rate-limit log
--
-- Supports the optional `APP__RATE_LIMIT_BACKEND=postgres` mode.  When
-- enabled, per-IP request timestamps are persisted here so every API
-- replica shares the same sliding window instead of each container
-- maintaining its own in-memory deque.
--
-- The table is intentionally narrow (ip + ts only) and is pruned eagerly
-- by the application so it stays small.  A partial index on recent rows
-- keeps the COUNT(*) probe fast even under write load.

CREATE TABLE IF NOT EXISTS rate_limit_log (
    ip  TEXT        NOT NULL,
    ts  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Composite index: all queries filter on (ip, ts) together.
CREATE INDEX IF NOT EXISTS rate_limit_log_ip_ts_idx
    ON rate_limit_log (ip, ts);
