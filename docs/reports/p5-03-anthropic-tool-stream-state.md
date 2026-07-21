# P5-03 Anthropic Tool stream state report

| Field | Value |
|---|---|
| Plan version | `v1.9` |
| Task | `P5-03` |
| Date | `2026-07-22` |
| Branch | `codex/p5-anthropic` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope / budget | `M`; pure response state, fixtures, property coverage, and documentation only |
| References | Matrix `B12`, `B14-B16`; [ADR-0036](../adr/ADR-0036-anthropic-tool-stream-state.md); [BC-PROTOCOL-004](../contracts/BC-PROTOCOL-004-anthropic-tool-stream-state.md) |

## Delivered boundary

`protocol-anthropic` now encodes Canonical Tool start/delta/end events into Anthropic `tool_use`
content blocks in both non-streaming Messages responses and typed SSE. A `call_id` stays the
client-visible Tool ID, and each call owns a stable block index plus independent accumulated JSON.
Two Tools may receive interleaved decoded argument fragments without mixing their output.

The encoder starts every Tool with `input: {}`. It emits incremental `input_json_delta` frames for
non-empty fragments, verifies the final `ToolCallEnd` object against the accumulated fragments,
then emits that Tool's block stop. A Tool that arrives only as a final non-empty object gets one
complete delta; explicit empty input and whitespace-wrapped `{}` normalize to `{}` without a
synthetic delta. Arrays and scalars fail closed because Anthropic `tool_use.input` must be an
object.

## Required-field boundary

P5-03 does not guess required parameter names: its response API receives no original request Tool
schema and it does not execute Tools. It proves an output is a complete JSON object, while P5-07's
request-and-execution composition must use the declared schema to reject a missing required
property before execution. This is an explicit scope boundary, not a claim that an absent property
is valid.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p protocol-anthropic` | PASS; 11 unit tests, 8 P5-03 integration/property tests, and doc tests |
| `cargo clippy --locked -p protocol-anthropic --all-targets --all-features -- -D warnings` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS; `proptest` is test-only and explicitly allowlisted for `protocol-anthropic` |
| `git diff --check` | PASS |
| `CHECK_REPORT_PATH=tmp/p5-03-fast-check.md ./scripts/check.sh fast` | PASS; workspace format/Clippy/tests, plan and CI guards, source/crate policies, links, Secret scans, and whitespace passed |
| `CHECK_REPORT_PATH=tmp/p5-03-serial-full-check.md ./scripts/check.sh full` | PASS after the SSE serialization review correction; workspace tests, format/Clippy, source/crate policy, links, tracked Secret scan, pinned-tool checks, `cargo deny`, and RustSec audit all passed. |

Review found and corrected four narrow issues before this evidence: the Tool encoder now rejects
non-object JSON input, whitespace-wrapped empty objects normalize to `{}` without a redundant
input delta, an empty decoded fragment is ignored rather than creating a false final mismatch, and
interleaved Canonical Tool states are buffered into non-overlapping Anthropic wire block lifecycles.
The fixed-seed property suite now also proves that every SSE delta/stop targets the one active
content block. The final review found no global Tool accumulator, no ID remapping, no implicit JSON
completion, no request-schema fabrication, and no HTTP/Provider dependency.

## Known limits, rollback, and next Task

This Task handles decoded Canonical fragments, not raw network bytes; UTF-8 byte splitting and
source-side parsing remain Provider ingress work. It also does not perform actual Tool execution,
authentication, or Claude Code E2E. Reverting it removes only pure codec behavior, fixtures,
property tests, and documents; it requires no migration, credential action, or network rollback.
P5-04 may now add the lossless Bridge capability analyzer while P5 retains one final Phase-level
remote Delivery Gate.
