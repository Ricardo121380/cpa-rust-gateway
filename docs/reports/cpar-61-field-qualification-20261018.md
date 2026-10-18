# #61 Build field qualification

Date: 2026-10-18 (Asia/Shanghai). Admission result: **BLOCKED**. No additional field is
enabled by this record. The review covers the fixed production `grok.build.responses` channel,
`https://cli-chat-proxy.grok.com/v1/responses`, standard Responses JSON/SSE, profile 1.0.50.
It does not extend Official/Console, Messages, native-v1, or a real model alias.

## Evidence and its limits

The existing authorized, value-free inventory in
[the offline report](cpar-native-chat-offline-20261017.md) identifies actual JSON summary,
standard counters, two extra source/tool counters, a nonzero `cost_in_usd_ticks`, and an
unexplained Usage object. This review does not open the private capture, disclose its raw values,
identify the anonymous object by guessing its key, or create a new Provider request.

Read-only checks of the first-party public `xai-org/grok-build` repository resolved `main` to
`2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`. The frozen files establish these narrower facts:

| Source | Applicable fact | Limit |
| --- | --- | --- |
| [sampler/client.rs, lines 124–165](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/client.rs#L124) | The SSE completed/incomplete decoder extracts `/response/usage/cost_in_usd_ticks` through `as_i64`, retains positive ticks separately, and handles context totals separately | Explicitly an SSE terminal override; it does not establish non-streaming JSON extraction or every deployed Build profile |
| [sampler/stream/responses.rs, lines 615–649](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/stream/responses.rs#L615) | Source token counters and cost are handled separately from conversion of response items | This client behavior supports semantic separation in that path; it is not a CPAR persistence receipt or a server contract |
| [sampling-types/conversation.rs, lines 829–863](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation.rs#L829) | Defines server cost in ticks, 1 USD = 10^10 ticks; nonpositive/unreported inputs normalize to absent in that client | CPAR must still retain original missing/null/zero and never turn zero/unknown into a confirmed free bill |
| [sampler/client.rs, lines 1276–1385](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/client.rs#L1276) | Non-streaming JSON is decoded into the typed Responses result without the SSE cost override | Omission by a typed client is not authority to omit an unknown field in CPAR |
| [config/endpoints.rs, lines 133–143](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-config/src/endpoints.rs#L133) | The proxy is used for OAuth/session inference; configuration can override the inference host | Prevents treating every configurable sampler result as an assertion for every fixed Build deployment |

The [source manifest](assets/cpar-61-20261018/public-source-manifest.json) records frozen URLs,
retrieval and exact downloaded file hashes. Public API documentation for a same-named xAI field
is not used as Build equivalence evidence.

## Field decisions

`JSON` locations below are relative to the response object. In SSE response snapshots, prefix
them with `$.response`; item/part events additionally use their explicit item/index correlation.

| Location / source / shape | Existing standard mapping or separation assessment | Decision |
| --- | --- | --- |
| `$.id`, `$.model`, `$.status`; Build JSON/SSE | Response identity/lifecycle and #60's exact public/upstream/native relation; optional `object` must be the fixed `response` discriminator | Consumed semantics, never separable metadata |
| `$.output[*].{id,type,status,role,call_id,name,arguments}` | Native item identity, ordering, tool name/ID, complete object arguments and result correlation | Preserve; unsupported kinds, status shapes and conflicts fail |
| `$.output[*].summary[*].{type,text}` | Actual Build summary inventory; indexed Responses summary is distinct from missing reasoning body | Preserve natively; Messages' exception is inapplicable |
| `$.output[*].content[*]`, reviewed phase/annotations/logprobs | Existing closed item/part mapping preserves content, hierarchy and reviewed null/empty forms; richer fixtures are synthetic | Existing representation only; no new field qualification from fixtures |
| Unknown root/item/part fields, including root `metadata`, timestamps and echoed controls without a complete mapping | Neither storage nor a familiar field name proves independence from request control, history, model or terminal semantics | BLOCKED; root now fails instead of silently discarding these fields |
| `$.usage.{input_tokens,output_tokens}` and detail cached/reasoning counters | Exact known u64 source counters, nullable/absent evidence and existing accounting/provenance | Preserve; never separate or round |
| `$.usage.total_tokens` | Checked input+output confirmation, not an invented operand or substitute context budget | Existing mapping; missing operand or arithmetic conflict remains unsupported |
| Actual `$.usage.cost_in_usd_ticks`; corresponding SSE terminal path | First-party Build code supplies a concrete SSE meaning for a positive i64 scalar. Cost is separately handled from items and source counters; it cannot authorize a CPAR settlement amount | **Candidate: SSE semantic evidence found; JSON/profile applicability incomplete; NOT ADMITTED** |
| Actual `$.usage.num_sources_used`, `num_server_side_tools_used` | Existing inventory includes explicit zero, but no scoped Build semantic/shape contract was found in the inspected sources | BLOCKED; zero and name do not grant qualification |
| Actual anonymous Usage object (only its existing key hash is public) | Meaning and child-counter accounting remain unknown; not inferred to be any public source field | BLOCKED |
| Source-only `usage.context_details` | The public client uses it for live-context totals, distinct from cumulative source counters | Not an ordinary cost annotation; no qualification or claim that it is the historical anonymous object |
| Real native model alias | Stock proof set remains empty; no newly approved authority | UNKNOWN/BLOCKED independently of every field above |

## Why no additional field was admitted

The cost candidate now has better SSE evidence than the earlier generic public-API reference.
It is not accurate to say its meaning is wholly unexplained. However, this material does not
complete the requested common JSON/SSE qualification for the actual production profile. In
particular the non-streaming typed client does not prove its extra field contract. This run
does not infer that proof from matching names or make a synthetic allowance to complete AC7.

For any future qualification, the review must explain absence/null/zero, exact numeric bounds,
JSON/SSE phase/profile applicability, and why removing that one field changes none of answer,
summary, tool execution/results, order, terminal status, request controls, standard Usage or model
identity. The public source's separate cost handling is useful evidence, not the remaining proof.

Minimum additional input is a first-party Build schema/source or an explicitly approved
channel contract tying that concrete field and shape to the actual JSON **and** SSE profile.
No fresh inference budget is necessary for the current preparation. A future evidence decision
must remain field-specific; it cannot authorize the other counters or anonymous object.

## Retention and conflict contract

There is no new success-path metadata store or reader in this candidate. Existing
[BC-OBS-006](../contracts/BC-OBS-006-native-usage-evidence.md) continues to retain only the
already received, validated complete Usage object on the strict failure path, with exact
Request/Attempt/owner, raw numeric lexemes, AEAD, fixed TTL and commit confirmation. This run
also exercises that contract for a native SSE failure terminal. Identical replay stays
idempotent; changed bytes/owner, missing keys, timeout and authentication conflicts cannot claim
retention. Root/item/part retention is not inferred from that Usage capability.

If a future admitted field is confined to Usage and the existing versioned payload can express
its required completeness, schema 35 may be reusable; that still requires success-before-bytes /
success-before-terminal confirmation tests. Fields outside Usage need an explicit versioned
retention/reader/compatibility decision first. No migration or binary-only rollback qualification
is implied by this report.
