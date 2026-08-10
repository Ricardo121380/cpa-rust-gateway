# Autoreg Jakarta → Oracle Singapore migration receipt

Date: 2026-08-10 (Asia/Shanghai)
Change request: `CR-P12-AUTOREG-MIGRATE-001`

## Boundary

This receipt covers the staged successor deployment of the Autoreg project on the
Oracle Singapore ARM64 VPS. It does not authorize a public DNS/Caddy cutover and it
does not stop the Jakarta predecessor. There was no intentional cutover or
persistent change to CPAR's active graph, the pre-existing old CPA pool, Caddy,
DNS, CC Switch, or the existing CPAR credential pool. One canary artifact briefly
followed the inherited old API sink and is contained in the canary section below.

## Transfer evidence

| Item | Result |
|---|---|
| Source project tree, including dirty state, SQLite console DB, task directories, and runtime state | `PASS` |
| Source project revision recorded | `9b7e5f8`, dirty worktree preserved |
| CPA auth files copied to the dedicated Autoreg pool | `PASS` (843-file baseline; one isolated canary artifact was added later, no automatic merge into CPAR) |
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

## Canary and sink-boundary evidence

The first Oracle registration canary was deliberately bounded to one account, one
worker, and one attempt. Two earlier runs stopped before registration because of
deployment defects (an empty host CloakBrowser mount and inherited Jakarta
proxy/FlareSolverr ports); neither changed an auth pool. After removing the empty
browser mount and remapping the Oracle-local ports, the canary completed with
`success=true`, `oauth_ok=true`, `cpa_ok=true`, and the optional legacy sink
disabled.

The canary exposed an important migration boundary defect: the copied source config
still had `cpa.prefer=api`, `cpa.using_api=true`, and the old local management API
address. The first sink therefore wrote exactly one canary artifact into the
pre-existing host CPA API pool before the error was noticed. The artifact was
verified against the task output by non-secret identity equality and moved to a
root-only quarantine backup. No pre-existing legacy credential was intentionally
removed or rewritten; the legacy service remained online and its own background
pool maintenance was not attributed to this migration.

Oracle Autoreg is now explicitly file-sink-only (`prefer=dir`, `using_api=false`),
and the `/opt/example-legacy-gateway/cpa/auths` path is a container mount of the dedicated host
directory `/opt/example-autoreg/cliproxyapi-auths`, not the host's legacy CPA directory. The
same generated canary payload was replayed through the application upload function
inside the Oracle console container; the dedicated pool increased from 843 to 844,
the new JSON shape passed required-key validation, and its file mode is `0600`.
The root-only config preimage is
`/var/backups/autoreg-oracle/20260810T045825Z-config-pre-file-sink.json`; the
temporary legacy artifact quarantine is recorded under
`/var/backups/autoreg-oracle/20260810T050228Z-legacy-sink-quarantine/`.

This proves Oracle registration plus isolated local credential sinking. It does not
prove Grok upstream availability, CPAR route activation, or a successful Build,
Console, or Web public request. A later explicit CPAR import/provider-binding
receipt is still required before any account enters a production native pool.

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

## Receipt status

`ORACLE_STAGED_VALIDATED_CANARY_REGISTERED_ISOLATED_SINK`: the ARM64 successor,
protected console, bounded registration canary, and dedicated file sink are
validated. Jakarta remains the predecessor and sole scheduler authority; no
single-active cutover, public endpoint change, CPAR production import, or source
retirement has occurred.

Independent review: [`autoreg-oracle-migration-20260810-review.md`](autoreg-oracle-migration-20260810-review.md)
