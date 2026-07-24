# P5-02 Exact token-count capability report

| Field | Value |
|---|---|
| Plan version | `v1.9` |
| Task | `P5-02` |
| Date | `2026-07-22` |
| Branch | `codex/p5-anthropic` |
| Status | `DONE` |
| Scope / budget | `M`; Canonical count contract, explicit Provider capability, Router seam, and one Anthropic HTTP route only |
| References | Matrix `A08`, `B22`, `B25`; [ADR-0035](../adr/ADR-0035-exact-token-count-capability.md); [BC-PROTOCOL-003](../contracts/BC-PROTOCOL-003-exact-token-count-capability.md) |

## Scope and invariant

P5-02 admits `POST /v1/messages/count_tokens` only after normal bearer authentication, duplicate-
safe Anthropic decode, and Snapshot model resolution. A successful response is possible only from
an explicitly exact capability and is exactly `{"input_tokens": <u64>}`. Absent exactness is an
explicit, safe rejection; this Task has no text/byte/tokenizer estimate, Provider transport,
credential lookup, Endpoint selection, real request, background worker, or configuration fallback.

The canonical request keeps the model reference submitted by the client. For a Snapshot Alias, the
separate `RouteId` handed to the executor proves the resolved authorized Route. This avoids both
alias-only routing and premature mutation of the data a later Provider encoder may need.

## Implemented boundary

- Added opaque `ExactInputTokenCount` and stable `TokenCountUnsupported/Model` safe error.
- Added Provider `ExactTokenCountAdapter` / two-state `TokenCountCapability`, with no estimator.
- Added Router `CountTokensExecution` / `CountTokensExecutor`; the direct adapter executor is
  deliberately limited to an explicitly exact capability until P5-05 owns Endpoint aggregation.
- Extended the pure Anthropic codec with count-request decoding and one-field exact response
  encoding; streaming count requests are rejected.
- Added Actix route registration, safe Anthropic pre-header errors, Snapshot model resolution, and
  default explicit rejection. The HTTP state only runs an injected count executor when it is
  supplied by composition.
- Added error mappings so the new frozen core category remains safe and exhaustive in both
  Anthropic and OpenAI protocol encoders.

## Review conclusion

Review passed after two bounded corrections. The handler comment was corrected to state the actual
invariant: the canonical request intentionally retains the Alias while the approved `RouteId`
proves resolution. Focused Clippy then required a `let-else` test form and merged equal protocol
error arms; neither changed the behavior or boundary.

The final review found no estimate path, no mutation or network path, no leakage of route/provider
details in the public success or error shapes, and no reverse HTTP/Provider dependency. Snapshot
resolution occurs before count execution; the E2E test proves the routed executor receives the
Route ID and the Responses executor remains unused.

## Local verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p protocol-anthropic -p gateway-http-actix -p gateway-provider -p gateway-router` | PASS; 125 unit/integration tests plus 2 explicitly ignored authorization-only harnesses; new exact, unsupported, and Snapshot-route E2E coverage passed. |
| `cargo clippy --locked -p protocol-anthropic -p gateway-http-actix -p gateway-provider -p gateway-router --all-targets --all-features -- -D warnings` | PASS after focused corrections. |
| `ruby scripts/check-crate-boundaries.rb` and `ruby scripts/check-source-policy.rb` | PASS; 21 workspace packages and 74 Rust files / 21 crate roots. |
| `scripts/secret-scan.sh --staged` and `git diff --cached --check` | PASS; no staged Secret or whitespace defect. |
| `CHECK_REPORT_PATH=tmp/p5-02-full-check.md ./scripts/check.sh full` | PASS in 65 seconds (`2026-07-21T17:08:40Z` to `2026-07-21T17:09:45Z`); shell/CI/plan guards, format, workspace Clippy/tests, source/crate policy, links, Secret scans, whitespace, pinned tools, `cargo deny`, and RustSec audit all passed. |

No ignored authorization-only harness ran. No real Provider credential, request, Endpoint, or
network probe was used.

## Rollback and next Task

Reverting this Task removes only the count contract and route boundary. It requires no migration,
credential rotation, Provider cleanup, or network rollback. The final local Full Gate and staged
review passed, so this commit records P5-02 as `LOCAL_PASS_PENDING_PHASE_GATE` and starts P5-03 as
the sole `IN_PROGRESS` Task. P5 still has one final Phase-level remote Delivery Gate.
