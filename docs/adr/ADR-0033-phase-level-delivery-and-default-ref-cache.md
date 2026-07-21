# ADR-0033: Phase-level delivery and default-ref quality-tool cache

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Change Request | `CR-EXEC-007` |
| Task | `P5-00` |
| Extends | [ADR-0023](ADR-0023-cache-visible-delivery-and-supply-chain-split.md) |
| Contract | [BC-DELIVERY-003](../contracts/BC-DELIVERY-003-phase-level-delivery-and-default-ref-cache.md) |

## Context

P4 proved that the pinned quality-tool cache is effective on the same branch ref, but GitHub did
not expose that branch cache to the final Phase tag. The P4 tag therefore rebuilt `cargo-deny` and
`cargo-audit` for about eight minutes even though the same cache key was warm on the development
branch. Per-Task branches and remote Code/docs closeouts also created more remote waits than the
single-session development model needs.

The new delivery flow must reduce remote runs without weakening local Task review, the final Fast
plus Full result, required-status fail-closed behavior, supply-chain checks, or Phase ordering.

## Decision

1. Each not-yet-started Phase uses one `codex/p<phase>-<short-name>` branch. Individual Tasks keep
   distinct commits and local evidence but ordinary Task commits do not trigger remote CI.
2. Automatic push CI is limited to `main` and annotated `phase-p*-complete` tags. Pull requests and
   explicit `workflow_dispatch` remain available; manual dispatch and tags always classify as code.
3. P5-00 is a one-time infrastructure exception. Its reviewed commit fast-forwards `main`, which
   runs Fast plus Full and creates the pinned quality-tool cache in the default-ref scope. No P5
   protocol implementation is included in that exception.
4. A Phase closeout tag points at the complete reviewed Phase target. Pushing the Phase branch and
   tag triggers exactly one normal formal delivery run: the tag run. The Phase branch push is not an
   automatic CI trigger.
5. GitHub Full may restore the default-ref cache, but the installer still verifies the exact pinned
   versions. A miss or mismatch rebuilds with `--locked`, and the supply-chain checks always run.
6. `LOCAL_PASS_PENDING_PHASE_GATE` can satisfy an explicit dependency only inside the same Phase.
   It cannot satisfy a cross-Phase dependency or allow a dependent Task to be marked `DONE`.

## Consequences

- Ordinary Phase development has no per-Task GitHub wait. Targeted local test/review remains
  mandatory, and high-risk Tasks still run local Full immediately.
- The first default-ref seed may be cold. Subsequent Phase tags can reuse it while tool and Rust
  versions remain unchanged.
- The final formal Gate is still Fast plus supplemental Full on one immutable SHA. Branch pushes
  without a corresponding accepted Phase tag are neither merge nor release evidence.
- Existing P0-P4 tags and Gates remain valid and are not reinterpreted by this decision.

## Alternatives considered

- Keep one branch per Task: rejected because it preserves repeated remote waits and ref-scoped
  cache misses.
- Disable the final supply-chain Gate: rejected because speed cannot replace dependency policy and
  RustSec evidence.
- Trust a branch cache for a tag: rejected because P4 directly proved that visibility assumption
  false.
- Run both a branch Code Gate and a tag Gate at closeout: rejected because the same SHA would be
  tested twice without adding independent correctness evidence.

## Validation and rollback

P5-00 validates the workflow trigger shape, same-Phase dependency state, cross-Phase rejection,
local Full, and one early `main` Fast plus Full Gate. That Gate must publish a default-ref cache
entry. G5 validates that `phase-p5-complete` restores the default-ref entry or safely records a miss,
then passes Fast plus Full and Required. Rollback restores automatic branch push CI and the prior
Task-level state rules; application data and provider behavior require no migration.
