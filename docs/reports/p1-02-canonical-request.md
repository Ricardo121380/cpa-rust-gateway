# P1-02 Canonical request report

| Field | Value |
|---|---|
| Plan | `v1.0` |
| Task | `P1-02` |
| Date | `2026-07-18` |
| Branch | `codex/p1-02-canonical-request` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added a framework-independent `CanonicalRequest` that retains the client-supplied requested
  model before public-model resolution, routing, credentials, or provider selection.
- Added ordered messages with text, historical Tool Call, Tool Result, and explicit opaque-content
  envelope forms. The opaque form is adapter-selected as `{"opaque":{"raw":...}}`; an unknown
  external tag is never silently reclassified by the core.
- Added Tool definitions with raw JSON Schema, historical Tool arguments/results, open-ended
  non-empty Thinking effort, and verbatim `prompt_cache_key` / `prompt_cache_retention` fields.
  Omitting request-level Thinking means no explicit effort request; `null`, an empty Thinking
  object, and an empty effort are rejected.
- Added `RawJson` and named `RawExtensions`, retaining raw JSON subtrees without `flatten`.
  Extensions reject duplicate field names at JSON and in-memory construction boundaries and expose
  controlled insertion plus deterministic read-only iteration for later adapters.
- Added desensitized fixture and round-trip coverage. Request-carried values are redacted from
  `Debug`, including prompts, Tool text, cache fields, thinking labels, and opaque JSON.
- Added direct `serde` / `serde_json` dependencies only to `gateway-core` and updated the crate
  boundary policy accordingly.

## Verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-core` | PASS; 24 unit tests plus doc tests |
| `cargo clippy --locked -p gateway-core --all-targets --all-features -- -D warnings` | PASS |
| `./scripts/check.sh fast` | PASS |
| `./scripts/check.sh full` | PASS; dependency policy and RustSec audit pass |
| `git diff --check` | PASS |

## Review

- Independent reviews verified that `requested_model` remains a route-agnostic client string and
  that P1-02 remains entirely in `gateway-core`.
- Review identified four boundary issues before acceptance: diagnostic leakage through derived
  `Debug`, unconstructible/enumerable extensions, a supplied Thinking object without effort, and
  explicit `thinking: null` being accepted as absence. All four were corrected with regression
  tests and contract updates.
- Final independent reviews: PASS. No CanonicalEvent, state machine, stream, HTTP, provider,
  routing, credential, or persistence functionality was introduced.

## Limits and next task

P1-02 defines only canonical request semantics. It does not implement event sequencing, Tool
argument streaming, `{}` normalization, HTTP encoding, provider execution, routing, or storage.
`P1-03` remains `PENDING` and must be explicitly marked `IN_PROGRESS` before work begins.
