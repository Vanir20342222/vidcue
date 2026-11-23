# vidcue-server

Self-hosted Vidcue backend. Plan and decisions: [docs/backend/PLAN.md](../../docs/backend/PLAN.md).

## Run

```sh
DATABASE_URL="host=/run/postgresql user=vidcue dbname=vidcue" \
VIDCUE_PUBLIC_URL=https://vidcue.example.org \
vidcue-server serve --bind 127.0.0.1:8080
vidcue-server invite --admin      # prints a one-time signup link (default 72 h)
vidcue-server reset-link alice    # 48 h link: new password + authenticator, signs out all devices
```

Migrations run on every start. `deploy/install.sh` (`PUBLIC_URL=… PROXY_IP=… sh install.sh ./vidcue-server`)
installs the binary, a sandboxed systemd unit with memory/task limits, a
peer-authenticated Postgres role, an nftables rule that lets only the reverse proxy
reach the service port, and a nightly `pg_dump` timer (`/var/backups/vidcue`, 14 kept).

**Server key.** `serve` needs a 32-byte key (`vidcue-server gen-key`; the installer
creates `/etc/vidcue/secret.key`, root-only, passed in as a systemd credential). It
encrypts authenticator seeds and keys the recovery-code hashes, so a database dump
alone reveals neither. Back it up *separately* from the database: without it,
existing authenticators stop working and every user needs a reset link.

| Variable | Meaning |
| --- | --- |
| `VIDCUE_TRUSTED_PROXIES` | Proxy addresses whose `X-Real-IP` is trusted (throttles are per client address) |
| `VIDCUE_PROXIES_ONLY` | `true`: refuse direct connections from anything but those proxies and loopback |
| `VIDCUE_SECRET_KEY_FILE` | Key file when not run under systemd |

## API (B1, B2)

All bodies are JSON. Authenticated calls send `Authorization: Bearer <device token>`.

| Method and path | Purpose |
| --- | --- |
| `GET /join#<invite>` | Browser signup page; the token stays in the URL fragment, which browsers never send, so it never reaches proxy logs |
| `POST /api/signup` `{invite, username, password}` | Starts signup, returns TOTP secret, `otpauth://` URI and QR SVG |
| `POST /api/signup/confirm` `{invite, signup_id, code, device_name?}` | Activates the account; with `device_name` also returns a device token |
| `POST /api/login` `{username, password, code, device_name?}` | Returns a device token |
| `GET /api/me`, `GET /api/devices`, `DELETE /api/devices/{id}` | Account and devices; revoking a device also revokes every device it paired. At most 20 devices; tokens unused for 180 days expire |
| `POST /api/pairing` `{password}` | Single-use 10-minute pairing code (at most 5 open), `vidcue://pair?server=…&code=…` URI and QR SVG |
| `POST /api/pairing/claim` `{code, device_name?}` | New device of the same account gets a token |
| `POST /api/sync/push` `{ops: [{id, collection, key, value?, clock}]}` | Per-op `applied` / `superseded` / `duplicate` / `rejected` |
| `GET /api/sync/pull?since=&limit=` | Changes after `since`, `next`, `more` |
| `POST /api/ws/ticket` | 30-second single-use ticket for `GET /api/ws?ticket=…` (device tokens are never accepted in URLs) |
| `GET /api/ws` | Bearer header or `?ticket=`; at most 3 sockets per device, server-to-client only. Events: `hello {seq}`, `changed {seq, device_id}`, `device_added`, `device_revoked`, `devices_revoked {keep?}`, `key_request`, `key_granted` |
| `POST /api/admin/invites` `{password, admin?, hours?}`, `GET /api/admin/users` | Admin only |
| `POST /api/admin/users/{id}/reset` `{password, code}` | Admin only: reset link for a user |
| `GET /reset#<token>`, `POST /api/reset` `{token, password}`, `POST /api/reset/confirm` `{token, code}` | Reset flow; confirm signs out every device and returns new recovery codes |
| `GET /api/account/recovery-codes`, `POST /api/account/recovery-codes` `{password, code}` | Remaining count; regenerate (shown once) |
| `POST /api/account/password` `{current_password, code, new_password, sign_out_others?}` | Change password |
| `POST /api/account/totp` `{password, code}`, `POST /api/account/totp/confirm` `{code}` | Replace the authenticator |
| `POST /api/keys/requests` `{public_key}`, `GET /api/keys/requests`, `GET /api/keys/requests/{id}`, `POST /api/keys/requests/{id}/grant` `{sealed_key}` | E2E key relay for devices that signed in without QR pairing |

`code` is always an authenticator code or a one-time recovery code (`XXXXX-XXXXX`).
Signup confirmation returns ten recovery codes.

**Throttling** (PostgreSQL `auth_failures`, survives restarts; each attempt is recorded
before the slow check runs, so parallel requests cannot slip past the limit):
logins 10 per address / 15 min, 5 per user and address / 15 min, 100 per user / hour;
5 wrong second factors per user / hour after a correct password; invite, pairing and
reset codes 30 per address / 10 min; re-authentication 5 per user / 15 min. At most 4
password hashes run at once (503 when busy). Per user: 100 000 records, 64 MiB.

### Sync collections

| Collection | Key | Value |
| --- | --- | --- |
| `positions` | `PlaybackHistory::key` | `vidcue_core::playback_history::Position`, online media only |
| `history` | event id | object with an online `url` |
| `saved` | `PlaybackHistory::key` | `Media` |
| `playlists` | playlist UUID | `{title, ...}` |
| `playlist_entries` | `<playlist UUID>/<entry UUID>` | `{position, media}`; `position` is a fractional index string (`[0-9A-Za-z]{1,64}`), so concurrent inserts and moves never conflict |
| `offline_playlists` | playlist id | `vidcue_core::offline::OfflinePlaylist` (metadata; files stay on the device) |
| `collection_watches` | watch id | `vidcue_core::watch::CollectionWatch`; its download options must leave hook, folders, template and endpoints at their defaults |
| `subscriptions` | channel id | `{channel_id, title, ...}` |
| `settings` | `Config` field name | field value; only portable fields (see `PORTABLE_SETTINGS`), validated through `Config::validate`. `download` accepts only `PORTABLE_DOWNLOAD_FIELDS`: the post-download hook, folders, filename template and service endpoints never sync |
| `eq_presets` | preset name | `{gains: [10 × dB]}` |
| `filters` | UUID | `{kind: channel\|keyword\|shorts\|watched\|live, value?}` |
| `secrets` | provider (`youtube`, ...) | `{v: 1, alg: "xchacha20poly1305", nonce, ciphertext}`, base64; encrypted on the device with the account key, never readable by the server |

### End-to-end key

Provider sessions are encrypted with a 32-byte account key that only devices hold.
QR pairing carries it in the URI fragment that the app adds locally
(`vidcue://pair?server=…&code=…#k=<key>`); the server only sees the code. A device
that signed in with a password posts an X25519 public key to `/api/keys/requests`;
a trusted device shows a fingerprint of that key, the user compares it with the
fingerprint on the new device, and the trusted device posts the key sealed to it.

## Tests

`cargo test -p vidcue-server` runs offline unit tests. The end-to-end API test needs a
disposable database and skips without one:

```sh
VIDCUE_TEST_DATABASE_URL=postgres://user:pass@127.0.0.1/vidcue_test cargo test -p vidcue-server
```

It drops and recreates the `public` schema of that database.
