-- B5: playback health log, stats cache, private recommendations and admin web sessions.
-- All timestamps are Unix milliseconds. Disabled users keep `active` so cleanup never deletes them.

ALTER TABLE users ADD COLUMN disabled_at BIGINT;

CREATE TABLE health_reports (
    id BIGSERIAL PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id UUID NOT NULL,
    kind TEXT NOT NULL,
    category TEXT NOT NULL,
    client_version TEXT NOT NULL,
    platform TEXT NOT NULL,
    at BIGINT NOT NULL,
    duration_ms INT,
    received_at BIGINT NOT NULL
);
CREATE INDEX health_reports_user_at ON health_reports(user_id, at);
CREATE INDEX health_reports_at ON health_reports(at);

-- Stats read history events by their own `at`; `::numeric` never fails for a JSON number.
CREATE INDEX sync_records_history_at ON sync_records (user_id, ((value->>'at')::numeric))
    WHERE collection = 'history' AND value IS NOT NULL;

CREATE TABLE stats_wrapped (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    year INT NOT NULL,
    tz_offset_min INT NOT NULL,
    computed_at BIGINT NOT NULL,
    value JSONB NOT NULL,
    PRIMARY KEY (user_id, year, tz_offset_min)
);

CREATE TABLE recs_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    music BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at BIGINT NOT NULL
);
CREATE TABLE recs_feed (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    built_at BIGINT NOT NULL,
    items JSONB NOT NULL
);
-- Public upstream responses shared by all users (no user data in keys or values).
CREATE TABLE recs_cache (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    fetched_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL
);

CREATE TABLE admin_sessions (
    token_hash BYTEA PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    csrf TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    last_used BIGINT NOT NULL,
    expires_at BIGINT NOT NULL
);
