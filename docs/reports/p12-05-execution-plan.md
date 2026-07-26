# P12-05 controlled Krill Staging execution plan

| Field | Value |
|---|---|
| Plan version | `v1.61` |
| Task | `P12-05` |
| Status | `IN_PROGRESS` — the accepted P12-04 loopback Staging instance is healthy, disabled at boot, and restored to the P12-05 preimage. The corrected `CR-P12-05-004` artifact passed Models plus OpenAI Responses non-streaming/SSE, then the controlled sequence stopped at Anthropic Messages before Tool or Explain. The later CR-005 artifact's ordered Staging Messages retry also stopped and rolled back. CR-012's one receipt-enhanced Messages diagnostic isolated the output-lifecycle repair. The independently verified CR-013 artifact then passed its fresh loopback Models plus exactly-one Messages validation, including `end_turn`/usage lifecycle checks, and fully rolled back. Tool and Explain remain intentionally unrun; P12-05 therefore remains in progress. A server-only, no-body `/models` discriminator passed, proving the selected Bearer and active HTTPS base URL work from the server; it does not replace the required loopback-Staging Models check. |
| Preconditions | P12-04 remains active but disabled at boot, has its own state directory and its two loopback listeners. The incumbent `cli-proxy-api` remains out of scope. |
| Approved remote exception | `CR-P12-05-001` is consumed by the rejected, never-activated `9d62339` artifact; `CR-P12-05-002` is consumed by the installed policy-alignment artifact; `CR-P12-05-004` is consumed by the validated-and-rolled-back attempt; `CR-P12-05-005` is consumed by its halted-and-rolled-back isolated sequence; `CR-P12-05-012` is consumed by its single receipt-enhanced diagnostic; and `CR-P12-05-013` is consumed by its successful one-request Messages validation and mandatory rollback. No later request is implied. |
| Credential authority | The operator authorized a read-only use of this Mac's CC Switch `krill` Codex configuration. It contains a current ChatGPT OAuth token set, a separately selected effective Bearer credential, and two configured HTTPS endpoints. The operator confirms the selected key and base URL are currently usable; this task must not refresh or re-login. Values, endpoint text, account identity, models, request/response bodies, and token fingerprints are never written to Git, reports, command lines, or chat. |

## Boundary

P12-05 may add one **temporary, isolated** configuration graph to the existing P12-04 Staging
SQLite database: one allowlisted Krill Upstream, one encrypted Bearer
Credential binding, one Public Model/Route/Candidate/Access Group, and one separately issued
Client Key.  It may issue the smallest useful real requests through the existing loopback data
listener to verify `Models`, OpenAI `Responses`, Anthropic `Messages`, a no-side-effect Tool
request, and protected Route `Explain`.

It may not modify the incumbent CPA, its paths, container, credentials, database, Caddy,
Cloudflare, DNS, public listeners, or public traffic.  It may not copy the CC Switch database,
refresh token, ID token, account identity, provider URL, or a complete HTTP body to the server or
repository.  The Staging credential is only the effective Bearer value selected by the current CC
Switch provider configuration, encrypted by the existing Staging control-plane key; it is removed
by the P12-05 rollback.  The presence of OAuth fields does not authorize substituting an OAuth
access, refresh, or ID token for that selected credential.

The P12-04 `serve` binary intentionally exposes readiness plus management only.  Before a real
request, P12-05 must add and review a production data-plane composition.  It must use the
repository's production `RouteSnapshot`, encrypted Credential-pool, egress-policy, direct
transport, retry/cancellation, and HTTP boundaries.  The P3 test-support aggregation harness is
evidence and a design reference only; it must not be linked into the production binary.

## Ordered execution

1. Read the CC Switch `krill` Codex entry only in a local memory-only helper.  Verify that
   the ChatGPT OAuth access, refresh, and ID token fields are present, then identify the distinct
   effective Bearer value and the active Responses endpoint selected by the provider configuration,
   without rendering any value or endpoint text.  First perform a bounded, direct, no-value
   protocol preflight against that active endpoint.  It may retain only endpoint ordinal, status
   class, content-type class, the non-secret header-profile result, and a bounded model-count/
   capability outcome.  The P12-only fixed `User-Agent` is permitted only after a generic-header
   profile fails and its minimal delta proves it necessary; a non-2xx result for that corrected
   profile stops before any Staging mutation.
