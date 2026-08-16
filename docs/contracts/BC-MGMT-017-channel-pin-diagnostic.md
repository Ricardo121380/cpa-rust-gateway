# BC-MGMT-017: Protected single-request Channel Pin diagnostic

Status: `DONE_WITH_BOUNDARY`

## Scope and entry point

The management-only operation is exposed as a protected `POST
/admin/operations/channel-pin` resource. It is not a `/v1/*` inference route
and is never callable with a public client key. Existing Management Key,
same-origin CSRF (for mutating management actions), admission, selected
`X-Config-Version`, and the required `If-Match` revision guard apply. The
request is bounded and rejects unknown fields.

The operator supplies exact, non-secret identities:

* `provider_id` — owning Provider;
* `channel_id` — exact channel/endpoint identity;
* `route_id` — route in the selected Config Version;
* `credential_id` — one credential bound to that route/channel;
* public `requested_model`, client `protocol` (`openai_chat_completions`,
  `openai_responses`, or `anthropic_messages`) and `mode` (`json` or `sse`).

The first slice constructs a fixed short probe request internally. It does not
accept arbitrary prompt/body, endpoint URL, headers, cookies, tokens, or
credential material from the caller.

## Preconditions and ownership

1. The selected Config Version exists, is the runtime-current active Version,
   and passes normal management admission. Draft or archived Versions cannot
   execute a Provider pin and fail closed with a snapshot conflict.
   The `If-Match` revision is carried into the executor and must equal the
   runtime composition revision at the execution boundary.
2. Provider, channel, route, model and credential are present in that version's
   graph and have the exact ownership relation requested by the operator.
3. The route has exactly one matching channel/credential binding. Zero or more
   than one match is a fail-closed rejection; no selector may choose a sibling.
4. Normal capability, enabled, expiry, Health/Quota/Circuit, capacity, egress,
   and credential unwrap/lease checks still apply.
5. No precondition failure sends a Provider request or acquires a durable
   mutation. The rejected action is still represented by a value-free audit
   record.

## Execution and event sequence

For an admitted request the required sequence is:

1. Create an opaque diagnostic request id and audit the operator action.
2. Pin the exact route/channel/credential against the selected snapshot.
3. Acquire the existing lease owner once; do not create a second scheduler or
   pool.
4. Build the existing protocol-specific JSON or SSE request from the fixed
   probe payload. Generic Chat/Responses/Anthropic adapters receive an output
   limit of `8`; the reviewed Codex OAuth Responses adapter intentionally omits
   the field because that upstream rejects it and already forces its bounded
   SSE projection.
5. Send to the admitted upstream at most once. The first slice admits only
   generic OpenAI Chat/Responses and Anthropic Messages adapters whose
   transport boundary has one inference send; `NativeExact` candidates and native adapters with hidden
   token/Statsig/bootstrap or refresh requests (currently Grok Console/Web)
   reject before lease/network and are deferred to a Provider-specific
   one-shot policy task.
6. Drain only the bounded adapter/event stream, record whether a semantic
   response started, release the lease, and emit one terminal value-free
   receipt.

The operation stops at the first failure. Retry budget, quota recovery,
credential rotation, sibling selection, cross-Provider fallback, refresh,
reauth, or proxy-pool fallback are prohibited for admitted adapters. A
retryable upstream error is still terminal for this diagnostic.

## Receipt and error contract

The JSON receipt contains only bounded fields:

| Field | Contract |
|---|---|
| `request_id` | opaque id, stable format, no secret or digest |
| `outcome` | `succeeded`, `failed`, or `rejected` |
| selected ids | provider/channel/route/credential ids already supplied by operator |
| `protocol`, `mode` | closed protocol/mode values |
| `upstream_sent` | boolean; false means no upstream send occurred |
| `attempt_count` | exactly `0` or `1` |
| `stage` | optional observed closed stage, no raw error text |
| `response_started` | boolean semantic-event projection |
| `observed_at_ms` | bounded observation timestamp captured before execution |

No receipt, log, metric, audit detail, or error response may include endpoint
URLs, request/response bodies, headers, cookies, tokens, ciphertext,
plaintext, client-key digests, raw Provider status, or raw Provider payloads.
Unknown internal errors map to the safe generic `failed` outcome with a closed
observed stage when available. `Cache-Control:
no-store` is required on the HTTP response.

The request and pre-execution actions are persisted through the existing
value-free audit boundary; their bounded resource id is the requested Route id.
The final `channel_pin_started` append occurs before any Provider call. No
post-send audit append is allowed to turn a completed one-shot request into an
ambiguous retryable error. Exact Provider/Channel/Credential attribution and
`upstream_sent` remain in the returned receipt; the audit schema deliberately
does not persist a second copy of those fields. The ordinary serving Attempt
event is intentionally not emitted for this management-only execution because
it precedes bounded source drain.
The first slice does not expose a separate audit-event identifier in the
receipt. HTTP errors use the existing management error envelope and fail closed. They
must distinguish invalid input/admission/version/ownership (`4xx`) from an
unavailable management/runtime facade (`5xx`) without disclosing Provider
details.

## Invariants

* one admitted invocation creates at most one upstream inference send;
* a driver failure causes no second driver call and no Provider switch;
* the selected credential is the one attributed in the receipt; the value-free
  audit record stores only the route resource id and pre-execution action;
* the request and pre-execution boundary are audited before any Provider call;
  the returned receipt is the terminal outcome and no post-send audit failure
  can induce a retryable response;
* a preflight rejection has `upstream_sent=false` and `attempt_count=0`;
* a sent request has `upstream_sent=true` and `attempt_count=1`, even when the
  response is an error or malformed;
* all leases and transient secret buffers are released/zeroized on every
  terminal path;
* ordinary public Chat/Responses/Messages serving and its retry policy are
  unchanged;
* no active Config Version, route graph, credential metadata, Health/Quota
  state, or production/staging setting is mutated by the diagnostic itself;
* Provider-specific capability and egress boundaries remain isolated;
* an adapter with an unreviewed auxiliary HTTP call is rejected before lease
  or network rather than weakening the one-send claim;
* `NativeExact` candidates are rejected before lease/network in this slice;
* the bounded source drain is limited to 45 seconds idle, 45 seconds total,
  and 4096 canonical events; at most two pins are in flight process-wide.

## Verification matrix

Focused tests must cover management auth/CSRF/version admission, unknown-field
and size limits, provider/channel/route/credential ownership, ambiguous and
foreign bindings, disabled/expired/health/quota/capacity rejection, exact
credential attribution, JSON and SSE success, first failure with zero retry,
upstream sent/not-sent, lease release, audit/no-secret assertions, no
cross-Provider fallback, and safe rollback/disabled-facade behavior. Tests use
mock drivers only; no Provider, staging, production, server, or CI mutation is
part of this contract.

## Frontend handoff

The backend owns the authoritative OpenAPI schema. If the route is added,
update `docs/openapi/management-v1.json`, run
`npm --prefix web/prism run sync-contract`, and append the exact paths and
required Claude Code UI follow-up to `docs/cross-boundary-log.md` in the same
commit. Prism may render closed receipt state and attribution only; it must not
send arbitrary body data or implement retry/fallback logic.
