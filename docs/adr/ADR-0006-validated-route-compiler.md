# ADR-0006: Validated Route Compiler

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-06`; `D01/D04/D10/D11/D20/D21/D31`, `H05/H06`, `J20`, `L17-L30`; [BC-ROUTER-001](../contracts/BC-ROUTER-001-validated-route-compiler.md) |

## Context

P2-05 can load a complete structural Config Version but deliberately does not decide whether the
graph is safe to publish. P2-02 left cross-table Public Model/Alias namespace conflicts,
Candidate capability/canonical-catalog validity, disabled references, and Access Group route
publication eligibility to this Task. P2-07 needs a secret-free, deterministic compiler output,
but must not receive mutable SQLite rows or perform validation on a request.

No persistent discovery Catalog exists before P4. P2-06 still needs a precise pre-publication
Catalog boundary so a Candidate cannot silently claim an unavailable upstream model merely because
the future discovery worker has not been implemented yet.

## Decision

- `gateway-control` owns the `RouteCompiler`. It accepts one P2-05
  `ControlPlaneConfiguration` and immutable, injected `gateway-catalog` views for Endpoint model
  Catalog state and Endpoint semantic capabilities. It returns a secret-free compiled
  configuration; it neither opens SQLite nor enters Router/Provider/HTTP request execution.
- `gateway-catalog` owns small, storage-neutral `CatalogView` and `EndpointCapabilityView` value
  types. P2-06 injects them explicitly; P4 later supplies their discovery-backed population and
  persistence without changing the compiler's decision boundary.
- Public Model `capabilities_json` is a required capability declaration. Candidate
  `capability_override_json` may remove a supported capability or assert one already supplied by
  the Endpoint profile, but it cannot elevate an Endpoint capability. The supported keys are
  `tools`, `parallel_tools`, `reasoning`, `json_schema`, `vision`, and `streaming`; all values are
  booleans. `parallel_tools` requires `tools`. Candidate-only
  `allow_unlisted_model` is also boolean and is the sole exception to normal Catalog admission.
- A Catalog entry in `manual`, `fresh`, or `stale` state is hard-eligible. `expired` and missing
  entries fail compilation unless that Candidate explicitly sets `allow_unlisted_model: true`.
  This is an audited configuration exception, not automatic discovery behavior.
- The compiler rejects duplicate Public Model names/Aliases, Alias-vs-Public-Model namespace
  collisions, duplicate `(upstream, api_format)` Endpoints, dangling or nonpublishable active
  Routes/Candidates/Access Group grants, disabled active Endpoint/Upstream references, absent
  active Endpoint-Credential bindings, Catalog rejection, and unmet or malformed capabilities.
  Structural SQLite constraints remain the first line of admission; compiler checks still protect
  manually assembled/imported graphs and make release errors deterministic.
- Candidates under a disabled Public Model are structurally parsed and reference-checked, but are
  not active Candidates. Their unavailable Catalog, Endpoint, Upstream, or binding state does not
  make a disabled model publishable or block a Version that omits it from the compiled view.
- P2-06 does not construct a `RouteSnapshot`, publish/activate/rollback a Version, create an
  atomic schedule, verify Client Keys, choose a Credential at runtime, query an actual Provider,
  or persist/discover Catalog data. Those concerns remain P2-07, P2-08, P3, and P4.

## Consequences

P2-07 can accept only compiler-approved, immutable public model, Alias, Access Group, Route, and
Candidate values, with no Credential ciphertext or Client Key digest in its output. The Compiler
will reject a draft that cannot safely serve an enabled Access Group, so a later publisher cannot
turn a schema-valid but semantically invalid configuration into an active Snapshot. Static/manual
Catalog test fixtures are explicit until P4 provides last-success discovery semantics.

## Alternatives considered

- Let P2-07 discover conflicts while publishing was rejected because it combines semantic
  validation with state transition and makes error testing/management preview opaque.
- Query SQLite or Provider metadata from Router was rejected because it violates the no-database,
  no-network request hot-path baseline.
- Treat every unknown Candidate model as valid until P4 was rejected because it would turn a
  missing Catalog proof into silent production routing. The explicit `allow_unlisted_model`
  exception keeps that risk visible in configuration and tests.
- Allow Candidate overrides to claim arbitrary unsupported features was rejected because a
  configuration typo could advertise Tool/Thinking/stream behavior a selected Endpoint cannot
  deliver. Overrides can only narrow or confirm the profile.

## Validation and rollback

Tests cover a valid multi-Candidate graph, every conflict-matrix rejection with a stable error
code, stale/expired/missing Catalog behavior, capability narrowing/escalation, disabled active
references, duplicate Endpoint API Format, and absence of sensitive data in compiler output.
No schema migration is added. Before P2-07, rollback is removal of the pure compiler code; it
does not change database activation state or request-path behavior.
