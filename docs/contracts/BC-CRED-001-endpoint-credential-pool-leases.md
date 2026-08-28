# BC-CRED-001 Endpoint Credential pool leases

| Field | Value |
|---|---|
| Contract | `BC-CRED-001` |
| Task | `P3-04` |
| Status | IN_PROGRESS |
| Domain | Endpoint-local Credential scheduling, bounded concurrency, and request-scoped release |

## Entry and boundary

`gateway-control::CredentialPoolCompiler` receives a complete persisted
`ControlPlaneConfiguration` and an external `SecretStore` only on the management/control path. It
validates references and decrypts eligible Endpoint/Credential bindings into
`gateway-upstream::EndpointCredentialPools`. Request code receives only that immutable pool set.

`gateway-router::RouteCredentialScheduler` receives one immutable `Arc<RouteSnapshot>` and one
matching `Arc<EndpointCredentialPools>`. It selects a non-secret Candidate through P3-03, then
immediately tries to acquire an Endpoint-local `CredentialLease`.

This contract does not make an HTTP request, expose a Provider Adapter, classify a response,
persist a lease, mutate health/cooldown/circuit/quota state, manage an Attempt exclusion set, retry,
or fail over after a semantic event. Those remain P3-05 through P3-10 work.

## Preconditions

- Credentials were encrypted with the existing length-delimited AAD over Config Version,
  Credential ID, and Upstream ID.
- A binding, its Endpoint, and its Credential share one configured Upstream.
- Only enabled Upstreams, Endpoints, and bindings with `CredentialStatus::Active` enter a runtime
  pool. Inactive rows remain validated control-plane data but cannot acquire a lease.
- A runtime Credential kind and decrypted secret are non-empty; revision and priority are
  non-negative; binding weight and concurrency are positive.
- Each Endpoint pool has unique Credential IDs. Each priority tier uses at most `1024` smooth
  weighted slots, and a two-stage scheduler is constructed from the same validated configuration
  generation as its Route Snapshot.

## Required behavior

| Concern | Required behavior |
|---|---|
| Control-path Secret handling | Reconstruct AAD and authenticate/decrypt before a pool is returned. Store/SecretStore do not enter the request path. |
| Endpoint isolation | Every Endpoint has a distinct pool and atomic cursor set. A Credential is selected only from the chosen Candidate's Endpoint. |
| Priority | Inspect lower numeric Credential priority tiers first. Inspect a lower-preference tier only after all bounded slots in every higher tier cannot acquire capacity. |
| Intra-Endpoint weight | Build a deterministic smooth weighted sequence before runtime selection. In one complete unsaturated tier cycle, each Credential appears exactly its positive configured weight count. |
| Concurrency | Reserve capacity with a per-Credential atomic compare-and-exchange. Never issue more than the configured concurrent leases for that Credential. |
| Saturation | Skip a saturated Credential or Endpoint pool. If no Candidate can acquire a lease, return `CredentialUnavailable/Credential` without identifiers or Secret text. |
| Release | A lease is non-cloneable. Dropping it, including request cancellation, releases exactly one acquired capacity unit. `release(self)` consumes the lease and has the same effect. |
| Two-stage fairness | Route Candidate selection happens before Credential selection. Adding keys or changing weights inside an Endpoint does not change the configured inter-Endpoint Candidate distribution. |

## Invariants

- No pool construction or selection logs raw Credentials, Authorization values, ciphertext, or
  plaintext Secret bytes. `CredentialSecret`, `CredentialLease`, pool input, and pool `Debug`
  forms are redacted.
- The only mutable request-path state is Endpoint-tier cursor and per-Credential lease count; it
  is atomic and scoped to a pool. There is no global scheduler lock, SQLite query, or unbounded
  queue/scan.
- A lease cannot be cloned or used after explicit release. A pool remains safe to drop while a
  lease exists because the lease retains its selected slot until releasing it.
- An AEAD/AAD failure, malformed binding graph, empty secret, invalid number, duplicate binding,
  or oversized schedule yields no partial pool set.
- P3-04 does not decide whether a Credential is healthy, cooling, quota-limited, circuit-open, or
  retry-excluded; later predicates may reject Candidates before pool acquisition.

## Error semantics

| Condition | Result |
|---|---|
| Unknown/mismatched binding relation, duplicate configured identity, invalid AAD | Safe `CredentialPoolCompileError`; no pool set returned. |
| AEAD key/version/ciphertext/AAD failure | Safe `CredentialPoolCompileError::SecretStore`; no plaintext or partial pool returned. |
| Empty/duplicate/malformed runtime input or over-1024 schedule | Safe `CredentialPoolBuildError`; no affected pool returned. |
| Unknown Route, predicate rejection, missing Endpoint pool, or all pools saturated | `GatewayError(CredentialUnavailable, Credential)`; no Candidate/Endpoint/Credential diagnostic. |

## Corresponding tests

- `gateway-upstream::credential_pool::tests` proves exact `5:1:1` in-tier weighting, saturation
  fallback, drop/explicit-release restoration, concurrent maximum lease enforcement, bounded-plan
  rejection, and secret redaction.
- `gateway-router::credential_scheduler::tests` proves concurrent `3:1` Candidate distribution
  remains independent of two equal Endpoint-A Credentials, yielding `1:1` local distribution;
  it also proves saturated Candidate fallback and caller-provided future eligibility filtering.
- `gateway-control::credential_pool_compiler::tests` proves stable AAD-authenticated decryption,
  redacted runtime material, fail-closed AAD mismatch, inactive Credential exclusion, rejection of
  orphaned inactive records, and duplicate-binding rejection before a pool is returned.
