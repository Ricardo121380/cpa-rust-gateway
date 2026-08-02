# P12-08D4 legacy CPA to CPAR protocol differential

| Field | Value |
|---|---|
| Plan | `v1.102` |
| Task | `P12-08D4` |
| Date | `2026-08-02` |
| Branch | `codex/p12-deployment` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Legacy reference | CLIProxyAPI `v7.2.101`, commit `42a00a2a6521b867c27f7ad096d08699db8e6d19` |
| Network or deployment change | None |

## Outcome

A new clean-room golden corpus now closes the D0-D4 three-protocol port. The reference side stores
only value-free semantic markers derived from the pinned legacy translator source and tests. The
gateway side is never accepted from a literal alone: every validation drives current CPAR Chat
Completions, Responses, Messages or Router response-projection code and compares the freshly
observed markers with the committed expectation.

The corpus contains ten mandatory, unique cases:

| Coverage | Cases | Classification |
|---|---:|---|
| Chat JSON and multi-boundary chunked SSE with Tool fragments and final Usage | 2 | `PARITY` |
| Responses JSON/SSE with Reasoning, Text, Tool where applicable, final Usage and terminal order | 2 | `PARITY` |
| Messages JSON/SSE with Thinking/Reasoning, Text, Tool where applicable, final Usage and terminal order | 2 | `PARITY` |
| Duplicate JSON and missing SSE terminal behavior | 2 | `INTENTIONAL_HARDENING` |
| Reasoning projected to Chat and multiple Chat choices | 2 | `UNSUPPORTED_FAIL_CLOSED` |

Result: six parity cases, two intentional hardening differences and two unsupported/fail-closed
differences. No case is unclassified and the accepted taxonomy contains no `REGRESSION` variant.

## Corpus and execution boundary

- The committed JSON corpus contains only scenario IDs, the exact pinned legacy reference,
  semantic marker sequences, a closed classification and a closed decision. It contains no
  Credential, account, endpoint, URL, header, captured production body, log or token.
- Newly minimized synthetic wire inputs live inside the offline probe. They use fixed local values
  and exercise all three real complete-JSON decoders and all three real SSE decoders at nontrivial
  byte chunk boundaries.
- The six parity probes require protocol-specific Reasoning, Tool and Usage markers rather than
  merely accepting any self-consistent expectation. Removing a required marker fails the corpus.
- Duplicate JSON is rejected by the real Chat decoder. A Chat SSE body without `[DONE]` is rejected
  as truncated rather than receiving the legacy synthetic-success terminal.
- The D2 Router projector rejects Reasoning-to-Chat. The Chat decoder rejects multiple choices
  because Release 1 Canonical owns one selected generation.
- Corpus size, case count, case IDs, unique subjects, marker count, pinned reference,
  classification/decision pair and forbidden metadata fields are all bounded and validated.

## Behavior classification

| Classification | Reviewed result |
|---|---|
| `PARITY` | Text/role lifecycle, Tool ID/name/argument fragment order, distinct Reasoning, final Usage, stop semantics and one legal terminal sequence are preserved across Chat, Responses and Messages JSON/SSE |
| `INTENTIONAL_HARDENING` | Duplicate JSON does not inherit permissive last/first-member behavior; incomplete SSE cannot synthesize success at EOF |
| `UNSUPPORTED_FAIL_CLOSED` | Private Reasoning is not degraded into Chat text; multiple Chat choices are not silently collapsed into the single-generation Canonical model |

The D0 request/response ledger has no remaining unclassified D1-D4 difference. Behaviors outside
Release 1 Text/Tool/Reasoning/Usage/History remain unavailable unless a later approved task extends
Canonical explicitly.

## Verification

| Command or evidence | Result |
|---|---|
| `cargo test --locked -p differential-gate` | PASS; existing P11 6/6 and new D4 3/3 tests |
| Ten mandatory D4 scenarios executed through current protocol/router code | PASS; 6 parity, 2 hardening, 2 unsupported/fail-closed |
| Missing case, stale expectation, relabelled decision and hollowed semantic coverage mutations | PASS; all rejected |
| Forbidden metadata and unknown `REGRESSION` classification mutations | PASS; rejected without echoing values |
| `cargo clippy --locked -p differential-gate --all-targets -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |

## Review conclusion and next boundary

- This is an offline compatibility audit, not a live Provider test. It proves the D1-D3 protocol
  transformations and safety differences represented by the corpus; it does not prove any account,
  endpoint, model, quota or production route is currently usable.
- No old CPA executable, server, Credential, endpoint, production body or log was invoked or copied.
- P12-08D0-D4 are locally complete and await the single P12 Phase Delivery Gate. P12-08 as a whole
  remains `IN_PROGRESS`.
- P12-08E1 next owns the Codex and generic OpenAI-compatible runtime vertical slice. Production
  graph composition, live traffic and cutover remain outside D4.
