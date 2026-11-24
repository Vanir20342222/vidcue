-- B1: accounts, invites, devices, pairing and the per-user sync record store.
-- All timestamps are Unix milliseconds.

CREATE TABLE users (
    id UUID PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    totp_secret BYTEA NOT NULL,
    -- Last accepted TOTP time step; codes at or below it are replays.
    totp_last_step BIGINT NOT NULL DEFAULT 0,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    -- FALSE until the authenticator code was confirmed during signup.
    active BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL
);

CREATE TABLE invites (
    token_hash BYTEA PRIMARY KEY,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    used_by UUID REFERENCES users(id) ON DELETE SET NULL,
    used_at BIGINT
);

CREATE TABLE devices (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_hash BYTEA NOT NULL UNIQUE,
    created_at BIGINT NOT NULL,
    last_seen BIGINT NOT NULL,
    revoked_at BIGINT
);
CREATE INDEX devices_user ON devices(user_id);

CREATE TABLE pairings (
    code_hash BYTEA PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_by_device UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    expires_at BIGINT NOT NULL,
    used_at BIGINT
);

-- One global sequence; pushes for a user are serialised with an advisory lock, so a
-- user's records become visible in sequence order and `seq > since` pulls never skip.
CREATE SEQUENCE sync_seq;

CREATE TABLE sync_records (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    collection TEXT NOT NULL,
    key TEXT NOT NULL,
    -- NULL is a tombstone.
    value JSONB,
    clock BIGINT NOT NULL,
    device_id UUID NOT NULL,
    seq BIGINT NOT NULL,
    PRIMARY KEY (user_id, collection, key)
);
CREATE INDEX sync_records_pull ON sync_records(user_id, seq);

-- Idempotency: a retried push of the same operation is acknowledged, not reapplied.
CREATE TABLE sync_applied (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    op_id UUID NOT NULL,
    applied_at BIGINT NOT NULL,
    PRIMARY KEY (user_id, op_id)
);
