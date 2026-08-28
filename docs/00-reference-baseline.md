# CPA v7.2.80 参考基线与架构

## 1. 冻结的源码口径

| 项目 | 值 |
|---|---|
| 仓库 | `router-for-me/CLIProxyAPI` |
| Tag | `v7.2.80` |
| 源码归档 | `https://codeload.github.com/router-for-me/CLIProxyAPI/tar.gz/refs/tags/v7.2.80` |
| 本次分析归档 SHA-256 | `43ddf12bfb78ddf8aa3b17e7002f42d8358de6b704f706c01d01e7e2d9a732ae` |
| License | MIT |
| 主语言 | Go |
| HTTP 框架 | Gin `v1.10.1` |

如果后续复制 CPA 的代码、算法实现或大段测试夹具，需要保留其 MIT License 和版权声明。仅参考公开 API 行为并重新实现时，也应在项目文档中注明参考来源。

### 1.1 渠道专项参考

CPA 不再是唯一参考实现。第一阶段渠道专项同时冻结以下快照：

| 项目 | 快照 | 主要参考范围 | License 处理 |
|---|---|---|---|
| `chenyme/grok2api` | `v3.0.0` / `ec6cddca7d2454996540adbf994f3c3d4ed2d2a1` | Grok Build/Web 双池、额度、会话、出口和安全存储 | MIT，可在保留声明后移植；默认优先重实现 |
| 服务器定制 Kiro-RS | `c49c75eb4f35af74714081505841cceb68b33d9d` | Kiro CLI/IDE、EventStream、Claude Code 兼容补丁 | MIT，可在保留声明后移植；默认优先重实现 |
| `looplj/axonhub` | `v1.0.0-beta4` / `7122f32994d9131e63a4217be3d58d33f187c350` | 多 Endpoint 渠道、三层模型命名、API Key Profile、模型同步和负载均衡 | 主体 Apache-2.0、`llm/` LGPL-3.0；默认 clean-room 行为参考 |
| `QuantumNous/new-api` | `923a17ca8a3f08878d583ab4203190d2bee12c93` | xAI 官方 API 的公开行为 | AGPL-3.0，只做 clean-room 行为参考 |
| `Wei-Shaw/sub2api` | `57914967cbb127ff715719c3879d881c10d75274` | OAuth 并发、Cache Identity、Quota 和安全桥接行为 | LGPL-3.0，只做 clean-room 行为参考 |

渠道专项取舍见 [Grok 与 Kiro 渠道参考实现分析](04-channel-reference-analysis.md)，聚合专项取舍见 [上游聚合、统一模型与自有 API 设计](05-upstream-aggregation-design.md)。

## 2. 分析边界

本基线只描述 CPA 本体：

- HTTP/SSE/WebSocket 接入
- 协议转换
- 模型与 Provider 注册
- 凭据生命周期和调度
- Provider Executor
- Management API
- 插件、存储、日志和热更新

不把 CPAMP 视为 CPA 内部模块。CPAMP 的 SQLite 持久化、统计面板、账号批量管理和配额展示属于外置控制与观测系统。

上游聚合是新项目新增的控制面边界，不按 CPA 的 Provider 配置或 New API 的 Channel 表直接照搬。它独立拆分为：

```text
Provider Adapter
Upstream + Endpoint + Credential
Public Model + Model Route + Route Candidate
Client Key + Access Group
```

## 3. CPA 架构形态

CPA 是单进程模块化网关：

```text
Client
  │
  ▼
Gin Router + Frontend API-Key Auth
  │
  ▼
Protocol Handler
OpenAI / Responses / Anthropic / Gemini
  │
  ▼
Model Registry + Provider Resolution
  │
  ▼
Auth Manager + Scheduler
  │
  ├── Credential state / cooldown / retry / refresh
  └── Alias / prefix / session affinity / model pool
  │
  ▼
Translator + Thinking + Payload Rules
  │
  ▼
Provider Executor
  │
  ▼
Upstream HTTP / SSE / WebSocket
```

旁路控制面：