2. Write and independently review the production runtime composition and its focused regression
   tests.  Bootstrap the Snapshot registry from the same isolated control database, compile only
   encrypted active Credential bindings, preserve the P12-04 management listener, and mount the
   authenticated data-plane routes only on `127.0.0.1:18180`.  Force direct transport (no system
   proxy), HTTPS-only DNS-pinned egress, a single exact host/port allowlist, no redirects, bounded
   response/SSE frames, and a one-candidate, one-attempt route.  The P12 runtime admits only the
   reviewed singleton graph: one active Bearer Credential/binding, active Client Key, Public
   Model, Access Group and route, with no aliases, unlisted-model exception only and no declared
   capability escalation.  A runtime may not start with a populated route if its data-plane
   composition, egress policy, Catalog/capability evidence, or credential pool is unavailable.
3. Run the focused tests, local Full gate, documentation gate, tracked-secret scan, and an
   independent review.  The current approved CR authorizes one replacement, revision-bound signing/
   deployment sequence before any server write; do not upload a local build or reuse an artifact
   whose signed revision predates the source.
4. Before the first P12-05 write, stop the isolated Staging unit and make a root-only timestamped
   snapshot of its `control.sqlite3` plus a value-free manifest.  Restart it and inject the
   current effective **Bearer credential only** through stdin into the protected loopback
   management API; no token is placed in a shell argument, environment, log, report, or file outside the
   encrypted Staging credential envelope.  Create the one temporary graph, validate it, publish
   it, and prove the data listener has not become public.
5. Issue at most the required validation sequence, stopping immediately on the first unexpected
   transport, status, lifecycle, authentication, protocol, or incumbent-continuity result:

   - authenticated `GET /v1/models` exposes exactly the temporary public model;
   - one short non-streaming `POST /v1/responses` yields the canonical completed lifecycle;
   - one short streaming `POST /v1/responses` yields a bounded SSE completed lifecycle;
   - one short `POST /v1/messages` yields the mapped Anthropic lifecycle, and one no-side-effect
     Tool request yields a tool-call representation without executing a tool;
   - protected route `Explain` selects the sole configured candidate without an upstream request.

   The receipt records only request type, outcome, status class, protocol/lifecycle class,
   redacted stable IDs, and whether an external request occurred.  It records neither values nor
   full bodies.
6. Independently review the receipt, listener table, root-only credential metadata, Staging
   control-plane state, encrypted-secret behavior, direct-egress enforcement, journal summary,
   and incumbent continuity.  Restore the P12-05 preimage after the acceptance evidence unless
   the operator explicitly directs that the temporary Staging graph remain for P12-06.  P12-06,
   P12-07, Caddy, Cloudflare, DNS, and any public exposure remain pending.

## Credential and protocol-preflight receipt

The replacement memory-only helper selected endpoint ordinal `2/2` and used direct HTTPS with no
proxy or redirects.  Its generic-header, no-request-body `GET /models` control returned `4xx`.
The minimal header delta proved that the fixed, non-secret
`User-Agent: codex_cli_rs/0.139.0` is sufficient; `OpenAI-Beta` is not required.  The corrected
profile returned status class `2xx`, content-type class `json`, and bounded model-count outcome
`9`.  No prompt, model name, endpoint text, credential value, response body, proxy setting, or
account identity was retained.  No Staging write occurred; the temporary Staging graph, Client
Key, encrypted Credential envelope, and Provider validation sequence remain pending the new
revision-bound artifact.

## Server-only `/models` discriminator

After the first corrected CR-005 Staging attempt returned a gateway `5xx` for Messages, a
separate server-local, no-request-body `GET /models` was run solely to distinguish a possible
server egress/credential failure from a request/decoder failure.  It received the selected
Bearer only through stdin, forced direct HTTPS with no proxy or redirect, used the already
verified fixed User-Agent, and retained no URL, Bearer, model name, response body, account
identity, or token fingerprint.  Its value-free result is recorded in
[the server Models preflight receipt](evidence/p12-05-server-models-preflight-20260725.md):
`2xx`, JSON, and a bounded `data` list of nine entries.

