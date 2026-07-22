# P6-03 Grok Build Responses request, stream, and error report

| Field | Value |
|---|---|
| Plan version | `v1.19` |
| Task | `P6-03` |
| Date | `2026-07-23` |
| Branch | `codex/p6-grok-build` |
| Status | `BLOCKED` — T15 reached the fixed endpoint exactly once and stopped as a `4xx` error-like response with safe category `unrecognized`; T14 remains unsent and prohibited. T1-T13 and T15 are closed; P6-04 remains pending. |
| Scope / budget | `M`; one Provider-private fixed HTTP/decoder boundary and one behavior contract. `CR-P6-03-005` exhausted T11/T12, `CR-P6-03-008` closed after the pre-dispatch T13 stop, and `CR-P6-03-009` closed after T15's non-success stop. The post-T15 local request-profile correction adds no external tuple, retry, server action, cache read, or P6-04+ behavior. |
| Task Card | Fixed CLI request profile, exact P2 egress handoff, bounded non-streaming/SSE decode, Tool consistency, redacted error signals, known OAuth source adapters, and a finite one-send-per-process live matrix. Prohibited: direct-tuple replay, route/key/scheduler mutation, socket/proxy/TUN mutation, status remediation, retry/failover, or P6-04+ behavior. |
| Execution channel | Current default model, `medium`; Luna is unavailable in this execution surface. No subagent was used because Provider protocol, Tool state, and secret-redaction review need one coherent final review. |
| References | Matrix `C28`; Behavior 4/5/12; [ADR-0044](../adr/ADR-0044-grok-build-responses-boundary.md); [BC-PROVIDER-003](../contracts/BC-PROVIDER-003-grok-build-responses-boundary.md) |

## Delivered local behavior

P6-03 adds a self-contained `provider-grok` Responses boundary for the fixed OAuth Build CLI
profile. Under `CR-P6-03-005`, it updates that profile to the verified current `grok-shell`
identity/version and adds client identifier/mode, protocol confirmation, selected-model override,
mode-specific content coding, and ephemeral process/request/trace associations. It builds
`POST https://cli-chat-proxy.grok.com/v1/responses` with a zeroizing Bearer value, then hands it to
the existing P2-admitted shared transport only when the admitted target is exactly the fixed URL.
The association values are generated only in memory and redacted from `Debug`; P6-03 does not
invent user, session, cache, or turn identity. It deliberately omits the reference client's
hop-by-hop `Connection` header because connection pooling belongs to `gateway-upstream`.

The response layer has separate bounded non-streaming and incremental SSE decoders. It rejects
duplicate JSON fields, malformed framing, unmatched response/item identity, truncated streams, and
ambiguous Tool completion. Tool arguments normalize blank and empty-object values to `{}`; nonempty
arguments must be a complete JSON object. The final Tool name, call id, and arguments must match
their earlier forms; each done item must actually be completed and the terminal output-item set must
exactly match the completed declared set. A single extension-free user Text has an explicitly
tested scalar easy-input encoding whose decode round-trips to the same Canonical request; all
other message shapes retain array input. Non-streaming identity/gzip bodies have independent 1 MiB
bounds before strict decoding. HTTP error parsing retains only a safe status/signal pair and never
raw body text.

The implementation intentionally does not import `provider-openai-compatible`; doing so would
violate the Provider-private crate boundary. It carries an equivalent restricted Responses subset
inside `provider-grok`, so Grok Build remains independently evolvable.

Under `CR-P6-03-008`, the credential boundary also accepts three strictly validated in-memory
representations in addition to the existing standard JSON, Device Code, and Refresh paths: CPA xAI
OAuth data with authoritative absolute `expired`, grok2api/`Sub2API` account data with authoritative
absolute `expires_at`, and one exact official-Grok-CLI issuer/client indexed-cache entry whose `key`
is the access token. The adapters reject duplicate names, conflicting absolute expiries, expired or
overlong lifetimes, wrong issuer/client identity, missing cache entry, and unsafe fields. Production
code remains path-free; only the ignored live harness can read a separately specified local cache
file (bounded to 64 KiB in zeroizing memory), and it rejects simultaneous JSON and cache sources.
See the [OAuth source analysis](p6-03-oauth-source-analysis.md).

