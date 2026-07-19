# BC-ROUTER-001 Validated Route Compiler

| Field | Value |
|---|---|
| Contract | `BC-ROUTER-001` |
| Task | `P2-06` |
| Status | IN_PROGRESS |
| Domain | Config Version semantic validation and secret-free Route compilation |

## Entry and boundary

`gateway-control::RouteCompiler` compiles one P2-05 `ControlPlaneConfiguration` with immutable `CatalogView` and `EndpointCapabilityView` inputs. It returns deterministic secret-free `CompiledRouteConfiguration` data for the later P2-07 publisher. It is management-time only: no SQLite connection, Repository, plaintext Credential, encrypted envelope, Client Key digest, Provider, HTTP request, runtime scheduler, or `RouteSnapshot` handle crosses into its output.

```text
P2-05 configuration + injected Catalog/Endpoint-capability views
  -> RouteCompiler
  -> namespace, reference, enablement, Catalog, and capability checks
  -> CompiledRouteConfiguration (no secret/digest/ciphertext)
  -> P2-07 RouteSnapshot publisher
```

## Capability declaration

`public_models.capabilities_json` is a JSON object of required features. A `true` requires that feature for every active Candidate; `false` makes no positive promise. Candidate `capability_override_json` can use the same capability keys plus Candidate-only `allow_unlisted_model`.

| Key | Meaning |
|---|---|
| `tools` | Tool calls are required/supported |
| `parallel_tools` | Parallel Tool calls; requires `tools` |
| `reasoning` | Explicit Thinking/Reasoning support |
| `json_schema` | JSON Schema response/Tool support |
| `vision` | Vision input support |
| `streaming` | Streaming output support |
| `allow_unlisted_model` | Candidate-only exception to missing/expired Catalog admission |

Unknown keys, non-boolean values, malformed JSON, `parallel_tools` without `tools`, and an override that asserts a capability absent from the Endpoint profile are compile errors. A `false` Candidate override only narrows the profile. An active Public Model's required feature set must be a subset of each active Candidate's effective profile.

## Catalog and hard eligibility

For `(endpoint_id, upstream_model)`, Catalog states `manual`, `fresh`, and `stale` are accepted. `expired` and missing entries are rejected unless the Candidate sets `allow_unlisted_model: true`. This remains explicit configuration metadata and never creates a discovery record.

An active Candidate is hard-eligible only when its Public Model, Upstream, Endpoint, Candidate, and relevant Access Group grant are active; its Endpoint profile satisfies capabilities; its Catalog status is accepted; and at least one enabled endpoint binding reaches an active Credential for the same Upstream. Runtime cooldown/quota/circuit state is intentionally not read here.

Candidates below a disabled Public Model are still structurally parsed and reference-checked, but
are not active Candidates: their runtime eligibility does not block compilation and they never
appear in the compiled view. This lets a disabled historical model remain stored without exposing
or validating it as publishable traffic.

## Compile-time conflict matrix

| Condition | Required result |
|---|---|
| Duplicate Public Model name or Alias | reject the complete graph |
| Alias collides with a Public Model name | reject the complete graph |
| Alias/Route/Candidate/Access Group reference missing logical target | reject the complete graph |
| Duplicate `(upstream_id, api_format)` Endpoint | reject the complete graph |
| Active Candidate uses a disabled Upstream/Endpoint or has no active binding | reject the Route |
| Active Candidate is absent/expired in Catalog without exception | reject the Route |
| Endpoint profile is missing or cannot meet requirements | reject the Route |
| Candidate override escalates profile capability | reject the Route |
| Enabled active Access Group grant targets a nonpublishable Route | reject the complete graph |

Errors expose a stable structural code and non-secret identifiers/configuration labels only. They never include a complete Client Key, digest bytes, plaintext Credential, ciphertext, Master Key, or Pepper.

## Deferred behavior

P2-06 does not publish/activate/rollback a Version, create weighted schedules, authenticate a request, select an actual Credential, execute a Provider, persist or discover Catalog data, or expose management HTTP/CLI. Those behaviors remain in P2-07, P2-08, P3, P4, and P2-10.

## Corresponding tests

- A valid multi-Candidate graph compiles into deterministic secret-free Public Model, Alias, Route, Candidate, and Access Group views.
- Each conflict-matrix row returns a stable error code; invalid input returns no partial output.
- Fresh/stale/manual Catalog entries succeed; expired/missing entries require the explicit Candidate exception.
- Capability narrowing succeeds; malformed, unknown, escalating, and incompatible declarations fail.
- Source/boundary tests prove Router, Provider, and HTTP receive no Repository or control-plane graph types.