This is evidence that the current selected CC Switch Krill base URL and Bearer are usable from
the server; no refresh or re-login is appropriate.  It did not write the Staging database,
change its service/listeners, create a temporary Client Key, or replace the required authenticated
loopback `GET /v1/models` assertion.  It is a one-request diagnostic record, not P12-05
acceptance evidence and not authorization to begin P12-06.

## One-request Responses structural classifier

`CR-P12-05-006` authorizes one next, server-local direct `POST /responses` solely to classify the
same short, non-streaming request shape that the P12 Messages path would construct.  The selected
base URL, Bearer, and configured upstream model remain in process memory and enter through stdin;
the fixed prompt is non-sensitive and has no Tool, side effect, or account mutation.  The response
must stay in a bounded in-memory pipe.  Its receipt may contain only HTTP status class,
content-type class, JSON-shape class, and the first safe structural gate accepted or rejected by
the current `decode_json_events` contract (for example: completed status, output array, allowed
item type, assistant message/content type, or text field).  It must never record values, output
text, IDs, endpoint text, model name, header values, or a body digest.

The first CR-006 launcher had a local result-capture defect: its selected input passed, and its
remote ephemeral classifier exited and removed itself, but the local zsh wrapper assigned the
captured safe output to a reserved variable before emitting it.  The result is absent and cannot
be reconstructed without risking a duplicate request.  The value-free
[CR-006 capture-failure record](evidence/p12-05-cr-006-capture-failure-20260725.md) therefore
treats that request as consumed even though it cannot prove whether remote validation stopped
before or after the HTTP send.

`CR-P12-05-007` separately permits one replacement classifier.  It sends at most one request,
does not retry either request, and persists its safe receipt root-only on the server before it
returns over SSH.  It neither writes nor starts the Staging graph and cannot count as its
Models/Messages/Tool/Explain acceptance.  A rejected gate means stop and review the minimal
decoder repair before any later Staging write; an accepted gate means review why the previous
gateway boundary still failed before reusing the already accepted CR-005 artifact sequence.  In
either case, P12-06 remains pending.

The replacement completed with `2xx`/JSON and its visible structure classifier reached
`accepted_structural_subset`; its root-owned `0600` receipt is recorded in
[the replacement classifier evidence](evidence/p12-05-server-responses-classifier-20260725.md).
This confirms direct server egress and the response's top-level/status/output/message subset,
but it deliberately does not retain the response and therefore is not a claim that every Rust
`decode_json_events` or `CanonicalResponse` detail passed.  In particular, it cannot recover the
prior Staging HTTP status or gateway error-envelope category.

`CR-P12-05-008` authorizes one receipt-enhanced repeat of the existing isolated Staging procedure
with the already accepted CR-005 artifact.  Its harness must classify only the exact HTTP status
and a closed set of Anthropic error-envelope types when Messages fails.  It must not print a
message, unknown error string, request/response body, or any credential/configuration value.  If
Messages succeeds, the already ordered Tool and Explain checks may run; otherwise it rolls back
and the safe status/type evidence determines whether a source repair is necessary.

That repeat stopped at Messages with `HTTP 502` and the closed Anthropic `overloaded_error` type,
then restored the preimage, listener boundary, encrypted-metadata boundary, and incumbent
continuity.  In the bounded one-attempt runtime, that result is compatible with a pre-first-event
protocol/truncation failure, but it does not reveal whether it occurred in the P12 conversion,
egress admission, or the full non-streaming decoder.  `CR-P12-05-009` therefore permits one
receipt-first direct classifier that mirrors the full safe Rust `decode_json_events` contract,
including non-empty text, Function-call JSON, Usage and reasoning-token constraints, and Canonical
lifecycle preconditions.  It remains a no-body diagnostic; its result decides whether a source
repair is justified and never substitutes for Staging acceptance.

The CR-009 classifier completed `2xx`/JSON with
`accepted_exact_nonstreaming_contract`; its root-only receipt is recorded in
[the full-decoder classifier evidence](evidence/p12-05-server-responses-full-decoder-20260725.md).
That classifier mirrored output validation but its handwritten input omitted the builder's fixed
`type:"message"` member.  It is therefore a near-shape result, not exact P12 outbound-body
evidence.  Under `CR-P12-05-010`, the same-artifact Staging retry again stopped at Messages with
the same `502/overloaded_error` result and rolled back; it did not reach Tool/Explain.

