# 目标 Rust 架构草案

本草案用于验证功能筛选能否落入清晰模块。最终模块会随功能矩阵确认而调整。

## 1. 总体结构

```text
                    Data Plane

Client
  -> Actix HTTP Adapter
  -> Client Key / Access Group View
  -> Inbound Protocol Adapter
  -> Canonical Request
  -> Public Model Resolver
  -> RouteSnapshot Candidate Plan
  -> Continuity Resolver
  -> Candidate Scheduler
  -> Endpoint Credential Lease
  -> Provider Adapter
  -> Resolved Upstream Endpoint + Shared Client/Egress Pool
  -> Canonical Event Stream
  -> Outbound Protocol Adapter
  -> Client

                    Control Plane

Admin API
  -> Config Service
  -> Upstream / Endpoint Service
  -> Credential Service
  -> Catalog Discovery Service
  -> Public Model / Access Group Service
  -> Route Compiler
  -> Runtime Event Store
  -> Immutable Snapshot Publish
```

## 2. Rust Workspace 草案

```text
crates/
  gateway-core/
  gateway-protocol/
  protocol-openai-responses/
  protocol-anthropic/
  protocol-openai-chat/
  gateway-provider/
  gateway-upstream/
  gateway-catalog/
  provider-grok/
    official/
    build/
    web/
  provider-kiro/
    conversation/
    endpoint/
    eventstream/
  provider-openai-compatible/
  provider-anthropic-compatible/
  gateway-router/
  gateway-auth/
  gateway-stream/
  gateway-observability/
  gateway-store/
  gateway-control/
  gateway-http-actix/

apps/
  gateway/

web/
  admin-ui/
```

## 3. 核心 Trait

### Provider 与能力接口

```rust
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> ProviderId;
    fn capabilities(&self) -> ProviderCapabilities;
}

#[async_trait]
pub trait InferenceAdapter: ProviderAdapter {
    async fn execute(
        &self,
        ctx: RequestContext,
        target: ResolvedUpstreamTarget,
        request: CanonicalRequest,
    ) -> Result<CanonicalResponse, GatewayError>;

    async fn execute_stream(
        &self,
        ctx: RequestContext,
        target: ResolvedUpstreamTarget,
        request: CanonicalRequest,
    ) -> Result<CanonicalStream, GatewayError>;
}

#[async_trait]
pub trait ModelCatalogSource: ProviderAdapter {
    async fn models(
        &self,
        endpoint: &UpstreamEndpoint,
        credential: &Credential,
    ) -> Result<Vec<ModelCapability>, GatewayError>;
}

#[async_trait]
pub trait CredentialRefresher: ProviderAdapter {
    async fn refresh(
        &self,
        credential: Arc<Credential>,
    ) -> Result<CredentialUpdate, GatewayError>;
}

#[async_trait]
pub trait QuotaSource: ProviderAdapter {
    async fn quota(&self, credential: &Credential) -> Result<QuotaSnapshot, GatewayError>;
}

pub trait ContinuityPolicy: ProviderAdapter {
    fn requirements(&self, request: &CanonicalRequest) -> ContinuityRequirements;
}
```

不要强迫 Web、Build、Kiro 和通用 OpenAI Adapter 实现它们并不具备的能力。Registry 根据小 Trait 注册真实能力，不创建“返回 unsupported”的万能 Provider 对象。

### Protocol Adapter

```rust
pub trait InboundProtocol: Send + Sync {
    fn decode_request(
        &self,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<CanonicalRequest, GatewayError>;
}

pub trait OutboundProtocol: Send + Sync {
    fn encode_response(
        &self,
        response: CanonicalResponse,
    ) -> Result<Bytes, GatewayError>;

    fn encode_event(
        &self,
        event: CanonicalEvent,
    ) -> Result<Option<Bytes>, GatewayError>;
}
```

### Routing Strategy

```rust
pub trait RoutingStrategy: Send + Sync {
    fn select(
        &self,
        request: &RouteRequest,
        candidates: &[RouteCandidate],
    ) -> Result<RouteSelection, RouteError>;
}
```

`RouteRequest` 不只包含模型名，还包含 Access Group、入口 API Format、Request Type、Tool/Thinking/模态要求、Continuity Key 和 Attempt 排除集合。`RouteSelection` 先返回 Candidate；具体 Credential 由 Endpoint 的凭据池在第二阶段租用。

### Endpoint Policy

```rust
pub trait EndpointPolicy: Send + Sync {
    fn id(&self) -> EndpointId;
    fn build_request(
        &self,
        ctx: &ProviderRequestContext,
        request: ProviderRequest,
    ) -> Result<http::Request<Bytes>, GatewayError>;
}
```

Kiro 的 `ide`/`cli` 使用 Endpoint Policy；它们不是两个可以混池的 Provider。Grok Official/Build/Web 则是三个 Provider Adapter，因为凭据、协议、Quota 和连续性状态本质不同。