The current-profile correction adds bounded runtime `flate2` gzip decoding and `getrandom` for
ephemeral non-secret associations; both are explicit `provider-grok` allowlist edges and neither
creates network I/O, state, or a diagnostic value surface. `tokio` remains test-only for the
ignored authorization harness and does not enter the library target or public API.

## Local verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p6_03_build_responses` | Current-profile PASS; 8 synthetic request/profile-redaction, scalar-semantic-equivalence, bounded-gzip, egress, chunk-equivalence, bounded-error, duplicate/truncation/atomicity, Tool-normalization/metadata, and terminal-item-set tests passed. |
| `cargo test --locked -p provider-grok --test p6_03_authorized_build_probe` | PASS; 8 no-network authorization/configuration/redaction tests passed, including safe body-taxonomy, standard-error-category, and 2xx-status redaction; the sole live test remains ignored without explicit authorization. |
| `cargo test --locked -p provider-grok --test p6_01_build_oauth` | `CR-P6-03-008` PASS; 7 strict synthetic source-import/redaction tests cover standard JSON, CPA, account, indexed cache, RFC3339 milliseconds, wrong identity, duplicate entries, expiry conflict, unknown source shape, absence, and bounded expiry. |
| `cargo test --locked -p provider-grok --test p6_02_refresh_runtime` | `CR-P6-03-008` PASS; 7 synthetic persistence/refresh tests include indexed-cache provenance round-trip through AEAD without plaintext in SQLite ciphertext. |
| `cargo test --locked -p provider-grok --test p6_03_authorized_build_probe` | `CR-P6-03-008` PASS; 9 no-network tests plus two ignored tests cover the mutually exclusive absolute cache-path source, 64 KiB bound, synthetic cache redaction, and no DNS/transport before configuration is complete. |
| Exact local cache preflight | PASS; one explicitly selected official-CLI-cache source was read only into zeroizing memory, validated by the new importer, and used to construct a non-network request. It did not refresh credentials or send a Provider request. |
| `cargo clippy --locked -p provider-grok --all-targets --all-features -- -D warnings` | `CR-P6-03-008` PASS. |
| `cargo fmt --all -- --check` and `git diff --check` | `CR-P6-03-008` PASS. |
| `ruby scripts/check-crate-boundaries.rb` | `CR-P6-03-008` PASS; the explicit `time` edge is limited to strict RFC3339 absolute-expiry parsing and admits all 21 workspace packages. |
| `./scripts/check.sh full` | `CR-P6-03-008` PASS; format, Clippy, workspace tests, source policy, secret scanner, crate boundaries, document links, whitespace, `cargo deny`, and RustSec audit passed. |
| Independent diff / secret / redaction review | `CR-P6-03-008` PASS for synthetic fixed values, source discriminator persistence, model/correlation `Debug` redaction, bounded cache input, raw cache non-retention, and the shared direct transport's explicit no-environment-proxy path. |

Review corrected three concrete semantic defects before this checkpoint: blank/empty-object Tool
input now consistently becomes `{}`, a completed function-call item must retain the originally
declared Tool name, and a terminal response cannot omit an already completed item or mark a done
item `in_progress`. The review also strengthened the atomicity test: a chunk containing a valid
record followed by malformed duplicate JSON cannot advance the retained decoder state.

## Authorized validation matrix and stop boundary

`CR-P6-03-001` was recorded before any real Build request and replaces the former one-probe
operational budget with a finite, documented matrix. T1-T8's redacted outcomes appear below.
Before each further request, the report names a candidate label, mode, network profile, and safe
result; the raw credential/model mapping remains in an ignored operator-controlled source and is
neither pasted into chat nor committed.

| Matrix dimension | Fixed boundary |
|---|---|
| Account | One user-specified CPA OAuth account only |
| Endpoint | Fixed `https://cli-chat-proxy.grok.com/v1/responses` only |
| Candidate modes | `non_streaming`, then `sse` for each static candidate |
| Per-invocation cap | `P6_03_MAX_EXTERNAL_REQUESTS=1`; the ignored harness has exactly one send |
| Duplicate behavior | No automatic retry, refresh, failover, candidate selection, or repeat of an identical tuple |
| Request / limit | Fixed `Reply with exactly: ready`; `max_output_tokens=32` |
| Forbidden operations | Account/server mutation, auth-directory enumeration, proxy/TUN configuration change, P6-04/P6-07 policy change |
| Evidence | Candidate label/category, mode, network profile, elapsed time, and redacted outcome only |