`CR-P12-05-011` permits one receipt-first direct full decoder classifier whose `input` message
exactly includes the P12 builder's type/role/content shape.  It is the only remaining direct
request needed to distinguish a Krill compatibility difference from a runtime attempt-stage
defect.  If it passes, no more external retry is permitted before source-level instrumentation;
if it fails, only its fixed gate may justify a minimal repair.

The exact-shape classifier completed with `2xx`/JSON and
`accepted_exact_nonstreaming_contract`; its root-only, value-free receipt is recorded in
[the exact-shape classifier evidence](evidence/p12-05-server-responses-exact-shape-20260725.md).
It used the same selected configuration, P12 input `type`/role/content construction, output
limit, and visible decoder constraints.  This excludes an external credential/base-URL failure
and a known outbound-body/decoder incompatibility as an explanation for the repeated Staging
`502/overloaded_error`.  It does not grant another external request or count as Staging
acceptance.

## Attempt-stage diagnostic plan

`CR-P12-05-012` records the next permitted source-level action. P12 will replace its local
no-op attempt observation with a bounded, process-local, non-persistent projection for the
existing protected loopback management `GET /admin/requests/{request_id}/attempts` path. The
projection reports only the requested opaque correlation, terminal `succeeded|failed`, and one
closed stage label: `request_conversion`, `egress_admission`, `http_transport`, `http_status`,
`content_type`, `body_read`, `decoder`, or `sse_bootstrap`. It reports no endpoint, credential,
model, URL, header, body, error string, status code, timestamp, token, or digest.

The store is bounded and non-blocking from the data-plane path. Lock contention, malformed
state, or capacity loss must make the management projection unavailable rather than changing a
request result or returning stale evidence. The existing attempt endpoint remains protected on
the separate loopback listener; no data-plane route, listener, proxy, provider configuration,
credential boundary, direct probe, or public exposure is added.

After focused regressions, the applicable local Full gate, secret scan, and an independent review,
this source change requires a new exact-SHA private OIDC/Sigstore artifact and independent
verification. Only then may the original isolated Staging procedure make one receipt-enhanced
Messages diagnostic request. It will restore the preimage regardless of outcome; Tool, Explain,
P12-06, and public exposure remain prohibited unless the P12-05 evidence sequence first completes.

For that one isolated diagnostic, the harness restarts the Staging process before its first
data-plane call. `GET /v1/models` does not allocate a request context, so the ensuing single
Messages request has the existing deterministic process-local correlation `p1-request-0`. The
harness may query only that opaque correlation over the protected management listener and must
require exactly one terminal row with no Endpoint/Credential field before writing its value-free
receipt. An absent, multiple, incomplete, or unavailable projection is a hard stop and rollback;
the harness must not invent a response header, query another identifier, or retry the request.

## CR-012 local review acceptance

The independent CR-012 review found no source-level blocker. The stage ledger has a fixed
eight-record cap, uses only non-blocking `try_lock` access on the data path, and fails the
management projection closed on contention, capacity loss, or terminal-correlation inconsistency.
It holds only opaque request/attempt correlation, one fixed terminal outcome, and the final
closed stage category; it neither retains nor serializes endpoint, credential, upstream, model,
URL, header, body, HTTP status, error text, timestamp, token, or digest.

The review verified that `AttemptOrchestrator` ignores event-admission outcomes for routing, so a
ledger failure cannot change a request result, retry policy, transport, or public error surface.
The optional `stage` member is confined to the existing authenticated, loopback-only Attempt
management route; it creates no data-plane route, listener, Provider request, or persistence.
Focused stage, OpenAPI, management-runtime, SPA, and lint regressions passed, and the complete
local Full gate passed in 130 seconds. See the [CR-012 local review]
(evidence/p12-05-cr-012-local-review-20260725.md) and [Full-gate receipt]
(evidence/p12-05-cr-012-local-full-gate-20260725.md).

