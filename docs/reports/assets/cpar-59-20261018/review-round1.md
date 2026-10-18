# CPAR #59 fixed candidate review, round 1

Baseline: `3369c730e18f095bb4d9e1b506061e985a475fb9`.
Candidate: `eaecc9ada7a0fc0c8e4a09fb67ddd72e9a72c718`.
Tree: `39f13bc8bed120acdf1eebd470133b0cdad19844`.
Scope: only the captured #59 increment; existing dirty content excluded.
Two independent read-only agents reviewed the immutable diff. No Provider calls or file mutations.

## Standards

FAIL. One hard violation P2, one heuristic P3; worst within Standards: P2.

P2: `crates/gateway-store/src/native_usage.rs:170-178` loaded only payload_json and
`verify_lineage` compared payload identities/counters. BC-OBS-002 requires derived
type/id/correlation/time to match indexed columns, or reads fail closed. Ordinary
`event_store::decode_stored_event` already enforces this. Corruption of request_id or
occurred_at_ms could therefore pass the private reader while failing the ordinary reader.

Resolution: reproduced via the public reader on an isolated corrupt file
(`index-integrity-red.log`), then reused the existing canonical event decoder, including all
indexed fields. `storage-index-green.log` covers Request/Attempt/Usage correlation and time
corruption and successful reads after restoring exact indexed values. The preliminary
`index-integrity-fixture-guard.log` is an admission-guard fixture failure, not behavioral RED.

P3, possible Duplicated Code: ordinary and sensitive confirmation methods both contain the
oneshot enqueue/2s timeout/counter block. This is a nonblocking judgement call. Kept the two
short methods with their different admission semantics; no new abstraction is needed to
correct #59. Both use the same bounded writer and confirmation result.

## Spec

PASS. Seven of seven acceptance conditions have implementation and matching evidence.
Missing/partial: 0; scope creep: 0; implemented incorrectly: 0; worst within Spec: none.

Verified public production compiler/factory + JSON/SSE routes and independent TCP receive
count; exact raw Usage/source/owner binding; atomic encrypted persistence and authorized
reader/CLI; replay/conflict/tamper/key/write/confirmation failures; one Usage/source/ledger
and unpriced native cost; schema/TTL/key/reopen/encrypted recovery; strict rejection and no
EOF read-ahead. The archived RED/GREEN logs and candidate source hashes were inspected.
No live Provider, parent three-client or release qualification is implied.

Standards: 1 hard + 1 heuristic (worst P2). Spec: 0 findings (worst none).
