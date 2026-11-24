# Encrypted off-site backups and a release JDK guard (B5)

- **Date:** 2026-11-24
- **PR:** <!-- add once the PR exists -->
- **Milestone:** B5 (operations), M9 (release tooling)
- **Platforms:** backend deployment, release tooling
- **Status:** live verified

## What it does
- `deploy/vidcue-backup.sh` also writes every nightly dump and the server key age-encrypted to
  `/var/backups/vidcue/offsite` when `/etc/vidcue/offsite-recipient.pub` lists recipients
  (the owner's SSH ed25519 keys). `deploy/vidcue-offsite-send.sh` serves only those encrypted
  files to a pulling machine through a forced SSH command; the installer installs it and `age`.
  The owner's PC pulls nightly and keeps 30 dumps; plaintext never leaves the server.
- `tools/android/sign-local.py` refuses to start with a JDK outside 17–24, which Gradle 8.14
  cannot run on; `docs/android/SIGNING.md` explains the `JAVA_HOME` workaround.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Backup + encryption | Ran the backup in the deployed container | dump and `secret-key.age` written, mode 600 | live |
| Forced command | `list` works; arbitrary commands refused (exit 2) | as intended | live |
| Pull + decrypt | Pulling machine fetched the dump and key; `age -d` of the dump starts with `PGDMP`, key 65 bytes | passed | live |
| Tool tests | `python3 -I -m unittest discover -s tools/android -p 'test_*.py'` incl. new JDK parse test | OK | fixture/unit |

## Not verified and limits
- A full restore of a pulled dump into PostgreSQL was not rehearsed.
- Retention pruning on the pulling side is untested until more than 30 dumps exist.

## For the plan and parity tables
- Backend backups: nightly, age-encrypted off-site copy on the owner's PC (2026-11-24).
