# P5-06 Anthropic semantic and HTTP boundary report

| Field | Value |
|---|---|
| Plan | `v1.9` |
| Task | `P5-06` |
| Date | `2026-07-22` |
| Branch | `codex/p5-anthropic` |
| Status | `DONE` |
| Scope / budget | `M`; explicit Thinking/stop/Usage-cache semantics and one authenticated Messages HTTP route; no Provider network implementation |
| References | Matrix `A07`, `B09`, `B23-B26`, `F01-F04`, `F09-F11`; [ADR-0039](../adr/ADR-0039-anthropic-semantic-http-boundary.md); [BC-PROTOCOL-005](../contracts/BC-PROTOCOL-005-anthropic-semantic-http-boundary.md) |

## Delivered boundary

- `ResponseEnd` now has explicit, debug-redacted optional stop reason/sequence fields. The
  Anthropic encoder requires a non-empty reason and uses it verbatim; no Tool-count fallback
  remains.
- Anthropic request decoding explicitly maps `thinking.type`, validates and retains
  `budget_tokens`, derives a single proven cache-retention semantic from compatible `ephemeral`
  cache controls, retains exact block-local controls, and rejects conflicting/unsupported controls.
- Anthropic response JSON/SSE encodes `ReasoningDelta` as `thinking` blocks, cache read/creation
  Usage fields with Anthropic names, explicit stop reason/sequence, and merged partial Usage.
  Unrepresentable generic reasoning/cache totals remain fail closed.
- `POST /v1/messages` now reuses existing authentication, Snapshot resolution, bounded Canonical
  stream, FSE commit, cancellation, and Usage observation. It emits only the resolved public model
  and labels request observations `AnthropicMessages`.

## Review

The closeout review followed the semantic value from the new `ResponseEnd` fields through
Canonical lifecycle validation into both completed Messages JSON and terminal SSE frames. It also
checked that partial Usage merging never fabricates an Anthropic field, block-local cache controls
remain raw placement-preserving extensions, and both JSON/SSE paths preserve the established
authenticated Snapshot, public-model, cancellation, and first-semantic-event boundaries. No
scope-expanding Provider transport, bridge admission, or secret-bearing diagnostic path was found.

## Evidence

| Check | Result |
|---|---|
| Core + Anthropic codec + Actix targeted tests | PASS |
| Frozen Thinking/cache/stop JSON and SSE fixture | PASS |
| Authenticated Messages JSON/SSE and Snapshot alias tests | PASS without external Provider traffic |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked -p gateway-core -p protocol-anthropic -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| `CHECK_REPORT_PATH=tmp/p5-06-fast-check.md ./scripts/check.sh fast` | PASS; workspace tests, source policy, docs links, Secret scan, and whitespace checks passed |
| `git diff --check` and staged Secret scan | PASS |

## Known limits and next task

- There is still no real Anthropic Provider request serializer or network implementation.
- P5-04 bridge admission remains fail closed for reconstructed Thinking/cache-control requests;
  raw block placement is retained precisely so it cannot be mistaken for a safe bridge.
- `reasoning_tokens`, generic `cached_tokens`, and opaque Usage extensions remain rejected because
  the Anthropic public Usage contract has no lossless field for them.
- P5-07 owns the local Claude Code `--bare` and Plan Mode client E2E; this task does not begin P6.
