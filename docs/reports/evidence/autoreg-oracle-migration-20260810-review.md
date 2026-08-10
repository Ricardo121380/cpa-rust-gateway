# Autoreg Oracle migration review

Date: 2026-08-10 (Asia/Shanghai)
Receipt: [`autoreg-oracle-migration-20260810.md`](autoreg-oracle-migration-20260810.md)
Change request: `CR-P12-AUTOREG-MIGRATE-001`

## Review scope

This is a value-free review of the staged Jakarta → Oracle Singapore migration,
the ARM64 deployment envelope, the bounded registration canary, and the credential
sink boundary. It does not review or authorize CPAR production route changes.

## Findings

| ID | Severity | Finding | Disposition |
|---|---|---|---|
| R-01 | P1 | The first two canary attempts stopped before registration because an empty host browser-cache mount hid the image binary and the copied config retained Jakarta dependency ports. | Fixed in the Oracle compose/config and covered by the pre-start policy hook. No account write occurred in either attempt. |
| R-02 | P1 | The first completed canary inherited `cpa.prefer=api`/`using_api=true` and wrote one generated artifact to the host's legacy CPA API pool. | Contained: identity-verified artifact moved to root-only quarantine; Oracle config is now `prefer=dir`/`using_api=false`; no pre-existing legacy credential was intentionally changed. |
| R-03 | P1 | Oracle registration success is not the same as CPAR or Grok upstream availability. | Explicitly separated. The canary is not imported into CPAR production and the Grok proxy-only `403` remains an egress observation. |
| R-04 | P2 | Oracle is a staged successor while Jakarta remains online. | Expected boundary. Scheduler is fenced on Oracle; no dual-active worker, DNS/Caddy cutover, or source retirement was performed. |
| R-05 | P2 | The first local Full gate exposed two pre-existing policy drift items: a test-only `expect()` rejected by strict Clippy and crate-boundary allowlists that did not include dependencies already committed for Codex OAuth. | Fixed without runtime behavior changes; strict Clippy, crate boundaries, and the complete local Full gate now pass. |

## Verification performed

- Oracle `autoreg-oracle.service` is active after the pre-start hook and
  `systemd-analyze verify` passes.
- WARP and Privoxy are healthy; FlareSolverr is loopback-only; console is
  loopback-only and unauthenticated API access returns `401`.
- Oracle console SQLite quick-check remains `ok`; the copied inspection scheduler
  remains fenced off.
- The dedicated auth pool contains the 843-file migration baseline plus one
  canary artifact, with required JSON keys and mode `0600`.
- Existing CPAR `/healthz` remains `200`; CPAR active config, routes, Caddy/DNS,
  old CPA public endpoint, CC Switch, and grok2api were not cut over or changed by
  this migration.
- No GitHub Actions run was started. The plan's CI optimization limits expensive
  verification runs only; it does not limit push, PR, review, rebase, or merge
  operations.
- `./scripts/check.sh full` passes after the review fixes, including workspace
  tests/doc-tests, strict Clippy, source/crate policy, docs/contracts, secret scan,
  dependency policy, and RustSec audit.

## Verdict

`STAGED_PASS_WITH_BOUNDARY`

The Oracle ARM64 successor and its isolated registration/file-sink path are ready
for a separately approved CPAR import/provider-binding canary. The migration is not
single-active and is not a production cutover. The next acceptance must explicitly
choose the provider pool (Grok Build OAuth, Console SSO, or Web session) and record
an import receipt before any production route is changed.

## Rollback

Stop/disable only the Oracle `autoreg-oracle` stack and restore the root-only Oracle
preimage. Leave Jakarta and production CPAR untouched. The canary quarantine is
recoverable; do not delete it without a separate cleanup authorization.
