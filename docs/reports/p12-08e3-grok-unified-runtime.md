# P12-08E3 Grok unified runtime

## Result

P12-08E3 is `LOCAL_PASS_PENDING_PHASE_GATE`. The P12 composition registry now binds the existing
Grok Build OAuth and xAI Official API-key Responses providers to the unified Canonical runtime.
Each adapter fixes its own target, credential shape, execution mode, response decoder, and failure
class before it can join a Route. Grok Web has a legal configuration identifier but remains
deliberately unbound and therefore default-disabled: the existing Web implementation proves a
fixture decoder and a one-request authorized Canary, not a reusable production transport. No
server, network request, real credential, Config Version, listener, or production traffic changed.

## Reference and porting boundary

The pinned behavioral reference is CLIProxyAPI v7.2.101 commit
`42a00a2a6521b867c27f7ad096d08699db8e6d19`:

- `internal/runtime/executor/xai_executor.go` and adjacent request/response files establish the
  fixed Build/Official execution modes, Responses request path, stream bootstrap, and error flow;
- `internal/runtime/executor/xai_executor_auth.go` establishes OAuth refresh and replacement
  behavior; and
- `internal/auth/xai/xai.go` establishes the supported xAI credential sources and expiry handling.

CPAR ports those observable behaviors through the already-reviewed `provider-grok` types. It does
not copy legacy foreground file reads, unrestricted target substitution, raw upstream diagnostics,
or hidden cross-account fallback. DNS-pinned egress, bounded bodies/frames, encrypted leases,
strict JSON, value-free errors, and Router-owned Credential/Quota/Health state remain authoritative.

## Runtime composition and credential boundary

`openai/responses` now has distinct runtime bindings for:

| Adapter | Credential | Fixed target | Runtime status |
|---|---|---|---|
| `grok.build.responses` | durable OAuth JSON with absolute expiry | Grok Build Responses | bound |
| `grok.official.responses` | non-empty xAI API key | xAI Official Responses | bound |
| `grok.web.responses` | Web account/session state | no general target admitted | unbound, fail-closed |

Build request-time import accepts only the three already-supported durable absolute-expiry shapes:
CPA xAI auth file, Grok account JSON, and official CLI indexed cache. The relative `expires_in`
token-response shape is intentionally rejected because replaying it at request time would extend an
expired credential. Official accepts only a strict API key. Both bound adapters require Canonical
transform mode and use the selected Endpoint's compiled DNS-pinned egress policy and transport
profile.

Composition compares both the stored base/path pair and the resulting URL with the provider's fixed
target. Substituting either a host or a path fails before a Credential lease can be sent. An enabled
Grok Web Endpoint likewise fails the entire immutable Version during composition because no runtime
factory is registered for it; it cannot silently borrow the Canary or a generic OpenAI adapter.

## Protocol, continuity, and failure isolation

- Build and Official reuse their production non-streaming and SSE Responses decoders, including
  Text, Tool calls, Reasoning summaries, Usage, terminal validation, chunk invariance, bounded
  compression/error bodies, and exactly one Canonical event source per attempt.
- Build retains its existing credential-scoped OAuth refresh, account catalog, quota, affinity,
  response ownership, and encrypted reasoning replay boundaries. This slice does not move those
  states into Official or Web.
- Official remains stateless for continuity. Its status-only pre-start classification now survives
  the adapter boundary when no optional runtime-state observer is installed: `401` remains an exact
  Credential failure, `429` a binding quota failure, and `408`/`5xx` an Endpoint transient failure.
  The bounded error body is consumed but never retained or used to invent ownership.
- The outer attempt orchestrator maps only retry ownership. It does not merge Build, Official, or
  Web Credential/Quota/Health state, and post-start semantic failures remain terminal stream events.

## Parity classification

| Behavior | Classification | Evidence |
|---|---|---|
| Build/Official Responses JSON and SSE execution | `PARITY` | injected production adapter suites and Canonical lifecycle tests |
| OAuth/API-key selection and fixed provider targets | `PARITY` | strict credential import and request-builder/factory tests |
| Tool, Reasoning, Usage, and terminal projection | `PARITY` | Build and Official provider regression suites |
| fixed-target DNS-pinned egress and bounded errors | `INTENTIONAL_HARDENING` | substituted host/path rejection and bounded classifiers |
| relative-expiry request-time credential import | `UNSUPPORTED_FAIL_CLOSED` | explicit rejection test prevents lifetime extension |
| general Grok Web production execution | `UNSUPPORTED_FAIL_CLOSED` | legal adapter ID is intentionally absent from runtime registry |

## Verification and review

The following local evidence passed before the phase-wide gate:

- `cargo test --locked --offline -p gateway-protocol -p provider-grok` — all unit, JSON/SSE,
  Tool/Reasoning/Usage, continuity, quota, isolation, Web decoder, and fail-closed tests passed;
  explicitly authorized live probes remained ignored;
- `cargo test --locked --offline -p gateway runtime::tests --no-fail-fast` — 65 passed, including
  fixed Grok target admission and unbound Web registry behavior;
- affected-package Clippy passed with warnings denied; and
- `./scripts/check.sh full` passed at task closeout.

Review found and fixed two integration gaps before closeout. First, durable Build credentials had no
single request-time importer, so the runtime now accepts exactly one known absolute-expiry source
and rejects relative expiry. Second, Official's no-observer path collapsed every non-2xx response
into a generic protocol error, preventing the outer orchestrator from applying the existing exact
failure owner; it now preserves the already-defined status-only safe classification after bounded
body consumption.

## Remaining boundary

P12-08E4 next connects the already-developed Kiro CLI/IDE provider to the same unified runtime and
does not require a currently usable live OAuth account. P12-08F1 still owns the final immutable
production graph, background refresh workers, and complete state-observer composition. P12-08G1
owns separately controlled real-credential receipts. This local slice does not claim Grok Web live
production readiness or a production deployment.