```text
config.yaml ─┐
auth JSON ───┼─> fsnotify watcher ─> runtime reconciliation
Management ──┘

Provider result ─> Usage Event ─> in-memory queue / external collector
```

## 4. 主要源码边界

| 目录 | 职责 |
|---|---|
| `cmd/server` | CLI 参数、配置与存储初始化、登录模式、服务启动 |
| `internal/api` | Gin Server、路由、中间件、Management API 注册 |
| `sdk/api/handlers` | OpenAI、Responses、Anthropic、Gemini Handler |
| `internal/translator` | 入口协议与上游格式之间的请求/响应转换 |
| `internal/runtime/executor` | Claude、Codex、xAI、Gemini、Kimi 等上游执行器 |
| `sdk/cliproxy/auth` | 凭据状态、调度、重试、冷却、模型池、执行编排 |
| `internal/registry` | 模型目录、按凭据注册、Provider 可用性 |
| `sdk/auth`、`internal/auth` | OAuth、Token 解析、刷新和持久化 |
| `internal/thinking` | Thinking suffix、effort、budget 和 Provider 映射 |
| `internal/cache`、`internal/signature` | Reasoning replay 与签名缓存 |
| `internal/watcher` | 配置及 auth 文件热更新 |
| `internal/api/handlers/management` | 配置、凭据、OAuth、日志、插件管理 |
| `internal/pluginhost`、`internal/pluginstore` | 动态库插件宿主和插件商店 |
| `internal/store` | PostgreSQL、Git、对象存储后端 |
| `sdk/cliproxy/usage`、`internal/redisqueue` | Usage event 分发和短期队列 |

## 5. 公共推理接口

### OpenAI/Responses

- `GET /v1/models`
- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/responses`
- `GET /v1/responses`，WebSocket 入口
- `POST /v1/responses/compact`
- `POST /v1/alpha/search`
- `POST /v1/images/generations`
- `POST /v1/images/edits`
- xAI/OpenAI 视频创建、编辑、扩展和查询接口

### Anthropic

- `POST /v1/messages`
- `POST /v1/messages/count_tokens`

### Codex 直连别名

- `/backend-api/codex/responses`
- `/backend-api/codex/responses/compact`
- `/backend-api/codex/alpha/search`

### Gemini

- `GET /v1beta/models`
- `POST /v1beta/interactions`
- `/v1beta/models/*action`

## 6. 内置 Provider Executor

- Codex HTTP/WebSocket 自动选择
- xAI HTTP/WebSocket 自动选择
- Claude
- Gemini Generative Language
- Gemini Interactions
- Vertex
- AI Studio
- Antigravity
- Kimi
- 任意 OpenAI-compatible Provider
- 插件提供的自定义 Provider

## 7. 核心设计债务

### 协议组合膨胀

CPA 采用大量协议对协议 Translator。入口协议和上游格式增加时，转换数量接近乘法增长。新项目应改为：

```text
Inbound Adapter -> Canonical Request/Event -> Provider Adapter
```

### 热路径职责过多

请求执行同时涉及模型解析、别名、调度、重试、转换、Thinking、签名、Usage 和插件 Hook。新项目应通过明确 Pipeline 阶段和不可变快照降低耦合。

### 插件能力过宽

插件可以介入认证、模型、路由、调度、请求、响应、Thinking、Usage、CLI 和 Management API，增加运行时不确定性。第一版应使用编译期 Provider Trait，而不是原生动态库。

### 状态可见性不完整

CPA 内部维护凭据级和模型级错误、冷却、Quota 状态，但外置面板未必能完整呈现 403 等账号状态。新项目需要把状态变化定义成稳定事件和查询 API。

## 8. 新项目不应继承的结构

- 不继承 CPA 的 Go 包目录。
- 不保留协议对协议的全连接 Translator 矩阵。
- 不让 Actix Request/Response 类型进入核心层。
- 不把配置文件同时当作运行时数据库。
- 不在首版引入动态二进制插件。
- 不让日志、SQLite 或管理面板进入请求热路径。
