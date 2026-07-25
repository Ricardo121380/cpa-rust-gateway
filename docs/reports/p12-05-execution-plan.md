# P12-05 controlled Krill Staging execution plan

| Field | Value |
|---|---|
| Plan version | `v1.50` |
| Task | `P12-05` |
| Status | `IN_PROGRESS` — a late management-to-runtime policy mismatch was repaired after the first revision-bound artifact had been independently verified but before it was activated. The accepted P12-04 loopback Staging instance still has no P12-05 configuration row; the repair's Full gate passed and `CR-P12-05-002` directly approves its exact-SHA artifact before any graph write. |
| Preconditions | P12-04 remains active but disabled at boot, has its own state directory and its two loopback listeners. The incumbent `cli-proxy-api` remains out of scope. |
| Approved remote exception | `CR-P12-05-001` is consumed by the rejected, never-activated `9d62339` artifact. `CR-P12-05-002` directly approves one replacement private GitHub OIDC/Sigstore artifact for the reviewed policy-alignment SHA and the same isolated Staging-only sequence. |
| Credential authority | The operator authorized a read-only use of this Mac's CC Switch `krill` Codex configuration. It contains a current ChatGPT OAuth token set, a separately selected effective Bearer credential, and two configured HTTPS endpoints. Values, endpoint text, account identity, models, request/response bodies, and token fingerprints are never written to Git, reports, command lines, or chat. |

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

1. Read the CC Switch `krill` Codex entry only in a local `0600`/memory-only helper.  Verify that
   the ChatGPT OAuth access, refresh, and ID token fields are present, then identify the distinct
   effective Bearer value and the active Responses endpoint selected by the provider configuration,
   without rendering any value or endpoint text.  First perform a bounded, direct, no-value
   protocol preflight against that active endpoint.  It may retain only endpoint ordinal, status
   class, content-type class, and a bounded model-count/capability outcome.  A non-2xx result
   stops before any Staging mutation.
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
   independent review.  `CR-P12-05-001` authorizes one replacement, revision-bound signing/
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

The local memory-only helper selected the second configured HTTPS endpoint and sent exactly one
direct, no-request-body `GET /models` preflight using only the selected effective Bearer.  It
returned status class `2xx`, content-type class `json`, and a bounded model-count outcome of `9`.
No prompt, model name, endpoint text, credential value, response body, proxy setting, or account
identity was retained.  The helper did not follow redirects and did not write to Staging; the
temporary Staging graph, Client Key, encrypted Credential envelope, and Provider validation
sequence remain pending the revision-bound artifact.

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
