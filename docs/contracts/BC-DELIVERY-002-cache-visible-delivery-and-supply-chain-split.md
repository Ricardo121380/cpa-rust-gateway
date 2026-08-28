# BC-DELIVERY-002: Cache-visible delivery and supplemental supply-chain split

| Field | Value |
|---|---|
| Contract | `BC-DELIVERY-002` |
| Change Request | `CR-EXEC-002` |
| ADR | [ADR-0023](../adr/ADR-0023-cache-visible-delivery-and-supply-chain-split.md) |
| First validation Task | `P4-02` |

## Entry points

- `.github/workflows/ci.yml` selects a code path of complete Fast followed by supplemental Full.
- `scripts/check.sh fast` remains the complete workspace fast check.
- `scripts/check.sh supply-chain` verifies pinned quality tools, dependency policy, and RustSec.
- `scripts/check.sh full` remains the local complete Fast-plus-supply-chain command.
- `docs/06-development-plan.md` records the cache-visible delivery-ref and one-closeout rules.

## Preconditions

1. A code change still follows the existing fail-closed classifier and requires Fast plus Full.
2. A sequential delivery ref is cache-visible, or the chosen new/shared ref has a documented seed.
3. The cache contains only pinned tool binaries and Cargo registry/git content; no credential,
   environment, real-test configuration, or provider data is cacheable.
4. Full's cache action exposes its hit/miss output before the supply-chain command is run.

## Required behavior

| Situation | Required result |
|---|---|
| GitHub Fast | Runs the complete Workspace fast check for a code scope. |
| GitHub Full | Runs only after Fast passes for the same workflow/SHA, verifies pinned tool versions, then runs `cargo deny check` and `cargo audit`. |
| Required gate | Fails unless classifier, Fast, and supplemental Full all pass for code scope; skipped results are asserted. |
| Local full | Runs the complete Fast set plus the supply-chain set; it is not weakened by the CI split. |
| Cache hit | Is written to `$GITHUB_STEP_SUMMARY`; version checks still run. |
| Cache miss | Reinstalls with `--locked`, is reported, and remains a successful Gate only when all checks pass. |
| Code closeout | Exactly one docs-only commit follows an accepted code Gate to record evidence and mark `DONE`. |

## Error and rollback semantics

- An unavailable, missing, or version-mismatched cache never bypasses pinned installation or
  supply-chain commands.
- A Fast or Full failure blocks the required delivery gate and freezes dependent work.
- A cache miss is performance evidence, not a lower-quality success. Two missed warm expectations
  require cache/ref/runner investigation before any attempt to relax a gate.
- Rollback restores `./scripts/check.sh full` in the GitHub Full job and retains the ordinary
  Fast + Full required-gate condition.

## Corresponding checks

- `scripts/check-ci-workflow.rb`
- `scripts/check.sh supply-chain`
- `./scripts/check.sh fast`
- `./scripts/check.sh full`
- P4-02 GitHub code Gate and one docs-only closeout Gate
