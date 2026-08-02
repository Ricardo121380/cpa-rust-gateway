# P12-08E1 Codex and OpenAI-compatible runtime

## Result

P12-08E1 is `LOCAL_PASS_PENDING_PHASE_GATE`. The existing production Chat Completions and
Responses vertical slices now share a strict runtime credential boundary and a bounded,
provider-aware failure classifier. Codex OAuth expiry, 401, quota, transient Endpoint health,
Usage, and Reasoning remain owned by distinct state boundaries. No server, credential, Config
Version, public listener, or production traffic changed.

## Reference and scope

The behavioral reference remains CLIProxyAPI v7.2.101 at commit
`42a00a2a6521b867c27f7ad096d08699db8e6d19`, specifically:

- `internal/runtime/executor/codex_executor_auth.go` for API-key precedence, access/refresh token
  replacement, account binding, and expiry;
- `internal/auth/codex/openai_auth.go` for the fixed Codex token target, OAuth client identity,
  refresh form, optional refresh-token rotation, and expiry calculation;
- `internal/runtime/executor/codex_executor_terminal.go` for `usage_limit_reached`, reset timing,
  401, 403, 429, and transient-status behavior; and
- the Codex and generic OpenAI-compatible executors for the already-ported Responses and Chat
  request/response paths.

The port keeps CPAR's earlier hardening: every request still crosses DNS-pinned egress admission,
unknown 403 does not permanently disable a Credential, raw upstream messages never drive state,
and incomplete JSON/SSE is never synthesized into success.

## Runtime credential and refresh boundary

`provider-openai-compatible` accepts exactly two encrypted secret payload shapes at lease use:

1. a non-empty opaque API-key/Bearer value; or
2. strict `codex_oauth` JSON containing access token, refresh token, positive Unix-millisecond
   expiry, and an optional account binding.

OAuth JSON rejects duplicate and unknown names. Debug output renders no token or account value.
An expired access token fails before request construction as
`CredentialUnauthorized/Credential`. The refresh transaction builder fixes the incumbent token URL,
client identity, grant, and scope; percent-encodes the refresh token; strictly applies a successful
response; retains the old refresh token when rotation is omitted; and updates access token plus
expiry atomically.

The ordinary request path performs no hidden refresh network call. A later production graph may
activate a refresh worker only by explicitly composing the fixed token target through DNS-pinned
egress and committing the returned encrypted credential. Until that composition exists, an expired
OAuth binding stays unavailable and requires controlled reauthorization; it is never refreshed on
every foreground request. API-key and unexpired OAuth execution are complete in this slice.

## HTTP failure ownership

Only non-2xx OpenAI-compatible responses use the new body-aware profile. At most 64 KiB is read;
oversized or malformed bodies fall back to status-only classification. The body is discarded after
classification and no raw message, endpoint, token, or payload enters events or reports.

| Evidence | Gateway result | State effect |
|---|---|---|
| `401` | `CredentialUnauthorized/Credential` | block only the exact Endpoint/Credential until controlled recovery |
| unknown `403` | `EgressRejected/Egress` | no Credential/account mutation |
| `429` or structured `usage_limit_reached` | `ProviderRateLimited/QuotaWindow` | record reset only on the exact Endpoint/Credential; healthy sibling remains schedulable |
| `408` or `5xx` | `ProviderTransient/Provider` | cool only the selected Endpoint and allow pre-semantic candidate fallback |
| other 4xx/malformed signal | `ProviderPermanent/Provider` | no retained Credential or quota mutation |

Positive integer `Retry-After`, `error.resets_in_seconds`, and future `error.resets_at` are projected
to the existing bounded quota registry. Structured `error.type`, relay-compatible `error.code`, and
top-level `type` are recognized; arbitrary prose is ignored.

## Protocol, Usage, and Reasoning closure

The E1 credential and failure layers are used by both production OpenAI-compatible adapters:

- native Chat Completions and native Responses preserve their validated payload and replace only
  the selected upstream model;
- all admitted Chat/Responses/Messages bridge pairs continue through the D1-D3 typed conversion
  registry before leasing a Credential;
- JSON and SSE use the production bounded decoders, including Tool lifecycle and terminal checks;
  and
- OpenAI Responses retains representable reasoning-token detail, while Chat/Anthropic projections
  deliberately retain only counters those target protocols can express.

Thus E1 does not create a parallel decoder or a provider-specific Canonical model. The previous D3
vertical-slice tests remain the execution proof; the new loopback test adds a real HTTP
`usage_limit_reached` envelope and reset handoff through `UpstreamClientPool`.

## Verification and review

The following local evidence passed:

- `cargo test --locked -p provider-openai-compatible` — 16 passed;
- `cargo test --locked -p gateway runtime::tests --no-fail-fast` — 62 passed before the E1-specific
  loopback was added;
- `cargo test --locked -p gateway codex_usage_limit_is_bounded_and_attributed_over_loopback` — 1
  passed over a real loopback TCP peer;
- `cargo test --locked -p gateway-router credential_unauthorized_blocks_only_its_binding_until_reauthorization`;
- `cargo test --locked -p gateway-router rate_limit_records_exact_quota_and_preserves_a_healthy_sibling`;
- `cargo test --locked -p gateway-router server_error_cools_the_endpoint_and_falls_back_to_another_candidate`;
- affected-package Clippy, formatting, documentation/link, whitespace, Secret, and required local
  Full gate recorded at commit closeout.

Review found and fixed one initial ownership defect before closeout: a 401 was correctly classified
but the Router retained state only for 403. The final implementation adds a distinct
`CredentialUnauthorized` health/account status, blocks only that binding, preserves healthy sibling
Credentials, and reuses the controlled recovery ticket without misreporting the state as
`Forbidden`.

## Remaining boundary

P12-08E2 next ports the Claude/Anthropic-compatible runtime. P12-08F1 still owns production graph
composition, including whether a Codex OAuth refresh worker and durable encrypted replacement are
enabled. P12-08G1 owns any separately controlled real credential/E2E receipts. None of those actions
was started here.
