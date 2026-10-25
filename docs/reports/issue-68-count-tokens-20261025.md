# #68 Messages count_tokens 本地实施记录

日期：2026-10-25（Asia/Shanghai）。工单为
[#68](https://github.com/Ricardo121380/cpa-rust-gateway/issues/68)，读取时 OPEN、无评论、无原生依赖。
本轮只授权本地实现、验证、代码审查和当前分支提交。

**状态：本票实现与本地行为验收通过；项目门禁 42 PASS / 2 FAIL，供应链仍被继承漏洞阻断。**

## 源码与隔离

- 工作区：`worktrees/cpar-68-count-tokens-20261025`；分支 `codex/68-count-tokens`。
- 共同基线：`444558a8de930f5f1febd5a40ec5bdbbc30a15fb`；进入时根目录、分支、HEAD 匹配且干净。
- 独立 `target`、`output/issue-68`；`CARGO_BUILD_JOBS=2`，测试 `TMPDIR` 位于本票 output。
- 所有计数 receiver 和公共测试监听均动态分配 loopback 端口，独立 SQLite 与合成凭据。
- 没有重建/重置 worktree，没有读取真实凭据、连接共享数据库或调用真实 Provider。

## 实施

`InputTokenCount` 区分 ProviderReported、ExactLocal 和 Estimated，旧精确 capability 保持其类型契约。
HTTP 传递授权快照、Alias/原始模型约束、认证身份与重复检查后的输入；Router 使用既有候选/
凭据选择器；生产 RuntimeFactory 安装真实 count executor。适用第一方调用 Anthropic count API，
其他支持路径使用 CPA O200kBase 估算。计数不进入 inference orchestrator，不写推理事件/Usage/ledger。

具体输入、渠道、故障、资源边界见[支持说明](../support/messages-count-tokens.md)，
测试映射见[BC-PROTOCOL-003](../contracts/BC-PROTOCOL-003-exact-token-count-capability.md)。
固定 CPA 来源与 MIT 署名保存在 [THIRD_PARTY_NOTICES](../../THIRD_PARTY_NOTICES.md)。

## 算法对照

从固定 CPA `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683` 的
`helps/claude_input_tokens.go` 提取计数收集函数，保留函数原文，使用独立 Go 模块和
`tiktoken-go/tokenizer v0.8.1` 执行 8 个合成样例。实际结果写入
[固定夹具](../../tests/fixtures/count-tokens/cpa-v8.json)。临时 oracle、Go 模块锁定和运行日志
保留在本票 `output/issue-68/go-oracle` 与 `logs/go-oracle.log`。
该比较只验证冻结算法样例一致，不是实际模型准确率或通用误差界。

## 验收与验证

Rust 受测提交：`1386e428725df3c8f736c3c1a4ab858c7b995a2c`。后续 `e6004ea6` 仅为
crate 边界清单登记已有 tokenizer 依赖，未修改 Rust 代码、测试或 Cargo 锁文件；最后补充本记录。

| #68 条件 | 本地结果与证据 |
|---|---|
| 三类来源、契约及 exact 保持 | PASS；显式来源 enum；无估算转 exact 或隐式来源转换；旧 exact capability/编码接口保留 |
| 生产 HTTP / 模型 / Alias / Router / executor | PASS；真实 RuntimeFactory 装配；API key / OAuth × native / 自定义源；公共名、Alias、原始模型 |
| 文本、system、工具与历史；不支持内容 | PASS；8 个固定 CPA 样例；图像/未知内容明确拒绝；工具结果畸形容器负例 |
| 标准响应和方法诊断 | PASS；成功 JSON 仅 input_tokens；固定非秘密方法头；8/8 算法对照，不声称模型误差上限 |
| 鉴权、模型、输入限制、上游故障 | PASS；坏认证、不可见模型、重复字段、超限、stream、畸形工具结果；上游 500/429/坏计数不回退、不补零 |
| 独立 receiver / 无推理 Usage 或 ledger | PASS；每种 native 凭据 11 次计数接收，估算 0 次上游接收；所有接收路径均为 count；实际 event writer drain 和 billing worker 检查均为空 |
| 回归、取消与精确能力 | PASS（本地）；陈旧快照拒绝、取消释放容量；工作区完整 Rust 测试通过；实际客户端仍由 #64 / 渠道票复验 |

完整门禁调用使用本票 target/TMPDIR 与 `CARGO_BUILD_JOBS=2`。`full-check-final.md` 和
`logs/full-final.log` 保留原始执行：前 36 项通过，第 37 项 Crate boundaries 因未登记新依赖失败。
其通过项目包括：

- Clippy：全部 targets / features，`-D warnings`。
- `cargo test --locked --workspace --all-features`：**1681 passed、0 failed、21 ignored**，含 doc tests；
  gateway 主组 **323 passed、0 failed、5 ignored**。忽略用例依照既有真实 Provider、实际客户端、
  私有捕获或专用 soak 门槛，不借本票启动。
- P12 serve envelope：PASS。
- Agent multi-turn and stream regression：PASS；真实 gateway + 自有 loopback TLS mock，真实 Provider 调用为 0。
- Source policy、secret scanner 自测及其他脚本检查：PASS。

边界修复只在 `provider-anthropic-compatible` 增加 `tiktoken-rs` 精确条目及 ADR-0127 理由；
其余 crate 边界与许可证/审计阈值均不变。修复后单独运行边界检查、`check.sh docs` 和
`check.sh supply-chain` 补齐余下检查；后者在 Dependency policy 停下后，又单独运行 cargo audit。
最终 44 项均执行，**42 PASS / 2 FAIL**；逐项汇总为本票 `output/issue-68/gate-coverage.md`。
Crate boundaries、docs（包括全量秘密扫描）、质量工具版本已通过。
**原始 Full 退出结果保留为 FAIL，不改写成一次 Full 全通过**。无 Rust 行为改动，因此不重复
已经通过的 1681 项 Rust 测试。

供应链的两项 FAIL 均为 `crypto-bigint 0.5.5` 命中 `RUSTSEC-2026-0387`（constant-time 运算的
timing variability）。`cargo deny` 的 bans/licenses/sources 已通过；advisories 失败；
单独 `cargo audit` 同样只报告此项。公告指向 [RustCrypto v0.7.6 修复](https://github.com/RustCrypto/crypto-bigint/pull/1365)。
本轮没有评估实际可利用性或宣布已有凭据泄露。

同一 advisory 数据库对共同基线锁文件的 `cargo audit --no-fetch --file ...`
复现同一漏洞，退出码同为 1。`crypto-bigint`、`elliptic-curve`、`ecdsa`、`p256` 的完整锁文件
条目逐项相同；路径为既有 `provider-grok → p256 → elliptic-curve → crypto-bigint`，与新增
估算 tokenizer 无关。`dependency-delta.json` 记录新增六个包且无删除；没有升级旧包。
原始证据为 `logs/supply-chain-final.log`、`logs/audit-final.log`、`logs/audit-baseline.log`。

该阻断未解决：需要单独评估 Grok 凭据加密依赖链升级与回归。本票未放宽 deny/audit 规则，
未添加 advisory ignore，也不把供应链状态标为通过。原有重复版本警告保留；新增依赖没有
引入已存在包的第二版本。

早期编译、Clippy 和夹具失败日志均保留：移除未声明 tracing 日志调用、补安全错误 helper；
禁用唯一候选应被配置发布拒绝、priority 必须非负；审查补丁切换期间的编码函数导出编译失败；
Clippy 的 Debug/doc/import/函数长度/字符串格式问题。修复保持原验收条件，没有放宽断言或依赖政策。

## Standards

固定评审点为共同基线；首轮双轴审查对象为 `1020165b77cb38fc9ff3688888a09605b85e4cb4`。
规范轴发现两项：

1. P2：旧 exact capability 既可来自上游，也可来自已证明的本地 tokenizer；不能统一转换为
   ProviderReported。已移除隐式转换，保留旧精确编码接口，并要求 Router 适配器构造者显式传入
   `ExactTokenCountSource::Upstream` 或 `ProvenLocal`。修复提交 `ddf93bdf`。
2. P3（判断性建议）：计数与既有 inference orchestrator 有相似的 Provider-scoped 选择准备
   流程。本票继续调用同一 scheduler/explain/lease 原语；保留这段边界装配以避免扩展推理重构。
   未来选择策略改变时需同步核对两处，作为已知维护负担保留。

## Spec

规格轴发现一项 P2：工具结果解析会接受嵌套数组、数组内裸字符串和裸对象，违反畸形输入失败
要求。`ddf93bdf` 分开容器与内容块解析，只接字符串或单层 text 对象数组；补三项公开 HTTP
负例，共用 receiver 接收数、无推理和零账本断言。没有发现额外范围扩张。

独立复核确认两处行为修复；复核另发现移除 From 后遗留的旧测试断言，已在 `8a2fa5f0` 删除，
并以显式来源测试替代其语义。`1386e42` 仅修正 Clippy 测试字符串格式。
首轮总数：Standards 2（最高 P2，已修复；剩余 P3 建议已评估保留），Spec 1（P2，已修复）。
后续编译和行为验证以完整门禁记录为准。

## 共享接口与集成

- Core 新增 `InputTokenCount`；Router `CountTokensExecutor` 返回该类型，handoff 增加
  精确 Snapshot、raw model constraint 和原始 Messages input。旧 exact Provider capability 未改。
- `encode_count_tokens` 保留原 ExactInputTokenCount 签名；新增 `encode_input_token_count` 处理
  来源计数。旧 Router capability 适配器构造时必须指定 `ExactTokenCountSource`，不猜测来源。
- 公开成功体不变，新增固定可选诊断 header；生产 reload 装配 count executor。
- 固定新增 `tiktoken-rs 0.12.0` 及其依赖；没有升级已有依赖。
  `scripts/check-crate-boundaries.rb` 同步记录该 Provider-only 依赖边，不改动其他 crate 的清单。
- 无 SQLite migration、ADR 新编号、管理路由或 management OpenAPI 变化。
- BC-PROTOCOL-003、ADR-0127 实施注记、ADR 索引、traceability 和当前路线图有必要增量。
- #72/#74 独立工作区未修改；尚未做三分支集成或远端 CI。

实际客户端、固定 Build 服务、真实 Provider、软件发布/部署均 NOT_RUN。#64 及相关渠道票的
客户端资格不由本票替代；无 push、合并、远端评论、关闭工单或部署。
