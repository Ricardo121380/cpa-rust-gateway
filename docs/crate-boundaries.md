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

gateway-store
  -> gateway-core + gateway-observability receiver + SQLite/Tokio writer primitives

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
  依赖 HTTP、SSE 编码、具体 Provider、路由或持久化类型。P5-08 的 `proptest` 是该 crate 的
  `dev-dependency`，仅用于固定种子取消性质测试；它不进入库目标、运行时依赖或公共 API。
- `gateway-http-actix` 可以直接使用 Tokio 和 `futures-util`，但仅用于 Actix handler 内的
  producer task、取消选择和 body polling；它不得直接依赖或暴露任何 Provider trait/type。P3-09
  与 P3-10 的 `dev-dependencies` 仅用于独立集成测试中组装两个受控 loopback 或显式授权的
  OpenAI-compatible Upstream。P3-10 的 `url` 仅解析精确测试 Endpoint，`zeroize` 仅保存
  短暂测试 Credential；它们以及具体 Provider/transport 依赖均不进入该 crate 的库目标或公共 API。
- `gateway-provider` 的 P1 Mock 只能拉取 Canonical Event；它可以使用 Tokio 的等待原语来
  表达确定性 fixture 延迟，但不得依赖 `gateway-stream`、HTTP、SSE、路由、Endpoint 或凭据。
- Provider 私有 Crate 不被 `gateway-core`、协议公共层或其它 Provider 引用。
- `provider-grok` 的 P6-01 OAuth 边界仅依赖 `serde`/`serde_json` 做有界且拒绝重复字段的
  本地 JSON 解析，`url` 仅验证固定 Device Code 验证 URI，`zeroize` 仅持有短生命周期 OAuth
  access/refresh/device/user code。P6-02 新增受限的 `gateway-store`/`rusqlite` 边，仅持久化
  Config Version + Credential 精确身份绑定的 AEAD 密文、key version 和 CAS revision；它不读取
  或修改控制面配置图、不进入 Router 热路径，也不创建 socket、TLS、代理或 Build 推理请求。
- `provider-grok` 的 P6-03 Build Responses 边仅在本 crate 内编码固定 CLI OAuth 请求、解析有界
  JSON/SSE，并在非流式响应上做有界 gzip 解码；它通过既有 `gateway-upstream` 类型交出 P2 已准入
  的精确 Target，但不创建 Client、socket、TLS、代理或真实请求。它可以依赖
  `protocol-openai-responses` 的 Canonical 请求类型，但绝不引用其它 Provider 私有 crate；
  `time` 仅在 `CR-P6-03-008` 的 bytes-only OAuth 来源适配中严格解析 RFC3339 绝对过期时间，
  不读取时钟、不创建网络 I/O，也不改变 P6-01 相对 `expires_in` 行为。
  `flate2` 仅用于 1 MiB 上限内的 gzip 解码，`getrandom` 仅生成不持久化、不诊断的进程/请求关联值；
  `tokio` 仅为 ignored 的授权单探针测试目标提供受限异步驱动，`gateway-router` 仅为
  `p6_09_inference_adapter` 的 Provider-to-Router fixture 纵向测试提供 dev-dependency；两者都不进入
  `provider-grok` 库目标或公共 API，也不形成运行时的 Provider→Router 依赖。状态/Quota/Cache/
  Continuity 策略仍由后续 P6 Task 所有。
- `provider-kiro` 的 P7-01 凭据边界仅依赖 `serde`/`serde_json` 对显式传入的有界 JSON 做严格
  解析，`zeroize` 只持有 Social/Enterprise OAuth 或 `ksk_` Secret，`gateway-store` 只为由精确
  Credential ID 关联数据绑定的 AEAD envelope 提供加解密。它不读取缓存、环境或数据库，不创建
  网络 I/O，也不决定 IDE/CLI Endpoint、API Region 或推理 Header；这些属于后续 P7 Task。
- `gateway-store` 不进入 Router 的请求选择路径；持久数据通过控制面编译为不可变 Snapshot。
- `gateway-router` 可以使用 `gateway-auth` 的无存储 Client Key HMAC 原语来认证其已编译
  Snapshot；该边不允许反向的 Router、Store 或 HTTP 依赖进入 `gateway-auth`。
- `gateway-router` 可以直接使用 Tokio 的受限超时原语来执行 Route 的累计 bootstrap
  deadline；它不得借此创建后台重试任务、无界队列、HTTP client、Provider 或 Stream 依赖。
- `gateway-observability` 可以直接使用 Tokio 的有界 Channel 原语来接收结构化事件；它不得
  依赖 Router、HTTP、Store、具体 Provider 或在请求路径内执行 SQLite/网络写入。
- `gateway-store` 可以消费 `gateway-observability::EventQueueReceiver`，以异步批写将已入队的
  事件落入 SQLite；这条边严格单向，`gateway-observability` 不得反向依赖 Store，且 Router/HTTP
  热路径不得等待 writer、SQLite 或其失败重试。
- 当前精确允许边由 `scripts/check-crate-boundaries.rb` 校验。
