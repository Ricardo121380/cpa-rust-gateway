# BC-ROUTER-005 Endpoint-format-isolated protocol routing

| Field | Value |
|---|---|
| Contract | `BC-ROUTER-005` |
| Task | `P5-05` |
| ADR | [ADR-0038](../adr/ADR-0038-endpoint-format-isolated-protocol-routing.md) |
| Status | `DONE` |
| Domain | Immutable Endpoint protocol identity and native Attempt selection |

## Boundary

```text
EndpointConfiguration.api_format
       | compile / publish
       v
SnapshotRouteCandidate.endpoint_api_format
       | exact P5 native-protocol filter
       v
AttemptOrchestrator::start_for_protocol(...)
       | existing EndpointId Health / Circuit / quota lookup
       v
matching Endpoint Credential pool and Attempt driver
```

The boundary reads only immutable Candidate configuration before scheduling. It does not inspect
a model name, Base URL, body, raw extension, Credential secret, or upstream response. It does not
make a network call, mutate a Snapshot, or turn an unknown format into a P5 protocol.

## Preconditions and selection

| Client protocol | Exact eligible `endpoint_api_format` | Other values |
|---|---|---|
| `OpenAiResponses` | `openai/responses` | Rejected before Health/Quota/pool lookup |
| `AnthropicMessages` | `anthropic/messages` | Rejected before Health/Quota/pool lookup |

Only exact P5 strings are recognized. `openai_responses`, `openai/chat_completions`, and any
future owner-specific format are not aliases at this boundary.

## Invariants

1. A Candidate carries its selected Endpoint's exact declared format from compiler to Snapshot;
   the value is not inferred from `upstream_model`, Upstream ID, or URL.
2. Candidate protocol filtering runs before Endpoint Health, quota, Credential availability, and
   lease acquisition. The native P5 entrypoint accepts only same-protocol `Canonical` Candidates;
   a mismatched or non-Canonical Candidate consumes no Credential capacity.
3. Health/Circuit target identity remains `EndpointId`. A Responses Endpoint failure may change
   only that Endpoint's state; it cannot mark a sibling Anthropic Endpoint unavailable by shared
   Upstream association.
4. An unrecognized Endpoint format is not eligible for either native P5 protocol. It remains in
   the immutable snapshot for its future explicit protocol owner.
5. This contract selects same-protocol Canonical endpoints only. `passthrough` and cross-protocol
   `lossless_bridge` eligibility must separately satisfy
   [BC-ROUTER-004](BC-ROUTER-004-protocol-transform-admission.md) with the complete
   request-local semantic input.
6. Public routing errors remain the existing secret-free `CredentialUnavailable` or final safe
   attempt failure; they do not expose formats, Endpoint IDs, URLs, model labels, or Credentials.

## Corresponding tests

- The RouteCompiler accepts two differently formatted Endpoints on one Upstream and retains
  `openai/responses` and `anthropic/messages` on their corresponding compiled Candidates.
- The protocol parser accepts only exact P5 format strings.
- A same-protocol `lossless_bridge` Candidate is rejected before a Credential lease, so it cannot
  bypass the request-local P5-04 admission requirement.
- The router E2E fails a native Responses attempt, confirms only its Endpoint becomes unavailable,
  then successfully starts an Anthropic attempt on the same Upstream with its own Endpoint and
  Credential lease.