### Predeclared tuples (before any live send)

The selected input values are held only in the ignored operator invocation. `build-static-01` was
identified from a read-only static CPA model catalog; `build-code-fast-01` is the second plausible
Build-model candidate. `direct` means no explicit proxy in the harness; `socks5` means the already
configured local SOCKS profile, without changing any proxy or TUN setting.

| Tuple | Candidate label | Mode | Network profile | Outcome |
|---|---|---|---|---|
| `T1` | `build-static-01` | `non_streaming` | `direct` | stopped: `response_protocol_failed` (6s) |
| `T2` | `build-static-01` | `sse` | `direct` | stopped: `response_protocol_failed` (4s) |
| `T3` | `build-static-01` | `non_streaming` | `socks5` | stopped: `response_protocol_failed` (4s) |
| `T4` | `build-static-01` | `sse` | `socks5` | stopped: `response_protocol_failed` (3s) |
| `T5` | `build-code-fast-01` | `non_streaming` | `direct` | stopped: `response_protocol_failed` (5s) |
| `T6` | `build-code-fast-01` | `sse` | `direct` | stopped: `response_protocol_failed` (3s) |
| `T7` | `build-code-fast-01` | `non_streaming` | `socks5` | stopped: `response_protocol_failed` (4s) |
| `T8` | `build-code-fast-01` | `sse` | `socks5` | stopped: `response_protocol_failed` (3s) |
| `T9-DIAG` | `build-static-01` | `non_streaming` | `socks5` | stopped: `response_protocol_failed`; `2xx`, expected content type, `error_like_object` (4s) |
| `T10-DIAG` | `build-static-01` | `non_streaming` | `socks5` | stopped: `response_protocol_failed`; `2xx`, expected content type, `error_like_object`, `unrecognized` (2s) |

T1-T8's same quick response category across both direct and SOCKS5 profiles rules out a timeout or
egress-admission failure for this matrix, but intentionally does not yet attribute the cause. The
T9-DIAG narrows this to an application-level 2xx error object. T10-DIAG is the second and final
pre-registered re-observation of that candidate/mode/network combination; it is an error-category
classification, not a functional retry.

### Predeclared isolated grok2api Build-route proxy reference (`CR-P6-03-004`)

This is deliberately **not** `T11`, a direct-Build retry, or P6-03 acceptance evidence. The user
authorized one server-local proxy reference call through the already deployed grok2api service.
Before sending, the operator selects exactly one existing enabled client key whose persisted model
grant is Build-routed, decrypts it only in server memory, obtains its current `/v1/models` catalog
once, and uses one model permitted by that same key for one non-streaming `/v1/responses` request.

| Boundary | Fixed value |
|---|---|
| Target | Existing server-local grok2api OpenAI-compatible interface only |
| Discovery cap | One authenticated `GET /v1/models` |
| Generation cap | One authenticated non-streaming `POST /v1/responses`; no retry/fallback/SSE |
| Prompt / limit | `Reply with exactly: ready`; `max_output_tokens=32` |
| Evidence | Status/content-type classes, bounded response-shape projection, elapsed time, and a post-call safe log projection only |
| Prohibited | API key/config/database export, raw model mapping/body/header output, operator-initiated account/server mutation, and any claim that this closes direct Build validation |

#### Proxy reference outcome

The predeclared reference was executed once on `2026-07-22` with the existing enabled Build-routed
client key selected only in server memory. The authenticated model catalog was a `2xx` JSON
`data` array with three permitted public entries; the selected entry is recorded only as the safe
class `grok_named`. The single non-streaming Responses call returned `2xx`, JSON, and the bounded
`completed_responses_shape` projection in 1,383 ms. Normal proxy usage accounting advanced, as
expected for an actual generation request.

The post-call grok2api access-log projection contained exactly one `/v1/models` record and one
`POST /v1/responses` record, both `2xx`, with no error marker. No plaintext key, encryption
material, model value/mapping, headers, request body, response body, or generated text was
retained in this report. The documentation review `./scripts/check.sh docs` then passed document
links, plan state (`114` tasks, `0 IN_PROGRESS`), tracked-secret scan, and Git whitespace.