The required next boundary is procedural, not a source change: commit the exact reviewed SHA,
produce and independently verify its private GitHub OIDC/Sigstore artifact, then make the one
receipt-enhanced isolated Staging Messages diagnostic and restore the P12-05 preimage regardless
of outcome. P12-06, Tool, Explain, direct classifiers, and public exposure remain prohibited.

## CR-012 diagnostic outcome and CR-013 lifecycle repair

The single authorized CR-012 Staging transaction passed its loopback Models check and sent one
Messages request. The protected, value-free attempt projection recorded a successful decoder-stage
attempt; the transaction then restored the original P12-05 preimage, retained only loopback
listeners, and confirmed incumbent continuity. No retry, Tool request, Explain request, direct
classifier, endpoint value, credential value, model, body, header value, or error text was
retained in this report.

That result establishes that the exact upstream non-streaming Responses payload passed the P12
decoder and that the failure was later in the Anthropic output lifecycle. The narrow CR-013 repair
therefore parses reported usage before visible output, emits an input-only non-final usage snapshot
only when the upstream actually supplied input tokens, emits final usage after the message closes,
and supplies an explicit completion reason. Text completions map to
`end_turn`; completed Function Calls map to `tool_use`. OpenAI Responses retains its complete
final usage, while Anthropic Messages keeps only the usage totals and cache-input values its
protocol can represent and omits OpenAI-only reasoning/cached sub-counters. A missing input usage
remains fail-closed rather than being estimated. The repair does not alter credentials, transport,
retry, egress, listeners, or generic Provider/codec contracts.

Before another Staging write, the exact reviewed SHA must pass focused HTTP-boundary regressions,
package tests, Clippy, Full/docs/Secret gates, and independent review, then produce a new private
OIDC/Sigstore artifact with independent verification. Only that artifact may create a fresh
temporary graph for readiness/listener checks, Models, and exactly one Messages validation; the
preimage must be restored irrespective of the result. Tool, Explain, direct probing, P12-06, and
public exposure remain outside this authorization.

The local source and boundary review is recorded in the
[CR-013 lifecycle review](evidence/p12-05-cr-013-local-review-20260726.md). Its required next
boundary is the fresh private artifact and independent verification, not another local or direct
Provider request.

## Krill compatibility repair

The P12 runtime now constructs its isolated transport request from the existing OpenAI Responses
outbound request, preserves its `accept`, `authorization`, and `content-type` values, and adds
only the fixed compatibility User-Agent.  It also rejects an admitted target that is not exactly
the parsed outbound URL, preserving the generic conversion's credential-to-target binding.  Its
regressions assert the exact four headers, POST/body preservation, and mismatch rejection.  It is
intentionally P12-scoped: `provider-openai-compatible` retains its three-header contract, and the
repair adds no credential source, endpoint, redirect behavior, proxy behavior, egress allowlist,
public listener, or request retry.

## Messages output-limit compatibility repair

The corrected `CR-P12-05-004` binary created, validated, and published the singleton graph,
then passed `GET /v1/models`, one OpenAI Responses non-streaming request, and one OpenAI Responses
SSE request. Its first unexpected result was the following Anthropic `POST /v1/messages`, which
returned a gateway `5xx`; the harness stopped before Tool or Explain and restored the preimage.
The controlled run did not show an upstream credential or base-URL failure: the Messages request
failed while P12 built its outbound OpenAI Responses representation.

The pure Anthropic decoder correctly validates required positive `max_tokens`, but retains it as
`anthropic.messages.max_tokens` because the Canonical core has no shared output-limit field. The
generic OpenAI Responses builder correctly rejects foreign root extensions, so P12 must not
forward that source namespace unchanged. `CR-P12-05-005` permits one narrow P12-only translation:
before a Credential is opened or any outbound request is constructed, a positive-integer
`anthropic.messages.max_tokens` becomes `openai.responses.max_output_tokens`. An existing target
extension, a malformed/non-positive source value, or every other foreign extension remains a
fail-closed error. The generic provider and the Anthropic decoder retain their existing contracts.

After the new exact-SHA artifact passes its local and independent provenance checks, a fresh
temporary graph may re-run only the necessary ordered evidence: readiness/listener checks,
`GET /v1/models`, Anthropic Messages, then the previously unreached no-side-effect Tool and
protected Explain. The previous OpenAI Responses non-streaming/SSE evidence remains scoped to the
identical code path: the translation is a tested no-op when the Anthropic source extension is
absent. Any unexpected result still stops immediately and restores the preimage.

