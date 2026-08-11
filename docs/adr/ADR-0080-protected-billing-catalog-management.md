# ADR-0080 Protected immutable billing catalog management

| Field | Value |
|---|---|
| Status | Accepted for P13-05C |
| Date | `2026-08-11` |
| Task | `P13-05C` |
| Contract | [BC-MGMT-013](../contracts/BC-MGMT-013-protected-billing-catalog-management.md) |

## Context

P13-05A/B provide immutable price catalogs, a restart-safe Usage materializer and a protected
billing read model.  Catalogs could only be inserted through an internal Store API, however, so
an operator had no authenticated, revision-guarded or auditable way to add a reviewed price
version.  Treating that Store primitive as a management feature would bypass the existing P10
control-plane admission and make a production price change difficult to attribute or roll back.

## Decision

P13-05C adds protected list, import and rollback routes under the existing management listener.
The selected Config Version is the admission and revision context.  A write is draft-only and
requires its exact `If-Match` revision; browser-originated unsafe requests also pass the existing
same-origin CSRF boundary.  Catalog insertion, the Config Version revision increment and the
value-free resource audit event commit in one SQLite transaction.

A catalog version is immutable and the management mutation is create-only: reusing any existing
identity fails with a safe conflict, regardless of whether the submitted prices match.  This is
distinct from the lower Store's exact-replay idempotence used for crash recovery.  Rollback never
edits or deletes history: it copies a retained predecessor into a new operator catalog version
with a caller-selected effective time.  Existing ledger rows are not repriced; the new version
affects only later materialization whose Attempt time selects it.

The write boundary accepts only `operator` and `imported` provenance, bounded unique
Provider/Channel/public-Model entries and non-negative integer micro-unit rates representable
exactly by JSON/TypeScript (`9_007_199_254_740_991` maximum).  Effective timestamps use the same
safe-integer boundary.
Receipts and list responses contain price identities and rates only.  They omit credentials,
Secrets, endpoint URLs, request content, client-key digests and source-event fingerprints.

## Consequences

- Operators can manage price versions without a second control plane or direct SQLite access.
- A stale editor cannot overwrite a newer management decision.
- Audit, revision and catalog data cannot partially commit.
- Historical catalogs and ledger decisions remain reproducible after rollback.
- Automatic catalog discovery, billing scheduling, Provider calls and a formal billing UI remain
  outside this task.

## Validation and rollback

Control and HTTP tests cover CSRF denial, import, immutable conflict, atomic revision/audit,
stale-revision rejection, forward-only rollback, deterministic listing and response redaction.
Operational rollback is a new catalog version.  A schema downgrade is not introduced by this
task because it reuses the P13-05A tables and P10 management audit/revision infrastructure.