This proves that the deployed grok2api Build route can currently perform one Responses-shaped
generation through its own OpenAI-compatible proxy boundary. It does **not** prove that the
separate direct `cli-chat-proxy.grok.com` request profile used by P6-03 is accepted, and it does
not create a new direct tuple or change the P6-03 stop boundary.

#### Safe profile comparison and remaining boundary

The deployed image tag's immutable source shows that its Build adapter sends a newer client
identity/version and additional client-identifier, mode, request-association, and model-override
metadata classes than the P6-03 frozen builder. The local P6 profile is therefore stale/incomplete
relative to this successful proxy reference. This is a concrete next-profile candidate, not a
causal conclusion: the successful route may use a different Build account, and no raw credential,
model value, or header was read into the evidence.

The latest server-side audit safely records `Build` + `Responses`, non-streaming, `2xx`, and an
under-five-second duration for the reference. It retains no attempt row for this successful call,
so the audit database cannot prove which upstream plane the proxy selected. P6-03 consequently
remains `BLOCKED`; a later direct validation would require a new explicit CR that changes the
frozen profile and registers new distinct tuples rather than repeating T1-T10.

The call is useful only if it shows whether the deployed Build route can produce a completed
Responses-shaped result through its own proxy. Normal proxy last-used/usage accounting is an
expected request side effect, but no operator-initiated state change is allowed. Regardless of
outcome, P6-03 remained `BLOCKED` at that time until its separate fixed-endpoint non-streaming and
SSE conditions could be met.

### Predeclared current-profile direct validation (`CR-P6-03-005`)

Before any new direct request, the approved CR changes the local Builder to the current Build
profile established by the server reference: current static client identity/version, client
identifier/mode, model override, and safe request-correlation metadata. For exactly one plain
user-text message, the Builder may use the reference profile's scalar input encoding only after
synthetic semantic-equivalence tests pass. The profile update is not a retry of the old profile.

| Boundary | Fixed value |
|---|---|
| Account / endpoint | The same user-specified CPA OAuth account; fixed `https://cli-chat-proxy.grok.com/v1/responses` only |
| Candidate | Opaque `build-profile-02`; its value is obtained only in operator memory from the known-good Build reference |
| Network profile | `direct` only; no SOCKS5, proxy, TUN, server, or account configuration change |
| Request cap | Two independently invoked ignored-harness processes, each with `P6_03_MAX_EXTERNAL_REQUESTS=1`; no retries, refresh, failover, or candidate selection |
| Request / limit | Fixed `Reply with exactly: ready`; `max_output_tokens=32` |
| Local prerequisite | Updated synthetic profile/semantic/redaction tests, Clippy, formatting, secret scan, full local gate, independent review, and local commit |
| Acceptance | Both modes must yield Canonical `ResponseStart`, text, and `ResponseEnd` with no `StreamError` |
| Prohibited replay | `T1` through `T10-DIAG` are permanently closed and must not be sent again |

| Tuple | Candidate label | Mode | Network profile | Outcome |
|---|---|---|---|---|
| `T11` | `build-profile-02` | `non_streaming` | `direct` | stopped: `4xx`, expected content type, `error_like_object`, `unrecognized` (harness 0.99s) |
| `T12` | `build-profile-02` | `sse` | `direct` | stopped: `4xx`, other content type, `error_like_object`, `unrecognized` (harness 0.98s) |

### Predeclared isolated grok2api account/allowance diagnostic (`CR-P6-03-006`)

This is neither `T13` nor a direct-Build retry. It diagnoses only whether the user-specified OAuth
account can be accepted and queried through the existing server-local grok2api management boundary.
The server currently has a multi-account Build pool, so a normal shared OpenAI-compatible generation
request cannot be attributed to this imported account and is explicitly excluded unless the existing
service exposes account binding without a route, key, scheduling, or enablement mutation.

