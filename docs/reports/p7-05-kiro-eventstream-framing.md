# P7-05 Kiro AWS EventStream framing report

`event_stream` adds a bounded pure decoder for Kiro's AWS binary stream envelope. It validates
both CRC32 layers, parses duplicate-free strict headers, retains payload bytes for a later semantic
decoder, and redacts all value-bearing diagnostics.

The fixed fixtures prove one-chunk, every binary split, and byte-at-a-time decoding are identical;
they also cover all defined AWS header types, prelude/message CRC errors, malformed and duplicate
headers, recovery to a following valid frame, truncated EOF, and the five-error terminal limit.

No Kiro payload JSON is interpreted here. In particular, this task does not claim Tool, Thinking,
SSE, model, network, quota, or error-classification behavior. The local Full Gate and independent
review passed, so this task is `LOCAL_PASS_PENDING_PHASE_GATE`.
