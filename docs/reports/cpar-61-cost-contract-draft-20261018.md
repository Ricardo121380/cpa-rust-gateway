# #61 cost field channel contract draft

Date: 2026-10-18 (Asia/Shanghai). **PROPOSED / NOT_ADMITTED**.
This is a concrete decision draft, not an implemented allowance or a server schema claim.
The prior [qualification record](cpar-61-field-qualification-20261018.md) and all failures remain.

User decision after reviewing this draft: **keep NOT_ADMITTED and wait for additional first-party
evidence**. The proposed rules below remain unapproved; no success metadata allowance is implemented.
The [decision receipt](assets/cpar-61-continuation-20261018/field-decision.json) records that boundary.

## Verified facts and exact limits

The channel is `grok.build.responses`, fixed URL
`https://cli-chat-proxy.grok.com/v1/responses`, current CPAR production request profile `1.0.50`.
Public JSON currently requests upstream JSON; public SSE requests upstream SSE.
The existing authorized value-free inventory establishes that the actual JSON Usage included
`cost_in_usd_ticks`. No private capture was reopened and no real inference was performed.

The frozen first-party source is `xai-org/grok-build@2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`.
This run independently retrieved and hashed the source slices listed in
[the spotcheck manifest](assets/cpar-61-continuation-20261018/public-source-spotchecks.json).
Three hashes match the previous five-file manifest exactly. Public `SOURCE_REV` is
`559751fdcec02d413e4c57c8832ab275e4f44980`; it is a monorepo revision, not proof of release 1.0.50.

| First-party evidence | Established fact | What it does not establish |
| --- | --- | --- |
| [sampler/client.rs:124](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/client.rs#L124) | Completed/incomplete SSE events extract `/response/usage/cost_in_usd_ticks` with `as_i64`, then retain positive ticks separately | Responses JSON schema, other phases, or deployment version |
| [conversation.rs:829](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation.rs#L829) and [858](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation.rs#L858) | Cost unit is 10^10 ticks/USD; that client treats nonpositive/unreported input as absent and documents REST zero backfill | Authority to discard original zero/null/missing or accept negative values in CPAR |
| [stream/responses.rs:615](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/stream/responses.rs#L615) | Source counters, cost extraction and response-item conversion are separate | CPAR standard semantics or persistence timing without implementation tests |
| [sampler/client.rs:1374](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/client.rs#L1374) | Non-streaming result is deserialized to a typed Response | A typed consumer omitting a field is not field admission |
| [version/src/lib.rs:11](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-version/src/lib.rs#L11) | Release pipeline injects `GROK_VERSION` | A correspondence between this source and the fixed shipping 1.0.50 profile |

A read-only scout additionally searched the frozen public archive's 4,487 regular files for the
exact field name; no production Responses server schema was found. That bounded negative is a
scout result; the main agent spotchecked the decisive source lines rather than repeating its scan.
No generic xAI API schema, Chat Completions field or test generator supplies missing authority.

## Proposed v1 scope and shape

The following is a proposed explicit CPAR channel contract. It requires approval precisely because
common JSON/SSE/profile applicability is not established by the source above.

| Dimension | Proposed rule |
| --- | --- |
| Channel/profile | Only fixed production `grok.build.responses` profile 1.0.50 and standard Responses; other channels/profiles remain strict |
| JSON path | `$.usage.cost_in_usd_ticks` in a validated completed/incomplete response object |
| SSE path | `$.response.usage.cost_in_usd_ticks` only in validated completed/incomplete terminal snapshots; the same shared validator as JSON |
| Presence | Missing stays missing; explicit null stays null in the complete encrypted object; neither creates a cost claim |
| Zero | Literal `0` retained exactly, interpreted only as reported zero/unreported evidence, never a confirmed free charge |
| Positive | A nonnegative integer JSON token in [1, 9223372036854775807], exact signed-64-bit range; 10^10 ticks/USD is diagnostic context only |
| Rejection | Negative including `-0`, fraction, exponent, string, bool, array/object and overflow remain unsupported; no float conversion, rounding, saturation or truncation |
| Other locations/fields | No root/item/part allowance, no other extra counter or anonymous object; known counters/detail/total arithmetic and all unknown/conflict checks remain |
| Failed response | Existing #59 retention and failed SSE semantics remain independently applicable; no failed response can become success |

These restrictions are deliberately narrower than everything a client parser might ignore.
Missing/null/zero are three distinct evidence states. Positive ticks remain non-settlement
evidence: no conversion into `cost_microunits`, billing price, quota or inferred Provider charge.

## Semantic separation obligation

The only proposed standard projection change is to omit this one qualified extra scalar while
retaining the entire received Usage object privately. Its applicability must be checked at the
Build boundary and carried with its evidence; a general projector must not trust a field name
or a test-only allowlist. The source's independent cost handling supports this separation, but
the common JSON/profile application is the explicit assumption requiring approval.

The implementation and public tests must prove unchanged answer and native summary, item/part
identity and order, tool name/call ID/full arguments/result association, intermediate and later
questions, terminal/error behavior, request controls, known Usage and three-part model identity.
No reasoning is reclassified, no Messages summary exception or native-v1 is introduced, and
unexplained counters/objects and real aliases retain their distinct blockers.

## Complete retention, readback and conflicts

First verify reuse of the current Usage observer and schema-35 `complete_usage_object` payload:
exact received raw Usage slice, source `build_json` or `build_sse_terminal`, exact Request/Attempt
and durable owner, one bounded private companion, existing AEAD/keys/AAD, fixed TTL and
owner-scoped authorized reader. The whole Usage slice retains original whitespace, member order,
numeric lexemes and known/unknown presence; this does not promise the full response envelope.
The proposal requires no root/item/part retention format and does not by itself require a migration.

Successful JSON bytes must wait for both complete evidence commit confirmation and standard
projection validation. Successful SSE terminal must wait for the same confirmation; queue admission
is insufficient. Missing keys, encryption/write failure, confirmation timeout, changed raw bytes,
foreign lineage or authentication conflict cannot succeed. After committed headers, emit at most
one legal failure and no later success. Preserve verified counters non-additively and one ledger
row per Attempt; a timeout remains UNKNOWN until authorized readback resolves it.
No extra read, resend, candidate/account/model switch or inventory backfill may obtain evidence.
Cancellation releases the upstream and lease even when local retention is pending.

## Minimum decision

Either obtain scoped first-party JSON/SSE/profile authority, or explicitly approve **this v1 CPAR
channel contract**, including its bounded presence/shape policy and the assumption that the actual
JSON field under fixed profile 1.0.50 has the same diagnostic unit and independent meaning as the
SSE candidate. Approval of ADR-0107's general direction does not approve this draft.

Until that decision, admission stays **NOT_ADMITTED**, success metadata implementation/fault
qualification is **BLOCKED**, and independent production Build cancellation/failure work continues.