| Boundary | Fixed value |
|---|---|
| Target | Existing server-local grok2api admin API and one exact current-server CPA OAuth JSON only |
| Server write | One multipart `POST /api/admin/v1/accounts/import`; no direct SQLite/configuration writes |
| Account proof | Internal association stays in server memory; only a safe import class and quota/result class may be recorded |
| Provider test | Exactly one `POST /api/admin/v1/accounts/{id}/refresh-quota` for the imported account |
| Generation | Prohibited unless the unmodified service can prove the call binds to this account; a shared `/v1/responses` result is not evidence |
| Prohibited | Direct T1-T12 replay, model-route/client-key/priority/enablement changes, proxy/TUN changes, credential export, raw response retention, and P6-04 start |

A successful account-specific quota refresh means grok2api can presently authenticate/query this
account's allowance plane; it does not prove the fixed direct P6 request profile is accepted. A
quota/rate-limit result supports the allowance hypothesis, while an authentication or import failure
instead narrows the account-state hypothesis. Neither outcome changes the P6-03 acceptance boundary.

#### Account/allowance diagnostic outcome

On `2026-07-22`, the source selection found two different same-named OAuth files. The only file
under the current (non-backup) CPA path was selected; the non-primary copy was neither read into
grok2api nor modified. The supported import endpoint returned `2xx`, and a read-only account-pool
projection changed from 125 to 126 enabled Build accounts. The source's in-memory account identity
then matched exactly one imported Build account; no OAuth content, account identity, internal ID,
JWT, model mapping, or body was retained in this report, Git, or command output.

The one account-specific `refresh-quota` call returned `502` in under five seconds. A pre-send
integer-ID string-construction error was caught before any HTTP request and the relevant access-log
projection was zero; the recorded `502` is therefore the first and only actual quota-refresh call.
Afterwards the safe stored-state projection was enabled + `active`, automatic Build route mode,
no persisted failure marker, and `buildSuperEntitled=false`; no quota window or whitelisted upstream
error class was available in the safe projection. The corresponding safe service-log projection contained only `502`, with
no `429`/quota-or-rate, credential, transport, or request-or-route marker.

This **does not establish that the account has exhausted quota**. It proves that grok2api accepted
and persisted the OAuth import, but its account-bound allowance query failed at its own `502`
boundary before a usable quota state was available. Because the deployment has a multi-account Build
pool, a normal shared `/v1/responses` generation cannot be attributed to this account; none was
sent. Proving an account-specific generation would require a separately authorized, reversible
route/key/scheduling isolation, which CR-P6-03-006 intentionally forbids. This diagnostic creates
no new P6 direct tuple and cannot distinguish the direct T11/T12 `4xx` between account entitlement,
remaining request-profile detail, or another upstream policy.

### Local official-CLI OAuth reauthentication (`CR-P6-03-007`)

On `2026-07-22`, the user completed one interactive `grok login --oauth` flow through the official
local Grok CLI. The CLI reported sign-in success; the account identity, authorization URL, callback
data, tokens, refresh token, cookies, and all other credential values were neither retained nor
printed. No model command, `/v1/responses` request, P6 ignored harness, token refresh, server call,
route change, or proxy/TUN operation occurred in this CR.

The subsequent local read-only projection of the known CLI auth cache found one indexed credential
entry with a future expiry class. Its credential representation contains a snake-case refresh field
and an absolute-expiry field, but lacked P6's then-supported `access_token` and `expires_in` input
names. At the time, the strict P6 importer correctly rejected it and no cache field was renamed,
copied, converted, exported, or supplied to the P6 harness.

This proves the local official CLI session is fresh and currently valid at the cache level, but it
does not prove that the fixed P6 direct request profile is accepted or that the earlier T11/T12
`4xx` outcomes were caused by an expired credential. It created no new tuple. `CR-P6-03-008`
subsequently authorizes a source-specific adapter and two newly registered tuples; it does not alter
the outcome or replay prohibition for any earlier tuple.

### Known OAuth source adaptation and T13/T14 boundary (`CR-P6-03-008`)

The read-only source comparison recorded in the [OAuth source analysis](p6-03-oauth-source-analysis.md)
established interoperable storage shapes, not a request-profile workaround. The local implementation
keeps standard JSON/Device Code/Refresh intact and adds strict bytes-only importers for CPA xAI,
grok2api/`Sub2API` account credentials, and the official CLI indexed cache. It intentionally does
not add `Sub2API` Web SSO/Cookie conversion, browser callbacks, file access to production code, or
any server/route/proxy/TUN mutation.

