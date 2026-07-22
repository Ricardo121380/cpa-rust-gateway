# P6-03 Grok Build Responses request, stream, and error report

| Field | Value |
|---|---|
| Plan version | `v1.13` |
| Task | `P6-03` |
| Date | `2026-07-22` |
| Branch | `codex/p6-grok-build` |
| Status | `BLOCKED` — local implementation/review pass, but the finite authorized live matrix returned a fixed-endpoint 2xx error object with no safe semantic attribution |
| Scope / budget | `M`; one Provider-private fixed HTTP/decoder boundary and one behavior contract. `CR-P6-03-001` authorizes a finite live model × mode matrix using one named CPA OAuth account; no account-state, quota, cache, continuity, server, or P6-04+ scope. |
| Task Card | Fixed CLI request profile, exact P2 egress handoff, bounded non-streaming/SSE decode, Tool consistency, redacted error signals, and a finite one-send-per-process live matrix. Prohibited: cross-Provider imports, socket/proxy/TUN mutation, status remediation, persistent state, retry/failover, or P6-04+ behavior. |
| Execution channel | Current default model, `medium`; Luna is unavailable in this execution surface. No subagent was used because Provider protocol, Tool state, and secret-redaction review need one coherent final review. |
| References | Matrix `C28`; Behavior 4/5/12; [ADR-0044](../adr/ADR-0044-grok-build-responses-boundary.md); [BC-PROVIDER-003](../contracts/BC-PROVIDER-003-grok-build-responses-boundary.md) |

## Delivered local behavior

P6-03 adds a self-contained `provider-grok` Responses boundary for the fixed OAuth Build CLI
profile. It builds `POST https://cli-chat-proxy.grok.com/v1/responses` with the frozen CLI identity
headers and a zeroizing Bearer value, then hands it to the existing P2-admitted shared transport
only when the admitted target is exactly the fixed URL. It deliberately omits the reference
client's hop-by-hop `Connection` header because connection pooling belongs to `gateway-upstream`.

The response layer has separate bounded non-streaming and incremental SSE decoders. It rejects
duplicate JSON fields, malformed framing, unmatched response/item identity, truncated streams, and
ambiguous Tool completion. Tool arguments normalize blank and empty-object values to `{}`; nonempty
arguments must be a complete JSON object. The final Tool name, call id, and arguments must match
their earlier forms; each done item must actually be completed and the terminal output-item set must
exactly match the completed declared set. HTTP error parsing retains only a safe status/signal pair
and never raw body text.

The implementation intentionally does not import `provider-openai-compatible`; doing so would
violate the Provider-private crate boundary. It carries an equivalent restricted Responses subset
inside `provider-grok`, so Grok Build remains independently evolvable.

The only new dependency edge is test-only `tokio` for the ignored authorization harness. It does
not enter the `provider-grok` library target or public API and is covered by the crate-boundary
allowlist.

## Local verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p6_03_build_responses` | PASS; 6 synthetic request, egress, chunk-equivalence, bounded-error, duplicate/truncation/atomicity, Tool-normalization/metadata, and terminal-item-set tests passed. |
| `cargo test --locked -p provider-grok --test p6_03_authorized_build_probe` | PASS; 8 no-network authorization/configuration/redaction tests passed, including safe body-taxonomy, standard-error-category, and 2xx-status redaction; the sole live test remains ignored without explicit authorization. |
| `cargo clippy --locked -p provider-grok --all-targets --all-features -- -D warnings` | PASS. |
| `cargo fmt --all -- --check` and `git diff --check` | PASS. |
| `ruby scripts/check-crate-boundaries.rb` | PASS; 21-package dependency direction remains valid; no Provider-to-Provider import exists. |
| `./scripts/check.sh full` | PASS; workspace format/Clippy/tests, source policy, secret scanner, crate boundaries, doc links, whitespace, `cargo deny`, and `cargo audit` all passed. |
| Focused secret review | PASS; committed values are synthetic; redacted `Debug` paths and bounded error envelopes do not retain request/response secrets or text. |

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

The call is useful only if it shows whether the deployed Build route can produce a completed
Responses-shaped result through its own proxy. Normal proxy last-used/usage accounting is an
expected request side effect, but no operator-initiated state change is allowed. Regardless of
outcome, P6-03 remains `BLOCKED`
until its separate fixed-endpoint non-streaming and SSE conditions are met.

### Live validation conclusion: BLOCKED

The supplied credential is structurally suitable for the intended Build profile: its access token
is a three-segment JWT whose safe non-PII claims identify the expected public OAuth client and
include `grok-cli:access`; it was also current when each probe projection was made. Both direct and
the already configured SOCKS5 profile received a response promptly, so this evidence does not
support a Clash/TUN routing or TLS timeout explanation.

The unresolved incompatibility is at the fixed endpoint's application layer: it returns an HTTP
2xx response with the expected JSON content type but an error-like object, rather than a completed
Responses object or SSE lifecycle. The final safe diagnostic found no whitelisted standard
`code`/`type`/`param` classification. The request body, raw error text, response identifier,
private model mapping, token, and headers were neither written nor recorded.

`CR-P6-03-001` through `CR-P6-03-003` now prohibit any further same-tuple request. P6-03 therefore
cannot meet its non-streaming plus SSE semantic acceptance condition, remains `BLOCKED`, and P6-04
must not start. To unblock it, provide either a verified current Grok Build model/request-profile
reference, or explicit approval for a new, narrowly specified redaction policy that can classify
the upstream error without retaining its free-form text.

On any non-2xx, timeout, malformed response, redaction failure, or unexpected semantic terminal
shape, that tuple ends without automatic retry/failover. The next distinct documented tuple may be
tested. P6-03 can enter `LOCAL_PASS_PENDING_PHASE_GATE` only after one candidate completes both
fixed-endpoint modes with the required Canonical start/text/end semantic shape and no stream error;
otherwise P6-04 remains pending.

## Timing and next task

| Measurement | Evidence / value |
|---|---|
| Task Card / resumed local work | `2026-07-22`; resumed from the existing P6-03 implementation checkpoint. The original pre-handoff start time is not claimed. |
| Local review/test pass | `2026-07-22T10:07+08:00` after one Clippy-directed test-type correction and one review-correction batch. |
| Final local full gate | `2026-07-22T10:34:09+08:00`; warm wall clock 19s. The durable local check log recorded Rust tests 9s, dependency policy 2s, RustSec audit 2s, and zero seconds for cached fixed-tool install/verification. |
| Live-matrix / diagnostic review | `2026-07-22T11:54–12:24+08:00`; T1-T8 plus T9/T10 sent exactly once each (10 total); three subsequent diagnostic-review local full gates all passed, with the final warm run 19s. |
| Code commit | This Task's local checkpoint commit (`P6-03: add Grok Build Responses boundary`); local only, not pushed. |
| Phase closeout tag / Delivery Gate | Not started; G6 is P6's single remote gate. |
| Repeated validations / rework | One necessary test type correction, one semantic review correction batch, then two full-gate findings fixed in scope (`struct_excessive_bools` in the ignored harness and its test-only `tokio` boundary allowlist). The live matrix exposed an intentionally over-coarse safe outcome; two finite redacted diagnostics then classified it as a 2xx error object with unrecognized standard metadata. No identical tuple was sent after T10. |

Rollback removes only the P6-03 module, synthetic fixtures/tests, and documentation. It requires
no database migration, credential revocation, server restart, proxy/TUN cleanup, or external
Provider action. The live matrix is exhausted under its approved policy. P6-03 is `BLOCKED` and
P6-04 remains pending until a new explicit unblocking decision is recorded.
