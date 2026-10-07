# B-01 targeted execution

The development runs use the baseline HEAD plus the fix worktree. `development-verification.json` records hashes of all four Rust files. Immutable-commit Full and final SHA binding follow after review.

## narrow-public

Command: `cargo test --locked -p gateway --bin gateway batch_c:: -- --nocapture`. Exit: **0**.

```text
   Compiling gateway-router v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-router)
   Compiling provider-grok v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/provider-grok)
   Compiling gateway-control v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-control)
   Compiling gateway-http-actix v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-http-actix)
   Compiling gateway v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/apps/gateway)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 17.40s
     Running unittests src/main.rs (target/debug/deps/gateway-eafc5f5031908db2)

running 10 tests
test runtime::batch_c::http::responses_history::responses_multipart_history_rejects_invalid_semantics_before_upstream_http ... ok
test runtime::batch_c::grok::web_unrepresentable_histories_tools_and_limits_reject_before_lease_or_dns ... ok
test runtime::batch_c::http::responses_history::responses_json_multipart_output_replays_exactly_through_public_http ... ok
test runtime::batch_c::official_real_factory_retains_tools_controls_usage_and_quota_ownership ... ok
test runtime::batch_c::http::responses_history::responses_sse_multipart_output_replays_exactly_through_public_http ... ok
test runtime::batch_c::http::codex_rejection_never_replays_without_the_clients_output_limit ... ok
test runtime::batch_c::kiro_real_factory_bridges_text_tools_and_effort_without_dropping_limits ... ok
test runtime::batch_c::grok::grok_native_metadata_survives_public_responses_and_rejects_lossy_bridges ... ok
test runtime::batch_c::grok::grok_factories_bridge_three_protocols_and_preserve_tool_rounds ... ok
test runtime::batch_c::http::ordinary_native_families_use_the_public_factory_and_preserve_identity ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 167 filtered out; finished in 4.32s

```

## final-router

Command: `cargo test --locked -p gateway-router --lib protocol_transform::tests::`. Exit: **0**.

```text
   Compiling gateway-router v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-router)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.42s
     Running unittests src/lib.rs (target/debug/deps/gateway_router-409ed1bc931fb5cd)

running 30 tests
test protocol_transform::tests::canonical_and_lossless_bridge_enforce_their_topology ... ok
test protocol_transform::tests::diagnostics_redact_client_values ... ok
test protocol_transform::tests::endpoint_api_formats_are_exact_and_do_not_infer_unknown_protocols ... ok
test protocol_transform::tests::all_three_native_pairs_keep_exact_payload_ownership ... ok
test protocol_transform::tests::a_clean_cross_protocol_bridge_is_admitted ... ok
test protocol_transform::tests::all_nine_protocol_pairs_prepare_typed_requests ... ok
test protocol_transform::tests::canonical_bridge_supports_native_and_cross_protocol_projection ... ok
test protocol_transform::tests::canonical_and_bridge_reject_extensions_and_opaque_content ... ok
test protocol_transform::tests::bridge_rejects_unrepresentable_tool_cache_thinking_and_roles ... ok
test protocol_transform::tests::automatic_tool_choice_is_exact_across_protocols ... ok
test protocol_transform::tests::chat_response_reasoning_is_admitted_without_lowering_capabilities ... ok
test protocol_transform::tests::passthrough_requires_same_protocol_and_an_exact_native_payload ... ok
test protocol_transform::tests::protocol_formats_agree_with_the_shared_api_format_table ... ok
test protocol_transform::tests::ordered_tool_history_is_preserved_when_every_target_can_represent_it ... ok
test protocol_transform::tests::requires_declared_target_capabilities_before_admission ... ok
test protocol_transform::tests::reviewed_registry_publishes_only_the_nine_pairs_with_their_exact_modes ... ok
test protocol_transform::tests::reasoning_only_chat_history_is_native_and_not_silently_bridged ... ok
test protocol_transform::tests::responses_tool_replay_preserves_item_id_without_admitting_unknown_extensions ... ok
test protocol_transform::tests::pi_responses_reasoning_controls_survive_projection_and_encoding ... ok
test protocol_transform::tests::pi_responses_continuation_survives_projection_and_encoding ... ok
test protocol_transform::tests::every_target_builder_accepts_the_router_approved_tool_projection ... ok
test protocol_transform::tests::same_protocol_responses_preserves_typed_cache_controls ... ok
test protocol_transform::tests::same_protocol_canonical_admits_only_valid_target_tool_choice ... ok
test protocol_transform::tests::target_tool_choice_validation_fails_closed ... ok
test protocol_transform::tests::thinking_effort_is_exact_between_openai_protocols_and_not_a_numeric_budget ... ok
test protocol_transform::tests::forced_tool_choice_uses_the_reviewed_cross_protocol_mapping ... ok
test protocol_transform::tests::typed_projection_rejects_missing_invalid_and_colliding_output_limits ... ok
test protocol_transform::tests::responses_reasoning_controls_keep_unknown_values_and_bridges_rejected ... ok
test protocol_transform::tests::responses_assistant_text_replay_preserves_known_metadata ... ok
test protocol_transform::tests::every_positive_output_limit_maps_exactly_across_all_pairs ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 154 filtered out; finished in 0.02s

```

