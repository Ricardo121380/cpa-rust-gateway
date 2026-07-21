# P4-00 execution acceleration report

- Plan version: `v1.2`
- Task: `P4-00`
- Date: 2026-07-21
- Status at this implementation commit: `LOCAL_PASS_PENDING_CI`; local evidence is recorded here,
  and GitHub acceptance plus a docs-only follow-up are required before `DONE`.
- Change Request: `CR-EXEC-001`
- ADR / Contract: [ADR-0021](../adr/ADR-0021-delivery-gate-classification-and-single-probe-diagnostic.md),
  [BC-DELIVERY-001](../contracts/BC-DELIVERY-001-delivery-gates-and-single-probe-diagnostic.md)

## Scope

P4-00 delivers only execution acceleration and controlled verification infrastructure. It adds no
Catalog, Health, Quota, public API, database, provider protocol, or production transport behavior.
P4-01 remains blocked until this task has remote acceptance.

## Delivered controls

- A fail-closed GitHub classifier separates explicit docs-only work from code work, includes
  deleted files in its comparison, forces tags/manual runs to code scope, and feeds a stable
  required-gate job.
- Docs-only checks run Markdown links, tracked Secret scan, task-state validation, and whole-tree
  whitespace validation. Code scope retains Fast and Full supply-chain gates.
- Full CI caches only pinned `cargo-deny`/`cargo-audit` artifacts and Cargo downloads. The cache
  key carries OS, Rust `1.97.1`, and the version-file hash; each run still verifies the expected
  versions and reinstalls with `--locked` when necessary.
- A plan-state guard enforces one `IN_PROGRESS` task and blocks P4 functional tasks until P4-00 is
  `DONE`.
- A separate ignored single-probe transport diagnostic has dedicated `P4_00_DIAGNOSTIC_*` input,
  exact one-request authorization/cap, explicit profile/egress admission, no retries/failover,
  finite reads/timeouts, and redaction-only output. It was not invoked.

## Local evidence

| Command | Result |
|---|---|
| `scripts/test-ci-change-classifier.sh` | PASS; docs/code/tag/dispatch plus deleted-code fail-closed cases. |
| `scripts/check-plan-state.rb` and `scripts/test-plan-state-check.sh` | PASS; 112 tasks, exactly one `IN_PROGRESS`, dependency blocking verified. |
| `scripts/test-install-quality-tools.sh` | PASS; fake cache miss installed both pinned tools; matching cache hit made zero installs. |
| `cargo test --locked -p gateway-http-actix --test p4_00_authorized_single_probe_diagnostic -- --nocapture` | PASS; 7 non-network tests passed, 1 deliberately ignored live diagnostic remained unrun. |
| `cargo fmt --all` | PASS. |
| `./scripts/check.sh docs` | PASS; document links, plan-state guard, tracked Secret scan, and whitespace. |
| `./scripts/check.sh fast` | PASS; full Workspace format/Clippy/test suite plus source, Secret, boundary, and document checks. |
| `./scripts/check.sh full` | PASS; Fast checks plus pinned-tool version verification, `cargo deny check`, and `cargo audit`. |

The local Full run emitted a non-blocking yanked-registry lookup timeout from `cargo audit`, but the
command exited `0` after completing its advisory scan. The required GitHub Full gate remains the
acceptance evidence for this implementation. No real Provider traffic is part of any local evidence
in this report.

## Efficiency baseline and measurement criteria

Recent P3 Full jobs took roughly 624–670 seconds, with pinned quality-tool installation about
463–497 seconds and the remaining Full check about 135–147 seconds. P4-00 does not claim a cold
cache improvement. Its measurable acceptance target is a warm Full `Install pinned quality tools`
step of at most 90 seconds, while docs-only commits should skip Fast and Full entirely.

The first P4-00 code run seeds or restores the cache; the subsequent P4-01 code run is the first
meaningful warm-Full measurement. A status-only docs commit after P4-00 remote acceptance will
measure the docs-only branch. Both remote results are added before final task completion.

## Boundaries, rollback, and next step

- The ignored diagnostic requires new, separate operator authorization for every real invocation;
  it cannot consume, extend, or replay P3-10 authorization.
- If classification or cache verification fails, the required job fails closed and code scope stays
  on Fast + Full. The rollback is to remove the new cache/classifier/diagnostic path and restore
  the previous all-code CI sequence.
- This implementation passed local review and is now `LOCAL_PASS_PENDING_CI`. Only after GitHub
  acceptance and the docs-only status verification can P4-00 become `DONE` and unlock P4-01.
