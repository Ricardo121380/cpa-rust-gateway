# 上游聚合、统一模型与自有 API 设计

本文定义新 Rust 网关的“中转站聚合”能力：接入多个第三方中转站或本机网关，汇总它们的模型，通过稳定的公开模型名路由，最终只向客户端暴露本项目自己的 Base URL 和 API Key。

这不是把 New API、AxonHub 或 Sub2API 再包一层。它是本项目控制面的一等能力，并与 Grok、Kiro 等 Provider Adapter 共用同一套路由、凭据、健康和可观测基础设施。

## 1. 已锁定的目标

第一版必须支持：

- 配置多个第三方中转站、本机服务或官方上游。
- 一个中转站拥有多个 Base URL、Endpoint 和上游协议。
- 同一 Base URL 通过不同 Endpoint 分别使用 Responses、Chat Completions 或 Anthropic Messages。
- 每个 Endpoint 绑定一个或多个凭据，并保留凭据级模型、Quota、并发和故障状态。
- 静态模型配置与上游模型自动发现并存。
- 把不同上游模型映射到稳定的公开模型名。
- 一个公开模型对应多个 Route Candidate，并执行轮询、加权、优先级和首事件前 Failover。
- 通过 Access Group 控制每个客户端 Key 可见和可调用的模型。
- `/v1/models`、推理请求、响应模型名和 Usage 使用一致的公开模型视图。
- 客户端只使用本项目的 Base URL 与 API Key，不接触任何上游 Token。

明确不做：

- 不尝试从中转站“读取或导出它的 API Key”。模型列表接口只能发现模型，不能反向取得上游密钥。
- 不因模型同名就假定协议、工具、Thinking 或多模态能力相同。
- 不把上游发现的所有模型自动公开给客户端。
- 不在请求热路径读取 SQLite、YAML 或实时请求 `/v1/models`。

## 2. 参考项目结论

本轮冻结的聚合参考如下：

| 项目 | 快照 | 可吸收的长处 | 不照搬的部分 |
|---|---|---|---|
| 服务器 AxonHub | `v1.0.0-beta4` / `7122f32994d9131e63a4217be3d58d33f187c350` | 三层模型命名、一个渠道多个 API Format Endpoint、Profile 过滤、模型同步、自适应负载均衡 | 配置层级较多；公开模型与运行时可用性仍需更严格的统一口径 |
| New API | `923a17ca8a3f08878d583ab4203190d2bee12c93` | `group/model/channel_id` Ability 索引、Token Group、Priority、Weight、上游模型变更检测 | Channel、协议和 Key 耦合较重；随机加权和按 Retry 切 Priority 不作为新核心算法 |
| Sub2API | `57914967cbb127ff715719c3879d881c10d75274` | 账号并发租约、Quota 门控、Sticky、模型映射、受限模型发现和失败诊断 | `/v1/models` 主要汇总账号映射键并可回退静态目录；Group、展示列表与调度规则存在平行配置 |

许可证边界：

- AxonHub 大部分目录为 Apache-2.0，`llm/` 为 LGPL-3.0。
- New API 为 AGPL-3.0。
- Sub2API 为 LGPL-3.0。
- 本项目默认只做 clean-room 行为重实现，不复制上述项目的受限实现代码。

### 2.1 最终组合方式

```text
AxonHub
  -> Upstream / Endpoint / Public Model 的概念分层

New API
  -> Access Group + Public Model + Candidate 的预编译索引

Sub2API
  -> Credential Lease + Quota + Concurrency + Sticky Runtime State

本项目
  -> Rust Canonical Pipeline + Immutable RouteSnapshot + 原子发布
```

## 3. 四层边界

### 3.1 Provider Adapter

Provider Adapter 是编译期代码，负责理解一种真实上游协议和语义，例如：

- `openai-compatible.responses`
- `openai-compatible.chat`
- `anthropic-compatible.messages`
- `grok.official`
- `grok.build`
- `grok.web`
- `kiro`

