# ADR-0102: Actual Attempt identity and effective capability evidence

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-10-02` |
| Task | CPAR execution evidence and Linux ARM64 acceptance candidate; parent #9, C #39–#50, aggregation #57 |
| Matrix / Contract | G01–G04, J04; [BC-OBS-005](../contracts/BC-OBS-005-attempt-execution-identity.md), [BC-OBS-002](../contracts/BC-OBS-002-append-only-sqlite-event-writer.md), [ADR-0100](ADR-0100-exact-protocol-semantics-and-usage-evidence.md) |

## Context

The new-build acceptance tool cannot prove which material, configuration, and transport executed
an old request from a later management inventory. Existing Attempt events already record Request,
Attempt, Route, Candidate, Endpoint, Credential and upstream model, and Usage/ledger have exact
source-event attribution. Their strict decoder and byte-exact replay make adding serialized fields
to historical event payloads an incompatible change. Native account revisions can differ from
actual lease material revisions. Empty public capabilities do not declare a candidate unsupported.

## Decision

1. Retain the existing serialized Attempt bytes. Attach bounded execution facts in memory to its
   existing confirmed event, and append schema 34 `gateway_attempt_execution` in the same SQLite
   transaction as the original event. Bind by Attempt ID, Request ID and SHA256 of the exact
   original payload. Identical original plus identical sidecar is idempotent; any difference fails.
   A missing legacy sidecar remains unknown and cannot be enriched by replay or inventory.
2. Capture configuration version/revision from the loaded executor, material revision from the
   leased credential, and channel/egress at the actual adapter and selected transport seam. The
   request retains its executor and lease. Reset capture before each retry and bind it to the exact
   candidate/credential/revision tuple. Early driver rejection uses `not_selected`; inability to
   determine facts uses `unknown`. No invented management/native revision is substituted.
3. Direct transport has no resource revision. Version-owned proxy selection records exact target,
   selected node and owning configuration revision. A process proxy records SHA256 of its validated
   secret-free actual transport identity. That fingerprint is separate from a resource revision;
   raw proxy addresses are not persisted. These types do not create a node revision mechanism.
4. Record the compiled candidate's six effective declarations. Missing override inherits existing
   profile/model rules. Continuation means stored/compact/WebSocket, or the existing Grok Build
   owned-reasoning path with Reasoning. Protocol applicability remains a separate acceptance rule.
   Protected effective-model sources include these facts only for the actual serving snapshot and
   requested Client Key/access scope. Existing optional fields stay absent in other embeddings.
5. Extend the existing protected Channel Pin receipt with the same actual execution facts, without
   adding a second Request/Usage/billing stream to this diagnostic. Its previous single-request,
   cancellation, redaction and authentication rules remain. The formal runner can freeze a plan
   from an existing receipt plus a fresh scoped declaration, then compare every ordinary request's
   exact Request/Attempt/Usage/ledger facts. Unknown blocks, contradiction fails, and neither permits
   another inference turn. Freeze and preflight themselves send no inference.

## Consequences

The Required queue, bounded confirmation, finite writer batch and existing backpressure remain.
SQLite work stays in the background Store writer. No bodies, headers, tokens, secrets, arbitrary
diagnostics or raw URLs enter the new observation. Original Request/Attempt/Usage/ledger sources
are preserved. This proves software observations for tested local executions, not real Provider
support. Kiro output-cap and Grok Web native multi-turn blockers remain independent.

Schema 34 is additive but old binaries reject newer schemas. A binary-only rollback is unsafe.
The new protected response fields are optional and generated contract synchronization is one-way;
there is no frontend business feature or permission change.

## Alternatives considered

- Add serialized fields to Attempt: rejected because strict historical decoding/replay changes.
- Join current inventory or native-account rows: rejected because rotation destroys historical
  truth and material/account revisions have distinct ownership.
- Build a new observation service or synchronous request-path SQL: unnecessary and violates the
  existing bounded event/Store boundary.
- Fill unknown declarations with false or reduce admission: rejected because acceptance must
  verify existing promises without silently changing them.

## Validation and rollback

The [behavior contract](../contracts/BC-OBS-005-attempt-execution-identity.md) maps to atomic storage,
strict replay, legacy unknown, schema 33/34 round trip, tampering, actual lease replacement, native
factory and full isolated gateway/TLS/SOCKS regressions. Formal tool regressions cover missing and
contradictory identities/declarations and zero later sends. Final evidence pins the tested source.

Down migration drops only the additive sidecar/triggers; original events and billing remain.
Evidence written under schema 34 is deliberately lost by down migration and must first be
preserved in a full post-upgrade snapshot. Deployment rollback needs a validated pre-upgrade
full state/credential backup and the old binary; capture any new accepted history before restoring.
Never delete history, broaden Client Key permissions or restore accounts as an acceptance shortcut.
Production snapshot rehearsal, signed release, deployment and real requests require their own
target-specific authorization and evidence.
