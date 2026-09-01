# P13-15A exact upstream model serving report

## Status

`PRODUCTION_LIVE_PASS / FORMAL_GATE_PENDING / P13-15B-E_PENDING`

P13-15A removes CPAR-invented aliases from the advertised model surface and makes an exact
upstream model ID part of request-time route admission. It does **not** claim that every Provider
channel already performs live model discovery. That source, freshness, durable materialization and
real isolation work remains P13-15B-E.

## Production rollout and real evidence

Revision `ac23f387bb71d2494d6b2d999096e2bd1c9c5d1e` was built as a Linux aarch64
binary, verified to embed that exact revision and installed from the immutable release directory
whose artifact SHA-256 is
`7e80336ac59224734f4f7fda532c61c861c223ff17baa36b3d68a391aa4e4aa5`. The service remained
active with zero restarts, `/healthz` returned `200`, and both the backup and live SQLite databases
passed `quick_check`.

The authenticated production catalog returned the exact, alias-free IDs
`gpt-5.6-terra`, `grok-4.20-0309` and `grok-4.5`. The old `grok-cpar-build` alias was absent. An
ID visible through several Routes without a unique policy remained omitted. This list is the exact
projection of currently materialized route candidates; it is not yet the live per-Credential
discovery promised by P13-15B-D. In particular, an official local Build cache also contained
`grok-4.6`, but CPAR correctly did not advertise it before a corresponding authorized exact route
has been discovered and materialized.

A public non-streaming `POST /v1/responses` using `grok-4.5` and a
`prompt_cache_key` returned `200`, `completed`, and the expected marker. Pi and CC Switch were then
migrated from the private alias to `grok-4.5`, kept the CPAR `/v1` base URL and
`openai-responses`, and a real Pi 0.84.3 call returned the exact expected marker. The server event
recorded `protocol=openai_responses`, `requested_model=grok-4.5`, `streaming=1` and a successful
upstream completion. Historical Messages rows were diagnostic probes and are not the saved or
accepted Pi configuration.

## Operational deviations and recovery

An earlier release-switch attempt checked the obsolete `/health` path, failed its own gate and
automatically rolled back; it was then repeated with `/healthz`. During an earlier database backup
refresh, SQLite `.backup` was mistakenly pointed at a pre-created target and produced an empty
backup candidate. The invalid candidate was detected immediately, the live database was restored
from the already verified shadow copy, and row/configuration checks found no loss. Subsequent
deployments use `VACUUM INTO` with a non-existent destination, require non-zero size plus
`quick_check`, and the `ac23f387` deployment completed without either deviation.

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
| revision-bound Linux aarch64 artifact and production health/SQLite checks | PASS |
| production `/v1/models` exact IDs with obsolete alias absent | PASS |
| real Grok Build non-streaming Responses with `prompt_cache_key` | PASS |
| Pi 0.84.3 streaming Responses through CPAR using exact `grok-4.5` | PASS |

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
4. a generic endpoint and multi-Credential/channel cases pass real list/inference isolation tests;
   the Grok Build inference and Pi migration portions have passed, but discovery provenance is not
   complete; and
5. the obsolete legacy input path is removed only after all-client rollback evidence and the
   formal Delivery Gate pass.

No `web/prism/**` file, Management OpenAPI operation or generated client is changed by P13-15A.
The frontend handoff is recorded in `docs/cross-boundary-log.md`.
