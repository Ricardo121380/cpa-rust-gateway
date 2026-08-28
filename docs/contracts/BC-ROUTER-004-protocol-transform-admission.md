# BC-ROUTER-004 Protocol transform admission

| Field | Value |
|---|---|
| Contract | `BC-ROUTER-004` |
| Task | `P5-04` |
| ADR | [ADR-0037](../adr/ADR-0037-protocol-transform-admission.md) |
| Status | `DONE` |
| Domain | Router-local admission of OpenAI Responses / Anthropic Messages transform Candidates |

## Boundary

```text
ProtocolTransformInput
  (source, target, mode, native-body availability, CanonicalRequest,
   response/capability requirements, Endpoint CapabilitySet)
                    |
                    v
      analyze_protocol_transform(...)
                    |
                    +-- Approved
                    +-- Rejected(stable secret-free reason)
```

The boundary is pure and read-only. It neither encodes a target request nor selects an Endpoint,
leases a Credential, touches health/quota/circuit state, writes an Event, changes a Snapshot, or
makes an HTTP/Provider/network call.

## Transform-mode matrix

| Route mode | Source / target | Native body | Result precondition |
|---|---|---|---|
| `Passthrough` | Same protocol | `Exact` | Native payload may be forwarded unchanged; unknown native fields are not reconstructed. |
| `Canonical` | Same protocol | Any | Every retained Canonical field must have the current same-protocol re-encoding proof. |
| `LosslessBridge` | Different protocol | Any | Every retained Canonical field must have the current cross-protocol bridge proof. |
| `CanonicalBridge` | Same or different registered protocol | Any | Same-protocol Canonical proof, or the current cross-protocol bridge proof. |

`Passthrough` with different protocols or without an exact native payload is rejected.
`Canonical` across protocols and `LosslessBridge` within one protocol are rejected. These are
configuration/shape failures, not fallbacks. `CanonicalBridge` is an explicit P12 native-provider
extension; it does not alter the rejection rules of the original three modes.

## Invariants

1. Canonical and Bridge conversions reject non-empty request, message, text-content, and Tool
   declaration extension namespaces. An unknown field is never dropped or guessed.
2. Canonical and Bridge conversions reject opaque content and historical Tool calls/results. The
   current contract proves Tool declaration capability only; it does not prove a historical Tool
   conversation conversion.
3. Canonical and Bridge conversions reject any Thinking setting and prompt-cache key/retention.
   P5-06 must add an explicit semantic mapping before those inputs can participate.
4. Current target roles are `system`, `user`, and `assistant` for Anthropic Messages, and
   `system`, `developer`, `user`, and `assistant` for OpenAI Responses. Other roles are rejected.
   Historical Tool inputs are rejected with their more-specific Tool reason before role admission.
5. `streaming` requires `Streaming`; declared Tools require both `Tools` and `JsonSchema`; an
   explicit JSON-Schema requirement requires `JsonSchema`; parallel Tools additionally require
   `ParallelTools`. These checks consume only the compiler-approved Endpoint capability set.
6. `ProtocolTransformRejection` values are stable and value-free. They must not embed client
   fields, model/URL identity, raw JSON, Tool names/IDs, or credentials. Debug output for the
   input redacts the Canonical request in full.
7. An `Approved` result is admission evidence only. It does not promise that a later HTTP,
   Provider, response, health, circuit, or retry operation succeeds.

## Corresponding tests

- Exact native pass-through accepts a retained extension only when source/target formats match;
  a missing body and cross-protocol pass-through fail closed.
- Canonical/Bridge topology, unknown extension classes, opaque content, historical Tool calls and
  results, Thinking, cache controls, and role incompatibility have explicit stable rejections.
- Streaming, Tools, Tool-schema, explicit JSON-Schema, and parallel-Tool checks each reject an
  insufficient Endpoint capability set; a complete set is admitted.
- A clean OpenAI Responses to Anthropic Messages Bridge is admitted, and `Debug` diagnostics do
  not include model or prompt values.
