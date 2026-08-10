# Autoreg Oracle single-active cutover

- Change boundary: `CR-P12-AUTOREG-MIGRATE-001`
- Date: 2026-08-10 (Asia/Shanghai)
- Scope: move Autoreg service authority from Jakarta to Oracle Singapore while preserving rollback.

## Pre-cutover controls

- Oracle `autoreg-oracle.service` and its console/sidecars were healthy and isolated.
- The CPAR import canary and public six-case Grok Build matrix passed before fencing the source.
- A root-only rollback preimage was created on Jakarta at
  `/var/backups/grok-register/oracle-cutover-20260810T072500Z`.
- A CPAR production binary/unit/Caddy/SQLite backup was retained at
  `/var/backups/cpa-rust-gateway/autoreg-cutover-20260810T071851Z`.

## Cutover action and observation

The Jakarta `grok-register` compose project was stopped with its explicit compose file. Its
Autoreg containers and listener were absent afterwards; the unrelated
`cpar-grok-web-relay-staging` container was intentionally left untouched. Oracle remained the sole
active Autoreg service. Three bounded checks after cutover all reported:

```text
cpar=active  quick_check=ok  loopback_listener=present
```

The protected Autoreg console health probe returned its expected authentication redirect (`302`),
which proves the auth boundary and service responsiveness; it is not reported as an unauthenticated
health `200`.

## Rollback

If the Oracle service or CPAR batch must be reverted: stop/disable the Oracle Autoreg scheduler,
remove the `autoreg-oracle-build-prod-20260810-01` CPAR batch/route links, restore the Jakarta compose,
configuration, SQLite and cursor from the preimage, and recheck the known-good Jakarta listener.
If the gateway binary itself must be reverted, restore the CPAR `current` symlink and unit/database
preimage from the Oracle cutover backup. Keep Jakarta source data and already-created external accounts
quarantined; do not claim that external registration was undone.

No DNS, Caddy public route, legacy CPA, CC Switch or grok2api state was changed by this cutover.

Verdict: `ORACLE_PRIMARY_JAKARTA_FENCED_ROLLBACK_READY`.
