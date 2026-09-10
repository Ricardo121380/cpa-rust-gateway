# Prism administrator password login

Status: accepted and implemented by Codex under the user's full-stack authorization.

The user requested a quieter V4 login page with an administrator username and password.
Bootstrap uses `admin`, a random initial password delivered through an owner-only local
file, and mandatory password change before access to management resources.

## Contract

- `POST /admin/auth/login`: bounded JSON username/password; exact configured Origin and
  actual management peer policy; no pre-existing key or CSRF required. Uniform 401 for
  incorrect credentials, 429 for the bounded login budget, 503 when not initialized.
- Returns a random opaque session, independent CSRF value, username, expiry and
  `password_change_required`. This is never the long-lived Management Key.
- `POST /admin/auth/password`: current/new password, session and CSRF required. Changing
  the password revokes all sessions and requires a new login. Initial sessions may only
  change the password or log out; all existing management resource routes remain denied.
- `POST /admin/auth/logout`: revoke the presented session. Expired/revoked sessions use
  the existing `404 management_access_denied` so the client clears queries and secrets.
- Session transport reuses `X-Management-Key` with a distinct `session_` namespace.
  Unsafe session requests always require their session CSRF, including without Origin.
  Legacy origin-free CLI Management Key access stays compatible.
- Absolute sessions last eight hours, initial password sessions ten minutes; at most 32
  sessions. A shared server budget permits ten password verifications per minute, two
  concurrently. Forwarded IP headers are not trusted for identity or rate limiting.
- Passwords use Argon2id v19, 19 MiB, two iterations, one lane, a fresh 16-byte salt;
  accept 12–128 Unicode characters within 512 UTF-8 bytes, without trimming/truncation.

## Persistence and bootstrap

An owner-only `admin-auth.sqlite3` in the state directory holds a single administrator
hash and password revision. This authentication store is separate from configuration
backups and control database schema 22; restore of a configuration must not restore old
administrator passwords. No public self-registration or unattended default password.
The explicit local `gateway admin-login init` command initializes it once and writes a
random password to a new mode-0600 file. Existing files/accounts are never overwritten.
Hashing and database work run outside the HTTP worker. Sessions are memory-only and die
on restart. Browser session material remains memory-only; refresh requires login.

## Acceptance

Real gateway: wrong credentials/origin/CSRF, oversized input, throttling, mandatory
first change, revoked old sessions, restart persistence, logout, CLI compatibility,
late login/old-session responses. UI: concise login, password visibility/autocomplete,
first change validation, mobile keyboard/overflow, dark/contrast/motion preferences.
No Provider inference is needed. Existing domain deployment and rollback apply only
to CPAR; DNS, Caddy routes, other services and historical billing data stay intact.
