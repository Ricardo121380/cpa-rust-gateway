# P1-09 Tool stream property-test report

| Field | Value |
|---|---|
| Plan | `v1.0` |
| Task | `P1-09` |
| Date | `2026-07-18` |
| Branch | `codex/p1-09-tool-chunk-properties` |
| Status | `DONE` |
| Result | PASS; independent review and GitHub Fast/Full CI complete |

## Delivered scope

Establish reproducible property coverage for the P1 boundary from already formed
`CanonicalEvent` Tool fragments to OpenAI Responses SSE and non-streaming output. The suite must
exercise two logically parallel, interleaved Tool Calls, arbitrary valid fragment schedules, and
the explicit canonical empty object used by `EnterPlanMode`, `ExitPlanMode`, and an ordinary
no-argument Tool.

## Scope boundary

This task tests the existing P1-03/P1-05 behavior; it does not add a Provider, an upstream
network-byte parser, JSON assembly, or empty-input normalization.

- A generated fragment is an already decoded UTF-8 `String`, split only at scalar boundaries.
- The one-byte regression uses ASCII JSON so every byte is also a valid string boundary.
- The three no-argument cases enter the Canonical boundary with an explicit `RawJson` value of
  `{}`. The assertion is preservation of that value, not automatic conversion of missing, empty,
  or whitespace-only upstream input.
- A different fragment schedule may contain a different number of
  `ToolCallArgumentsDelta` events. The invariant is each call's reassembled argument projection,
  final `function_call_arguments.done` value, and non-streaming Function Call output; it is not
  byte-for-byte equality of the raw Canonical Event vector.
- Raw network-byte chunk invariance, Unicode bytes split inside a scalar, AWS EventStream framing,
  and source-side Plan Mode normalization remain future Provider/Kiro work and are not claimed by
  this task.

## Delivered coverage

1. Added a `proptest` development dependency only to `protocol-openai-responses`, with the crate
   boundary allowlist and lockfile updated together.
2. Added an integration test that builds a valid response containing two Tool Calls whose argument
   fragments are interleaved while preserving each call's local order. It must drive both
   `CanonicalResponse::try_new` and `OpenAiResponsesSseEncoder`.
3. Added a one-byte ASCII regression containing escaped JSON syntax and the three named no-argument
   Tools. It asserts zero argument-delta frames and final string value `"{}"` for every no-argument
   Tool.
4. Added a fixed-seed property suite covering randomized scalar-boundary splits and interleavings.
   It checks call ID/name stability, per-call delta reassembly, exactly one arguments-done and
   output-item-done frame per call, monotonic SSE sequence numbers, exactly one completion frame,
   and equivalent non-streaming Function Call arguments.
5. Added a negative regression that changes a completed Tool's full arguments after its
   deltas and requires the existing safe `UpstreamProtocolError/Stream` rejection.
6. Added a separately invoked random-seed runner. It prints and accepts `P1_09_SEED` so any
   failing random schedule is reproducible, and it must avoid writing an untracked proptest
   failure-persistence file.

## Delivered files

| File | Purpose |
|---|---|
| `Cargo.toml` | Shared test-only dependency version |
| `Cargo.lock` | Locked dependency graph |
| `crates/protocol-openai-responses/Cargo.toml` | Test-only dependency declaration |
| `crates/protocol-openai-responses/tests/p1_09_tool_chunk_properties.rs` | P1-09 property and regression coverage |
| `scripts/check-crate-boundaries.rb` | Explicit dependency-boundary policy update |
| `scripts/run-p1-09-property.sh` | Random-seed generation and replay command |
| `docs/reports/p1-09-tool-stream-property-tests.md` | Scope, verification, and review evidence |
| `docs/reports/README.md`, `docs/traceability.md` | In-review evidence link and task tracking |

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p protocol-openai-responses --test p1_09_tool_chunk_properties` | PASS; three tests plus one explicitly ignored random-seed test |
| `P1_09_SEED=428937513 scripts/run-p1-09-property.sh` | PASS; reproducible 256-case random suite |
| `scripts/run-p1-09-property.sh` | PASS; generated and recorded seed `14057364930279805079`, 256 cases |
| `cargo clippy --locked -p protocol-openai-responses --all-targets --all-features -- -D warnings` | PASS |
| `./scripts/check.sh fast` | PASS |
| `./scripts/check.sh full` | PASS; dependency policy and RustSec audit pass |
| `git diff --check` | PASS |
| GitHub Actions run `29650157066` | PASS; Fast gate and Full supply-chain gate |

## Independent review

Independent read-only review passed. It confirmed that the property model exercises P1-03 Tool
lifecycle and P1-05 per-call SSE accumulation through actual public APIs; it verifies stable call
ID/name mappings, one completion per call, explicit `{}` preservation, and rejection of mismatched
final arguments. The review also confirmed that `proptest` is test-only, its lockfile and crate
boundary policy are synchronized, fixed/random seeds are reproducible, failure persistence is
disabled, and the runner script quotes values under strict shell mode.

## GitHub verification and approved phase boundary

GitHub Actions run `29650157066` completed with both Fast and Full gates passing, so P1-09 is
`DONE`. `CR-P1-G1-001` was approved on `2026-07-19`: G1 uses this suite's established invariant
that arbitrary valid, already-decoded Tool argument fragment schedules preserve the same Tool
semantic projection, while raw `ToolCallArgumentsDelta` boundaries may differ. This task still
does not claim raw network-byte partition invariance, EventStream parsing, or source-side
empty/whitespace normalization; those require a future Provider ingress implementation.
