# Crate boundaries

本文件定义 P0 Workspace 的编译期依赖方向。业务实现只能沿箭头方向依赖，禁止为方便而反向引用 HTTP、Provider 或 Store 类型。

```text
apps/gateway
  -> gateway-http-actix + gateway-control + gateway-observability

gateway-http-actix
  -> protocol adapters + gateway-control/router/stream/auth/observability + Actix-local Tokio/Future primitives

gateway-control
  -> upstream/catalog/access/router/auth/store/observability

gateway-router
  -> core/auth/provider/upstream/catalog/access/continuity + Tokio timeout primitive

gateway-observability
  -> gateway-core + Tokio bounded-channel primitives

provider-*
  -> core/provider/upstream + matching protocol/continuity/stream

protocol-*
  -> gateway-core + gateway-protocol (+ serde JSON codec primitives where required)

gateway-provider
  -> gateway-core + Tokio pull-only time primitives

gateway-stream
  -> gateway-core + gateway-protocol + Tokio bounded-channel/cancellation primitives

leaf foundations
  -> gateway-core

gateway-core
  -> no internal crate
```

## 不变量

- `gateway-core` 不依赖 Actix、SQLite 或任何具体 Provider。
- Actix Web 只能出现在 `gateway-http-actix` 的依赖闭包入口。
- `gateway-stream` 只承载有界 Canonical Event 交付、背压、取消和语义事件提交边界；不得
  依赖 HTTP、SSE 编码、具体 Provider、路由或持久化类型。
- `gateway-http-actix` 可以直接使用 Tokio 和 `futures-util`，但仅用于 Actix handler 内的
  producer task、取消选择和 body polling；它不得直接依赖或暴露任何 Provider trait/type。P3-09
  的 `dev-dependencies` 仅用于独立集成测试中组装两个受控 loopback OpenAI-compatible Mock
  Upstream，不进入该 crate 的库目标或公共 API。
- `gateway-provider` 的 P1 Mock 只能拉取 Canonical Event；它可以使用 Tokio 的等待原语来
  表达确定性 fixture 延迟，但不得依赖 `gateway-stream`、HTTP、SSE、路由、Endpoint 或凭据。
- Provider 私有 Crate 不被 `gateway-core`、协议公共层或其它 Provider 引用。
- `gateway-store` 不进入 Router 的请求选择路径；持久数据通过控制面编译为不可变 Snapshot。
- `gateway-router` 可以使用 `gateway-auth` 的无存储 Client Key HMAC 原语来认证其已编译
  Snapshot；该边不允许反向的 Router、Store 或 HTTP 依赖进入 `gateway-auth`。
- `gateway-router` 可以直接使用 Tokio 的受限超时原语来执行 Route 的累计 bootstrap
  deadline；它不得借此创建后台重试任务、无界队列、HTTP client、Provider 或 Stream 依赖。
- `gateway-observability` 可以直接使用 Tokio 的有界 Channel 原语来接收结构化事件；它不得
  依赖 Router、HTTP、Store、具体 Provider 或在请求路径内执行 SQLite/网络写入。
- 当前精确允许边由 `scripts/check-crate-boundaries.rb` 校验。