The ignored harness preserves its existing JSON environment entry for historical evidence. For this
new matrix only, it also accepts one explicit absolute local CLI-cache path as a mutually exclusive
input; the path and contents never enter output, logs, Git, or the report. The cache is read once,
at most 64 KiB, into zeroizing memory immediately before building the one request. The local
adapter/review gate must pass before any row below may be sent.

| Tuple | Credential source | Mode | Network profile | Current outcome |
|---|---|---|---|---|
| `T13` | `official-cli-cache-01` | `non_streaming` | `direct` | stopped: `local_configuration_gate_before_dispatch`; no DNS, HTTP, cache read, refresh, or Provider send |
| `T14` | `official-cli-cache-01` | `sse` | `direct` | not sent under CR-P6-03-008: T13 stop boundary |

Each row is a separate ignored-harness process with `P6_03_MAX_EXTERNAL_REQUESTS=1`, the existing
opaque Build candidate, fixed short prompt, and `max_output_tokens=32`. No refresh, retry,
failover, proxy/TUN change, or T1-T12 replay is permitted. A failure of either row ends this matrix
immediately and returns P6-03 to `BLOCKED`; only both required Canonical success lifecycles can move
the task to `LOCAL_PASS_PENDING_PHASE_GATE`.

### Historical CR-P6-03-008 T13 local configuration stop

At `2026-07-23T00:33+08:00`, the one registered T13 ignored-harness process exited at its local
configuration gate before it printed `result=started`. A shell-semantics check established that the
wrapper's same-command environment assignments expanded to empty values when passed through
`env -i`; therefore authorization parsing stopped before credential import, `prepare`, DNS, HTTP,
refresh, or the sole `send` call. This is a local invocation error rather than an upstream response,
and its safe outcome is `local_configuration_gate_before_dispatch`. No credential, cache path,
model mapping, request/response body, raw header, or association value was written to the report,
logs, or Git.

The one-process/no-retry boundary is nevertheless consumed: `CR-P6-03-008` permits no second T13
process. T14 was not started because T13 did not reach acceptance. P6-03 was consequently
`BLOCKED` until the separately approved `CR-P6-03-009`; that CR does not alter this historical
outcome.

T11 reached the fixed endpoint and stopped on a `4xx` JSON error-like object with only the
whitelisted safe category `unrecognized`; T12 likewise stopped on a `4xx` error-like object, but
without the requested SSE content-type class. Neither response produced any Canonical success
event. These outcomes rule out a timeout or local egress-admission failure for the two current
direct sends, but do not safely attribute the `4xx` to credentials, model access, or a remaining
request-profile detail.

`CR-P6-03-001` through `CR-P6-03-003` permanently prohibit any further same-tuple request.
`CR-P6-03-005` registered only the distinct updated-profile tuples above, and both are now
exhausted. `CR-P6-03-008` is closed after T13's local stop; `CR-P6-03-009` is its only separately
registered replacement boundary. P6-04 must not start while P6-03 is `IN_PROGRESS` or `BLOCKED`.

### Corrected-wrapper live boundary in progress (`CR-P6-03-009`)

The user approved exactly one replacement non-streaming process because T13 stopped before any
egress. It is not an automatic retry: it is a newly registered T15 process with separately reviewed
wrapper construction. Before it, exactly one ignored no-network preflight must use the same
T15 configuration. That preflight may read the bounded cache and prepare the request, but may not
resolve DNS, make HTTP, refresh, retry, or call `send`.

| Tuple | Credential source | Mode | Network profile | Current authorization |
|---|---|---|---|---|
| `T15` | `official-cli-cache-corrected-01` | `non_streaming` | `direct` | stopped: `4xx`, expected content type, `error_like_object`, `unrecognized` (harness 1.83s) |
| `T14` | `official-cli-cache-01` | `sse` | `direct` | not sent: T15 stop boundary |

Both processes must retain the fixed short prompt, `max_output_tokens=32`, one isolated harness
process and `P6_03_MAX_EXTERNAL_REQUESTS=1`. No refresh, retry, failover, candidate selection,
proxy/TUN change, T1-T13 replay, or additional tuple is permitted. A preflight or T15 failure stops
the boundary and leaves T14 unsent; a T14 failure also stops the boundary.

