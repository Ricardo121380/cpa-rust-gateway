# A-01 RED → GREEN 执行证据

日期：2026-10-07，Asia/Shanghai。固定起点 `602874b9ed9943c146c8d12f77ab1cc1adbfa167`；修复源码 `42bc473f2eba8c34484b54aaf09fff455aa13c9c`。

## 源码身份

定向测试先在开发工作区执行；冻结后核对 5 个受影响 Rust 文件 SHA256 与修复提交一致。权威 Full 从该提交的 clean detached checkout 执行，前后均 clean，并再次运行新增 callback 及所有相邻 workspace 回归。它仅复用 `CARGO_TARGET_DIR` 编译缓存，没有把主工作区未跟踪源码纳入验证。完整文件 hash 与日志 hash 见 [verification.json](verification.json)。

RED 使用旧产品提交加第一条新回归，退出 101，实际 202／预期 409；原始状态失败回执见 [red-foreign.md](red-foreign.md)。编译错误和错误 shape／Clippy 修正不作为 RED，也不冒充最终通过。

## 实际命令与结果

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| Callback regression | `cargo test --locked -p gateway-http-actix --test managed_resource_inventory legacy_codex_callback_` | 5/5，38 组合 PASS |
| Management HTTP inventory | `cargo test --locked -p gateway-http-actix --test managed_resource_inventory` | 49/49 PASS |
| Strict targeted Clippy | `cargo clippy --locked -p gateway-control -p gateway-store -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| Adjacent regression | `cargo test --locked -p provider-openai-compatible runtime_credential::tests::` | 19/19 PASS，exit 0 |
| Adjacent regression | `cargo test --locked -p gateway-store credential_refresh::tests::` | 6/6 PASS，exit 0 |
| Adjacent regression | `cargo test --locked -p gateway credential_refresh::ordinary::tests::` | 7/7 PASS，exit 0 |
| Adjacent regression | `cargo test --locked -p gateway-control oauth_persistence` | 2/2 PASS，exit 0 |
| Adjacent regression | `cargo test --locked -p gateway-http-actix --lib management_resources::codex_enrollment::tests::` | 2/2 PASS，exit 0 |
| Fixed-source Full | `./scripts/check.sh full` | 45/45 PASS，exit 0，293.398 秒 |
| Docs closeout | `./scripts/check.sh docs` | 7/7 PASS，exit 0，36.427 秒 |

定向检查还执行 `cargo fmt --all -- --check`、`git diff --check`、源码策略、crate boundaries、contract tests、秘密扫描自测及 `secret-scan.sh --all`，结果通过。冻结提交 Full 包含上述仓库门禁及 workspace Clippy、workspace tests、完整 Fast／供应链检查；[Full 45 步](full-check.md) 与 [精确运行回执](full-receipt.json) 是不可变提交上的执行证据。

## 不可变 Full 中的 callback 回归

```text
test legacy_codex_callback_rejects_changed_draft_without_partial_credential_write ... ok
test legacy_codex_callback_preserves_concurrent_winner_after_token_exchange ... ok
test legacy_codex_callback_rejects_different_identity_without_changing_account ... ok
test legacy_codex_callback_updates_same_identity_and_preserves_disabled_intent ... ok
test legacy_codex_callback_rejects_unverifiable_identity_without_changing_account ... ok
test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.79s
```

Full 的 Rust tests 步骤合计 **1458 passed，0 failed，12 ignored**，来自 124 个 harness 结果。新增 5 个 callback 测试全部运行，没有 ignored。管理 HTTP 49/49 包含首次 Codex 授权及必要相邻行为。

## 原有未默认运行的测试

以下是该次 Full 实际记录的 ignored 项，保留其原触发条件；不把它们写成已完成的真实渠道、特定耗时、专用客户端或生产证据。

```text
test runtime::tests::m1_mock_request_confirmation_timing ... ignored, isolated M1 timing run, five samples per mode; no provider network
test authorized_loopback_soak_writes_a_value_free_receipt ... ignored, P11-04 runs only through scripts/run-p11-04-soak.sh
test authorized_real_endpoints_validate_non_streaming_and_sse_paths ... ignored, requires explicit P3_10_* real-test configuration and user authorization
test authorized_single_probe_uses_one_target_one_mode_and_one_send ... ignored, requires dedicated P4_00_DIAGNOSTIC_* authorization and one-request configuration
test local_claude_code_bare_covers_normal_tool_parallel_tool_and_plan_mode ... ignored, requires P5_07_CLAUDE_CODE_BIN and launches a loopback-only local Claude Code --bare client
test event_store::durability_tests::crash_child ... ignored, subprocess crash fixture
test event_store::durability_tests::historical_production_projection_survives_migration_and_partial_read ... ignored, requires CPAR_M1_HISTORY_PROJECTION owner-readable input
test random_seed_tool_chunk_interleavings_are_replayable ... ignored, run scripts/run-p1-09-property.sh to generate or replay P1_09_SEED
test authorized_build_probe_uses_one_target_one_mode_and_one_send ... ignored, requires dedicated P6_03_* authorization, one target, one mode, and one-request configuration
test official_cli_cache_preflight_builds_without_network ... ignored, requires explicit P6_03_LOCAL_CACHE_PREFLIGHT and one official-CLI cache path; no network
test authorized_official_probe_uses_one_target_one_mode_and_one_send ... ignored, requires dedicated P8_07_* authorization, one Official API key, one target, one mode, and one-request configuration
test authorized_web_canary_uses_one_fixed_target_and_one_send ... ignored, requires P9_09_* authorization, one temporary grok2api Web SSO token, one target label, and one-request configuration
```

## 供应链告警与边界

`cargo deny` 和 `cargo audit` 均 PASS；该次 deny 记录 14 条 duplicate-version 告警，如下。Cargo manifests／lockfile 未修改；本轮不升级依赖，后续依赖任务继续按项目质量门禁评估。

```text
warning[duplicate]: found 2 duplicate entries for crate 'block-buffer'
warning[duplicate]: found 2 duplicate entries for crate 'chacha20'
warning[duplicate]: found 2 duplicate entries for crate 'const-oid'
warning[duplicate]: found 2 duplicate entries for crate 'cpufeatures'
warning[duplicate]: found 2 duplicate entries for crate 'crypto-common'
warning[duplicate]: found 2 duplicate entries for crate 'digest'
warning[duplicate]: found 3 duplicate entries for crate 'getrandom'
warning[duplicate]: found 2 duplicate entries for crate 'http'
warning[duplicate]: found 2 duplicate entries for crate 'rand_core'
warning[duplicate]: found 2 duplicate entries for crate 'shlex'
warning[duplicate]: found 2 duplicate entries for crate 'socket2'
warning[duplicate]: found 2 duplicate entries for crate 'syn'
warning[duplicate]: found 2 duplicate entries for crate 'thiserror'
warning[duplicate]: found 2 duplicate entries for crate 'thiserror-impl'
```

外部 token exchange 为合成替身；注册的 Actix HTTP、管理鉴权、磁盘 SQLite、AEAD 和最终事务 CAS 均真实执行。没有 Provider/OAuth 外发、收费请求、生产连接、部署、远程 Gate、签名或 GitHub 写入。Full 中带 production/live 名称的 harness／oracle 仅为受控本地测试。

本地原始日志保留在 `output/cpar-a01-20261007/`，未将 token/code/key/ciphertext/解密材料复制到报告。敏感一致性在进程内使用 boolean 断言；发布状态仍 BLOCKED。

文档 Gate 在源码 HEAD `42bc473f` 上检查本轮 staged 文档收口，2026-10-07 18:02:32–18:03:08 +08:00；主工作区原有用户修改保留，未进入 index。终态回执与日志 hash 加入 [verification.json](verification.json)；结果补记为纯元数据，提交前另检查 JSON、引用及 whitespace。原始文档 Gate 回执保留在本地 `output/cpar-a01-20261007/docs-check.md` 和 `docs-run-receipt.json`。
