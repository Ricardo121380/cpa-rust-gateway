# BC-PROTOCOL-017: CPA request controls and model names

| Field | Value |
|---|---|
| Task | #69 / OPT-02, P0 then P1 |
| Decision | [ADR-0130](../adr/ADR-0130-cpa-request-controls-model-names.md) |
| Fixed source and behavior | [Control/name matrix](../support/cpa-request-controls-model-names.md) |

## Boundary

Source decoding retains signed Messages budgets >=-1 and rejects malformed types/duplicates.
ADR-0126 effort extraction remains the body precedence. Compatible projection normalizes controls
against the selected actual upstream model using frozen Claude/Codex metadata. Metadata never
adds a capability, route grant or credential entitlement. Unknown/custom models retain explicit
fallback plus existing adapter guards. Only adapters with a documented target tier mapping emit
that field. Explicit lossless behavior remains separate; native valid JSON remains preserved.

Explicit prefix input materializes an ordinary version-scoped Alias. Complete exact names win
before suffix parsing, including names with parentheses and names forbidden to this Key. Valid
suffix controls override valid body thinking, reuse the same normalizer and never create an
upstream model. Malformed losing controls still reject. Authorization, model listing, effective
models, Explain, publish/rollback and count resolution use the same stored graph/pinned Snapshot.

Inference native overlays touch only thinking/effort JSON. Request metadata diagnostics retain
the original body. Optional canonical name intent is absent from old serialized payloads and
retained for new owned continuations. No migration or historical rewrite is performed. Count
uses its existing algorithm/transport and never creates inference Usage, ledger or quota history.

## Evidence boundary

Actual public JSON/SSE receiver tests, management HTTP validation and Snapshot publish/rollback
are local controlled evidence. Full and reviews must bind to this ticket's actual source and
lock, not historical branch results. Actual #62/#64 clients, fixed service, real Provider, remote
CI, release and deployment are independently qualified. Cost remains unknown/unadmitted.

## Corresponding tests

- `cpa_69_special_budgets_and_frozen_model_levels`
- `cpa_69_claude_model_ranges_and_adaptive_targets`
- `cpa_69_service_tier_mapping_and_malformed_controls`
- `cpa_69_suffix_controls_keep_lossless_and_capability_guards`
- `cpa_69_model_controls_reach_independent_receiver`
- `cpa_69_responses_controls_reach_claude_receiver`
- `cpa_69_prefixed_suffix_names_keep_permissions_and_native_wire`
- `cpa_69_native_messages_and_chat_suffix_overlay_preserves_options`
- `cpa_69_named_controls_keep_two_tool_cycles_and_followup`
- `cpa_69_count_names_reuse_authorized_resolution_without_inference`
- `cpa_69_management_prefix_alias_is_explicit_validated_and_persisted`
- `cpa_69_prefix_graph_publish_rollback_and_pinned_names`
