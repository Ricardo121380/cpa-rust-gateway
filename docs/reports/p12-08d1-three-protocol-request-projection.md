# P12-08D1 three-protocol typed request projection

| Field | Value |
|---|---|
| Plan | `v1.99` |
| Task | `P12-08D1` |
| Date | `2026-08-02` |
| Branch | `codex/p12-deployment` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Legacy reference | CLIProxyAPI `v7.2.101`, commit `42a00a2a6521b867c27f7ad096d08699db8e6d19` |
| Network or deployment change | None |

## Outcome

The Router now prepares either an exact native request or a target-shaped Canonical request for
all nine Chat Completions, Responses and Messages source/target pairs. Cross-protocol requests no
longer depend on the old blanket rejection of Tool history and Thinking: ordered typed Tool calls,
Tool results, output-token limits and the pinned legacy Reasoning levels are admitted only where
the selected target can encode them. The public rejection reasons remain value-free.

This slice stops at request preparation. It does not publish new runtime pairs, translate a
response, contact an upstream, change a Config Version, or alter production traffic.

## Implemented boundary

- `ProjectedProtocolRequest::NativeExact` preserves caller-owned bytes only for an exact
  same-protocol `Passthrough` route.
- `ProjectedProtocolRequest::Canonical` carries a cloned target-shaped request for same-protocol
  Canonical mode or one of the six explicit cross-protocol bridges.
- Output limits map exactly between `openai.chat.max_tokens`,
  `openai.responses.max_output_tokens`, and `anthropic.messages.max_tokens`; Messages requires one
  positive integer, while collision, zero, negative, floating-point and foreign fields reject.
- Leading Responses/Chat `developer` maps to Messages `system`; a later `developer`, later
  `system`, invalid role/content shape, empty message or malformed Tool definition rejects.
- Assistant Tool calls and correlated `tool` results retain order, ID, name, arguments and output.
  Chat/Responses reject error results or non-string result forms that their current builders cannot
  re-encode; Messages retains its broader typed result representation.
- Responses Reasoning levels use the frozen legacy effort/budget table. Messages
  `enabled`/`adaptive`/`disabled` maps back only through the documented positive-budget subset.
- Explicit Thinking now requires the Endpoint `reasoning` capability. Historical Tool content also
  requires `tools` and `json_schema`, not merely a current request Tool declaration.
- The approved projection is exercised against all three real upstream request builders without
  opening a socket.

## Behavior classification

| Classification | Result |
|---|---|
| `PARITY` | Nine pair topology; exact native ownership; ordered Text and Tool history; target model remains separately selected; output-limit mapping; documented Reasoning effort/budget mapping |
| `INTENTIONAL_HARDENING` | Explicit pair modes only; positive output limits; target-specific roles and Tool result shapes; Endpoint capability checks include historical Tool and Reasoning semantics; safe value-free diagnostics |
| `UNSUPPORTED_FAIL_CLOSED` | Opaque image/audio/file blocks, foreign or nested extensions, prompt-cache controls, signed/encrypted thinking, unknown Reasoning labels, wire-level Tool-choice/sampling controls that lack a portable Canonical field, and target shapes the current typed builders cannot re-encode |

No observed D1 difference remains unclassified.

## Verification

| Command or evidence | Result |
|---|---|
| `cargo test --locked -p gateway-router protocol_transform` | PASS; 16 focused tests |
| Nine typed source/target pair matrix | PASS |
| Three exact native same-protocol pairs | PASS |
| `tests/fixtures/router/p12-08d1-typed-tool-history.json` through Chat, Responses and Messages builders | PASS |
| Positive output-limit property test across all nine pairs | PASS |
| `cargo clippy --locked -p gateway-router --all-targets -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `./scripts/check.sh docs` | PASS |
| `git diff --check` | PASS |

## Review conclusion and remaining boundary

- The projection API and its `Debug` representation do not expose model, prompt, Tool, output or
  extension values.
- Native payload availability is still a proof obligation; Canonical reconstruction never claims
  byte identity.
- The fixture is synthetic and contains no account, endpoint, credential, production body or log.
- P12-08D2 owns non-streaming and SSE response conversion, bounded buffering and terminal
  lifecycle parity. P12-08D3 alone may publish these pairs in the runtime and Route Explain.
- D3 must keep a pair ineligible whenever a decoded request retains Tool-choice, sampling or other
  protocol controls outside this D1 typed subset; native same-protocol mode remains their only
  preservation path until Canonical gains an explicit contract.
- P12-09 production cutover remains pending.
