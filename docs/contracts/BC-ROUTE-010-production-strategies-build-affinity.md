# BC-ROUTE-010: Production strategies and ordinary Build affinity

Status: `LOCAL_IMPLEMENTATION`; #70 / OPT-03; [ADR-0133](../adr/ADR-0133-production-routing-build-affinity.md).

## Required behavior

- Route `smooth_weighted_round_robin` preserves the prior serving comparator and old priority
  direction. Explicit `round_robin`, `weighted_round_robin`, `fill_first` select candidate and
  endpoint credential policy independently. Priority remains lower-is-better. Explicit candidate
  tiers precede policy: priority, known quota confidence, separately enabled price evidence.
- RR ignores weights, WRR yields configured shares over complete independent-identity periods,
  fill-first holds the first qualified ID and exits when it cannot lease. Missing capacity or
  temporary exclusions do not donate rejected weighted slots to a survivor. Weights remain
  positive, bounded by existing compiler/schedule limits; invalid publication preserves active state.
- No policy infers a Provider from an adapter or falls across Provider scope. Actual lease time
  rechecks configuration, protocol/permissions, catalog admission, Health, Quota, expiry, revision
  and capacity. Lease ownership, retry budget, first-semantic closure and release stay with the
  existing orchestrator and RAII lease. Applying billing prices never enables routing prices.
- Ordinary Build cache preferences reuse the existing HMAC identity and store, scoped to exact
  client Key/Provider/upstream model/derived cache identity, with canonical public model and exact
  serving namespace additionally qualified. Raw cache identities and proxy material are not stored
  or logged. Missing cache identity means ordinary strategy selection without a new binding.
- Read preferences once before the lease loop on a blocking task. A preference never reserves
  capacity or bypasses a higher applicable priority/confidence tier. Failed qualification means
  normal selection. A successful bootstrap writes the actual binding, 15-minute sliding expiry,
  and exact egress/config/candidate/revision fingerprint; it is not completion/cache-hit evidence.
- Every replacement atomically retains a closed break reason and nullable estimated loss.
  Unmeasured loss is NULL. Concurrent misses may select independently; the last successful
  bootstrap wins and every intervening replacement is recorded. Configuration or revision changes
  invalidate old hints. Maximum retained affinities and breaks are 4096 each; oldest updates are
  evicted, expired records removed on successful writes. Store failures fail safely.
- Strict stored continuation/compact/reasoning/WebSocket and diagnostic pins do not consume or
  update soft affinity. They retain exact lineage, no sibling fallback and at-most-once execution.
- Protected Route CRUD, validate, publish and rollback preserve the explicit policy. Explain
  reports `selection_policy`, remains side-effect free, and is a fixed-input projection. Billing,
  old ledger/frozen exports, schema40 review evidence, #69 name/count authorization and #75 backup
  format/empty-target/key boundaries retain their existing contracts.
- Migration41 preserves old values and adds qualification/NULL-loss support. A downgrade that
  would discard new semantics is rejected. No all-Provider session framework or #71 rules are added.

## Corresponding tests

- `issue70_public_http_policies_rotate_candidates_and_credentials`
- `issue70_public_http_affinity_expiry_owner_capacity_revision_and_health`
- `issue70_public_http_failover_and_postsemantic_no_retry`
- `issue70_public_http_concurrent_capacity_releases_every_lease`
- `issue70_public_http_stored_lineage_never_uses_soft_fallback`
- `issue70_explicit_policy_validation_publication_and_rollback_preserve_legacy`
- `issue70_management_policy_crud_is_explicit_revisioned_and_closed`
- `issue70_affinity_migrates_schema40_and_bounds_unknown_break_history`

## Evidence boundaries

Synthetic local HTTP/receiver and local Full evidence do not imply unmodified client success,
fixed Build activation, native alias qualification, provider cache retention, real-provider cost,
remote CI, a published artifact or deployment. Historical failures and UNKNOWN remain in the
implementation report and its source-bound logs.