At `2026-07-23T00:52+08:00`, T15 ran once after its corrected no-network preflight. It reached the
fixed endpoint and stopped as a `4xx` error-like response with the expected non-streaming content
type and only the whitelist-safe category `unrecognized`; it emitted no Canonical success event.
This rules out the former wrapper configuration failure and a local egress-admission failure for
T15, but does not safely attribute the response to credential, model, quota, or another upstream
policy. T14 was not started. P6-03 is `BLOCKED`; a new explicit CR would be required for any further
direct action. P6-03 can enter `LOCAL_PASS_PENDING_PHASE_GATE` only after both modes obtain the
required Canonical start/text/end semantic shape with no stream error; P6-04 remains pending.

### Post-T15 read-only official-CLI request-profile correction

On `2026-07-23`, a static-only audit of the locally authenticated official Grok CLI distribution
reported version `0.2.106` and a macOS arm64 executable. It did not open the OAuth cache, invoke a
model command, resolve DNS, or make an HTTP request. Its immutable string table contains the Build
profile token sequence `user-agent` then `xai-grok-workspace/`, together with the current
`x-grok-client-version` value `0.2.106`, and contains neither the gateway's previous
`grok-shell/0.2.106 (linux; x86_64)` value nor a platform-specific equivalent.

The frozen Builder is consequently corrected to the evidence-supported versioned workspace value
`xai-grok-workspace/0.2.106`, with a synthetic regression assertion. This removes a demonstrated
profile discrepancy but does **not** attribute T15's safe `4xx` to that discrepancy or alter any
closed tuple. It creates no permission to replay T15 or send T14: P6-03 remains `BLOCKED` until a
new explicit CR separately specifies a distinct direct tuple and stop condition.

## Timing and next task

