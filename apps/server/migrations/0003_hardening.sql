-- Security hardening after the 2026-11-23 review.

-- Attempts are recorded before checking and removed on success, so rows need an id.
ALTER TABLE auth_failures ADD COLUMN id BIGSERIAL PRIMARY KEY;

-- Which device paired this one: revoking a device also revokes devices it added.
ALTER TABLE devices ADD COLUMN created_by UUID REFERENCES devices(id) ON DELETE SET NULL;
CREATE INDEX devices_created_by ON devices(created_by);

-- Recovery codes are now keyed HMACs bound to the user; unkeyed SHA-256 rows cannot be
-- verified any more and are dropped (users regenerate them).
DELETE FROM recovery_codes;

CREATE INDEX sync_applied_age ON sync_applied(applied_at);
CREATE INDEX pairings_user ON pairings(user_id);
