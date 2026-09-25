# CPAR Agent repair production receipt — 2026-09-25

## Deployed result

- Production: existing Oracle `new-vps`, `cpa-rust-gateway.service`,
  <https://cpar.142857142.xyz/admin-ui/>.
- Running revision: `f4ebf4d75c111f8e0fa8e4f9d342a01bfb8da723`.
- Signed ARM64 executable SHA-256:
  `653659534f7f83d7e2ab54e8834c90e07eef55a9158ba17f11d42b47653a867c`.
- Completed: `2026-09-25T08:43:53Z` (16:43:53 Asia/Shanghai).
- Stop-to-ready: 1,029 ms. Actual `/proc/<pid>/exe` bytes match the signed artifact.
- Previous binary retained: `3a8e5af60738c925dcf44d94baf376227870a16c`.
- Active schema remains 28; no configuration publication or permission expansion in this rollout.
- Frontend assets match the previously deployed version exactly. No browser visual work or
  browser acceptance is claimed for this backend-only repair.

## Exact-revision gates

- [Formal delivery gate](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36112892555):
  Fast, supply-chain and required delivery jobs all succeeded at `f4ebf4d`.
- [Release artifact workflow](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36113591702):
  native ARM64 and x86_64 builds, container smoke and signing succeeded at the same revision.
- Both downloaded artifacts passed manifest/receipt/revision/SBOM validation and Sigstore
  verification against this repository's release workflow and branch identity.
- Fresh CI Rust results: 1,295 passed, 0 failed, 9 ignored. New Agent gate also passed in CI.
- On the Oracle host, the exact signed ARM64 binary passed the four three-round Agent flows
  plus existing error/cancellation/catalog/billing scenarios in an isolated network namespace,
  with synthetic credentials/state and a TLS loopback mock. No real Provider inference.
- Final-revision isolated production-copy rehearsal passed old→new→old→new startup, API,
  effective permission, account inventory and historical-row checks. Existing administrator
  store bytes were unchanged in the rehearsal.

## Production readback

- Service ready; both listeners remain loopback-only at 18180/18181.
- Public health, all four frontend assets, CSP, and rejection of unauthenticated management
  inventory/request access passed.
- Seven account inventory items and effective group/key model permissions unchanged.
- Preserved all pre-cutover 2,241 gateway event rows and 595 billing ledger rows.
- Preserved resource identities and administrator store; SQLite quick/foreign-key checks passed.
- No DNS/Caddy/Autoreg change, history deletion, real inference or OMP source edit.
- Private rollback material includes a pre-cutover SQLite backup and credentials backup. The
  deploy fallback switches binary while retaining latest state; it does not overwrite live data.

## One unresolved intermittent CI observation

The first candidate `7468d6d` failed its first Linux CI Agent run on the third JSON round with
HTTP 503 ([failed run](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36111423910)).
The original assertion lacked the error code/runtime state, so that occurrence's exact cause is
not established. It is not described as a diagnosed or fixed defect.

`f4ebf4d` adds safe failure-code/runtime evidence without changing backend behavior. The same
backend passed 10 fresh macOS fixtures, a signed ARM64 test and five further isolated Linux
fixtures; the final exact revision passed full Linux CI and its signed-artifact test. This supports
the continuation repair, but does not prove the intermittent observation impossible. Future
failures are hard gate failures with diagnostics; the harness does not replay failed requests,
remove reasoning, relax assertions or wait out a request error to hide it.

## Remaining acceptance boundary

The actual OMP approval→write→read flow was not rerun with a real model: that task's recorded
live budget was exhausted. This report proves the CPAR repair and its deployment, not a new
successful real-provider/OMP UI run. Encrypted reasoning remains deliberately unsupported
without an ownership-aware path. New stored histories containing the `Reasoning` variant
cannot be replayed by the old binary after rollback; retain data and roll forward or start a new
conversation. See [implementation report](cpar-agent-compatibility-20260925.md) for scope.

## Evidence locations

- Local private rollout directory: `/private/tmp/cpar-agent-release-20260925-final/`:
  `formal-gate.json`, `formal-pass.log`, both `verified-*.json` records,
  `artifact-gate.json`, `rehearsal.json`, `production-receipt.json`.
- Remote owner-protected directory: `/var/tmp/cpar-agent-f4ebf4d75c11/`.
- Earlier failed CI and supplemental repeated-run evidence are retained in
  `/private/tmp/cpar-agent-release-20260925/` and local `output/agent-compatibility-20260925/`.
- Reports contain no credentials. Private rollout evidence is not committed.
