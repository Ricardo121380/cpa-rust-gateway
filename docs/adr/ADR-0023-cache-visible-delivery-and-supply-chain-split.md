# ADR-0023: Cache-visible delivery and supplemental supply-chain split

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Change Request | `CR-EXEC-002` |
| Task | `P4-02` delivery-flow validation |
| Extends | [ADR-0021](ADR-0021-delivery-gate-classification-and-single-probe-diagnostic.md) |
| Contract | [BC-DELIVERY-002](../contracts/BC-DELIVERY-002-cache-visible-delivery-and-supply-chain-split.md) |

## Context

ADR-0021 correctly made code changes pass Fast plus Full supply-chain validation and cached pinned
quality tools. P4-00 and P4-01 measurements exposed two remaining wait sources: GitHub Actions
cache visibility is ref-scoped in this setup, and Full repeated Fast's complete Workspace checks on
a separate runner. A new P4-01 branch therefore paid about 495 seconds for a cold tool install,
while an ordinary same-ref warm run restored the tools and verified their versions in about one
second. Two docs-only status commits also added wait without adding functional evidence.

The speed change must retain a fail-closed Fast and supply-chain proof, must not change task
concurrency, and must not turn a cache result into a security assertion.

## Decision

1. Sequential P4 code Tasks use a cache-visible delivery ref. P4-02 remains on
   `codex/p4-01-catalog-singleflight`; a new ref must first seed its own cache or use an approved
   shared/default ref. This changes delivery mechanics only, not the rule that at most one Task is
   `IN_PROGRESS`.
2. GitHub Fast remains the complete Workspace fast check. GitHub Full depends on that same
   workflow/SHA's Fast result and runs only pinned quality-tool version verification, `cargo deny
   check`, and `cargo audit`. The required job remains fail-closed unless both code jobs succeed.
3. Local `./scripts/check.sh full` remains comprehensive: it performs every Fast check and then
   the same supply-chain checks. `./scripts/check.sh supply-chain` is the CI-only supplemental
   mode and cannot claim Fast coverage by itself.
4. Full writes the quality-tool cache hit/miss value to the GitHub job summary. A miss is not a
   correctness failure because versions are still verified and missing/mismatched tools reinstall
   with `--locked`, but it must appear in the Task report.
5. A code Task contains its implementation, tests, ADR, contract, and report skeleton. Once its
   code Gate passes, exactly one docs-only closeout records the immutable code-Gate evidence and
   marks the Task `DONE`; no follow-up docs commit is made merely to copy that closeout run ID.

## Consequences

- The remote code gate still establishes both Workspace and supply-chain evidence, but it stops
  paying twice for format, Clippy, and tests. Local full remains the single comprehensive command
  for developer review and phase-level validation.
- Cache hits must be observable. The operational warm-install target is at most 10 seconds; the
  plan hard ceiling remains 90 seconds. Warm code workflow target is at most four minutes excluding
  GitHub queue; docs-only target is at most 45 seconds.
- Existing completed history remains valid. This ADR extends the delivery mechanics in ADR-0021 and
  does not change provider authorization, public APIs, schemas, persisted data, branch protection,
  or task dependencies.

## Alternatives considered

- Disable Full for ordinary code changes: rejected because lockfile, dependency policy, advisory,
  and supply-chain evidence must remain part of every code delivery.
- Cache without fixed version verification: rejected because a cache is an acceleration, not an
  authority over the installed binary.
- Use a new branch for every small sequential Task: rejected for this delivery path because the
  measured ref-scoped cache miss defeats the intended warm-cache benefit.
- Make two status commits to capture every workflow ID: rejected because external GitHub statuses
  are immutable evidence and a second self-referential run adds no functional proof.

## Validation and rollback

P4-02 validates the `supply-chain` script mode, workflow structure, cache-summary marker, local
complete Full gate, and a normal cache-visible GitHub code Gate without a manual warm rerun. Its
single docs-only closeout validates the status path. If the delivery mechanism proves incorrect or
unsafe, restore the Full job's `./scripts/check.sh full` command, stop relying on the delivery ref,
and restore the prior closeout cadence; no application data or public interface requires migration.