它处理请求编码、响应解码、流式状态机、错误分类、模型发现和凭据刷新，但不决定客户端能看到什么模型，也不决定该请求最终选择哪个中转站。

### 3.2 Upstream 与 Endpoint

`Upstream` 是一个配置出来的上游实例，例如“公益站 A”“本机 CPA”“Krill”或“xAI 官方”。

`UpstreamEndpoint` 是该实例的一条具体协议表面。一个 Endpoint 只声明一种上游 API Format，但多个 Endpoint 可以共享 Base URL 和凭据。

```text
Upstream: station-a
  ├── Endpoint: station-a-responses
  │     adapter = openai-compatible.responses
  │     base_url = https://station-a.example/v1
  │     inference_path = /responses
  │     models_path = /models
  ├── Endpoint: station-a-chat
  │     adapter = openai-compatible.chat
  │     base_url = https://station-a.example/v1
  │     inference_path = /chat/completions
  │     models_path = /models
  └── Endpoint: station-a-anthropic
        adapter = anthropic-compatible.messages
        base_url = https://station-a.example
        inference_path = /v1/messages
        models_path = /v1/models
```

“一个 Endpoint 同时支持三种协议”在配置层拆成三条 Endpoint。这样不会发生请求从 Responses 错送到 Chat 路径，健康、探测和能力也能分别记录。

### 3.3 Public Model 与 Model Route

`PublicModel` 是客户端看到的稳定模型名，例如 `minimax-m3`。

`ModelRoute` 定义该公开模型的调度策略；`RouteCandidate` 定义一个具体可尝试路径：

```text
PublicModel: minimax-m3
  -> ModelRoute
       ├── Candidate A: station-a-responses / minimax-m3
       ├── Candidate B: station-b-responses / MiniMax-M3
       └── Candidate C: station-c-anthropic / minimax-m3
```

Candidate 至少固定：

```text
(public_model, upstream, endpoint, upstream_model, credential_scope,
 request_capability, transform_mode, priority, weight)
```

### 3.4 Client Key 与 Access Group

`ClientKey` 是本项目签发给客户端的密钥。它绑定一个 `AccessGroup`，后者控制：

- 可见 Public Model。
- 可使用的 Model Route。
- 可选的 Endpoint、Upstream 或 Tag 限制。
- 请求额度、RPM、并发和到期时间。

第一版不需要复制 AxonHub 的多 Profile 体系。一个 Key 绑定一个生效 Access Group 足以覆盖自用场景；Profile 切换以后再扩展。

## 4. 核心数据模型

### 4.1 配置实体

| 实体 | 核心字段 | 说明 |
|---|---|---|
| `Upstream` | `id, name, kind, enabled, tags, egress_policy_id` | 中转站或官方上游实例 |
| `UpstreamEndpoint` | `id, upstream_id, adapter_id, api_format, base_url, inference_path, models_path, transport, enabled` | 一条具体上游协议表面 |
| `UpstreamCredential` | `id, upstream_id, kind, ciphertext, key_version, status, revision` | 加密保存的上游凭据 |
| `EndpointCredentialBinding` | `endpoint_id, credential_id, enabled, priority, weight, concurrency` | 决定哪些凭据可用于哪个 Endpoint |
| `PublicModel` | `id, model_name, status, display_name, capabilities` | 客户端稳定模型视图 |
| `ModelAlias` | `alias, public_model_id` | 仅负责入口别名，不直接指向上游 |
| `ModelRoute` | `id, public_model_id, policy, max_attempts, bootstrap_timeout` | 公开模型的候选集合与策略 |
| `RouteCandidate` | `id, route_id, endpoint_id, upstream_model, transform_mode, priority, weight, capability_override` | 一条真实可调度路径 |
| `AccessGroup` | `id, name, status, limits` | 客户端访问和配额边界 |
| `AccessGroupRoute` | `access_group_id, route_id, enabled` | Group 可使用哪些路由 |
| `ClientKey` | `id, prefix, secret_digest, access_group_id, status, expires_at` | 对外密钥只保存摘要 |

