# P13-15A exact upstream model serving report

## Status

`LOCAL_PASS / PRODUCTION_ROLLOUT_PENDING / P13-15B-E_PENDING`

P13-15A removes CPAR-invented aliases from the advertised model surface and makes an exact
upstream model ID part of request-time route admission. It does **not** claim that every Provider
channel already performs live model discovery. That source, freshness, durable materialization and
real isolation work remains P13-15B-E.

## Delivered behavior

- Authenticated `GET /v1/models` serializes the stable, deduplicated union of exact
  `SnapshotRouteCandidate.upstream_model` values visible to the Client Key's Access Group.
- Only compiler-retained, hard-eligible candidates contribute an ID. Expired catalog admissions,
  inactive bindings and unauthorized routes do not appear.
- A request using an advertised exact ID resolves inside the same pinned immutable Route Snapshot
  and Access Group. The routed executor then rejects every candidate whose exact upstream model
  differs, before Credential lease or Provider I/O.
- If the same exact upstream ID is visible through more than one Route, it is omitted from the
  public list and direct request resolution fails closed as ambiguous. CPAR does not invent a
  Provider-prefixed alias to hide the collision.
- OpenAI Chat Completions, OpenAI Responses, Anthropic Messages and Responses WebSocket share the
  same exact-ID resolver. Pi remains OpenAI Responses; this routing work does not change its
  protocol to Messages.
- Legacy Public Model names and aliases remain accepted only as an unadvertised migration path.
  They are not upstream discovery evidence and are scheduled for removal only after P13-15E proves
  rollback-safe client migration.

## Tenant-isolated Grok Build cache identity

The same release fixes the Pi/Grok Build `prompt_cache_key` boundary. Ingress retains only the
authenticated `ClientKeyId`, never the presented Client Key. The Build adapter derives an opaque
HMAC identity from a dedicated 32-byte deployment credential, the Client Key identity, the exact
upstream model and the raw client cache key. The raw key is never forwarded and two CPAR tenants
cannot intentionally select the same upstream cache namespace.

The deployment credential is independent of the Master Key, backup key and Client Key pepper.
The systemd unit therefore declares a sixth `LoadCredential` named `grok-build-cache-key`.

## Verification

| Check | Result |
|---|---|
| `cargo test --workspace --all-features --no-fail-fast` | PASS |
| strict workspace Clippy with `unwrap_used` and `expect_used` denied | PASS after replacing two test-fixture `expect` calls and keeping explicit audited flow allowances |
| `cargo fmt --all -- --check` | PASS |
| access-group visibility and hard-eligibility projection | PASS |
| exact-ID Responses admission and downstream response model | PASS |
| duplicate exact ID across visible Routes | PASS, returns `Ambiguous` |
| raw `prompt_cache_key` absence and Client-Key-derived opaque identity | PASS |
| systemd six-credential checker and deployment documentation | PASS |

`./scripts/check.sh docs` passed document links, 107 contract references, the one-active-task
plan-state guard, deployment thresholds and the tracked-secret scan. It then stopped only on the
pre-existing extra EOF blank line in Claude Code-owned `web/prism/src/features/models/model.ts`.
P13-15A does not modify that frontend file; the backend diff itself passes `git diff --check`.

## Explicit remaining boundary

P13-15A serves exact IDs already present on immutable route candidates. It does not turn a manually
configured candidate into authoritative upstream discovery. P13-15 remains `IN_PROGRESS` until:

1. each Grok Build/Web/Console, Official, Kiro and generic compatible channel owns its exact
   Endpoint/Credential-scoped source;
2. successful snapshots, freshness, last-success fallback and isolated removal evidence are
   durable;
3. discovered IDs materialize exact Credential-scoped routes atomically;
4. Grok Build and a generic endpoint pass real list/inference isolation tests; and
5. Pi/CC Switch migrate to a discovered exact ID and the formal Delivery Gate passes.

No `web/prism/**` file, Management OpenAPI operation or generated client is changed by P13-15A.
The frontend handoff is recorded in `docs/cross-boundary-log.md`.
