# Crate boundaries

本文件定义 P0 Workspace 的编译期依赖方向。业务实现只能沿箭头方向依赖，禁止为方便而反向引用 HTTP、Provider 或 Store 类型。

```text
apps/gateway (binary-only deployment composition root)
  -> gateway-http-actix + gateway-control + gateway-observability + gateway-auth/store
     + gateway-protocol api_format adapter registry
     + protocol-anthropic/provider-anthropic-compatible Messages boundary
     + provider-kiro native Kiro adapter boundary
     + Actix-local HTTP/Future primitives + libc credential-file guard + zeroize

gateway-http-actix
  -> protocol adapters + gateway-control/router/stream/auth/observability + Actix-local Tokio/Future primitives

gateway-control
  -> upstream/catalog/access/router/protocol/auth/store/observability

gateway-router
  -> core/auth/protocol/provider/upstream/catalog/access/continuity + Tokio timeout primitive

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

differential-gate (tests/differential, test-only sink; nothing depends on it)
  -> gateway-core + gateway-store + gateway-upstream + provider-grok + provider-kiro

gateway-core
  -> no internal crate
```

## 不变量

- `gateway-core` 不依赖 Actix、SQLite 或任何具体 Provider。
- `tests/differential` 的 `differential-gate` 是 P11-01 差分门禁的唯一 test-only 汇点。它可以依赖
  `gateway-core`、`gateway-store`、`gateway-upstream`、`provider-grok` 与 `provider-kiro`，以便
  用真实代码计算 gateway 侧投影；但任何 workspace member 都不得依赖它，也不得把它引入运行时、
  部署装配或 SBOM 运行时闭包。它没有 HTTP client、文件系统遍历、环境变量读取、参考实现读取或
  凭据类型。
- `apps/gateway` 是唯一的二进制部署装配根，不是可被其它 crate 引用的库。P12-02 的
  `gateway serve` 在此处显式绑定两个 loopback listener，并从 systemd `LoadCredential` 目录
  读取严格校验、短暂零化的 Management/CSRF/Master/Backup/Client-Key Pepper。故该二进制可以
  直接依赖 Actix、`futures-util`、`gateway-auth`、`gateway-store`、`libc` 和 `zeroize`；它不
  装配推理数据面，不得向任一 library crate 下推 HTTP、SQLite、credential-file 或部署依赖，且
  其它 workspace member 不得依赖 `gateway`。
- Actix Web 只能出现在 `gateway-http-actix` 的依赖闭包入口。
- `gateway-stream` 只承载有界 Canonical Event 交付、背压、取消和语义事件提交边界；不得
  依赖 HTTP、SSE 编码、具体 Provider、路由或持久化类型。P5-08 的 `proptest` 是该 crate 的
  `dev-dependency`，仅用于固定种子取消性质测试；它不进入库目标、运行时依赖或公共 API。
- `gateway-http-actix` 可以直接使用 Tokio 和 `futures-util`，但仅用于 Actix handler 内的
  producer task、取消选择和 body polling；它不得直接依赖或暴露任何 Provider trait/type。P3-09
  与 P3-10 的 `dev-dependencies` 仅用于独立集成测试中组装两个受控 loopback 或显式授权的
  OpenAI-compatible Upstream。P10-02 在 HTTP 管理 Scope 内以 `url` 规范化一个精确 Origin、以
  `zeroize` 保存短暂 Management Key/CSRF token，并以 `subtle` 做常量时间比较；这三项不接触
  数据面路由、Provider/transport 依赖或公共推理 API。具体 Provider/transport 依赖仍仅存在于
  相应的独立测试目标，不能进入该 crate 的库 API。
- `gateway-provider` 的 P1 Mock 只能拉取 Canonical Event；它可以使用 Tokio 的等待原语来
  表达确定性 fixture 延迟，但不得依赖 `gateway-stream`、HTTP、SSE、路由、Endpoint 或凭据。
- Provider 私有 Crate 不被 `gateway-core`、协议公共层或其它 Provider 引用。
- `protocol-anthropic` 同时拥有两个方向的纯编解码：面向客户端的 `decode_request`/`encode_response`
  与面向上游的 `encode_upstream_request`/`decode_upstream_response`。上游方向只使用
  `gateway-core` 的 Canonical 类型和 `serde_json`，不引入 HTTP、传输、Endpoint、凭据或
  Provider 依赖，因此未新增任何 crate 依赖边；`provider-anthropic-compatible` 负责在其上组装
  Target、Header 与凭据。
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
  `flate2` 仅用于 1 MiB 上限内的 gzip 解码，`getrandom` 仅生成不持久化、不诊断的进程/请求关联值；部署组合根
  `gateway` 额外使用它生成 Provider account-pool 管理快照的进程实例 nonce，用于拒绝重启后复用旧
  cursor；该值不进入诊断、日志、凭证或持久化状态。
  `tokio` 仅为 ignored 的授权单探针测试目标提供受限异步驱动。P8-05 的 `gateway-router` 运行时
  依赖仅接收已脱敏的 `RuntimeQuotaRegistry` 和 exact-target quota 类型，以将 Official Header 观察
  写入 Router-owned 状态；它不选择 Route/Public Model/凭据、不开 HTTP、不读取 Build/Web 状态，且
  不形成 Router→具体 Provider 的反向依赖。P6 的 Build 状态/Quota/Cache/Continuity 仍由其专属
  模块所有，`p6_09_inference_adapter` 仍提供额外的 Provider-to-Router fixture 纵向测试。
