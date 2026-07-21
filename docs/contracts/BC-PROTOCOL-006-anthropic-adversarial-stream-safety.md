# BC-PROTOCOL-006 Anthropic adversarial stream safety

| Field | Value |
|---|---|
| Contract | `BC-PROTOCOL-006` |
| Task | `P5-08` |
| ADR | [ADR-0041](../adr/ADR-0041-deterministic-anthropic-adversarial-evidence.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Deterministic adversarial evidence for retained Anthropic input, Tool terminality, and cancellation |

## Preconditions and bounds

1. All corpus values, models, Tool IDs/names, event schedules, and property seeds are synthetic
   constants committed with the test. No test reads a `.env`, process credential, repository
   configuration, network socket, or Provider response.
2. The fixed request corpus has explicit expected classifications. The two Anthropic property
   suites run 256 cases each with fixed seeds; malformed Tool schedules contain 1 through 64
   generated opcodes. The cancellation suite runs 128 fixed-seed cases with 1 through 16 repeated
   cancellation calls.
3. `proptest` failure persistence is disabled. A failure is reproducible from the committed seed
   and test; no generated case becomes an on-disk corpus artifact.

## Required behavior

| Concern | Required behavior |
|---|---|
| Unknown fields | Accepted unknown root, message, and text-block fields retain exact raw JSON under their explicit `anthropic.*` extension namespace. Retention alone never proves a cross-protocol bridge or execution capability. |
| Invalid request shape | Duplicate names at any nesting level, structurally incomplete JSON, invalid Tool role/context, and ambiguous cache semantics produce `ClientRequestError/Request`; they are not normalized into a different semantic request. |
| Unknown content type | A structurally valid future content block remains one opaque canonical content value rather than being treated as text, Tool input, or executable instruction. |
| Truncated Tool | A Tool begun with partial JSON cannot end its message or completed response. It emits no Anthropic `message_delta` or `message_stop` success termination. |
| Malformed Tool schedule | Duplicate/unknown Tool events, non-object completed arguments, premature response/message end, and terminal stream error schedules do not panic and cannot yield a completed Anthropic response. |
| Cancellation before FSE | Repeated cancellation leaves first-semantic delivery uncommitted, forbids transparent retry, rejects later producer delivery as `Cancelled/Request`, and makes the consumer finish without normal completion. |
| Cancellation after FSE | Once downstream marks the first semantic event delivered, repeated cancellation keeps retry forbidden and rejects later producer delivery/consumer receive with the same request-owned cancellation outcome. |

## Failure semantics

| Condition | Result |
|---|---|
| Corpus expectation is not met | The fixed test fails by case identifier only; it does not print the input body. |
| Generated protocol property panics | The fixed-seed suite fails with a static no-input diagnostic. |
| Generated malformed Tool sequence could complete | The suite fails; a partial Tool must never be rendered as a successful Anthropic termination. |
| Cancellation race violates its monotonic gate | The suite fails; no retry, later send, or normal terminal result is accepted after cancellation. |
| Test dependency resolution changes | The local full dependency/RustSec gate fails closed; no test is silently skipped. |

## Corresponding tests

- `fixed_adversarial_request_corpus_is_total_and_classified`
- `fixed_seed_unknown_fields_are_retained_without_panic`
- `truncated_tool_is_rejected_without_a_successful_anthropic_termination`
- `fixed_seed_malformed_tool_streams_never_panic_or_complete`
- `fixed_seed_cancellation_schedules_preserve_the_delivery_boundary`
