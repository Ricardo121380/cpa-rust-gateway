# ADR-0050 Kiro AWS EventStream framing

| Field | Value |
|---|---|
| Status | Accepted |
| Task | `P7-05` |
| Contract | [BC-PROVIDER-008](../contracts/BC-PROVIDER-008-kiro-eventstream-framing.md) |

`provider-kiro` owns a pure incremental decoder for the AWS EventStream binary envelope used by
Kiro. It verifies prelude and message CRC32, retains only fully validated headers and payloads,
and exposes framing errors without payload, header-value, or checksum-value diagnostics.

The decoder is bounded to 16 MiB per frame, 32 MiB pending input, and five consecutive recovery
failures. It resynchronizes malformed preludes at a later valid prelude and drops a single
length-validated frame on message CRC or header parsing failures. This permits a following valid
frame to be decoded without allowing an infinite scan; the fifth consecutive malformed condition
stops the decoder.

This decision deliberately stops before Kiro payload JSON interpretation, Canonical event
generation, Tool/Thinking semantics, HTTP I/O, or Provider error classification. Those steps
remain separately testable P7 responsibilities.
