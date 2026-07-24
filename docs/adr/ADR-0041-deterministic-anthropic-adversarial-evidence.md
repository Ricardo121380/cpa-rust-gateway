# ADR-0041: Deterministic Anthropic adversarial protocol evidence

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-22` |
| Task | `P5-08` |
| Contract | [BC-PROTOCOL-006](../contracts/BC-PROTOCOL-006-anthropic-adversarial-stream-safety.md) |

## Context

P5-01, P5-03, and P5-06 establish pure request decoding, unknown-field retention, Tool stream
state, and Anthropic termination semantics with frozen happy-path and focused negative fixtures.
Those tests do not by themselves demonstrate that variation in retained unknown values, malformed
input, truncated Tool state, or repeated cancellation cannot panic, fabricate completion, or reopen
transparent retry.

The evidence must remain deterministic and safe to retain in Git. A general production-capture
fuzzer would risk preserving client bodies, while non-replayable random input would make a failure
harder to reproduce and review.

## Decision

1. Add a fixed, synthetic request corpus with explicit accept or `ClientRequestError/Request`
   outcomes. It covers retained unknown fields, unknown opaque content, nested duplicate names,
   structurally truncated JSON, invalid user Tool use, and ambiguous cache control.
2. Add two fixed-seed, bounded `proptest` suites in `protocol-anthropic`: 256 unknown-extension
   shapes must retain exact raw values at root/message/content boundaries, and 256 malformed or
   truncated Tool schedules must neither panic nor produce an Anthropic success termination.
   Failure persistence is disabled; diagnostics name only a fixed suite/seed, never a client body.
3. Add a fixed-seed, bounded cancellation property in `gateway-stream` covering repeated
   cancellation before and after actual first semantic delivery. Cancellation must remain
   request-owned, prevent transparent retry, reject later producer delivery, and end the consumer
   without a normal completion.
4. `proptest` is declared only as a `gateway-stream` development dependency. It is already a
   workspace-locked dependency, so the lock change records a direct test edge only and introduces
   no new resolved package version or production runtime dependency.

## Consequences

- P5 gains replayable adversarial evidence without a network listener, Provider, real credential,
  ambient configuration, fuzz corpus persistence, or product-path behavior change.
- Retained fields remain protocol data, not execution approval. P5-04's lossless-bridge admission
  still rejects or excludes a request whose complete semantic preservation cannot be proved.
- A Tool stream must fail closed at truncation; it cannot emit a `message_delta` or `message_stop`
  that makes a partial Tool response appear complete.

## Alternatives considered

- Save arbitrary fuzz inputs or live Claude Code requests: rejected because they can capture client
  data and make secret review dependent on a test harness.
- Use a fresh random seed and persisted shrinking corpus: rejected because a failing CI result
  would not be deterministic from the committed source alone.
- Treat a truncated Tool as an empty-object Tool: rejected because only an actually empty completed
  input is allowed to normalize to `{}`.
- Test cancellation only before first delivery: rejected because first semantic delivery and
  cancellation close transparent retry for different monotonic reasons.

## Validation and rollback

The fixed corpus, both 256-case protocol suites, and the 128-case cancellation suite pass under
their committed seeds. Targeted tests and Clippy cover `protocol-anthropic` and `gateway-stream`;
the P5 closeout full gate covers the added development dependency and lockfile. Reverting P5-08
removes only test evidence and its existing locked test edge. It does not change a database,
Credential, endpoint, configuration, or externally visible service behavior.