### 4.2 发现与运行时实体

| 实体 | 核心字段 | 说明 |
|---|---|---|
| `ModelDiscoveryRun` | `endpoint_id, credential_id, started_at, status, http_status, error_class, etag` | 每次模型同步尝试 |
| `DiscoveredModel` | `endpoint_id, credential_id, model_id, first_seen_at, last_seen_at, state` | 必须保留到凭据粒度 |
| `CatalogSnapshot` | `endpoint_id, credential_id, version, observed_at, stale_at, expires_at` | 最后成功模型快照 |
| `HealthSnapshot` | `endpoint_id, credential_id?, model_id?, state, score, observed_at` | Endpoint、凭据或模型健康状态 |
| `QuotaSnapshot` | `credential_id, model_id?, windows, source, confidence, observed_at` | Quota 与恢复时间 |
| `RuntimeLease` | `credential_id, request_id, acquired_at` | 内存中的并发占用，不逐请求落表 |

`DiscoveredModel` 不能只存 Endpoint 的全局并集。同一个中转站的不同 Key 可能有不同模型权限；调度必须知道哪个 Credential 实际看到了该模型。

## 5. 模型发现与目录生命周期

### 5.1 支持的目录来源

每个 Endpoint 可选择：

- `static_only`：只使用人工配置模型。
- `openai_list`：读取 `data[].id`。
- `anthropic_list`：读取 Anthropic 模型列表结构。
- `gemini_list`：读取 `models[].name` 并规范化 `models/` 前缀。
- `custom_json`：管理端明确配置受限 JSON Path；第一版可推迟。
- `provider_native`：由 Grok、Kiro 等专用 Adapter 查询真实目录。

静态模型和发现模型使用集合并集，但保留来源：

```text
effective_catalog = manual_models ∪ accepted_discovered_models
```

### 5.2 发现流程

```text
Scheduler
  -> per endpoint + credential Singleflight
  -> URL / Egress Policy 校验
  -> 受限超时与响应体大小
  -> 鉴权 Header 注入
  -> 解析并规范化模型 ID
  -> 写 ModelDiscoveryRun
  -> 生成新的 CatalogSnapshot
  -> 计算 added / suspected_removed / unchanged
  -> 按策略接受或等待人工确认
  -> 触发 RouteSnapshot 重编译
```

默认安全策略：

- 单次发现失败不清空最后成功快照。
- 新模型进入 `discovered`，默认不自动创建 Public Model 或 Route。
- 可对可信 Upstream 开启 `auto_accept_additions`。
- 模型从上游列表消失一次时进入 `suspected_removed`，不立即删除。
- 建议默认在连续 3 次成功同步均缺失且至少经过 24 小时后，才将发现模型标记为 `removed`。
- 人工配置模型永不被自动删除。
- Alias 或 Mapping 的源模型不因上游列表缺失而删除；只验证其目标是否仍存在。

### 5.3 快照新鲜度

目录状态分为：

```text
fresh -> stale -> expired
```

- `fresh`：正常参与 RouteSnapshot 编译。
- `stale`：保留使用并在管理端告警；适合上游模型接口短时故障。
- `expired`：不再用于自动生成新 Candidate；已有人工 Route 是否保留由 Route Policy 决定。

最后成功快照必须包含时间和来源，不能把静态回退伪装成实时发现。

## 6. 公开模型、Alias 与冲突规则

### 6.1 固定解析顺序

```text
客户端模型名
  -> 精确 PublicModel 匹配
  -> 精确 ModelAlias 匹配
  -> 得到唯一 PublicModel ID
  -> 获取 AccessGroup 可见 Route
  -> 根据请求能力筛选 RouteCandidate
  -> Candidate 内写入真实 upstream_model
```

