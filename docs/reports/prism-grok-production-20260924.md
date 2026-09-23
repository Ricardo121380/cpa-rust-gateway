# Prism V2 and Grok/Pi compatibility production release

Production cutover completed at **2026-09-24 00:21:33 Asia/Shanghai**
(2026-09-23 16:21:33 UTC), after the user explicitly authorized deploying both
the backend repair and the committed frontend updates.

## Deployed scope

- Revision: `3a8e5af60738c925dcf44d94baf376227870a16c` on
  `codex/prism-grok-production-20260924`.
- Latest committed Prism V2 frontend through `ac7a8d5`: workspace/dialog updates,
  computer-use findings, responsive behavior, text insets and spacing corrections.
- Grok/Pi Responses repair: preserve the reviewed encrypted-reasoning include and
  summary controls; classify wholly incompatible protocol projection as a client
  request error instead of misleading credential unavailability.
- Schema remains 28. No configuration activation, model connection, account import,
  dependency update, DNS/Caddy/firewall or Autoreg change was part of this release.
- The earlier backend-only artifact `fa72f6f` passed its checks but was superseded
  before cutover. Production changed directly from `ca75a443` to `3a8e5af`.

The Grok incident and synthetic regression details are recorded in
[the repair evidence](evidence/grok-build-pi-reasoning-20260923.md).

## Exact-revision verification

- [Formal delivery gate](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35886398237):
  Fast, full supply-chain and required aggregate jobs passed for `3a8e5af`.
  Rust results: 1,285 passed, 9 ignored across 118 successful test-suite reports.
- [Signed release artifacts](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35886403548):
  native ARM64 and x86_64 builds, network-disabled container startup checks, SBOMs,
  manifests and Sigstore signing passed. Both downloaded artifacts were independently
  checked for expected revision, checksums, manifest/receipt structure and exact
  workflow branch identity using Cosign.
- The isolated checkout also passed all 403 frontend tests in 53 files and the
  management SPA contract/generated-client/reproducible double-build check.
- ARM64 executable SHA-256:
  `6f4eb0924987734b4ca56178fe3fde213cbd1e51a7d40dacb7053f869348b94a`.
- Frontend resource version: `4096b00e6d6d5ad7b40e3366`. All four public resources
  exactly match the independently built candidate assets.

## Production and rollback evidence

A network-isolated copy of current production data successfully ran
`ca75a443 → 3a8e5af → ca75a443 → 3a8e5af`. Schema, effective permissions,
administrator store and historical identities/rows were retained.

The formal switch ran under a transient systemd service and exclusive deployment
lock. Before switching it backed up the stopped control and administrator databases.
Stop-to-ready was **1,021 ms**. Post-cutover checks verified the running process's
executable hash, authenticated system/accounts/request-summary APIs, loopback-only
listeners, public HTTPS health, strict CSP and unauthenticated management rejection.

Existing 7 accounts, 2,231 historical event rows and 593 ledger rows were retained.
Active configuration and effective model permissions were unchanged. A subsequent
readback showed `active/running`, zero automatic restarts and zero error/panic events
in the new process's observed journal. A `panicked: 0` refresh statistic was explicitly
distinguished from an actual panic.

Immediate compatible rollback target:
`ca75a44351374aac62a40b13ba878a3487345db6`. **Keep the latest state and rotating
credentials; rollback changes the binary only. Do not restore stale databases.**
Protected server-side backups and release receipts remain under
`/var/tmp/cpar-prism-grok-3a8e5af60738`.

## Browser acceptance and limits

EgoLite reloaded the existing production tab and loaded the new resource version.
The first load exceeded the browser helper's 15-second deadline: main.js eventually
completed in about 82 seconds in that browser connection. After loading, the real
login page rendered correctly at 1440×900, 1280×720 and 390×844 without horizontal
overflow; desktop and mobile screenshots were visually inspected. The network
observation is not a general performance guarantee.

Reload returns the UI to login by its existing in-memory-session design; the
session module is unchanged from the prior production version. Authenticated
production workspaces were not re-run in the browser after reload. Server-side
authenticated read checks and the isolated production-copy checks did pass, and
the frontend's earlier local workspace/dialog evidence remains in
[the spacing report](prism-v2-spacing-polish-20260923.md) and its linked reports.

No real Provider inference request was made during this deployment. The patch is
live, but an OMP/Grok end-to-end successful response still requires a separate live
acceptance request; deployment health and synthetic protocol tests do not establish
upstream inference success. This release does not newly connect `grok-4.6`.

## Receipts

- [Production cutover](evidence/prism-grok-production-20260924/production-receipt.json)
- [Disconnected rollback rehearsal](evidence/prism-grok-production-20260924/rehearsal.json)
- [Post-deployment process state](evidence/prism-grok-production-20260924/production-after.json)
- [Browser readback](evidence/prism-grok-production-20260924/browser-after.json)
- [Validation counts](evidence/prism-grok-production-20260924/validation.json)
- [ARM64 artifact verification](evidence/prism-grok-production-20260924/verified-artifact.json)
- [x86_64 artifact verification](evidence/prism-grok-production-20260924/verified-x64.json)
