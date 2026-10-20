# ADR-0115: Native namespaced function identity with separate Build activation

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-20 |
| Task / Matrix / Contract references | #62 C1–C3 only; #58 T02/T03/T08/T10; BC-CORE-002/003, BC-ROUTER-004, BC-PROVIDER-003, BC-PROTOCOL-009 |

## Context

The independently frozen public request is rejected at the function-only declaration decoder.
Merely accepting a namespace there would lose group descriptions and make same-name children
ambiguous at execution, response validation and later history. The Responses create schema defines
native groups and separate namespace/name/call_id fields. It does not qualify the fixed Build
1.0.50 service. The user's authorization covers faithful local software and controlled proof;
C4/C5/C6, request-metadata consumption, Claude controls and #61 remain independent blockers.

## Decision

Keep legacy flat functions in the existing typed declarations with unchanged serialization.
When any namespace occurs, retain the entire ordered declaration array in the existing
`openai.responses.tools` raw extension and leave typed declarations empty. Reject a collision
between the two representations. A closed `ResponsesToolSet` view owns the reviewed function-only
shape and execution identities. It preserves group names/descriptions/order and child names,
schemas/descriptions/strict presence. Unknown members, custom/deferred/nested groups, duplicate
groups or functions in one scope reject. Cross-namespace same-name children remain distinct.
Bounds are 256 KiB retained declaration JSON, 128 top-level nodes, 128 children per group,
256 total functions and existing 64-byte ASCII function/namespace names. Requests remain subject
to the existing HTTP/JSON nesting and body limits. Limits reject; they do not truncate.

The identity is `(namespace?, name)`. Call IDs independently correlate calls and results. The
protocol view is reused by Router admission and Build encoding; Router gains a narrow production
dependency on the pure Responses codec, already used by its test matrix. This avoids duplicate
protocol parsers and leaves core independent of protocol, Provider, transport and Router. The
dependency graph stays acyclic; no core type or schema is rewritten.

Validate auto/none/required against the whole declaration set. A flat forced selector addresses
only an actual top-level function. Namespaced forced selectors, concatenated names and invented
selector members remain rejected. Preserve parallel/serial values, require actual ParallelTools
capability for true, and reject a second call under serial. Namespace qualification never grants
ParallelTools. The stock Build capability ledger continues to lack that capability; controlled
full HTTP loops use serial. Pure Router tests separately prove the qualified parallel behavior.

History validates declaration membership, complete object arguments and the core's existing
ordered, unique call/result lifecycle. Supplied result name/namespace strings confirm the exact
preceding call ID. Absent/null/string members remain distinct in extensions and on re-encoding;
null is unspecified, not a synthesized name. A call's namespace, when present, must be a string.
No result gains a name/namespace merely because the corresponding call supplied one.

Build validates native namespace at item start, preserves it in both native item metadata and
the semantic ToolCallStart, and checks exact presence/value at item.done and terminal. Standard
argument events correlate through stable item ID/output index; supplied reviewed call_id/name
must confirm the started call. No namespace field is invented in argument-event schemas. Full
arguments are validated before completion. In the controlled namespace path ToolCallEnd and
function OutputItemEnd are held until terminal identity/order/arguments validate. Validated Usage
is emitted first, so unadmitted Usage cannot expose executable tool completion before failing
projection. Failure, truncation, cancellation or a contradictory terminal never flushes held
completions and never retries the attempt.

### Local qualification seam and production boundary

The normal application rejects every namespace-family request before lease/send unless the
actual adapter/target/mode is qualified. There is currently no production source of this
qualification: **BUILD_ACTIVATION_BLOCKED**. Headers, User-Agent, request JSON, configuration,
existing capability bits and transport injection alone cannot qualify it. Non-Build adapters,
bridges, exact passthrough and other targets do not inherit namespace admission.

The owning gateway tests explicitly set a private `cfg(test)` composition field alongside an
owned local transport. The dependency crate exposes its local-only adapter constructor through
the explicit `controlled-test-support` feature, enabled by the gateway dev-dependency. Gateway
`cfg(test)` is not assumed to propagate to dependencies. The stock gateway contains neither the
test composition field nor its qualifying call, including builds with all features. The Build
builder and decoder also default to unqualified. No upstream URL override, user configuration
or runtime activation API is added. Controlled receive success is LOCAL_CONTROLLED evidence;
it grants no qualification to the actual fixed endpoint/profile or its source/release mapping.

## Consequences

No SQL migration, Canonical struct field, payload v1 format, AAD, reader schema, key or TTL changes.
The existing encrypted extension/event envelope stores the new values. Stored-response
envelope serialization plus independent reopen/owner isolation verify the affected path; existing legacy byte
serialization/reopen/key-rotation tests remain. Older readers can retain opaque envelopes but
older execution code rejects these semantics. Readability is not old-binary execution permission.
No full #65 recovery matrix is implied or repeated.

The unmodified Codex observation stays separate from this synthetic native-function loop. Its
C4/C5/C6 fields, actual parallel requirement and #61 are not removed. A 400, zero receive count,
passing negative test, or later rejection point is not Codex compatibility or #62 completion.

## Alternatives considered

- Flatten or concatenate names: loses grouping/model-facing descriptions and violates the contract.
- Rewrite core Tool/Thinking or payload v1: unnecessary for this existing protocol envelope.
- Gate namespaces on Tools/ParallelTools or client identity: confuses capabilities with field authority.
- Enable every dependency whenever gateway is tested: Rust cfg does not provide that guarantee.
- Complete tools at arguments.done and validate terminal later: exposes false executable completion.

## Validation and rollback

Retain the public baseline rejection and new controlled JSON and 1/7/31-byte SSE paths. Use the
production compiler/factory, actual leased Build adapter, fixed outbound URL, numeric loopback
handoff through UpstreamClientPool, real public listener, independent receiver and bounded SQLite
writer. Verify two dependent tool cycles, result presence, complete history, follow-up, known
Usage/ledger and a separate stored envelope reopen. Test controls, duplicate/conflicting identities, invalid
arguments, done/terminal drift and unqualified zero-receive assembly. Regress flat/Pi/summary/
Usage/cancellation boundaries and separately run the unchanged isolated actual CLI driver.
Bind final Full with its explicitly enumerated documentation/rule overlay and separate Standards
and Spec reviews to immutable source/manifest/lock identities in the slice report.

Rollback restores namespace rejection without deleting retained data. Preserve all existing keys
and history, and verify the intended execution reader before any separately authorized recovery.
This decision authorizes no real Provider/account/paid request, remote write, merge, release or
deployment. The remaining fixed-service qualification is in BC-PROTOCOL-009; controlled success
does not discharge it.