通用中转站 Endpoint 不复用 Kiro 的 `EndpointPolicy` 概念硬编码逻辑。它们通过 `UpstreamEndpoint` 数据实体声明 `adapter_id + api_format + base_url + path + transport`；一个 Endpoint 只绑定一种 API Format。

## 4. 热路径状态

### 不可变数据

- 模型到 Candidate 的索引。
- Provider 能力。
- Upstream、Endpoint 和 Catalog 硬资格。
- Public Model、Alias 和路由规则。
- 客户端 API Key / Access Group 权限视图。
- 每个 Priority Tier 的预编译加权调度序列。

使用不可变 `RouteSnapshot`，通过 `ArcSwap` 原子发布。请求热路径不读取 YAML、SQLite，也不获取全局读锁。

### 可变数据

- 凭据状态。
- 并发计数。
- Cooldown/Quota 恢复时间。
- Session/Cache Affinity。
- Response Ownership、Reasoning Replay、Web Conversation State。
- Candidate Round-robin Cursor 与 Credential Pool Cursor。
- Endpoint/Credential Health、Circuit 和短期 Quota 状态。

按 Provider/模型/凭据分片，避免单一全局 Mutex。

## 5. 推荐依赖方向

```text
gateway-core
  ↑
gateway-protocol      gateway-provider      gateway-upstream
  ↑                         ↑                      ↑
protocol-*             provider-*          gateway-catalog
          \                |                    /
       gateway-router
                         ↑
                  gateway-http-actix
```

禁止反向依赖：

- `gateway-core` 不依赖 Actix。
- Provider 不依赖任何入口协议类型。
- Provider Adapter 不决定 Public Model、Access Group 或 Route Policy。
- Upstream Endpoint 不包含客户端权限。
- Grok 子 Provider 不共享 Credential/Quota/Error Runtime State。
- Protocol Adapter 不直接选择凭据。
- Router 不执行 HTTP。
- Continuity Store 不决定模型 Alias。
- Store 不参与每个 SSE Chunk 的处理。

## 6. 第一阶段垂直链路

```text
M0 Shared Core
  POST /v1/responses + POST /v1/messages
  -> Client API Key
  -> Inbound decode
  -> CanonicalRequest
  -> Route/Continuity/Credential Lease
  -> Mock Provider
  -> Canonical Event
  -> target protocol encode

M0-A Aggregation Control Plane
  -> Upstream / Endpoint / Credential
  -> Public Model / Alias / Route / Candidate
  -> Access Group / Client Key
  -> Config Version + Route Compiler + ArcSwap

M0-B Aggregation Data Plane
  -> two OpenAI-compatible Responses endpoints
  -> one public model
  -> smooth weighted round-robin
  -> endpoint credential lease
  -> first-event failover

M0-C Catalog + Explain
  -> per endpoint + credential model discovery
  -> last successful catalog snapshot
  -> /v1/models from RouteSnapshot
  -> route explain + attempt diagnostics

M0-D Compatible Anthropic Endpoint
  -> anthropic-compatible/messages
  -> protocol capability filter
  -> lossless bridge gate

M1 grok.build
  -> Device OAuth/OAuth import
  -> Build Responses HTTP
  -> model + billing + cache affinity + reasoning replay

M2 kiro
  -> Social/IdC/ksk credential
  -> Kiro Conversation Request
  -> IDE/CLI Endpoint Policy
  -> AWS EventStream
  -> Anthropic/Claude Code output

M3 grok.official
  -> API Key
  -> official Responses HTTP
  -> official quota/tool/reasoning capability

M4 grok.web
  -> SSO + browser egress session
  -> Web Conversation State
  -> Statsig + Web quota

M5 compatible relay expansion
  -> OpenAI Chat endpoint after public Chat ingress is enabled
  -> more discovery formats and import adapters
```

每条链路内部仍按“非流式 -> SSE -> 两凭据 -> Affinity -> Quota/Error Failover”递进。M0-A 到 M0-D 先证明多中转站汇总和自有 Key 的公共底座；M1 到 M4 仍是已锁定的第一阶段专用渠道包。

## 7. 性能边界

- 使用 `Bytes` 传递请求和流 Chunk。
- 每条流使用 bounded channel。
- 上游 Client 全局共享连接池。
- connect、first-byte、idle、total timeout 分开。
- 日志与 SQLite 使用独立有界队列批量写入。
- Body 日志默认关闭或采样。
- 路由表通过不可变快照读取。
- Credential/Quota/Capability 变化通过事件增量更新候选索引。
- 模型发现只在后台更新 CatalogSnapshot，失败不清空最后成功快照。
- Candidate 选择与 Credential Lease 使用不同原子 Cursor，避免站内 Key 数扭曲站间权重。
- Token Refresh 按 Credential 分片 Singleflight。
- AWS EventStream 与 SSE 各自使用有界增量解码器。
- 不在请求热路径执行动态插件或磁盘访问。

