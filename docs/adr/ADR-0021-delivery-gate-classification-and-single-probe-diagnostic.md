# ADR-0021: Delivery gate classification and controlled single-probe diagnostic

- Status: Accepted
- Date: 2026-07-21
- Task: `P4-00`
- Change Request: `CR-EXEC-001`
- Contract: [BC-DELIVERY-001](../contracts/BC-DELIVERY-001-delivery-gates-and-single-probe-diagnostic.md)

## Context

P0-P3 showed two avoidable delivery delays without identifying a reason to weaken quality gates.
The pinned `cargo-deny` and `cargo-audit` installation dominated Full CI, while a report-only
change repeated the same Rust and supply-chain checks. Separately, the P3-10 four-probe acceptance
harness correctly proves two-target compatibility, but its fixed shape is intentionally too broad
for locating one authorized target/mode transport problem.

The delivery solution must fail closed, retain supply-chain verification, keep the plan's one-code-
Task discipline, and never turn a diagnostic command into implicit real-provider authorization.

## Decision

1. CI first classifies a change as `docs` or `code`. Only `README.md` and `docs/**/*.md` are
   docs-only; all other paths, deleted files, empty/unknown comparisons, tags, and manual dispatch
   select `code`. A stable `Required delivery gate` job verifies that exactly the selected path
   passed.
2. The Full job caches only the pinned quality-tool binaries and Cargo registry/git downloads. Its
   key includes runner OS, Rust `1.97.1`, and `tools/quality-tool-versions.env`; the existing
   installer still verifies both versions and reinstalls with `--locked` on a miss or mismatch.
3. `LOCAL_PASS_PENDING_CI` is validated as a non-DONE state. A local guard permits at most one
   `IN_PROGRESS` Task and blocks any active P4 functional Task until `P4-00` is `DONE`.
4. The ignored P4-00 diagnostic accepts one opaque target label and exactly one mode. It has a
   dedicated authorization namespace, an exact external-request cap of one, P2 egress admission,
   a direct-or-local-DNS-SOCKS5 profile, one transport `send`, no retry/failover, finite timeouts,
   a 64 KiB discarded-read cap, and redaction-only console summaries. It is independent of the
   P3-10 aggregation harness and cannot reuse its authorization/configuration.

## Consequences

- Code-affecting changes still receive Fast plus Full supply-chain validation; a docs-only success
  is visibly distinct and cannot claim Full coverage.
- The first cache population can remain slow. The measurable warm-install target is at most
  90 seconds, not a promise that a cold runner or unrelated Rust build is instant.
- The diagnostic can identify one authorized transport compatibility result, but it neither proves
  end-to-end aggregation behavior nor replaces a later formal multi-target acceptance harness.
- No GitHub branch-protection setting is changed by this decision. The workflow supplies a stable
  required-gate job; repository policy remains an operator-controlled setting.

## Alternatives considered

- Run Full only on phase tags: rejected because ordinary code, workflow, lockfile, and security
  changes need early supply-chain evidence.
- Trust a cache hit without version verification: rejected because the cache is an acceleration,
  not a supply-chain assertion.
- Add a mode/target switch to P3-10: rejected because it would blur diagnostic and acceptance
  budgets, privacy evidence, and stop conditions.
- Permit generic proxy variables in the diagnostic: rejected because the selected egress profile
  must be explicit and reproducible.

## Validation and rollback

- Unit tests prove docs/code/tag/dispatch classification, including deleted-code paths; cache-miss
  installation followed by a version-matched no-install hit; plan-state blocking; and zero-
  authorization/single-cap/profile/timeout/payload/redaction diagnostic invariants.
- Fast and Full local gates plus the corresponding GitHub run are required before `P4-00` is
  `DONE`. A separate docs-only status commit verifies the docs branch of the workflow.
- Rollback removes the cache/classifier/diagnostic path, restores the prior all-code workflow, and
  returns the plan state to its pre-P4-00 rule. It does not alter P0-P3 artifacts or provider
  behavior.
