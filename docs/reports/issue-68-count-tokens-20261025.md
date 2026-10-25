# #68 Messages count_tokens 本地实施记录

日期：2026-10-25（Asia/Shanghai）。工单为
[#68](https://github.com/Ricardo121380/cpa-rust-gateway/issues/68)，读取时 OPEN、无评论、无原生依赖。
本轮只授权本地实现、验证、代码审查和当前分支提交。

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

实施中的编译失败原始日志保留在本票 output：首轮 HTTP 日志引用缺少 tracing 依赖，
已改为固定诊断响应头；后续补齐 count 模块自己的安全 transport error helper。
最终受测源码、逐项验收、完整门禁与双轴审查将在本记录收口时登记。

## 共享接口与集成

- Core 新增 `InputTokenCount`；Router `CountTokensExecutor` 返回该类型，handoff 增加
  精确 Snapshot、raw model constraint 和原始 Messages input。旧 exact Provider capability 未改。
- `encode_count_tokens` 保留原 ExactInputTokenCount 签名；新增 `encode_input_token_count` 处理
  来源计数。旧 Router capability 适配器构造时必须指定 `ExactTokenCountSource`，不猜测来源。
- 公开成功体不变，新增固定可选诊断 header；生产 reload 装配 count executor。
- 固定新增 `tiktoken-rs 0.12.0` 及其依赖；没有升级已有依赖。
- 无 SQLite migration、ADR 新编号、管理路由或 management OpenAPI 变化。
- BC-PROTOCOL-003、ADR-0127 实施注记、ADR 索引、traceability 和当前路线图有必要增量。
- #72/#74 独立工作区未修改；尚未做三分支集成或远端 CI。

实际客户端、固定 Build 服务、真实 Provider、软件发布/部署均 NOT_RUN。#64 及相关渠道票的
客户端资格不由本票替代；无 push、合并、远端评论、关闭工单或部署。