The [CR-005 local review](evidence/p12-05-cr-005-local-review-20260725.md) passed its focused
cross-crate source/target checks, package checks, and a complete local gate. It also records the
corrected test-boundary attempt: a test-only direct Anthropic-codec dependency was rejected by the
crate-boundary gate and removed before the passing rerun. The remaining blocker is procedural: a
new exact-SHA artifact must be independently verified before any fresh graph is written.

## Credential-handling deviation and corrective control

During a post-artifact local re-check, a helper briefly used a new local `0600` temporary
selection spool to pass the selected Bearer between processes. It was removed within that helper
and an absence check found no remaining matching file. No value was rendered, committed,
transferred, sent to the server, written to the Staging database, or used for a server-side
Provider request. Nevertheless, that invocation is **not** evidence for P12-05's memory-only
credential boundary. Under the operator's approved direct-approval convention,
`CR-P12-05-003` permits the unchanged isolated scope to continue only after a fresh pure-memory
preflight has replaced this evidence. The corrected helper must not create a plaintext file or
place the Bearer in a shell argument, environment, log, report, or repository; the only later
server persistence remains the encrypted Staging Credential envelope.

## Local composition evidence

The production composition, singleton-graph guards, bounded non-streaming Tool mapping, SSE frame
bounds, and `active_singleton_graph_builds_an_encrypted_runtime_without_a_send` regression passed
the focused `gateway` tests and the complete local gate on 2026-07-25; see
[p12-05-local-full-gate-20260725.md](evidence/p12-05-local-full-gate-20260725.md).  The new
regression creates the active SQLite graph and encrypted fixture Bearer but makes no outbound
request.  This is source-level evidence only.  Because it changes the signed server binary, it
does not authorize a server write, reuse of P12-04's older artifact, or any Provider request.

## Source review outcome

Review found and repaired one source-level guard before acceptance: a non-empty explicit
`allowed_cidrs` exception could have accompanied the otherwise exact host/port policy.  The
singleton guard now requires an empty CIDR list and its focused regression rejects a private-CIDR
exception.  No source-level blocker remains.  The data listener now receives the same immutable
registry as lifecycle publication, while the data executor independently opens only the active
configuration at process bootstrap.  A subsequent publication therefore fails closed until the
isolated unit restarts rather than combining a new Snapshot with an old encrypted pool.  The
review also confirmed that the request path has no SQLite, file, proxy, or token-config read; the
sole request-time egress path is the DNS-pinned direct transport.  The routing guard permits
exactly one active Bearer binding and `max_attempts=1`, so this task cannot silently fan out or
retry a real validation request.  The remaining blocker is procedural: a newly signed,
revision-bound Linux artifact must be obtained before Staging configuration or Provider testing.

## Integration-review correction

The staging preparation review found that the P12 runtime admitted only
`PriorityFailover`, while the protected management contract deliberately accepts and persists only
`SmoothWeightedRoundRobin`. A temporary singleton graph therefore could not be constructed
through the sole approved management path, even though either policy has the same one-candidate,
one-attempt behavior for this task. The runtime guard and its positive encrypted-runtime fixture
now use `SmoothWeightedRoundRobin`; the focused `gateway` regression, complete `gateway` package
test suite, Clippy check, and the protected management-routing contract test pass. The previously
verified `9d62339` artifact was staged only long enough to run `serve --help`, was never linked as
`current` or started, and was removed after the mismatch was discovered. A Full gate and a new
explicit revision-bound artifact approval remain mandatory; the old artifact is not eligible for
P12-05. The new complete gate receipt is
[p12-05-policy-alignment-full-gate-20260725.md](evidence/p12-05-policy-alignment-full-gate-20260725.md).

## CR-004 replacement artifact acceptance