- `provider-grok` 的 P9-01/P9-02 Web 边只接收调用方显式传入的受限 Cookie export、`SecretStore`、
  User-Agent、TLS-profile label、时间与 `UpstreamProxy` 值；它们仅构建零化/AEAD 凭据和不可变
  browser-egress-session 指纹，既不读取浏览器/Profile/Cookie jar/环境代理，也不创建 DNS、socket、
  TLS、HTTP 或代理/TUN 动作。P9-03 才可在独立的固定 Web 请求边界使用这些值。
- `provider-kiro` 的 P7-01 凭据边界仅依赖 `serde`/`serde_json` 对显式传入的有界 JSON 做严格
  解析，`zeroize` 只持有 Social/Enterprise OAuth 或 `ksk_` Secret，`gateway-store` 只为由精确
  Credential ID 关联数据绑定的 AEAD envelope 提供加解密。P7-02 新增 `url`，只验证由严格 API
  Region 派生的两条固定 HTTPS URL（IDE/CLI）；纯 Policy/Request/Profile/EventStream/Semantic 模块
  不读取缓存、环境或数据库，不创建网络 I/O，也不接受任意 endpoint 覆盖。P7-06 新增只读
  `gateway-catalog` 边，唯一用途是复用 P4-02 已验证的显式 Fresh/Stale/Expired timing policy；Kiro 的
  模型/订阅快照仍存于 `provider-kiro`，不向 Catalog 写入、不开启 SQLite/Route 依赖，也不把 Provider
  反向暴露给 Control、Router 或其它 Provider。P7-09 的 `KiroInferenceAdapter` 只经显式注入的
  `gateway-upstream` DNS-pinned transport 发送已构造的一次请求；它不读取 Kiro-RS、代理环境、凭据缓存，
  也不隐式 refresh/retry/failover。
- `provider-anthropic-compatible` 的 Anthropic Messages 出站请求边与 `provider-openai-compatible`
  完全对称，只做纯请求装配：请求体整体委托给 `protocol-anthropic::encode_upstream_request`，本
  crate 逐字节转发该编解码器返回的完整 JSON 文本，不增删任何成员，因此不需要自己的 `serde_json`
  边；唯一新增的 `zeroize` 仅持有请求作用域的 `x-api-key` 值。它不创建 socket、TLS、代理、
  超时、连接池、Credential 租约、路由，也**不实现** `gateway-provider` 的
  `InferenceAdapter`——该 trait 在构造期绑定 Credential/模型/Endpoint，与聚合控制面按 attempt
  选择 Candidate 与 Credential 的边界不兼容；`gateway-upstream` 仅用于 `EndpointUrl` 合成与把
  P2-09 已准入的精确 Target 交给共享传输。上游认证固定为单一 `x-api-key` 加必需的
  `anthropic-version`，不叠加 `Authorization: Bearer`：本网关自身的 Anthropic Messages 入站在
  同时出现两种方案时按 `ClientUnauthorized` 拒绝，故双头会让 Kiro-RS 之类的
  Anthropic-compatible 中转链路直接失败。需要额外 Header 的调用方在该边界之上组合，不得放宽
  固定的四个 Header。响应方向的 wire 解码同样属于 `protocol-anthropic`（与 OpenAI 侧对称），本
  crate 只把它再导出：装配根选定一个 Provider 后，不应为了消费该 Provider 的返回而再去引用
  编解码器 crate，因此该格式的两个方向都从这一个边界出入。
- `gateway-protocol` 只承载 `ApiFormat` 封闭词表与对适配器泛型的 `ApiFormatAdapterRegistry`；它不
  引入任何新依赖（仍只声明 `gateway-core`），也不接触 Provider、transport、Router 或 HTTP 类型。
  `gateway-router` 的新边仅用于让 `ProtocolFormat` 复用同一份字符串表；`gateway-control` 的新边仅
  用于 Route Compiler 在发布前拒绝不可服务的 `api_format`；`apps/gateway` 的新边仅用于在装配期把每个
  Endpoint 绑定到其声明格式的适配器，并只经 `provider-anthropic-compatible` 这一个边界取得该格式的
  请求装配与响应/SSE 解码。这三条边严格单向，`gateway-protocol` 不得反向依赖它们中的任何一个。
- 一个 `api_format` 可由多个 `adapter_id` 服务，注册表因此按 `adapter_id` 索引而非按格式固定槽位。
  Kiro 在线格式上是 `anthropic/messages`，但经自有凭据族、派生 host 与 `profileArn` 注入到达，
  因此是同一格式的第二个实现（`kiro.messages`），而不是新的格式。`ApiFormat::adapter_ids()` 是
  格式与其合法适配器集合的唯一来源，发布期与装配期都按它判定成员关系，两侧不会漂移。
- `apps/gateway` 对 `provider-kiro` 的边仅用于原生 Kiro 适配器：请求转换、`profileArn` 放置、
  AWS EventStream 解码与 Kiro 失败分类都留在该 crate 内。装配根不因此获得 `gateway-provider`
  的边——`provider-kiro` 重导出其 `InferenceAdapter` 与 `CanonicalEventSource`，与其它 Provider
  crate 暴露上游边界的方式一致。该边严格单向。
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
