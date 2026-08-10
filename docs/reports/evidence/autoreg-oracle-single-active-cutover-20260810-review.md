# Review: Autoreg Oracle single-active cutover

## Scope

Reviewed the source fencing action, Oracle liveness checks, rollback preimages and production
invariance for the Autoreg migration.

## Findings

- `PASS`: Jakarta Autoreg compose was explicitly stopped and no Jakarta Autoreg listener/process
  remained; the unrelated staging relay was not stopped.
- `PASS`: Oracle is the only active Autoreg service and remained healthy through three post-cutover
  checks.
- `PASS`: rollback preimages exist for both Autoreg and CPAR, with root-only permissions.
- `PASS`: CPAR public edge, active Config Version, legacy CPA, Caddy/DNS, CC Switch and grok2api are
  unchanged.
- `BOUNDARY`: automatic registration scheduling is still disabled/manual. This is intentional; the
  cutover proves single-active service ownership, not unattended account creation.

Verdict: `PASS_WITH_EXPLICIT_MANUAL-SCHEDULER-BOUNDARY`.
