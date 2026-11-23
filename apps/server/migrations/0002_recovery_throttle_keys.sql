-- B2: persistent throttling, account recovery and the E2E key relay.

-- Failed attempts per key ("login:<user>", "pairing", ...). Survives restarts.
CREATE TABLE auth_failures (
    key TEXT NOT NULL,
    at BIGINT NOT NULL
);
CREATE INDEX auth_failures_key_at ON auth_failures(key, at);

-- One-time recovery codes, usable instead of an authenticator code.
CREATE TABLE recovery_codes (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash BYTEA NOT NULL,
    used_at BIGINT,
    PRIMARY KEY (user_id, code_hash)
);

-- Authenticator replacement started from a signed-in device; active after a code confirms it.
ALTER TABLE users ADD COLUMN totp_pending BYTEA;
ALTER TABLE users ADD COLUMN totp_pending_at BIGINT;

-- Admin-issued reset links: new password and authenticator, all devices signed out.
CREATE TABLE account_resets (
    token_hash BYTEA PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    -- Filled when the user submits the new password; applied on code confirmation.
    password_hash TEXT,
    totp_secret BYTEA,
    used_at BIGINT
);

-- A device without the account's end-to-end key asks another device for it. The server
-- only relays an X25519 public key and the sealed key; it can read neither secret.
CREATE TABLE key_requests (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    public_key TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    sealed_key TEXT,
    granted_by UUID REFERENCES devices(id) ON DELETE SET NULL
);
CREATE INDEX key_requests_user ON key_requests(user_id);
