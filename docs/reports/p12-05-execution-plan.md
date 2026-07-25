# P12-05 controlled Krill Staging execution plan

| Field | Value |
|---|---|
| Plan version | `v1.53` |
| Task | `P12-05` |
| Status | `IN_PROGRESS` — the accepted P12-04 loopback Staging instance is healthy, disabled at boot, and restored to the P12-05 preimage. The corrected `CR-P12-05-004` artifact passed Models plus OpenAI Responses non-streaming/SSE, then the controlled sequence stopped at Anthropic Messages before Tool or Explain. `CR-P12-05-005` directly approves the resulting P12-only output-limit compatibility repair and one new exact-SHA private artifact before a fresh temporary graph. |
| Preconditions | P12-04 remains active but disabled at boot, has its own state directory and its two loopback listeners. The incumbent `cli-proxy-api` remains out of scope. |
| Approved remote exception | `CR-P12-05-001` is consumed by the rejected, never-activated `9d62339` artifact; `CR-P12-05-002` is consumed by the installed policy-alignment artifact; `CR-P12-05-004` is consumed by the validated-and-rolled-back attempt. `CR-P12-05-005` directly approves one new private GitHub OIDC/Sigstore artifact for the reviewed P12-only Messages repair and the same isolated Staging-only sequence. |
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