## 8. 已冻结的架构决策

本节原有待定项已经由开发计划 `v1.0` 的 `BL-01` 至 `BL-22` 冻结；完整且优先级更高的定义见 [Rust AI Gateway 详细开发计划](06-development-plan.md#3-已冻结的技术基线)。对应结论如下：

- Release 1 内建 SQLite 请求事件明细，但只通过有界异步队列写入，SQLite 不进入请求热路径。
- 公开入口只包含 OpenAI Responses 与 Anthropic Messages；OpenAI Chat Completions 延后到 P13。
- Grok Web 进入 Release 1，但由 Feature Flag 隔离并经过独立 Gate；未经 Canary 批准不得在生产启用。
- Session/Cache Identity 至少隔离 `client_key + provider + upstream_model`；不同连续性状态使用独立命名空间。
- 管理 API 先于 UI；管理端采用独立 TypeScript SPA，不进入推理热路径。
- CPA、grok2api、Kiro-RS 与 New API 只允许一次性迁移或临时兼容接入，不提供运行时持续同步。
- Grok Web Function Tool Emulation 默认关闭，并在能力元数据中明确标记模拟行为。
- Catalog 默认 `Fresh 6h / Stale 24h / Expired 72h`；模型移除要求连续 3 次成功缺失且持续不少于 24h。
- Release 1 Client Key 实现 Access Group、到期、RPM、并发和可选 Token 上限，不实现美元计费。

## 9. Provider 拓扑

```text
Provider Registry
  ├── grok.official
  │     ├── APIKeyCredential
  │     ├── OfficialModelCatalog
  │     └── HeaderQuotaSource
  ├── grok.build
  │     ├── OAuthCredential
  │     ├── DeviceOAuth + Refresh
  │     ├── BuildModelCatalog + Billing
  │     └── CacheAffinity + ResponseOwnership + ReasoningReplay
  ├── grok.web
  │     ├── SSOCredential
  │     ├── BrowserEgressSession + Statsig
  │     ├── WebModelCatalog + REST/gRPC Quota
  │     └── WebConversationState
  ├── kiro
  │     ├── Social / IdC / APIKey Credential
  │     ├── IDE / CLI EndpointPolicy
  │     ├── Dynamic ModelCapability
  │     └── AWS EventStream Decoder
  ├── openai-compatible
  │     ├── Upstream instances
  │     ├── responses endpoints
  │     ├── APIKeyCredential bindings
  │     └── Discovered + configured ModelCapability
  └── anthropic-compatible
        ├── Upstream instances
        ├── messages endpoints
        └── APIKeyCredential bindings
```

共享的是 Core 类型、调度原语、事件和安全设施；不共享的是 Provider 私有协议和运行时状态。

## 10. 建议的内部模块边界

`provider-grok` 可以是一个 Crate，但内部三个 Adapter 必须有独立 `ProviderId` 和状态命名空间：

```text
provider-grok/src/
  lib.rs
  shared/        # 仅 OAuth DTO、Responses 辅助和明确可共享的校验
  official/
  build/
  web/
```

`provider-kiro` 内部按协议层拆分：

```text
provider-kiro/src/
  credential/
  model_catalog/
  conversation/
  endpoint/{ide,cli}.rs
  eventstream/
  error_classifier.rs
```

禁止把 Kiro-RS 的 Axum Handler 或 grok2api 的 Go Service 结构原样映射成 Rust 模块；模块边界以本项目的 Canonical Pipeline 为准。

聚合控制面建议按实体而不是参考项目表结构拆分：

```text
gateway-upstream/
  upstream.rs
  endpoint.rs
  credential_binding.rs
  egress_policy.rs

gateway-catalog/
  discovery.rs
  snapshot.rs
  diff.rs
  freshness.rs

```

> 实现说明（2026-07-27）：草案里的 `gateway-access/` 与 `gateway-continuity/` 两个预留 crate 从未
> 承载代码。它们的职责在实现中落到了别处，且已在那里完整交付，因此这两个空壳连同三条指向它们的
> 依赖边一并移除（`CR-P12-06-003`）：
>
> - Client Key、Access Group、Public Model 与 Route 编译：`gateway-control`（编译与发布）与
>   `gateway-router`（不可变 Snapshot 中的只读视图）。
> - Cache Affinity、Response Ownership、Reasoning Replay 与 Web Conversation State：
>   `provider-grok`（`continuity_state.rs`、`official_runtime.rs`、`build_responses.rs`），因为
>   BL-12/BL-14 要求这些状态按 Provider 家族隔离，而不是集中在一个共享 crate 里。

详细实体、冲突规则和 `minimax-m3` 轮询示例见 [上游聚合、统一模型与自有 API 设计](05-upstream-aggregation-design.md)。
