# CPAR 后端架构与 CPA 源码对照

当前方向：把 CPA/CLIProxyAPI 的网关能力用 Rust 实现。已有 Go 逻辑先查源码再移植；
Rust 负责清楚的模块组织、资源生命周期和类型表达。已有的 Kiro、Grok Web/Console
等扩展保留在各自 Provider 中，不扩张为通用框架。

2026-10-23 按用户要求移除旧 Prism 前端。后端独立构建和运行，管理 API 保留；
新前端以后依据接口重新设计。决定见 [ADR-0124](adr/ADR-0124-cpa-first-backend-and-frontend-retirement.md)。

## 参考源码

当前功能对照和后续移植使用[状态入口记录的固定 CPA 版本](engineering-roadmap.md)。
本页的 Go 目录映射已核对该版本；已实现、待接线、待启用和待实现的差异在状态入口统一维护。
ADR-0125 等既有实现仍记录 v7.2.158 的来源，不因参考更新就宣称已经对齐新版本。

旧差分测试仍分别绑定 v7.2.80 和 v7.2.101，详见[参考基线](00-reference-baseline.md)。
新移植工作记录所用 Go commit 和符号；更新基线时保留旧预期与变化原因，不把旧测试重标为新版本。
CPA 的 MIT 代码和测试可以按许可证移植并保留署名，不必为已有行为重新发明方案。

## 一条请求如何执行

```text
客户端
  → gateway-http-actix：HTTP、鉴权、JSON/SSE/WebSocket
  → protocol-*：客户端协议与 Canonical 请求/事件转换
  → gateway-router：模型、路由、账号租约、尝试与取消
  → provider-*：上游请求、凭据刷新、响应解码
  → gateway-upstream：HTTP、TLS、代理和连接管理
  → 原路径返回客户端；事件写入 gateway-store
```

```text
管理 API (/admin)
  → gateway-control：配置、账号、模型和运维操作
  → gateway-store：SQLite、加密凭据、事件和备份
  → runtime/reload：构建并发布新的运行代
  → runtime/management：查询实际运行态、解释路由和显式恢复
```

公开推理和管理使用两个监听器。管理接口的认证、Origin/CSRF、权限及配置 revision
继续由后端执行。当前没有 `/admin-ui` 页面、静态文件服务器或 npm 构建步骤。

## Go 与 Rust 的职责对应

以下 Go 路径相对状态入口记录的固定 CLIProxyAPI commit；Rust 路径相对本仓库。

| 职责 | CPA Go 入口 | Rust 入口 |
|---|---|---|
| CLI、启动、装配 | `cmd/server`、`sdk/cliproxy/service_lifecycle.go` | `apps/gateway/src/main.rs`、`deployment.rs`、`runtime.rs` |
| HTTP 与鉴权 | `internal/api`、`sdk/api/handlers`、`sdk/access` | `crates/gateway-http-actix`、`crates/gateway-auth` |
| 协议转换 | `internal/translator`、`sdk/translator` | `crates/protocol-*`、`crates/gateway-router/src/protocol_transform.rs`、`response_transform.rs` |
| 账号选择与执行 | `sdk/cliproxy/auth/conductor*`、`selector.go` | `crates/gateway-router/src/credential_scheduler.rs`、`provider_scoped_selector.rs`、`attempt_orchestrator.rs` |
| Provider 执行器 | `internal/runtime/executor` | `crates/provider-*`，由 `apps/gateway/src/runtime.rs` 装配 |
| OAuth 与凭据刷新 | `internal/auth`、`sdk/auth`、`sdk/cliproxy/auth/conductor_refresh.go` | `crates/gateway-http-actix/src/management_resources/`、`crates/provider-*`、`apps/gateway/src/credential_refresh.rs` |
| 模型目录 | `internal/registry` | `crates/gateway-catalog`、`apps/gateway/src/runtime/catalog_refresh.rs` |
| 配置与热更新 | `internal/config`、`internal/watcher` | `crates/gateway-control`、`crates/gateway-store/src/control_plane.rs`、`apps/gateway/src/runtime/reload.rs` |
| 管理接口 | `internal/api/handlers/management` | `crates/gateway-http-actix/src/management_*`、`crates/gateway-control` |
| 流式响应 | `sdk/api/handlers/stream_forwarder.go` | `crates/gateway-stream`、`crates/protocol-*/src/` 的 SSE 编解码器 |

CPA 的 xAI/Grok Build 执行器和 Device OAuth 对应
`internal/runtime/executor/xai_executor*`、`internal/auth/xai` 和 `sdk/auth/xai.go`。
能力支持、生产准入及实际验证分别查状态入口与支持清单。

## Rust 模块归属

现有 workspace 保持单进程模块化结构，不新增服务、插件系统或重复接口层。

| 模块 | 拥有的实现 |
|---|---|
| `gateway-core`、`gateway-protocol` | 共享请求/事件、标识及协议注册类型 |
| `protocol-openai-chat`、`protocol-openai-responses`、`protocol-anthropic` | 请求解析、响应与 SSE 编解码 |
| `gateway-router` | 路由快照、能力准入、调度、尝试和协议投影 |
| `gateway-provider`、`provider-*` | 执行器接口及具体上游行为 |
| `gateway-upstream` | 凭据池、出口与实际网络传输 |
| `gateway-auth`、`gateway-catalog` | 客户端认证、权限及模型目录 |
| `gateway-control` | 管理用例、配置编译和发布 |
| `gateway-store` | SQLite、加密保存、证据 reader、备份恢复 |
| `gateway-stream`、`gateway-observability` | 流控制、取消、事件与指标 |
| `gateway-http-actix` | HTTP 路由、管理鉴权与请求/响应传输 |
| `apps/gateway` | 进程启动、依赖装配和运行态与接口的连接 |
| `tests/differential` | 固定版本 CPA 行为与 Rust 的差分样例 |

具体编译依赖由 `scripts/check-crate-boundaries.rb` 检查。

## 常见修改从哪里开始

- **协议差异**：查 CPA 对应 translator/handler，然后改 `protocol-*` 或现有投影；
  使用输入/输出样例验证，只有生命周期受影响时才加运行时验收。
- **新增或修正渠道**：查 CPA executor/auth，改对应 `provider-*`；
  在 `runtime.rs` 的注册、能力和 dispatch 处接线。CPA 未覆盖的渠道再查其专项参考。
- **管理接口**：维护 `docs/openapi/management-v1.json`、HTTP handler 和对应 control/store 用例。
  运行态查询与显式恢复在 `runtime/management.rs`，不再混在请求执行实现中。
- **启动、重载、部署**：分别进入 `deployment.rs`、`runtime/reload.rs` 和 `deploy/`。
- **新前端**：以后独立设计；从 OpenAPI、登录和权限契约开始，不恢复旧 Prism 构建耦合。

## 2026-10-23 已实施的架构整理

已从 `runtime.rs` 迁出管理运行态模块，复用已有 `ManagementRuntimeFacade`，
没有新增 trait、crate 或 feature flag。协议、账号调度、持久化格式和原有恢复语义保持。
旧前端源码、嵌入模块、Cargo build script 和专属检查工具退出当前树。
OpenAPI operation ID 校验进入现有 Rust 契约测试；API 登录、监听器隔离和进程启动继续验证。

历史设计、阶段计划和验收报告保留日期与源码身份。代码位置以本页为准；
当前完成情况和下一项以[状态入口](engineering-roadmap.md)为准。
