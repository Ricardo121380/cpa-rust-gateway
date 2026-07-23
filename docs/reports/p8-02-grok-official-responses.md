# P8-02 Grok Official Responses HTTP/SSE report

| Field | Value |
|---|---|
| Plan version | `v1.38` |
| Task | `P8-02` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-G7-001` |
| Scope / budget | `M`; synthetic text-only Official HTTP/SSE boundary; no real Provider traffic, server, route, or state change |
| References | Matrix `C01`、`C03`、`C31`、`G24`、`G27`; [ADR-0055](../adr/ADR-0055-grok-official-responses-boundary.md); [BC-PROVIDER-013](../contracts/BC-PROVIDER-013-grok-official-responses-boundary.md) |

## Delivered evidence

`provider-grok` now contains a native `grok.official` Responses vertical slice separate from
Build OAuth. It constructs a fixed `POST https://api.x.ai/v1/responses` request with only standard
JSON/Bearer headers, and grants the shared client access only after exact DNS-pinned target
admission. The API key stays in zeroizing request-scoped authorization memory; diagnostics redact
the target, key, bearer value, model, body, response IDs, and text.

The P8-02 request codec is intentionally text-only. It preserves extension-free text messages in
the standard Responses roles and rejects Tools, Thinking, Search-like opaque data, cache fields,
history Tool data, extensions, and unknown roles before transport dispatch. This makes P8-04 an
explicit semantic expansion rather than allowing lossy requests today.

The non-streaming and SSE decoder emits a validated Canonical lifecycle for completed assistant
`output_text`, including final usage when present. Strict duplicate-field JSON, bounded SSE
records, response/item correlation, terminal completion, and arbitrary chunk boundaries are
enforced. Before the canonical start event every failure remains a generic safe provider error;
after it, malformed/truncated transport and decoder failures produce exactly one `StreamError`.
No P8-03 status/quota/billing behavior is inferred.

No xAI endpoint, credential source, OAuth cache, server process, route, proxy/TUN rule, or
production configuration was read or changed. No Official E2E was sent. P7's G7 remains blocked
on Kiro account reauthentication, so P8 cannot close, push a Phase Delivery Gate, merge, release,
or claim `DONE` before the CR's prerequisites pass.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p8_02_official_responses` | PASS; 6 synthetic tests cover exact POST/headers/egress/redaction, non-stream canonical lifecycle, chunk-invariant SSE, deferred-semantic rejection, `response.failed`, and pre-/post-start failures. |
| `cargo test --locked -p provider-grok` | PASS; 56 active tests passed and 2 pre-existing explicitly authorized P6 live harness tests remained ignored. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --all-targets --all-features -- -D warnings`, `git diff --check` | PASS. |
| [`./scripts/check.sh full`](p8-02-local-full-check.md) | PASS; workspace format/Clippy/tests, source and crate-boundary policies, document links, Secret checks, dependency policy, and RustSec audit all passed locally. |
| Focused code review | PASS: no Build dependency, strict text-only reject behavior, exact egress, bounded buffers, strict JSON/SSE framing, and one-terminal-stream-error rule are retained. |

## Rollback and next task

Rollback removes the P8-02 Official Responses module, fixtures/tests, ADR, contract, report, and
traceability/index links. It has no external effect. P8-03 is the next local task after final P8-02
review: it will add explicit Rate/Quota/Reset/Billing metadata, without sending a real request or
changing the G7/P8 closeout boundary.
