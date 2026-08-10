# Autoreg Oracle Console production canary receipt

Date: 2026-08-10
Status: `PASS_WITH_ROLLBACK_WINDOW`
Scope: one Oracle Autoreg Console SSO, explicitly imported into the existing CPAR Console
pool, then verified through the public CPAR data plane

## Baseline and rollback

Before import, the production CPAR SQLite database passed `PRAGMA quick_check`, the gateway was
active, the public client-key `/v1/models` preflight returned a 2xx response, and the active
config version remained the existing P12-09 successor. A root-only online backup was created at:

```text
/var/backups/cpa-rust-gateway/autoreg-console-public-canary-05da177.sqlite3
```

The backup checksum and all credential material remain on the Oracle host only. The existing
Jakarta source data and migration rollback preimage remain preserved.

## Explicit import and probe

The same task-67 source record used by the isolated receipt was sent through the root-only
adapter pipe into batch `autoreg-oracle-console-prod-canary-20260810-01`. Import and the
batch-scoped native Console probe both passed (`1/1`), with account attribution, health, quota,
and all three canonical protocol projections complete. No route, Caddy, DNS, old CPA, CC Switch,
or active Config Version mutation was required; the existing native Console route selected the
newly imported, higher-priority canary account.

## Public data-plane matrix

The matrix used the real public CPAR host `cpar.example.invalid` and the existing root-only client
key. It sent exactly six requests, with no retry and no cross-provider fallback:

| Protocol | JSON | SSE |
|---|---:|---:|
| Responses | PASS | PASS |
| Chat Completions | PASS | PASS |
| Messages | PASS | PASS |

Receipt summary: `attempted_calls=6`, `successful_calls=6`, `value_free=true`. Every response
had the expected content type and semantic terminal condition. The value-free receipt is kept
root-only on Oracle; no token, cookie, account identity, request body or response body entered
the repository.

## Post-canary state

The production batch remains `applied` as the operator-approved Console account addition. The
database still passes `quick_check`; the gateway remains active; Oracle Autoreg is the only active
Autoreg service; the Jakarta Autoreg compose/service is stopped and fenced; and the Autoreg
registration scheduler remains disabled/manual. The scheduler was not enabled by this canary,
because the concrete live reauth executor and unattended replenishment wiring are a separate
change boundary.

Rollback remains available through the batch-scoped `grok-rollback` command and the root-only
database backup. A rollback must remove this batch before any restore of the preimage; it must not
modify the active Config Version, Caddy/DNS, old CPA, CC Switch or the preserved Jakarta source.

## Excluded boundary event

An earlier invocation of a legacy harness that did not implement `--help` emitted 12 unrelated
production requests before this canary. It changed no database, route, credential or config state
and is excluded from both the six-call Console matrix and this acceptance result. The harness is
not used by the new receipt path.
