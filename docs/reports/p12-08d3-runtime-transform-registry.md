# P12-08D3 runtime transform registry and Route Explain

| Field | Value |
|---|---|
| Plan | `v1.101` |
| Task | `P12-08D3` |
| Date | `2026-08-02` |
| Branch | `codex/p12-deployment` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Legacy reference | CLIProxyAPI `v7.2.101`, commit `42a00a2a6521b867c27f7ad096d08699db8e6d19` |
| Network or deployment change | None |

## Outcome

The reviewed three-protocol registry is now connected to route selection and request execution.
Only the nine explicit Chat Completions, Responses and Messages source-to-target pairs can be
considered. A request-local conversion and capability predicate runs before the scheduler touches a
Credential pool or starts an upstream Attempt. A rejected candidate therefore consumes no lease,
does not invoke its driver and produces no Attempt row.

The path choice is deterministic: an exact same-protocol native body requires `Passthrough`; a
same-protocol typed request uses `Canonical`; a cross-protocol typed request requires
`LosslessBridge`. The OpenAI Responses and Anthropic Messages providers now revalidate native mode
and replace only the selected upstream model. Kiro remains Canonical-only because it shares
Messages semantics but does not speak the native Anthropic HTTP body.

Decoded upstream events now pass through the D2 stateful response projector before they reach the
client encoder. Production Responses JSON and SSE use the protocol-owned bounded decoder; the old
runtime decoder remains test-only until D4 consumes its regression corpus.

## Publication and Explain boundary

- Route Explain accepts all three client protocols and applies the same registered-pair topology
  and response capability rule used by runtime admission.
- An unavailable transform is reported only as the value-free reason
  `protocol_transform_unavailable`; no request text, model value, Endpoint, URL, Credential,
  native body or extension value enters that reason.
- Explain remains projection-only: selecting a candidate does not advance scheduling cursors,
  acquire a lease or create an upstream Attempt.
- A Chat client cannot publish a candidate whose capability contract says the upstream may return
  Reasoning. Runtime projection also rejects this before request conversion, so Reasoning is never
  degraded to Chat text.
- The current deployed Config Version was not changed. Existing capability-empty candidates do not
  gain streaming, Tool, JSON Schema, parallel Tool or Reasoning admission by implication. The later
  P12-08F1 capability ledger must prove those features before such routes can be published.

## Behavior classification

| Classification | Result |
|---|---|
| `PARITY` | Explicit three-by-three protocol registry, native same-protocol preservation, typed Canonical execution, client-protocol response encoding and explainable candidate selection |
| `INTENTIONAL_HARDENING` | Request-local eligibility before lease/attempt, capability-proof requirement, strict native-mode revalidation, Kiro native-body exclusion, protocol-owned bounded Responses decoder and value-free rejection reasons |
| `UNSUPPORTED_FAIL_CLOSED` | Unregistered pair, mismatched transform mode, unavailable exact native body, undeclared request capability, Reasoning-capable source to Chat, Kiro native Anthropic payload and any D1/D2 unrepresentable semantic |

No observed D3 difference remains unclassified.

## Verification

| Command or evidence | Result |
|---|---|
| `cargo test --locked -p gateway-router -p provider-openai-compatible -p provider-anthropic-compatible -p gateway-http-actix -p gateway` | PASS; target package unit, integration and doc tests passed; existing explicitly ignored live/soak harnesses remained ignored |
| Reviewed registry across all nine pairs and exact mode topology | PASS |
| Request-local rejection before pool lease and driver invocation | PASS; zero active lease and zero attempt invocation |
| Native Responses and Messages provider revalidation | PASS; only model replacement is permitted |
| Runtime Responses strict JSON/SSE decode and D2 event projection | PASS |
| Route Explain Chat projection and zero-Attempt assertion | PASS |
| `cargo clippy --locked -p gateway-router -p provider-openai-compatible -p provider-anthropic-compatible -p gateway-http-actix -p gateway --all-targets -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |

## Review conclusion and remaining boundary

- Registry admission and execution call the same request projection function, preventing Explain,
  scheduler and Provider path selection from silently drifting.
- Rejection occurs before Secret access, lease acquisition, scheduling mutation, HTTP composition
  or network I/O. Post-FSE response projection errors remain terminal and do not retry another
  upstream.
- Tests use synthetic values and loopback peers only. No server, Credential, Config Version,
  listener, DNS, Caddy, Cloudflare or production traffic was read or changed.
- P12-08D4 next owns the offline legacy differential and safety-deviation review. D3 does not mark
  P12-08 or P12 complete and does not authorize production cutover.
