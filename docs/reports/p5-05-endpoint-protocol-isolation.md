# P5-05 Endpoint protocol isolation report

| Field | Value |
|---|---|
| Plan version | `v1.9` |
| Task | `P5-05` |
| Date | `2026-07-22` |
| Branch | `codex/p5-anthropic` |
| Status | `DONE` |
| Scope / budget | `M`; compiler-to-Snapshot Endpoint format propagation and native routing isolation |
| References | Matrix `L08`, `L21`, `L22`, `L40`; [ADR-0038](../adr/ADR-0038-endpoint-format-isolated-protocol-routing.md); [BC-ROUTER-005](../contracts/BC-ROUTER-005-endpoint-format-isolated-protocol-routing.md) |

## Delivered boundary

`EndpointConfiguration.api_format` now survives compilation and Snapshot publication on each
`SnapshotRouteCandidate`. The Router recognizes only exact P5 formats and exposes a
protocol-filtered same-protocol Canonical Attempt entrypoint. Its Candidate predicate runs before
Health, Quota, and Credential lease admission, while all runtime failure state continues to be
keyed by `EndpointId`.

The controlled E2E builds one Upstream with separate `openai/responses` and
`anthropic/messages` Endpoints. A Responses connection failure opens only the Responses
Endpoint's cooldown. A subsequent Anthropic request selects its own Endpoint/Credential and
succeeds, proving the shared Upstream ID does not contaminate protocol routing or Circuit state.

## Review

The review checked the data flow from the persisted config field through the compiler and snapshot
publisher, then checked that the runtime filter precedes every Health/Quota/Credential-pool read.
The final code does not infer format from model names or URLs, does not add any loose alias, and
does not claim that a native filter itself approves a cross-protocol bridge. P5-04 remains the
mandatory semantic admission boundary for a future bridge caller.

## Verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p gateway-router -p gateway-control -p gateway-http-actix` | PASS; exact-format, compiler propagation, and same-Upstream single-protocol-fault isolation coverage included |
| `cargo clippy --locked -p gateway-router -p gateway-control -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` and staged Secret scan | PASS |
| `CHECK_REPORT_PATH=tmp/p5-05-fast-check.md ./scripts/check.sh fast` | PASS |

No real Provider credential, request, Endpoint, or network probe was used.

## Rollback and next Task

Reverting this Task removes only in-memory Candidate format metadata and native protocol filtering;
it requires no database migration, Credential rotation, or external cleanup. P5-06 is the sole
next code Task and owns the explicit Thinking, stop-reason, Usage/cache, response-model, and
client-facing Messages semantics. P5 still has one Phase-level remote Delivery Gate after G5.
