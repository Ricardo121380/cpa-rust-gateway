# BC-PROVIDER-008 Kiro AWS EventStream framing

| Field | Value |
|---|---|
| Task | `P7-05` |
| ADR | [ADR-0050](../adr/ADR-0050-kiro-eventstream-framing.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

| Concern | Requirement |
|---|---|
| Envelope | Accept only complete AWS EventStream frames: big-endian total/header lengths, 12-byte prelude, prelude CRC, headers, payload, and final message CRC. Frame length is at least 16 bytes and at most 16 MiB. |
| Integrity | Verify the prelude CRC over its first 8 bytes before waiting for the declared frame, then verify message CRC over all frame bytes except the trailing CRC. Any mismatch is explicit. |
| Headers | Parse all defined AWS types with strict bounds and UTF-8 for names/string values. Reject empty, duplicate, malformed, truncated, or unknown-type headers rather than overwriting or lossy decoding them. |
| Chunk invariant | A frame sequence decoded from one chunk, every one-boundary split, or byte-at-a-time chunks yields the same ordered validated frame projection. Partial bytes yield no frame. |
| Bounded recovery | Prelude failures resynchronize to a later valid prelude; a message-CRC/header failure discards only its complete validated-envelope frame. At five consecutive failures the decoder clears its buffer and becomes terminal. |
| EOF | An empty drained decoder finishes successfully. EOF with buffered partial bytes returns `TruncatedFrame` and becomes terminal. |
| Diagnostics | Frame payloads, header names/values, selected event labels, CRC numbers, and buffer contents are excluded from public `Debug` and error strings. |
| Deferred | Payload JSON, Event-to-Canonical mapping, Tool/Thinking semantics, HTTP I/O, model discovery, account/quota/error classification, and real Kiro validation are not part of this boundary. |

## C batch adapter integration regression

The HTTP inference source treats a framing/semantic failure as terminal, independently of this
lower-level decoder's bounded resynchronization policy. It discards queued Canonical events from
the failing pull and marks the source finished before returning a pre-start error or one
post-start `StreamError`. Every later pull returns `None`; a valid frame earlier in the same
chunk cannot restart output after the error. The regression
`p7_09_native_inference_adapter::corrupt_frame_discards_queued_events_and_seals_the_source`
checks both states and one transport call. It does not claim output-cap mapping or real Kiro
acceptance.
