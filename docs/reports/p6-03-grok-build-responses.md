# P6-03 Grok Build Responses request, stream, and error report

| Field | Value |
|---|---|
| Plan version | `v1.9` |
| Task | `P6-03` |
| Date | `2026-07-22` |
| Branch | `codex/p6-grok-build` |
| Status | `IN_PROGRESS` — local implementation/review pass; dedicated authorized test-account validation remains required |
| Scope / budget | `M`; one Provider-private fixed HTTP/decoder boundary and one behavior contract; no account-state, quota, cache, continuity, server, or real-traffic scope |
| Task Card | Fixed CLI request profile, exact P2 egress handoff, bounded non-streaming/SSE decode, Tool consistency, and redacted error signals only. Prohibited: cross-Provider imports, socket/proxy/TUN mutation, status remediation, persistent state, or P6-04+ behavior. |
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
| `cargo test --locked -p provider-grok --test p6_03_authorized_build_probe` | PASS; 5 no-network authorization/configuration/redaction tests passed; the sole live test remains ignored without explicit authorization. |
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

## Pending authorized validation and stop boundary

No real Build request has been sent. P6-03 cannot become
`LOCAL_PASS_PENDING_PHASE_GATE`, and P6-04 must not start, until an operator explicitly authorizes
a dedicated test-account probe. The authorization must state the opaque target label, chosen mode
(`non_streaming` or `sse`), exact maximum external-request count, network profile, and stop
condition. The credential/model mapping must stay in an ignored operator-controlled location; it
must not be pasted into chat, committed, or recovered from prior P3 authorization.

On any non-2xx, timeout, malformed response, redaction failure, or unexpected semantic terminal
shape, the probe stops without automatic retry/failover and without modifying proxy/TUN settings or
P6-04/P6-07 policy. Only a safe outcome category may be recorded in this report.

## Timing and next task

| Measurement | Evidence / value |
|---|---|
| Task Card / resumed local work | `2026-07-22`; resumed from the existing P6-03 implementation checkpoint. The original pre-handoff start time is not claimed. |
| Local review/test pass | `2026-07-22T10:07+08:00` after one Clippy-directed test-type correction and one review-correction batch. |
| Final local full gate | `2026-07-22T10:34:09+08:00`; warm wall clock 19s. The durable local check log recorded Rust tests 9s, dependency policy 2s, RustSec audit 2s, and zero seconds for cached fixed-tool install/verification. |
| Code commit | This Task's local checkpoint commit (`P6-03: add Grok Build Responses boundary`); local only, not pushed. |
| Phase closeout tag / Delivery Gate | Not started; G6 is P6's single remote gate. |
| Repeated validations / rework | One necessary test type correction, one semantic review correction batch, then two full-gate findings fixed in scope (`struct_excessive_bools` in the ignored harness and its test-only `tokio` boundary allowlist). The final full gate was re-run only after those fixes. |

Rollback removes only the P6-03 module, synthetic fixtures/tests, and documentation. It requires
no database migration, credential revocation, server restart, proxy/TUN cleanup, or external
Provider action. The next allowed operation is the explicitly authorized P6-03 test-account
validation; P6-04 remains pending.