The exact private replacement artifact for revision
`7a5dead43b257f487e526051acf544578841e6ae` completed the required independent checks before
any Staging write.  Its successful workflow run, private artifact identifier, manifest/ELF/SBOM/
OCI digests, nine-file extraction check, repository verifier, OCI inspection, and keyless
Cosign `verify-blob` result are recorded in the value-free
[CR-004 artifact acceptance](p12-05-cr-004-artifact-acceptance-20260725.md).  The signing
identity is the repository's pinned `release-artifact.yml` workflow on the current deployment
branch and the GitHub Actions OIDC issuer.  This document does not make a server-installation or
Provider-validation claim: the P12-05 preimage, encrypted temporary graph, and ordered loopback
acceptance remain next.

## CR-005 replacement artifact acceptance

The exact private replacement artifact for revision
`2aef274e624c9bb35547623b5383f14731fae515` completed the required independent checks before
any Staging write.  Its GitHub `release-artifact` run `30166446304` completed successfully for
the same revision.  The private artifact `8621799024` extracted to exactly nine regular files;
the repository verifier, OCI inspection, and independent keyless Cosign `verify-blob` all
passed.  The value-free digests, signing identity, workflow receipt, and Linux x86-64 ELF result
are recorded in the [CR-005 artifact acceptance](p12-05-cr-005-artifact-acceptance-20260725.md).

This acceptance authorizes no implicit deployment or provider activity.  The next permitted step
is the existing root-only P12-05 preimage procedure, followed by the narrow listener/readiness,
Models, Messages, no-side-effect Tool, and Explain sequence.  P12-06 remains pending.

## CR-013 exact artifact acceptance

The exact private artifact for revision
`49f8c0f3eb6326d3f2ed6cc612ec8ffd10915938` completed the required independent checks before
any CR-013 Staging write. Its GitHub `release-artifact` run `30185145888` completed successfully
for the same revision. The private artifact `8626803209` extracted to exactly nine regular files;
the repository verifier, OCI inspection, and independent keyless Cosign `verify-blob` all passed.
The value-free digests, signing identity, and workflow receipt are recorded in the
[CR-013 artifact acceptance](p12-05-cr-013-artifact-acceptance-20260726.md).

This acceptance authorizes only a fresh root-only P12-05 preimage procedure using that artifact,
then loopback readiness/listener checks, Models, and exactly one non-streaming Messages response
that must validate its `end_turn` and numeric usage lifecycle in memory. The transaction must
restore the preimage irrespective of outcome. Tool, Explain, direct probes, P12-06, and public
exposure remain prohibited.

## CR-013 controlled Staging outcome

The one allowed transaction completed with Models `pass`, exactly one external Messages request,
Messages `2xx`, and an in-memory valid `end_turn` plus numeric usage lifecycle. The protected
management projection recorded one terminal `succeeded/decoder` attempt. Its root-only receipt
also confirms completed progress, no failure stage, restored database preimage, restarted
Staging, and uninterrupted incumbent continuity. The independent post-review re-confirmed that
the temporary harness was removed, the current release link was restored, the Staging unit remains
active but disabled at boot, both listeners remain loopback-only, and the temporary graph is gone.
The value-free evidence is recorded in the [CR-013 isolated Staging receipt]
(evidence/p12-05-cr-013-staging-receipt-20260726.md).

This successful result consumes CR-013 and closes its Messages-only scope. It is not an implicit
authorization for Tool, Explain, direct probing, P12-06, or public exposure; P12-05 itself remains
in progress pending an explicit scope decision for its remaining evidence.

## Stop conditions and rollback

- A missing selected Bearer credential, a preflight result outside `2xx`, an endpoint that cannot
  meet the declared HTTPS/direct-egress policy, an ambiguous model capability, a Static-Key-only
  request shape, or any attempt to substitute an OAuth access/refresh/ID token is a hard stop.
- Any public/non-loopback listener, proxy use, redirect, egress allowlist mismatch, plaintext
  token persistence, unreviewed artifact, real request beyond the sequence above, failed
  canonical lifecycle, Staging health failure, or incumbent CPA state change is a hard stop.
- Rollback restores only the root-only P12-05 preimage of
  `/var/lib/cpa-rust-gateway/control.sqlite3` while the isolated unit is stopped, removes any
  temporary test Client Key from operational tooling, restarts the same isolated unit, and repeats
  the loopback/readiness/incumbent checks.  It never restores, stops, restarts, or rewrites the
  incumbent CPA.