第一版不支持正则 Alias。正则更适合管理端批量生成显式 Alias，避免请求热路径出现难解释的优先级和捕获替换。

### 6.2 编译期拒绝的冲突

- Public Model 名称重复。
- Alias 与另一个 Public Model 名称冲突。
- Alias 指向 Alias 或形成循环。
- 同一 Route 中完全相同的 `(endpoint, upstream_model, credential_scope)` 重复。
- Candidate 引用不存在或禁用的 Endpoint。
- Candidate 指定的上游模型不在人工目录或未过期的发现快照中，除非显式开启 `allow_unlisted_model`。
- Endpoint API Format 无法满足 Candidate 声明的能力。
- Access Group 引用了不可发布的 Route。

发现模型永远不能静默覆盖已有 Public Model、Alias 或 Route；它只能生成待审核差异。

## 7. 多协议与同一渠道多接口

### 7.1 Endpoint 是协议能力边界

每条 Endpoint 声明：

- `api_format`：例如 `openai/responses`、`openai/chat_completions`、`anthropic/messages`。
- `transport`：HTTP、SSE 或 WebSocket。
- `request_types`：Chat、Embedding、Image 等。
- `semantic_capabilities`：Tool、Parallel Tool、Reasoning、JSON Schema、Vision、Streaming。
- `models_path` 和模型发现格式。
- Header、Body 与错误分类策略。

同一 Upstream 的不同 Endpoint 分别探测和熔断。Responses 端点故障不能自动把 Anthropic 端点也标记为失败。

### 7.2 转换模式

Candidate 的基础 `transform_mode` 有三种；P12 原生 Provider 另增加显式的 `canonical_bridge` 扩展：

| 模式 | 含义 | 默认优先级 |
|---|---|---:|
| `passthrough` | 入站与上游格式一致，只做安全的模型/Header 改写 | 最高 |
| `canonical` | 通过本项目 Canonical Request/Event 正常转换 | 正常 |
| `lossless_bridge` | 跨协议桥接，只有能力分析证明无语义丢失时可用 | 最低 |
| `canonical_bridge` | 原生协议走 Canonical，其他注册协议走已审查的 lossless bridge | P12 原生 Provider |

不允许因为某个中转站宣称“同一模型支持三种协议”，就隐式放宽既有模式或复制账号池。P12 原生 Provider 只能使用显式 `canonical_bridge`，并分别验证三种入口的能力结果。

跨协议时至少检查：

- Tool 定义和 Tool Call 是否能完整表达。
- Tool ID、并行 Tool、空参数和流式 JSON 是否兼容。
- Reasoning/Thinking 是否允许保留、转换或明确丢弃。
- Structured Output、Stop、Usage 和缓存字段是否等价。
- 当前请求是否包含目标协议无法表达的未知字段。

无法证明等价时排除 Candidate，而不是静默删字段。

## 8. 路由、轮询与 Failover

### 8.1 两阶段调度

```text
Stage 1: Route Candidate
  选择哪个中转站、Endpoint 和上游模型

Stage 2: Credential Lease
  在该 Endpoint 绑定的凭据池中选择具体 Key/账号
```

这避免“一个站有 10 个 Key、另一个站只有 1 个 Key”时，按 Key 轮询导致第一个站获得十倍流量。中转站权重和站内凭据权重分别计算。

### 8.2 候选筛选顺序

1. Public Model 与 Access Group。
2. 请求类型、入站协议和语义能力。
3. Upstream、Endpoint、Candidate 是否启用。
4. Catalog 是否允许该上游模型。
5. Credential 是否存在且可用于该模型。
6. Response Ownership、Web Conversation 等强连续性。
7. Cache/Session Affinity。
8. Credential、Quota、Cooldown、Circuit 和并发状态。
9. Priority Tier。
10. Route Policy 和 Weight。
11. 获取 Credential Lease 后执行。

### 8.3 策略

第一版内建：

