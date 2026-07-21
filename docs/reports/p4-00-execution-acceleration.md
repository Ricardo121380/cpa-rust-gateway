# P4-00 execution acceleration report

- Plan version: `v1.2`
- Task: `P4-00`
- Date: 2026-07-21
- Status: `DONE` after the corrected code Gate passed. This pure-document closeout is itself
  submitted to prove the docs-only route; P4-01 remains unstarted until that gate passes.
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

## First GitHub code-gate result and correction

GitHub Actions run [29797956689](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29797956689)
correctly classified the implementation as `code`, skipped Docs-only, and stopped Full after Fast
failed. The stable required job then failed as designed. The cause was limited to the classifier's
black-box test: GitHub supplies `GITHUB_OUTPUT` to every step, so a command-substitution test read
an empty stdout result while the production classifier correctly wrote its output file.

The correction clears only `GITHUB_OUTPUT` for the test subprocess; production classification still
uses the GitHub output file. The corrected classifier passed with `GITHUB_OUTPUT` explicitly set,
then Fast and Full local review passed again. Its replacement GitHub code-gate result is recorded
below.

## Accepted GitHub code-gate evidence

GitHub Actions run [29798236173](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29798236173)
passed for correction commit `cd22b66`.

| Job / step | Result and duration |
|---|---|
| Classify delivery gate | PASS; selected `code` in about 4 seconds. |
| Docs-only gate | Correctly skipped for this code change. |
| Fast gate | PASS; job about 175 seconds, `Run fast gate` about 148 seconds. |
| Full supply-chain gate | PASS; job about 655 seconds. |
| Restore pinned quality-tool cache | Explicit cache miss on the new versioned key; completed in under one second. |
| Install pinned quality tools | PASS; cold installation about 482 seconds (8m02s). |
| Run full gate | PASS; about 142 seconds. |
| Cache save | PASS; versioned cache saved after successful Full gate. |
| Required delivery gate | PASS; verified the `code` path's Fast + Full results. |

The cold result is consistent with the prior 463–497 second installation baseline and does not yet
test the warm target. P4-01's later code run will restore this saved cache and is the first valid
warm-install measurement. This closeout commit changes only Markdown, so its GitHub run must select
and pass `Docs-only gate`; only then may P4-01 start.

## Efficiency baseline and measurement criteria

Recent P3 Full jobs took roughly 624–670 seconds, with pinned quality-tool installation about
463–497 seconds and the remaining Full check about 135–147 seconds. P4-00 does not claim a cold
cache improvement. Its measurable acceptance target is a warm Full `Install pinned quality tools`
step of at most 90 seconds, while docs-only commits should skip Fast and Full entirely.

The first P4-00 code run seeded the cache; the subsequent P4-01 code run is the first meaningful
warm-Full measurement. This status-only docs commit measures the docs-only branch before P4-01
begins.

## Boundaries, rollback, and next step

- The ignored diagnostic requires new, separate operator authorization for every real invocation;
  it cannot consume, extend, or replay P3-10 authorization.
- If classification or cache verification fails, the required job fails closed and code scope stays
  on Fast + Full. The rollback is to remove the new cache/classifier/diagnostic path and restore
  the previous all-code CI sequence.
- The code-gate portion is accepted. The docs-only status verification is the final P4-00 delivery-
  efficiency proof; P4-01 remains blocked until it is green.