| Measurement | Evidence / value |
|---|---|
| Task Card / resumed local work | `2026-07-22`; resumed from the existing P6-03 implementation checkpoint. The original pre-handoff start time is not claimed. |
| Local review/test pass | `2026-07-22T10:07+08:00` after one Clippy-directed test-type correction and one review-correction batch. |
| Earlier local full gate | `2026-07-22T10:34:09+08:00`; warm wall clock 19s. The durable local check log recorded Rust tests 9s, dependency policy 2s, RustSec audit 2s, and zero seconds for cached fixed-tool install/verification. |
| Current-profile review / full gate | `2026-07-22T15:35:55+08:00`; PASS after staging the reviewed current-profile diff. It reran the full local quality, workspace-test, documentation, boundary, secret, dependency-policy, and RustSec set. |
| Live-matrix / diagnostic review | `2026-07-22T11:54–12:24+08:00`; T1-T8 plus T9/T10 sent exactly once each (10 total); three subsequent diagnostic-review local full gates all passed, with the final warm run 19s. |
| CR-P6-03-005 direct matrix | `2026-07-22`, after the current-profile local checkpoint; T11 and T12 each ran in an independent process with a one-request cap, direct no-environment-proxy transport, fresh in-memory expiry projection, and no retry/refresh/failover. Both stopped in under one harness second with the safe outcomes recorded above. |
| CR-P6-03-006 account/allowance diagnostic | `2026-07-22`; selected only the current CPA copy after a read-only duplicate check, imported it once through grok2api's supported admin API (`2xx`), then sent exactly one account-bound quota refresh (`502`, under 5s). No shared-pool generation was sent because 126 enabled Build accounts prevent account attribution without a new route/key/scheduling change. Safe post-state: enabled + active, auto mode, no persisted failure marker, `buildSuperEntitled=false`; no safe quota/rate/credential/transport/root-cause class was available. |
| CR-P6-03-007 local official-CLI OAuth diagnostic | `2026-07-22`; one user-completed `grok login --oauth` session succeeded. The safe cache projection found an indexed credential entry with future expiry, but not the strict P6 input names `access_token` + `expires_in`; no transformation, P6 harness, model request, token refresh, server call, or network/proxy change followed. |
| CR-P6-03-008 focused local gate | `2026-07-23T00:23+08:00`; formatting, `p6_01_build_oauth` (7), `p6_02_refresh_runtime` (7), `p6_03_authorized_build_probe` (9 plus 2 ignored), Clippy, whitespace, crate-boundary review, independent redaction review, and the warm full local gate all passed. |
| CR-P6-03-008 real-cache preflight | `2026-07-23T00:23+08:00`; one exact ignored preflight read the user-authenticated local official CLI cache and constructed the fixed non-streaming request without DNS, HTTP, refresh, retry, server action, proxy/TUN change, Token output, or model request. |
| CR-P6-03-008 T13 stop | `2026-07-23T00:33+08:00`; exactly one T13 ignored-harness process stopped at local authorization/configuration parsing before `result=started`. A local shell-semantics check proved that the wrapper passed empty same-command assignment values into `env -i`; no credential cache was read and no DNS, HTTP, refresh, retry, proxy/TUN action, or Provider send occurred. T14 was not started. |
| P6-03 blocked-state full gate | `2026-07-23T00:43+08:00`; after staging the reviewed blocked-state evidence and a semantics-preserving local-value rename that removed a secret-scanner false positive, formatting, Clippy, the full workspace tests, source/crate-boundary/document checks, tracked-secret scan, dependency policy, and RustSec audit all passed. |
| CR-P6-03-009 corrected no-network preflight | `2026-07-23T00:51+08:00`; one exact ignored preflight with the corrected `env -i` wrapper passed for T15's label/mode. It read the bounded local cache and constructed the fixed request without DNS, HTTP, refresh, retry, proxy/TUN action, Token output, or Provider send. |
| CR-P6-03-009 T15 direct matrix | `2026-07-23T00:52+08:00`; one isolated direct non-streaming T15 process reached the fixed endpoint after the preflight and stopped as `4xx`, expected content type, `error_like_object`, `unrecognized` in 1.83s. It produced no Canonical success lifecycle; T14 was not sent. |
| Post-T15 profile correction | `2026-07-23`; static official-CLI audit corrected only the frozen User-Agent from the contradicted Linux shell value to `xai-grok-workspace/0.2.106`. No OAuth cache was opened and no network action occurred. `cargo test --locked -p provider-grok` passed 31 executed tests with 2 explicitly unauthorized tests ignored; focused Clippy, formatting, whitespace, and the warm full local gate passed. |
| Server-side preflight hygiene | One incorrect schema lookup briefly created a zero-byte, unreferenced file under the current grok2api data mount. It was immediately removed; the postcheck confirmed its absence and the container still running. No configuration, credential, or database-table row was changed. |
| Code commit | Current-profile local checkpoint (`P6-03: update current Grok Build profile`); local only, not pushed, and required before either T11 or T12. |
| Phase closeout tag / Delivery Gate | Not started; G6 is P6's single remote gate. |
| Repeated validations / rework | One necessary test type correction, one semantic review correction batch, then two earlier full-gate findings fixed in scope (`struct_excessive_bools` in the ignored harness and its test-only `tokio` boundary allowlist). `CR-P6-03-008` initially exposed one expected missing `time` crate-boundary declaration; it was added with a documented RFC3339-only purpose and the full gate then passed. The earlier live matrix exposed an intentionally over-coarse safe outcome; two finite redacted diagnostics then classified it as a 2xx error object with unrecognized standard metadata. No identical tuple was sent after T10. `CR-P6-03-005` then used its only two new-profile direct sends: T11 and T12 each stopped as an unrecognized 4xx error object, so no further direct probe is permitted. The registered T13 process later exposed a local wrapper assignment-expansion error before dispatch; its one-process/no-retry policy prevents correction without a new CR. `CR-P6-03-009` then validated the corrected wrapper without network and consumed its sole T15 send, which reached the endpoint but stopped as a safe unrecognized 4xx error-like response; T14 remains unsent. The resulting blocked-state review also found that a scanner treated the pre-existing token-named local assignments as a secret-looking literal; neutral local-value names preserve the import behavior and let the tracked-secret gate verify the actual staged content. |

Rollback removes only the P6-03 module, synthetic fixtures/tests, and documentation. It requires
no gateway database migration, server restart, proxy/TUN cleanup, or direct-Provider action. The
CR-P6-03-006 grok2api import is an intentional external persistent state requested by the user; it
is not deleted automatically and would need a separate explicit server-change authorization. T1-T13
remain exhausted permanently; T15 is consumed and T14 remains unsent. No further direct tuple is
registered. P6-04 remains pending until P6-03's dual-mode acceptance condition is met under a new
explicit CR.