- `round_robin`：同一 Priority 内等权轮询。
- `smooth_weighted_round_robin`：同一 Priority 内按 Weight 平滑轮询，作为默认策略。
- `priority_failover`：先耗尽高优先级层，再进入下一层。

后续策略：

- `fill_first`：优先耗尽订阅或免费额度。
- `least_loaded`：根据实时并发与排队选择。
- `cost_aware`：在健康和能力满足后按成本优化。

Rust 热路径不使用全局调度锁。Route Compiler 为每个 Priority 预生成有界加权调度序列，运行时使用原子 Cursor；不可用 Candidate 被跳过，同一请求通过 Attempt Exclusion Set 防止重复尝试。

### 8.4 Failover 边界

- 只允许在 `FirstSemanticEvent` 前透明切换。
- 每个 Route 限制总 Attempt、不同 Endpoint、不同 Upstream 和累计 Bootstrap 时间。
- 请求错误、Schema 错误和不可安全重放的副作用不重试。
- 同一公开模型跨 Provider 或跨协议 Failover 必须在 Route 中显式列出。
- 流已经开始后只能发送目标协议允许的错误并结束，不能重放到另一站。
- 强连续性状态存在但原账号不可用时返回 Continuity 错误，不伪造新会话。

## 9. `/v1/models` 的唯一口径

公开模型列表从同一个 `RouteSnapshot` 生成，不另维护展示列表。

一个模型对某个 Client Key 可见，当且仅当：

1. Public Model 已启用。
2. Client Key 的 Access Group 允许该 Route。
3. 至少有一个 `hard-eligible` Candidate：Upstream、Endpoint、Route、目录和至少一个 Credential 均处于可发布状态。
4. 对协议特定的模型列表视图，至少一个 Candidate 能满足该入站协议或无损转换要求。

临时 429、短 Cooldown、瞬时 Circuit Open 不立即从 `/v1/models` 隐藏模型，避免客户端模型列表抖动。它们属于 `runtime-available` 状态，由请求调度和管理诊断接口处理。长期 Unauthorized、Forbidden、过期目录或人为禁用会触发 RouteSnapshot 重编译并移除模型。

对外响应只显示 Public Model 名称；不暴露 Upstream、Endpoint、Credential ID 或真实模型名。管理接口可以返回扩展可用性矩阵。

## 10. 自有 API Key

### 10.1 Key 格式与存储

建议格式：

```text
rgw_<public_prefix>_<random_secret>
```

- 创建时只返回一次完整 Key。
- 数据库保存可检索 Prefix 和 `HMAC-SHA256(server_pepper, full_key)`。
- 验证使用常量时间比较。
- 服务端 Pepper 与数据库分离，通过 systemd Credential 或只读 Secret 文件加载。
- 日志只记录 `client_key_id` 和 Prefix，不记录完整 Key。

### 10.2 鉴权后的路由视图

鉴权后直接取得：

```text
ClientKeyView {
  client_key_id,
  access_group_id,
  allowed_route_snapshot,
  quota_policy,
  rate_limit_policy
}
```

它随 RouteSnapshot 一起预编译。请求热路径不查询 ClientKey、AccessGroup 或模型权限表。

## 11. 控制面与热路径

### 11.1 RouteSnapshot

```text
RouteSnapshot {
  version,
  client_keys: key_prefix -> ClientKeyView,
  public_models: public_model_name -> PublicModelView,
  routes: (access_group, public_model, request_class) -> CandidatePlan,
  endpoint_catalogs,
  compiled_weighted_schedules
}
```

`ArcSwap<RouteSnapshot>` 原子发布。每个请求只读取一次 Snapshot，并在整个请求生命周期保持该版本，避免流式请求中途配置变化。

### 11.2 发布事务

```text
Management Change
  -> 写入候选 Config Version
  -> 完整 Schema / URL / Alias / Route / Capability 校验
  -> 编译 RouteSnapshot
  -> 校验至少一条管理与健康路径仍可用
  -> 提交 Active Config Version
  -> ArcSwap 原子发布
  -> 发 ConfigPublished 事件
```

