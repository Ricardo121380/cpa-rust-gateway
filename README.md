# CPA-inspired Rust Gateway

本目录用于规划一个基于 CPA 功能经验、但从零实现的高性能 AI 网关。

规划阶段已经完成并锁定开发计划 `v1.0`，P0 工程基线与 21-package Rust Workspace 已建立，但尚未实现推理业务。HTTP 技术路线确定为 `Rust + Actix Web`；框架只负责接入层，协议、路由、凭据和 Provider 核心不依赖 Actix 类型。

第一阶段渠道方向已经锁定为 `Grok Provider Family + Kiro + OpenAI-compatible`。Grok 不作为单一 Provider：官方 API、Build OAuth 与 Web/Console 使用独立 Adapter、凭据池、Quota 和连续性状态；Kiro 同时支持 CLI/IDE 端点，行为以服务器定制 Kiro-RS 为参考。

上游聚合也已锁定为核心能力：项目将接入多个第三方中转站或本机网关，把 `Upstream`、协议 `Endpoint`、上游凭据、公开模型与客户端 Key 分层建模。客户端最终只使用本项目自己的 Base URL、API Key 和稳定模型名；同一公开模型可显式轮询多个经过能力验证的上游 Candidate。

## 当前状态

- 阶段：`P1 - Canonical Core + Mock 垂直链路`（G0 已通过；`P1-01` 尚未开始）
- 执行计划：`v1.0`，状态 `Locked for execution`
- CPA 参考版本：`router-for-me/CLIProxyAPI v7.2.80`
- Release 1：核心范围、技术基线、阶段顺序和门禁已冻结
- Rust Workspace：21-package 骨架已创建并通过 P0-03 验证
- 服务器部署：尚未开始

## 文档索引

1. [参考基线与 CPA 架构](docs/00-reference-baseline.md)
2. [功能筛选矩阵](docs/01-feature-selection-matrix.md)
3. [关键行为与兼容性契约](docs/02-behavior-contracts.md)
4. [目标 Rust 架构草案](docs/03-target-architecture-draft.md)
5. [Grok 与 Kiro 渠道参考实现分析](docs/04-channel-reference-analysis.md)
6. [上游聚合、统一模型与自有 API 设计](docs/05-upstream-aggregation-design.md)
7. [Rust AI Gateway 详细开发计划（后续唯一执行基线）](docs/06-development-plan.md)
8. [需求追踪索引](docs/traceability.md)
9. [架构决策记录（ADR）](docs/adr/README.md)
10. [可执行行为契约](docs/contracts/README.md)
11. [阶段与任务验证报告](docs/reports/README.md)
12. [Crate 依赖边界](docs/crate-boundaries.md)
13. [质量与供应链门禁](docs/quality-gates.md)

## 本地检查

```bash
./scripts/check.sh fast
./scripts/check.sh full
```

`fast` 执行格式、Clippy、测试、源码、架构、文档、Workflow 和 Secret 检查；`full` 在此基础上执行固定版本的 `cargo-deny` 与 `cargo-audit`。首次运行完整门禁前执行 `./scripts/install-quality-tools.sh`。

## 决策标记

| 标记 | 含义 |
|---|---|
| `Keep` | 第一版必须实现，并尽量保持 CPA 兼容行为 |
| `Later` | 架构预留，第一版后实现 |
| `Drop` | 明确不进入新项目 |
| `Replace` | 需求保留，但不沿用 CPA 的设计或行为 |
| `New` | CPA 本体没有，作为新项目新增能力 |
| `待定` | 用户尚未做最终选择 |

## 执行流程

1. 后续开发严格以 [详细开发计划](docs/06-development-plan.md) 中的 Phase、Task ID、依赖、交付物和 Gate 为准。
2. 全计划同时最多一个 Task 处于 `IN_PROGRESS`，未通过当前 Gate 不进入下一 Phase。
3. 功能矩阵中仍为 `待定` 的项目不属于当前 Release 1，不得顺手实现。
4. 任何范围、顺序、公开行为或门槛变化都必须创建 Change Request，并取得用户明确批准。
5. 下一步从 `P1-01` 开始；当前保持 `PENDING`，本次 G0 收尾不实现任何 P1 功能，也不修改服务器。

## 实施原则

- CPA 是行为参考和对照实现，不是新项目的代码骨架。
- 不按 Go 包结构逐文件翻译到 Rust。
- 先定义统一内部协议，再实现入口协议和 Provider Adapter。
- Grok Official、Build、Web 只共享明确的领域组件，不共享凭据池或故障状态。
- Kiro 的 Anthropic 转换、Conversation 协议、CLI/IDE Endpoint 和 AWS EventStream 分层实现。
- Provider Adapter 只理解协议；Upstream/Endpoint 描述配置实例，Public Model/Route 描述客户端模型视图，Client Key/Access Group 描述访问边界。
- 一个 Endpoint 只绑定一种上游 API Format；同一中转站的 Responses、Chat、Anthropic 接口建成独立 Endpoint。
- `/v1/models` 与推理路由使用同一个不可变 Route Snapshot，不维护平行的展示模型清单。
- 所有透明重试必须发生在向客户端发送首字节之前。
- 管理、日志和统计不得阻塞推理请求热路径。
- 新旧网关在灰度切换前必须通过差分测试。