## pending-tool

Command: `cargo test --locked -p gateway-core --lib canonical_request::tests::`. Exit: **0**.

```text
   Compiling gateway-core v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.12s
     Running unittests src/lib.rs (target/debug/deps/gateway_core-20f6f51d8af3c0f2)

running 7 tests
test canonical_request::tests::raw_json_rejects_an_incomplete_value ... ok
test canonical_request::tests::supplied_thinking_requires_a_non_empty_effort ... ok
test canonical_request::tests::known_content_rejects_missing_required_fields ... ok
test canonical_request::tests::reasoning_only_assistant_cannot_overtake_pending_tool_results ... ok
test canonical_request::tests::optional_thinking_and_multiple_tools_preserve_their_structure ... ok
test canonical_request::tests::canonical_request_debug_redacts_client_values ... ok
test canonical_request::tests::canonical_request_round_trips_through_json_and_memory ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 0.00s

```

## reasoning-chat

Command: `cargo test --locked -p protocol-openai-chat --lib reasoning_only_output_can_be_replayed_as_assistant_history`. Exit: **0**.

```text
   Compiling gateway-core v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-core)
   Compiling protocol-openai-chat v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/protocol-openai-chat)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.22s
     Running unittests src/lib.rs (target/debug/deps/protocol_openai_chat-54cf68406b5f461e)

running 1 test
test tests::reasoning_only_output_can_be_replayed_as_assistant_history ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.00s

```

## http-tool-history

Command: `cargo test --locked -p gateway-http-actix --lib three_protocols_reject_orphan_tool_results_before_execution`. Exit: **0**.

```text
   Compiling gateway-router v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-router)
   Compiling gateway-control v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-control)
   Compiling provider-grok v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/provider-grok)
   Compiling gateway-http-actix v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-http-actix)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.94s
     Running unittests src/lib.rs (target/debug/deps/gateway_http_actix-4ee39835b62d5bfe)

running 1 test
test tests::three_protocols_reject_orphan_tool_results_before_execution ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 0.01s

```

## http-item-shape

Command: `cargo test --locked -p gateway-http-actix --lib responses_rejects_non_string_item_type_before_execution`. Exit: **0**.

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.13s
     Running unittests src/lib.rs (target/debug/deps/gateway_http_actix-4ee39835b62d5bfe)

running 1 test
test tests::responses_rejects_non_string_item_type_before_execution ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 0.00s

```

## targeted-clippy

Command: `cargo clippy --locked -p gateway-router -p gateway --all-targets --all-features -- -D warnings`. Exit: **0**.

```text
    Checking gateway-core v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-core)
    Checking gateway-protocol v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-protocol)
    Checking gateway-provider v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-provider)
    Checking gateway-auth v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-auth)
    Checking gateway-observability v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-observability)
    Checking protocol-openai-chat v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/protocol-openai-chat)
    Checking protocol-anthropic v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/protocol-anthropic)
    Checking protocol-openai-responses v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/protocol-openai-responses)
    Checking gateway-stream v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-stream)
    Checking gateway-upstream v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-upstream)
    Checking gateway-store v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-store)
    Checking provider-anthropic-compatible v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/provider-anthropic-compatible)
    Checking gateway-catalog v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-catalog)
    Checking gateway-router v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-router)
    Checking provider-openai-compatible v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/provider-openai-compatible)
    Checking provider-kiro v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/provider-kiro)
    Checking provider-grok v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/provider-grok)
    Checking gateway-control v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-control)
    Checking gateway-http-actix v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-http-actix)
    Checking gateway v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/apps/gateway)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.24s
```
