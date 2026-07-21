# ADR-0038: Endpoint-format-isolated protocol routing

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P5-05` |
| Contract | [BC-ROUTER-005](../contracts/BC-ROUTER-005-endpoint-format-isolated-protocol-routing.md) |

## Context

The control plane already persists one `api_format` per Endpoint and rejects duplicate formats
within one Upstream. Before P5-05, the route compiler discarded that property while constructing
a `CompiledRouteCandidate` and `RouteSnapshot`. The runtime could therefore see an Endpoint ID
but not prove whether it spoke OpenAI Responses or Anthropic Messages before selecting it.

Runtime Health, quota, and Circuit state already use `EndpointId` as their target identity. That
is the correct isolation boundary, but it only prevents contamination if a protocol-specific
request first selects the matching Endpoint instead of treating a same-Upstream model as enough
evidence of protocol compatibility.

## Decision

1. Compile and publish each selected Endpoint's exact non-secret `api_format` into its immutable
   `SnapshotRouteCandidate`. It remains an Endpoint attribute; no model-name, Base URL, or
   Credential inference is introduced.
2. `ProtocolFormat` recognizes only the exact P5 values `openai/responses` and
   `anthropic/messages`. Unknown/future formats remain representable in the Snapshot but are not
   eligible for either P5 native protocol path.
3. `AttemptOrchestrator::start_for_protocol` and its observed variant filter Candidates by this
   exact format and accept only same-protocol `Canonical` Candidates before reading Health, quota,
   or a Credential pool. The existing unfiltered start APIs remain available to later
   Provider-owned routes that have not yet declared a P5 protocol.
4. Native protocol selection is not a pass-through or cross-protocol bridge. A future caller that
   asks a candidate to transform a Canonical request must first apply the P5-04 admission analyzer
   with the request-local semantic facts; this Task neither serializes nor approves a bridge.
5. Health/Circuit writes retain their existing `EndpointId` target. A failure on a Responses
   Endpoint therefore cannot make a sibling Anthropic Endpoint unavailable merely because both
   share an `UpstreamId`.

## Consequences

- A single Upstream can expose one native Responses Endpoint and one native Messages Endpoint for
  the same public Route without protocol guessing.
- A non-P5 Endpoint remains safe for its future owner rather than being accidentally coerced by a
  loose alias such as `openai_responses`.
- Protocol filtering happens before a Credential lease, so a mismatched Endpoint does not consume
  capacity or create a misleading Health/Attempt observation.
- The change adds no schema migration, Credential material, network traffic, or HTTP handler.

## Alternatives considered

- Infer protocol from model names or shared Base URLs: rejected because both are expressly
  non-protocol identity and would reintroduce same-Upstream contamination.
- Make every unknown `api_format` an OpenAI-compatible fallback: rejected because a format string
  is only safe after its owning protocol adapter declares an exact mapping.
- Share Circuit state by Upstream: rejected because the behavior contract fixes Endpoint as the
  probe, Health, and Circuit boundary.

## Validation and rollback

The compiler test proves that two distinct formats on one Upstream are retained by their compiled
Candidates. The router E2E creates those two Endpoints, makes only the Responses attempt fail,
observes the Responses Endpoint cooldown, then proves an Anthropic attempt on the same Upstream
still obtains its own Credential lease and succeeds. Unit tests also prove exact format parsing
does not accept legacy-looking or unrelated strings, and that a same-protocol
`lossless_bridge` Candidate cannot acquire a lease through the native entrypoint. Reverting this
task removes only the Snapshot field and protocol-filtered orchestration seam; no persisted data
or runtime state needs migration or cleanup.