任何编译错误都拒绝发布，不把半套配置暴露给数据面。重启时从 Active Config Version 重建快照；保留上一版本用于一键回滚。

### 11.3 动态状态

以下状态不要求每次变化都重编译 RouteSnapshot：

- 并发计数和等待队列。
- Round-robin Cursor。
- 短期 Cooldown 与 Circuit。
- Health EWMA。
- Quota 窗口。
- Cache Affinity。

它们保存在按 Upstream/Endpoint/Credential 分片的 Runtime State 中。长期状态变化通过事件异步触发快照重编译。

## 12. 安全边界

### 12.1 上游 Secret

- 使用 AEAD 加密落盘，建议 `XChaCha20-Poly1305` 或 `AES-256-GCM`。
- 每条 Secret 使用独立 Nonce，并保存 `key_version` 以支持主密钥轮换。
- 主密钥与 SQLite 分离；备份数据库不等于备份主密钥。
- Authorization、`x-api-key`、Cookie、OAuth JSON 和自定义 Secret Header 默认全部脱敏。

### 12.2 自定义 Base URL 与 SSRF

每个 Upstream 绑定 `EgressPolicy`：

- 默认只允许 HTTPS。
- Host、端口、CIDR 和 DNS 解析结果都要校验。
- 默认拒绝私网、链路本地和云元数据地址。
- 本机 CPA、Kiro-RS、grok2api 等地址通过显式私网 Allowlist 放行。
- Redirect 默认关闭；开启时只允许同源或重新执行完整校验。
- 模型列表和推理请求使用相同 EgressPolicy。
- 限制响应体、Header、连接时间和模型 ID 长度。

## 13. 管理与诊断接口

第一版管理 API 至少包括：

```text
POST   /admin/upstreams
POST   /admin/upstreams/{id}/endpoints
POST   /admin/upstreams/{id}/credentials
POST   /admin/endpoints/{id}/test
POST   /admin/endpoints/{id}/models/discover-preview
POST   /admin/endpoints/{id}/models/discover-apply

POST   /admin/public-models
POST   /admin/public-models/{id}/routes
POST   /admin/routes/{id}/candidates
POST   /admin/routes/{id}/validate
GET    /admin/routes/{id}/explain

POST   /admin/access-groups
POST   /admin/client-keys
DELETE /admin/client-keys/{id}

GET    /admin/catalog/status
GET    /admin/runtime/availability
GET    /admin/requests/{request_id}/attempts
```

`route explain` 输入 Client Key、模型、入口协议和请求能力，返回：

- 命中的 Public Model 与 Alias。
- 所有 Candidate。
- 每个 Candidate 的保留或排除原因。
- Priority、Weight、Affinity、Quota 和 Health 决策。
- 最终 Endpoint、上游模型和脱敏 Credential ID。

## 14. `minimax-m3` 配置示例

假设三个中转站都提供 `minimax-m3`：

```yaml
public_model: minimax-m3
route:
  policy: smooth_weighted_round_robin
  max_attempts: 3
  bootstrap_timeout_ms: 20000
  candidates:
    - endpoint: station-a-responses
      upstream_model: minimax-m3
      transform_mode: passthrough
      priority: 0
      weight: 100
    - endpoint: station-b-responses
      upstream_model: MiniMax-M3
      transform_mode: canonical
      priority: 0
      weight: 100
    - endpoint: station-c-anthropic
      upstream_model: minimax-m3
      transform_mode: lossless_bridge
      priority: 1
      weight: 100
```

行为：

1. Responses 请求优先在 A、B 之间平滑轮询。
2. A 或 B 在首事件前失败时尝试同层另一个 Candidate。
3. C 只有在请求能无损桥接到 Anthropic，并且高优先级层不可用时才参与。
4. 每个站内部再轮询其绑定的可用 Credential。
5. 客户端始终请求并看到 `minimax-m3`，Usage 同时保存真实 Endpoint 与 upstream model。

