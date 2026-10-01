# BC-OBS-005: Attempt execution identity and effective capability evidence

| Field | Value |
|---|---|
| Contract | `BC-OBS-005` |
| Task | CPAR C execution evidence; #9, #39–#50, #57 |
| ADR | [ADR-0102](../adr/ADR-0102-attempt-execution-identity-and-capability-evidence.md) |
| Extends | [BC-OBS-001](BC-OBS-001-bounded-request-attempt-usage-events.md), [BC-OBS-002](BC-OBS-002-append-only-sqlite-event-writer.md) |

## Entry and preconditions

The existing request-scoped `EndpointAttemptDriver` supplies facts to ordinary, pinned and
continuation Attempt emission after its actual lease/transport selection. A production executor
has an immutable loaded configuration identity; older/test embeddings can omit it. Existing
authenticated effective-model and Channel Pin endpoints expose optional protected evidence.
No public inference response gains account identifiers, body echoes or revision fields.

## Event sequence and invariants

1. Request admission pins the existing Snapshot/executor. Each actual attempt leases a credential,
   clears earlier capture, validates conversion and selects its actual transport.
2. The terminal Attempt retains existing route/candidate/endpoint/model/credential and Request ID.
   Its optional identity adds actual material revision, configuration version/revision, channel,
   typed egress and all six compiled declarations. A pre-selection rejection is `not_selected`.
3. Confirmed persistence appends original and sidecar atomically. The sidecar is append-only and
   binds to exact original bytes by SHA256. Required admission/flush failure remains explicit.
4. Usage names the actual Attempt. Ledger source is the exact Usage event. The protected collector
   reads a consistent SQLite snapshot and compares those sources; it never joins by a time window.

The identity is bounded to necessary identifiers, closed declarations and typed egress facts.
Management account, native account and material lease revisions have independent meanings.
Rotation and snapshot publication do not rewrite earlier requests; retries record the selected
candidate and material of that attempt. Configuration-owned egress revision is scoped to its
version; Direct is explicitly not applicable, process-proxy fingerprint is not a resource revision.

Public `{}` capabilities and absent overrides cannot supply false declarations. The protected
source uses the serving candidate's compiled rules and the requested Client Key scope. Six booleans
are complete only when this source is available; absence remains unknown. Responses-only stored,
continuation, compact and WebSocket applicability is shown separately for Chat/Messages.

## Error and compatibility semantics

| Condition | Result |
|---|---|
| Legacy/missing sidecar or declaration | Unknown; formal tool BLOCKED; no later send |
| Wrong Request/hash/revision/candidate/capabilities | Invalid read or formal FAIL; no later send |
| Identical original and sidecar replay | No duplicate |
| Same original with changed/missing/new sidecar | Conflicting replay; full transaction rollback |
| Atomic sidecar insert fails | Original Attempt is not committed |
| Error/cancellation/timeout after actual selection | Preserve the attempt's actual identity and outcome |
| Driver ends before transport selection | `not_selected`, never fabricated Direct |
| Schema 34 down/up | Original bytes/history survive; sidecar loss is explicit, no backfill |

Binary-only downgrade is refused by the existing schema gate. Full production rollback requires
backups and retention of new accepted history, as specified in ADR-0102.

## Corresponding tests

- `execution_identity_is_atomic_bound_and_replay_exact`
- `execution_identity_legacy_unknown_and_schema_rollback_preserve_attempts`
- `execution_identity_invalid_revision_and_corrupt_binding_fail_closed`
- `actual_execution_identity_retains_refreshed_lease_and_configuration`
- `actual_execution_identity_proxy_fingerprint_and_owned_reasoning_are_explicit`
- `ordinary_native_families_use_the_public_factory_and_preserve_identity`
- Python formal oracle: `python3 scripts/test-cpar-new-build-acceptance.py`.
- Actual gateway/SQLite/TLS/SOCKS: `python3 scripts/acceptance/cpar-batch-b-http.py <owned-output>`;
  covers three protocols JSON/SSE, failures/cancel/timeouts, changed retry candidates, in-flight
  configuration publication, real relay/credential replacement and protected pin redaction.
- `./scripts/check.sh full`.
