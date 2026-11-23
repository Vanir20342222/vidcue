-- B4: friends, activity sharing, send-to-a-friend inbox, shared playlists and Jam rooms.
-- All timestamps are Unix milliseconds. Presence lives in memory only.

-- Friendships: one row per unordered pair, user_a < user_b (UUID byte order).
CREATE TABLE friendships (
    user_a UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_b UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    requested_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    accepted_at BIGINT,               -- NULL while pending
    created_at BIGINT NOT NULL,
    PRIMARY KEY (user_a, user_b),
    CHECK (user_a < user_b)
);
CREATE INDEX friendships_b ON friendships(user_b);

CREATE TABLE social_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    share_activity TEXT NOT NULL DEFAULT 'off'
        CHECK (share_activity IN ('off','now_playing','history')),
    updated_at BIGINT NOT NULL
);

CREATE TABLE inbox (
    id UUID PRIMARY KEY,
    to_user UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    from_user UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    media JSONB NOT NULL,             -- slim online Media
    note TEXT NOT NULL DEFAULT '',
    created_at BIGINT NOT NULL,
    read_at BIGINT
);
CREATE INDEX inbox_to ON inbox(to_user, created_at);

-- Shared playlist changes are serialised per playlist (advisory lock), so a member's
-- `seq > since` pull never skips an entry.
CREATE SEQUENCE shared_seq;
CREATE TABLE shared_playlists (
    id UUID PRIMARY KEY,
    owner UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    seq BIGINT NOT NULL               -- bumped on title/member changes
);
CREATE TABLE shared_playlist_members (
    playlist_id UUID NOT NULL REFERENCES shared_playlists(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    added_at BIGINT NOT NULL,
    PRIMARY KEY (playlist_id, user_id)
);
CREATE INDEX shared_playlist_members_user ON shared_playlist_members(user_id);
CREATE TABLE shared_playlist_entries (
    playlist_id UUID NOT NULL REFERENCES shared_playlists(id) ON DELETE CASCADE,
    entry_id UUID NOT NULL,
    value JSONB,                      -- {position, media}; NULL is a tombstone
    clock BIGINT NOT NULL,
    device_id UUID NOT NULL,
    user_id UUID NOT NULL,            -- author of the winning write (no FK: survives removal)
    seq BIGINT NOT NULL,
    PRIMARY KEY (playlist_id, entry_id)
);
CREATE INDEX shared_playlist_entries_seq ON shared_playlist_entries(playlist_id, seq);
CREATE TABLE shared_applied (
    playlist_id UUID NOT NULL REFERENCES shared_playlists(id) ON DELETE CASCADE,
    op_id UUID NOT NULL,
    applied_at BIGINT NOT NULL,
    PRIMARY KEY (playlist_id, op_id)
);

CREATE TABLE jam_rooms (
    id UUID PRIMARY KEY,
    host UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    guests_can_control BOOLEAN NOT NULL DEFAULT FALSE,
    playback JSONB NOT NULL,          -- JamPlayback
    version BIGINT NOT NULL DEFAULT 1,-- bumped on every change of anything in the room
    created_at BIGINT NOT NULL,
    last_active BIGINT NOT NULL,
    ended_at BIGINT
);
CREATE INDEX jam_rooms_host ON jam_rooms(host);
CREATE TABLE jam_members (
    room_id UUID NOT NULL REFERENCES jam_rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('host','guest')),
    joined_at BIGINT NOT NULL,
    PRIMARY KEY (room_id, user_id)
);
CREATE INDEX jam_members_user ON jam_members(user_id);
CREATE TABLE jam_invites (
    room_id UUID NOT NULL REFERENCES jam_rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invited_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (room_id, user_id)
);
CREATE INDEX jam_invites_user ON jam_invites(user_id);
CREATE TABLE jam_queue (
    room_id UUID NOT NULL REFERENCES jam_rooms(id) ON DELETE CASCADE,
    item_id UUID NOT NULL,
    media JSONB NOT NULL,             -- slim online Media
    added_by UUID NOT NULL,
    added_at BIGINT NOT NULL,
    PRIMARY KEY (room_id, item_id)
);
CREATE TABLE jam_votes (
    room_id UUID NOT NULL,
    item_id UUID NOT NULL,
    user_id UUID NOT NULL,
    PRIMARY KEY (room_id, item_id, user_id),
    FOREIGN KEY (room_id, item_id) REFERENCES jam_queue(room_id, item_id) ON DELETE CASCADE
);
