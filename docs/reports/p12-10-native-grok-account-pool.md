# P12-10 native Grok account pool

Status: `P12-10E_DONE_P12-10F_NEXT`

## Corrected architecture

CPAR is the final reverse proxy and must own the Grok account pool itself. It must not depend on a
running grok2api process in the final topology. The current grok2api `v3.0.10` deployment and its
exact source revision `c27f0545197b3edf41d5deedcc2c3c3597887766` are behavior and migration
references only. After parity, migration, rollback and observation pass, grok2api may be stopped.

The rejected alternative was a CPAR OpenAI-compatible upstream pointing to grok2api loopback. A
successor graph for that alternative was briefly published but failed readiness before serving
traffic. Newapi was never changed. The graph was rolled back to its predecessor in an offline
transaction equivalent to the normal activation state transition, the old signed binary was
restored, SQLite `quick_check` passed, CPAR Models returned 200, Newapi status returned 200, and the
unused successor plaintext client keys were deleted. The proxy code and plan were reverted.

## Existing CPAR foundation

This is not a new proxy implementation from zero:

- `provider-grok` already implements Build OAuth import/refresh, Build Responses, Official API-key,
  Web credential/session/quota/failure primitives, continuity and strict decoders.
- `RouteCredentialScheduler` and `EndpointCredentialPools` already provide weighted, bounded,
  multi-credential leases per Endpoint. Runtime Health and Quota already exclude only the affected
  credential/model binding and support controlled recovery.
- the control store already encrypts multiple credentials and P6 runtime tables retain Build
  refresh, catalog, quota, billing, affinity and continuity state.

The missing production capability is a native account lifecycle around those primitives: durable
multi-account import, Console SSO/provider execution, proactive refresh workers, account-level
configuration and observability, and a migration path from the existing grok2api account store.

## Reference behavior to port

The frozen grok2api source separates Build, Web and Console account identities; account links do not
share quota or health. Its useful invariants are:

- enabled/auth-active/cooldown/quota/model-capability eligibility before lease;
- priority and per-account maximum concurrency;
- refresh due time jitter before expiry, bounded refresh concurrency and persistent failure/backoff;
- exact `reauthRequired`, account cooldown, model quota block and quota-recovery states;
- Console SSO as a stateless Responses provider that can serve Responses, Chat and Messages;
- a Console/Web 403 normally rebuilds the bound egress/browser session and is not automatically
  proof that an account credential is invalid;
- batch import, refresh, quota synchronization and cleanup remain control-plane work and never run
  inside the inference hot path.

CPAR ports these behaviors into its Rust layers and preserves its stricter rules: immutable routing
snapshots, DNS-pinned egress, encrypted-at-rest secrets, no raw credential logging, bounded queues,
single-owner refresh transactions and explicit rollback.

## Execution slices

| Slice | Deliverable | Acceptance |
|---|---|---|
| P12-10A | Freeze the `v3.0.10` behavior map and CPAR gap matrix | exact source revision, schema/state/scheduler/refresh/Console mappings reviewed |
| P12-10B | Native Grok account aggregate and encrypted batch-import boundary | Build/Web/Console identities, links, auth state, priority/concurrency and redacted CRUD/import tests |
| P12-10C | Compose accounts with existing credential pools | weighted lease, saturation, cooldown, quota/model block, exclusion, recovery and restart tests |
| P12-10D | Proactive credential/quota workers | jittered refresh, singleflight lease, bounded concurrency, persistent backoff/reauth and crash recovery |
| P12-10E | Native Grok Console runtime and Web production binding | strict target/header/request/JSON/SSE/Tool/Usage/error fixtures; Chat/Responses/Messages bridge matrix |
| P12-10F | Memory-stream migration adapter from grok2api | no plaintext temp file; source/accepted/rejected/link counts, transactional rollback and idempotent rerun |
| P12-10G | Controlled live subset and full-pool parity | direct CPAR native attempts, account attribution, quota/cooldown correctness, three protocols and rollback |
| P12-10H | Decommission rehearsal and representative observation | grok2api stop/start rollback drill, then CPAR-only 72h/real-traffic gate before permanent retirement |

P12-10A is complete in the [native Grok gap matrix](p12-10a-native-grok-gap-matrix.md). P12-10B is
complete in the [native account-pool report](p12-10b-native-grok-account-pool.md). P12-10C is
complete in the [native scheduling report](p12-10c-native-grok-scheduling.md). P12-10D is complete
in the [native worker report](p12-10d-native-grok-workers.md). P12-10E is complete in the
[Console/Web runtime report](p12-10e-grok-console-web-runtime.md). P12-10F is the next
implementation slice.

## Migration boundary

The current grok2api database remains unchanged while implementation is incomplete. Migration must
use a root-only streaming exporter/importer: decrypt with grok2api's current key only in the source
process, pass bounded records through a pipe, validate and re-encrypt immediately with CPAR's master
key, and retain only value-free counts/receipts. Copying grok2api ciphertext is invalid because its
envelope and key ownership are different. No account is deleted from grok2api until CPAR-only parity
and rollback drills pass.
