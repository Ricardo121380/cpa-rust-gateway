# Autoreg Jakarta → Oracle Singapore migration receipt

Date: 2026-08-10 (Asia/Shanghai)
Change request: `CR-P12-AUTOREG-MIGRATE-001`

## Boundary

This receipt covers the staged successor deployment of the Autoreg project on the
Oracle Singapore ARM64 VPS. It does not authorize a public DNS/Caddy cutover and it
does not stop the Jakarta predecessor. CPAR, the old CPA, Caddy, DNS, CC Switch, and
the existing CPAR credential pool were not changed.

## Transfer evidence

| Item | Result |
|---|---|
| Source project tree, including dirty state, SQLite console DB, task directories, and runtime state | `PASS` |
| Source project revision recorded | `9b7e5f8`, dirty worktree preserved |
| CPA auth files copied to the dedicated Autoreg pool | `PASS` (843 files; no automatic merge into CPAR) |
| CloudMail environment files copied | `PASS` (root-owned, mode `0600`) |
| Source-side archive creation | Not used; Jakarta disk was near capacity |
| Transfer method | SSH stream into root-owned Oracle paths; no local plaintext archive |

Oracle paths:

- `/opt/example-autoreg/project`
- `/opt/example-autoreg/cliproxyapi-auths`
- `/opt/example-autoreg/cloudmail`

The Oracle target also has a root-only rollback preimage under
`/var/backups/autoreg-oracle/20260810T041926Z/`. The backup excludes the large
browser cache and separately covers project data, the dedicated auth pool, and
CloudMail configuration.

## ARM64 runtime evidence

The source Dockerfile pins an amd64 Google Chrome repository. The Oracle overlay
therefore installs ARM-compatible Playwright/CloakBrowser Chromium and builds a
native ARM64 Privoxy sidecar from Debian rather than using the amd64-only public
Privoxy image. The stack is managed by `autoreg-oracle.service`.

| Component | Listener / check | Result |
|---|---|---|
| WARP | loopback `21180`, container health | `healthy` |
| Privoxy | loopback `24080`, container health | `healthy` |
| FlareSolverr | loopback `28191`, `/health` | HTTP `200` |
| Autoreg console | loopback `18650`, unauthenticated API | HTTP `401` (protected) |
| Autoreg console | authenticated `/api/health` | HTTP `200` |
| Autoreg console | authenticated `/api/meta` | HTTP `200` |
| Autoreg console | authenticated `/api/platforms` | HTTP `200` |
| Oracle inspection scheduler | copied value was enabled; fenced to `false` before canary | `PASS` |
| Oracle console SQLite | `PRAGMA quick_check` after scheduler fence | `ok` |
| CPAR service | existing `/healthz` | HTTP `200` (unchanged) |
| Jakarta Autoreg predecessor | existing console container | still running |

The proxy-only request to the Grok site returned an upstream HTTP `403`; this is a
network/egress observation only and is not counted as account authentication or
registration success.

## Operating policy

Autoreg is the account registration/re-authentication tool for Grok-related native
pools, not a new Provider, CPAR upstream, or public data plane. Newly generated
credentials enter CPAR only through an explicit import and provider binding; the
migration does not silently copy them into CPAR production. A new account must be
classified by its actual envelope and explicitly bound to Grok Build OAuth, Console
SSO, or Web session native pool; those provider shapes cannot be inferred from a
plan name, exchanged, or used for cross-pool fallback.

Separately, the official Codex/ChatGPT account channel is intentionally plan- and
envelope-agnostic: any plan may use CPA JSON, Sub2API JSON, or official OAuth. This
is a Codex channel rule and does not turn Autoreg into a Codex provider.

For GitHub, the optimization applies only to expensive Actions verification runs.
Normal branch creation, `git push`, PR creation/update, review, and merge cadence
remain available as needed. A Fast/Full/release evidence run is reserved for phase
closeout, a protected merge gate, or an explicit manual dispatch; reducing those
runs does not throttle Git operations.

## Rollback

Keep the Jakarta service online until an operator accepts the Oracle successor. A
safe rollback is to stop only the `autoreg-oracle` compose project and restore the
Oracle preimage if needed. No source data was deleted and no production CPAR route
was changed by this migration.
