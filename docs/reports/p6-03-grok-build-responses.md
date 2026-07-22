# P6-03 Grok Build Responses request, stream, and error report

| Field | Value |
|---|---|
| Plan version | `v1.15` |
| Task | `P6-03` |
| Date | `2026-07-22` |
| Branch | `codex/p6-grok-build` |
| Status | `BLOCKED` — `CR-P6-03-005` T11/T12 each sent exactly once with the reviewed current profile, but neither yielded the required Canonical `ResponseStart`/text/`ResponseEnd` lifecycle. |
| Scope / budget | `M`; one Provider-private fixed HTTP/decoder boundary and one behavior contract. `CR-P6-03-005` authorizes exactly two new fixed-profile direct tuples after local implementation/review; no account-state, quota, cache, continuity, server, or P6-04+ scope. |
| Task Card | Fixed CLI request profile, exact P2 egress handoff, bounded non-streaming/SSE decode, Tool consistency, redacted error signals, and a finite one-send-per-process live matrix. Prohibited: cross-Provider imports, socket/proxy/TUN mutation, status remediation, persistent state, retry/failover, or P6-04+ behavior. |
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

The current-profile correction adds bounded runtime `flate2` gzip decoding and `getrandom` for
ephemeral non-secret associations; both are explicit `provider-grok` allowlist edges and neither
creates network I/O, state, or a diagnostic value surface. `tokio` remains test-only for the
ignored authorization harness and does not enter the library target or public API.

## Local verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p6_03_build_responses` | Current-profile PASS; 8 synthetic request/profile-redaction, scalar-semantic-equivalence, bounded-gzip, egress, chunk-equivalence, bounded-error, duplicate/truncation/atomicity, Tool-normalization/metadata, and terminal-item-set tests passed. |
| `cargo test --locked -p provider-grok --test p6_03_authorized_build_probe` | PASS; 8 no-network authorization/configuration/redaction tests passed, including safe body-taxonomy, standard-error-category, and 2xx-status redaction; the sole live test remains ignored without explicit authorization. |
| `cargo clippy --locked -p provider-grok --all-targets --all-features -- -D warnings` | Current-profile PASS. |
| `cargo fmt --all -- --check` and `git diff --check` | Current-profile PASS. |
| `ruby scripts/check-crate-boundaries.rb` | Current-profile PASS; the explicit `flate2`/`getrandom` allowlist update admits all 21 workspace packages. |
| `./scripts/check.sh full` | Current-profile PASS; format, Clippy, workspace tests, source policy, secret scanner, crate boundaries, document links, whitespace, `cargo deny`, and RustSec audit all passed against the staged current-profile diff. |
| Independent diff / secret / redaction review | Current-profile PASS for synthetic fixed values, model/correlation `Debug` redaction, bounded gzip input/output, raw request/response non-retention, and the shared direct transport's explicit no-environment-proxy path. |

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

### Live validation status: BLOCKED

For each tuple, the sole authorized OAuth file was projected only in process memory and its usable
lifetime was calculated from its `.expired` field; no persisted `expires_in` was reused. The opaque
Build model value was selected only in process memory from a read-only successful Build/Responses
audit reference. No credential, model mapping, request/response body, raw header, or association
value was written to the report, logs, or Git.

T11 reached the fixed endpoint and stopped on a `4xx` JSON error-like object with only the
whitelisted safe category `unrecognized`; T12 likewise stopped on a `4xx` error-like object, but
without the requested SSE content-type class. Neither response produced any Canonical success
event. These outcomes rule out a timeout or local egress-admission failure for the two current
direct sends, but do not safely attribute the `4xx` to credentials, model access, or a remaining
request-profile detail.

`CR-P6-03-001` through `CR-P6-03-003` permanently prohibit any further same-tuple request.
`CR-P6-03-005` registered only the distinct updated-profile tuples above, and both are now
exhausted. P6-04 must not start while P6-03 is `BLOCKED`.

No further direct tuple is registered. P6-03 can enter `LOCAL_PASS_PENDING_PHASE_GATE` only after
a newly authorized, newly documented validation plan obtains both fixed-endpoint modes with the
required Canonical start/text/end semantic shape and no stream error; until then P6-04 remains
pending.

## Timing and next task

| Measurement | Evidence / value |
|---|---|
| Task Card / resumed local work | `2026-07-22`; resumed from the existing P6-03 implementation checkpoint. The original pre-handoff start time is not claimed. |
| Local review/test pass | `2026-07-22T10:07+08:00` after one Clippy-directed test-type correction and one review-correction batch. |
| Earlier local full gate | `2026-07-22T10:34:09+08:00`; warm wall clock 19s. The durable local check log recorded Rust tests 9s, dependency policy 2s, RustSec audit 2s, and zero seconds for cached fixed-tool install/verification. |
| Current-profile review / full gate | `2026-07-22T15:35:55+08:00`; PASS after staging the reviewed current-profile diff. It reran the full local quality, workspace-test, documentation, boundary, secret, dependency-policy, and RustSec set. |
| Live-matrix / diagnostic review | `2026-07-22T11:54–12:24+08:00`; T1-T8 plus T9/T10 sent exactly once each (10 total); three subsequent diagnostic-review local full gates all passed, with the final warm run 19s. |
| CR-P6-03-005 direct matrix | `2026-07-22`, after the current-profile local checkpoint; T11 and T12 each ran in an independent process with a one-request cap, direct no-environment-proxy transport, fresh in-memory expiry projection, and no retry/refresh/failover. Both stopped in under one harness second with the safe outcomes recorded above. |
| Server-side preflight hygiene | One incorrect schema lookup briefly created a zero-byte, unreferenced file under the current grok2api data mount. It was immediately removed; the postcheck confirmed its absence and the container still running. No configuration, credential, or database-table row was changed. |
| Code commit | Current-profile local checkpoint (`P6-03: update current Grok Build profile`); local only, not pushed, and required before either T11 or T12. |
| Phase closeout tag / Delivery Gate | Not started; G6 is P6's single remote gate. |
| Repeated validations / rework | One necessary test type correction, one semantic review correction batch, then two full-gate findings fixed in scope (`struct_excessive_bools` in the ignored harness and its test-only `tokio` boundary allowlist). The earlier live matrix exposed an intentionally over-coarse safe outcome; two finite redacted diagnostics then classified it as a 2xx error object with unrecognized standard metadata. No identical tuple was sent after T10. `CR-P6-03-005` then used its only two new-profile direct sends: T11 and T12 each stopped as an unrecognized 4xx error object, so no further direct probe is permitted. |

Rollback removes only the P6-03 module, synthetic fixtures/tests, and documentation. It requires
no database migration, credential revocation, server restart, proxy/TUN cleanup, or external
Provider action. T1-T12 remain exhausted permanently; no further direct tuple is registered. P6-04
remains pending until P6-03's dual-mode acceptance condition is met under a new explicit CR.