如果希望三个站严格等价轮询，应为三者配置同一种已验证协议、相同 Priority 和相同 Weight；不要把未经验证的跨协议桥接伪装成等价候选。

## 15. 与现有服务器项目的关系

现有服务可以按两种方式接入：

| 服务 | 推荐接入方式 | 原因 |
|---|---|---|
| CPA | `openai-compatible.responses` Upstream Endpoint | 继续复用现有 Grok/Codex 凭据和代理能力 |
| grok2api | 首期作为兼容 Upstream；后续原生 `grok.web/build` Adapter | 先稳定迁移流量，再逐步吸收专有能力 |
| Kiro-RS | 首期可作为 Anthropic-compatible Upstream；原生 `kiro` Adapter 完成后切换 | 保持现有可用链路和回滚路径 |
| AxonHub | 仅作行为对照或临时上游，不形成长期网关套网关依赖 | 新项目最终应直接拥有路由与观测状态 |
| New API | 可通过 OpenAI/Anthropic-compatible Endpoint 临时接入 | 迁移期复用现有渠道，不把其数据库当新项目数据源 |

上线迁移顺序：

1. 只读导入现有渠道元数据，Secret 单独重新加密。
2. 对每条 Endpoint 做 `/models`、最小非流式、SSE 和 Tool 探测。
3. 先创建不公开的 Public Model 与 Route。
4. 使用测试 Access Group 和 Client Key 做差分流量。
5. 验证模型名、协议、Usage、TTFT、缓存和 Failover。
6. 再把生产客户端切到新 Base URL；现有网关保留固定版本作为回滚。

## 16. 实现切片

### M0-A：聚合数据骨架

- SQLite Schema：Upstream、Endpoint、Credential、Public Model、Route、Access Group、Client Key。
- Secret AEAD、Client Key 摘要、EgressPolicy。
- Config Version、Route Compiler 和 `ArcSwap` 发布。

### M0-B：最小可用聚合链路

- 两个 OpenAI-compatible Responses Endpoint。
- 一个公开模型映射到两个 Candidate。
- 自有 Client Key、`/v1/models`、`POST /v1/responses`。
- 等权轮询、并发租约、首事件前 Failover。

### M0-C：目录与诊断

- `/v1/models` 自动发现、最后成功快照、差异预览。
- Route Explain、Attempt、健康和模型同步诊断。
- Access Group 模型过滤。

### M0-D：多协议

- Anthropic-compatible Messages Endpoint。
- Protocol View、Capability Filter 和无损桥接门禁。
- 同一 Upstream 多 Endpoint 的独立健康和故障状态。

完成 M0 后再按既定顺序进入 Grok Build、Kiro、Grok Official 和 Grok Web 专用 Adapter；它们复用同一聚合控制面，不另建平行路由系统。

## 17. 验收用例

- 两个中转站都提供 `minimax-m3`，连续 100 次请求分布符合配置权重。
- 同一站两个 Key 不改变站与站之间的目标流量比例。
- Responses 与 Anthropic Endpoint 使用同一 Base URL 时仍分别探测、熔断和统计。
- 某 Key 无该模型权限时不会被该模型调度，但同站其它 Key 仍可用。
- 一次模型同步失败不会清空公开模型。
- 模型连续消失达到 Removal Policy 后，RouteSnapshot 才移除相关 Candidate。
- Client Key 只能看到 Access Group 允许且存在 hard-eligible Candidate 的模型。
- 429、Quota、403、连接失败分别产生可解释排除原因。
- 首事件前可以 Failover；首事件后绝不透明重放。
- 日志、错误、导出和备份中不出现完整客户端 Key 或上游 Secret。
- 配置中出现 Alias 冲突、无效 Endpoint 或悬空 Route 时整版拒绝发布。
