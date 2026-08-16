# 关键行为与兼容性契约

本文件描述新网关必须先定义、再编码的行为。CPA 源码和线上响应只作为参考；最终以本文件确认后的契约为准。

## 1. 请求生命周期

```text
Accepted
  -> Authenticated
  -> Parsed
  -> Routed
  -> CredentialSelected
  -> UpstreamConnecting
  -> UpstreamAccepted
  -> FirstSemanticEvent
  -> Streaming
  -> Completed | Failed | Cancelled
```

### 不变量

1. 在 `FirstSemanticEvent` 前可以透明切换 Provider、模型或凭据。
2. 一旦向客户端发送任何语义事件，不再透明切换上游。
3. SSE Keepalive 注释不算语义事件，但必须记录是否已经提交 HTTP Header。
4. 客户端取消必须向上传播，停止 Provider 请求和 Usage 计时。
5. 每次实际上游尝试都产生独立 Attempt 记录；一个外部请求可以包含多个 Attempt。

## 2. 凭据状态机

建议状态：

```text
Active
Refreshing
CoolingDown
QuotaExceeded
Unauthorized
Forbidden
Disabled
Invalid
```

状态变化建议：

```text
Active --401--> Refreshing
Refreshing --success--> Active
Refreshing --invalid refresh token--> Unauthorized

Active --403 quota evidence--> QuotaExceeded
Active --403 permission/ban evidence--> Forbidden
Active --403 without account evidence--> Credential unchanged + EgressRejected
Active --429--> CoolingDown
Active --408/5xx--> CoolingDown(transient)

Any --operator disable--> Disabled
Disabled --operator enable--> Active or validation-required
```

### 已冻结的行为

- 无账号级证据的 403 归类为短期 `EgressRejected`，只影响对应出口/会话；不得直接把 Credential 标记为 `Forbidden`。
- `QuotaExceeded` 到已知 Reset 后进入受控探测；没有可靠 Reset 时按 Provider 策略限频探测，不立即恢复全量调度。
- `Unauthorized` 完全退出调度，直到重新授权或凭据刷新流程明确恢复成功。
- Unauthorized、Forbidden、Quota 和其它长期状态在重启后恢复；纯瞬时冷却是否恢复由状态的持久化级别决定。

## 3. 模型解析顺序

建议固定顺序：

1. 使用客户端 API Key 取得预编译的 Access Group 视图。
2. 解析入口协议、请求类型、模型名和语义能力要求。
3. 先精确匹配 Public Model，再查找精确 Model Alias。
4. 从 RouteSnapshot 获取 `(access_group, public_model, request_class)` 的 Candidate Plan。
5. 应用 Endpoint API Format、工具、模态、Thinking、流式和 Catalog 硬过滤。
6. 应用 Upstream、Endpoint、Credential、Quota、Cooldown、Circuit 和并发过滤。
7. 应用 Response Ownership、Web Conversation 等强连续性。
8. 应用 Session/Cache Affinity。
9. 先按 Priority Tier，再按 Route Policy 和 Weight 选择 Route Candidate。
10. 在 Candidate 的 Endpoint 凭据池内获取 Credential Lease。
11. 得到最终 Provider Adapter、Endpoint 和上游模型名。
12. 保存客户端模型名、Public Model、Candidate 和 Credential，用于响应与 Usage 回写。

### 不变量

- Alias 只解析到 Public Model，不直接指向 Provider、Endpoint 或真实上游模型。
- `/v1/models` 暴露的模型必须至少存在一个 `hard-eligible` Candidate；短期 429/Cooldown 不导致列表抖动。
- 响应中的模型名必须遵循同一条 force-mapping 规则。
- Usage 同时记录 requested model、Public Model、route alias、Upstream、Endpoint、API Format 和 upstream model。
- `grok.official`、`grok.build`、`grok.web` 是不同 Provider ID；相同品牌或模型名不代表可以互相降级。
- 跨 Provider 或跨协议 Failover 必须由 Route Policy 明确列出、通过语义能力门禁，并且只能发生在首个语义事件前。

## 4. Tool Call 流状态机

每个 Tool Call 独立维护：

```text
Declared
  -> ArgumentsStreaming
  -> ArgumentsComplete
  -> Emitted
```

必须支持：

- 多 Tool Call 交错到达。
- `arguments` 在任意字节位置切片。
- Unicode 和转义序列跨 Chunk。
- 无参数工具最终规范化为 `{}`。
- 空白参数规范化为 `{}`。
- 上游结束时未闭合 JSON 必须产生明确流错误，不能转发半个 Tool Call。
- Tool ID 在协议转换前后保持稳定映射。

### 回归样例

- `EnterPlanMode` 空参数。
- `ExitPlanMode` 空参数。
- 普通无参数 MCP 工具。
- 参数由 1 字节 Chunk 组成。
- 两个并行工具交错发送参数。
- 客户端在 Tool Call 中途取消。

## 5. SSE 事件契约

Canonical Event 至少包括：

```text
ResponseStart
MessageStart
TextDelta
ReasoningDelta
ToolCallStart
ToolCallArgumentsDelta
ToolCallEnd
UsageDelta
MessageEnd
ResponseEnd
StreamError
```

入口协议 Adapter 负责把 Canonical Event 编码为 Anthropic 或 OpenAI Responses 事件。

### 不变量

- 每个 Start 最多对应一个 End。
- `ResponseEnd` 后禁止再输出任何语义事件。
- Usage 可以在结束事件前更新，但最终值只能提交一次。
- 上游异常结束必须输出目标协议允许的错误事件；若 Header 尚未提交，可返回普通 HTTP 错误。
- Keepalive 不改变事件状态机。

### Responses WebSocket 投影

- `GET /v1/responses` 只接受经过 Client Key 鉴权的 text `response.create`；它不是 Realtime API。
- WebSocket 与 SSE 共享同一 Canonical Event 状态机、Provider capability、route/credential lease、
  Usage、stored response 和 exact continuity；WebSocket 不建立第二套 Provider 执行链。
- 每个 Responses lifecycle event 投影为一个 JSON text message，不包含 SSE `data:` framing。
- frame、fragment、message、event、byte、pending turn、write/idle/turn/session timeout 都必须有界；
  disconnect/Close/timeout/backpressure 必须取消 Canonical source 并释放 lease。
- downstream WebSocket 不代表 upstream Provider 必须使用 WebSocket；Provider-native transport 是独立能力。
- P13-10A 不支持 Realtime、`response.append`、binary/media、Chat/Messages WebSocket 或 browser Origin。

详细约束见 [BC-RESP-004](contracts/BC-RESP-004-public-responses-websocket.md)。

## 6. 重试和失败切换

### 可透明重试

- DNS/连接失败。
- TLS 握手失败。
- 首字节前连接中断。
- 可重试的 408、429、5xx。
- 401 刷新成功后的同凭据重试。
- 明确可切换凭据的 403/Quota 错误。

### 不可透明重试

- 已经发送客户端语义事件。
- 请求包含无法安全重放的外部副作用。
- 上游已经完成 Tool Call，但下游仅收到部分参数。
- 客户端已经取消。

### Attempt 上限

需要同时限制：

- 最大总尝试次数。
- 最大不同凭据数。
- 最大不同 Endpoint 数。
- 最大不同 Upstream 数。
- 最大不同 Provider 数。
- 最大累计首字节等待时间。

## 7. Session 与缓存契约

建议把两种粘性区分开：

```text
Conversation Affinity
Cache-Key Affinity
```

### Cache-Key Affinity

- 同一个 `prompt_cache_key` 优先选择原凭据和原 Provider。
- 原凭据不可用时允许 Failover，并产生 cache-affinity-broken 原因事件。
- 客户端 `prompt_cache_key` 必须原样保留在 Canonical Request 中。
- Provider Adapter 可以派生稳定、版本化、租户隔离的上游 Cache Identity，但不得每轮随机重写。
- 派生 Identity 至少隔离客户端 API Key、Provider 和上游模型，且不能向上游泄露原始 Key。
- 路由时不得把不同客户端 API Key 的缓存身份混在一起。

### Conversation Affinity

- 由入口 Adapter 从明确字段提取 Conversation Key。
- 不优先使用“前几条消息 Hash”作为长期身份；只能作为无字段时的弱回退。
- 粘性记录有 TTL 和容量上限。

## 8. Thinking 和 Reasoning

- 客户端显式 effort 优先于默认值。
- 不再通过复制 `-thinking` 模型名向客户端暴露同一模型。
- Provider Adapter 负责把 Canonical Thinking 转成厂商参数。
- Usage 必须区分“请求显式 effort”和“上游实际 reasoning tokens”。
- 缺少显式 effort 不等于模型没有进行推理。

## 9. Usage 口径

每个外部 Request：

- request_id
- client_key_id
- access_group_id
- requested_model
- public_model
- route_alias
- provider
- upstream_id
- endpoint_id
- upstream_api_format
- upstream_model
- final_auth_id
- attempt_count
- requested_at
- first_semantic_event_at
- completed_at
- status

每个 Attempt：

- attempt_id
- auth_id
- provider
- upstream_id
- endpoint_id
- upstream_api_format
- upstream_model
- started_at
- first_byte_at
- ended_at
- HTTP status/error class
- retry decision

Token 明细：

- input_tokens
- output_tokens
- reasoning_tokens
- cache_read_tokens
- cache_creation_tokens
- cached_tokens（若上游只提供这一口径）

缓存率必须先确认分母，不能把 output/reasoning token 混入 Prompt Cache 命中率。

## 10. Provider Family 隔离契约

### Grok

固定 Provider ID：

```text
grok.official
grok.build
grok.web
```

三者必须拥有独立的：

- Credential Schema 与账号池。
- 模型和能力目录。
- Quota/Billing 状态。
- HTTP/TLS/出口会话。
- 错误分类和熔断状态。
- Continuity Policy。

不变量：

- 一个来源的 401/403/429 不改变另外两个来源的健康状态。
- SSO 转换出的 Build OAuth 是一份有血缘关系的新凭据，不与原 Web SSO 共享运行时状态。
- 对外同名模型必须能解释最终命中了哪个 Provider、账号和上游模型。
- Web/Console Tool Emulation 不得被宣称为 Native Tool Calling。

### Kiro

Kiro 是独立 Provider；`ide` 与 `cli` 是同一 Provider 下的 Endpoint Policy。共享 Kiro Conversation 语义、模型能力和凭据调度，但请求 Header、URL、Origin、Thinking 包装和 `profileArn` 注入由 Endpoint Policy 决定。

## 11. Continuity 状态契约

以下状态必须使用不同命名空间或存储表：

```text
CacheAffinity
ResponseOwnership
ReasoningReplay
WebConversationState
```

### CacheAffinity

- Key：tenant + provider + upstream model + cache identity。
- Value：credential + optional egress + expires_at + reason。
- 只表示“优先选择”，除非 Provider 明确要求强绑定。

### ResponseOwnership

- Key：tenant + downstream response ID。
- Value：provider + credential + upstream response ID + expires_at。
- `previous_response_id`、Response GET/DELETE 必须先查询 Ownership。
- Ownership 存在但原账号不可用时返回明确的 continuity 错误，不得静默换账号伪造续接。

### ReasoningReplay

- 加密 Reasoning、Tool Call 和必要 Assistant 状态按 tenant/provider/model/session 隔离。
- 写入前验证 Provider 签名形态；读取后去重，禁止重复注入客户端已经携带的状态。
- 成功响应明确没有可重放状态时删除旧条目；Store 故障时保留旧条目并记录降级。
- 不把一个客户端 API Key 的 Reasoning 状态暴露给另一个 Key。

### WebConversationState

- 保存本地 Response ID 到 Web Account、Conversation ID、Parent Response ID 和 Egress Session 的映射。
- 后续请求必须命中同一账号；出口切换是否允许由 Web Adapter 明确判断。
- 状态过期或账号不可用时返回可诊断错误，不回退到新会话。

## 12. Grok 特有契约

### Quota 与错误

- Quota Snapshot 必须包含 `source`、`observed_at`、`reset_at`、`confidence` 和原始窗口类型。
- Billing、Response Header、Web REST、Web gRPC-Web 和本地估算不能互相冒充。
- Free、Pay-as-you-go、订阅月额度和 Web 周额度分别建模。
- Grok Web 403 默认先归类为 `EgressRejected`；只有存在账号级证据时才修改 Credential/Account 状态。
- OAuth `invalid_grant` 退出调度；普通 5xx 不得永久封账号。
- 429 必须区分免费额度耗尽、账号级限流和 Provider 高流量。

### Cache

- Cache Affinity 与上游 Cache Identity 同时稳定，单独稳定其中一个不算满足契约。
- Affinity 断裂时记录原账号、目标账号、原因和预估缓存损失。
- Free 账号的特殊上游路由技巧只能在 Capability Probe 验证后启用，并受 Feature Flag 控制；不得靠静默注入工具改变客户端可观察语义。

### Chat/Responses Bridge

- 只有请求字段、消息内容和工具语义都可无损转换时才允许桥接。
- 未知字段、Tool/Function Call、Stop、Reasoning 或结构化内容不能被静默丢弃。
- 无法证明等价时走原生入口或返回 Unsupported，不做“尽量转换”。

## 13. Kiro 特有契约

### Credential 与 Endpoint

- Social、IdC/Enterprise、`ksk_` API Key 是不同 Credential Kind。
- Token Refresh 按凭据 Singleflight；一个账号刷新不能阻塞整个 Kiro 池。
- `auth_region` 用于认证，`api_region` 用于推理 Host；两者不得混用。
- CLI/IDE Endpoint Policy 决定 URL、Content-Type、Target Header、Origin 和 Thinking 包装。
- Builder 占位 `profileArn` 不进入不需要它的查询 Header；生成请求要求时按已验证规则注入请求体。

### Model

- 每凭据保存 `ListAvailableModels` 和同步时间。
- `/v1/models` 只展示至少有一个可调度 Candidate 的模型。
- 单凭据同步失败不覆盖其它凭据的成功快照。
- 全部同步失败时优先使用最后成功快照；静态列表只是最后回退，并明确标记 stale。
- 不暴露 `-thinking` 重复模型；Thinking 是能力与请求参数。

### AWS EventStream

- 校验 Total Length、Header Length、Prelude CRC、Message CRC 和最大帧长。
- 不完整帧等待更多字节；损坏帧只能按明确恢复规则处理。
- 连续解析错误达到上限后输出 `StreamError`，不得继续猜测帧边界。
- Chunk 切分不影响最终 Canonical Event 序列。

### Tool Compatibility

- 无必填字段工具在流结束时可将空输入规范化为 `{}`。
- 非空但不完整 JSON 必须报错，不能自动补括号或执行。
- 缺少必填参数的 Tool Call 不转发执行。
- `AskUserQuestion`、Plan Mode 和 Tool Name 映射都有固定夹具回归测试。

## 14. Credential 并发更新契约

- 每个 Credential 有单调递增 Token Version 或等价 Revision。
- Refresh 结果使用条件更新；旧请求不能覆盖新 Token。
- 标记 Unauthorized/Forbidden/Quota 前，比较请求选中时的 Credential Revision。
- Revision 已变化时重新读取并重新分类，不执行破坏性状态更新。
- Refresh Lock 有超时；超时属于 Provider/State Store 故障，不自动封账号。
- Secret 持久化使用 AEAD，日志和审计只保存稳定 ID、指纹和脱敏摘要。

## 15. 统一错误分类

统一内部错误类别：

```text
ClientRequestError
ClientUnauthorized
RouteNotFound
CredentialUnavailable
CredentialUnauthorized
CredentialForbidden
CredentialQuotaExceeded
EgressRejected
EgressUnavailable
ProviderRateLimited
ProviderTransient
ProviderPermanent
UpstreamProtocolError
StreamTruncated
InternalError
Cancelled
```

Provider Adapter 只负责把上游响应分类成内部错误；入口 Adapter 再编码成 OpenAI/Anthropic 错误格式。

## 16. 差分测试要求

同一组脱敏请求分别发送到 CPA v7.2.80 和新网关，比较：

- 状态码和 Header 白名单。
- 非流式 JSON 语义。
- SSE 事件类型、顺序和终止状态。
- Tool ID、Name 和最终 Arguments。
- 模型 Alias 回写。
- Reasoning 和 Usage。
- 账号状态及重试轨迹。

差异分为：

- `Intentional`：新设计明确替换 CPA 行为。
- `Compatible`：字段顺序等非语义差异。
- `Regression`：新网关丢失或改变了要求保留的行为。

## 17. 上游聚合实体契约

固定边界：

```text
Provider Adapter
  = 编译期协议实现

Upstream
  = 某个中转站、官方服务或本机网关实例

Upstream Endpoint
  = 一种 API Format + Base URL + Path + Transport

Public Model / Model Route
  = 客户端模型名与可尝试 Candidate

Client Key / Access Group
  = 对外鉴权和模型权限
```

不变量：

- 一个 Endpoint 只声明一种上游 API Format。
- 同一 Upstream 可以有多个 Endpoint，并可共享 Credential。
- Responses、Chat Completions、Anthropic Messages 即使共用 Base URL，也必须是不同 Endpoint。
- Endpoint 级健康、探测和 Circuit 独立；一个协议故障不自动污染同站其它协议。
- Route Candidate 必须固定 Endpoint 和 upstream model，禁止在执行阶段靠模型同名猜协议。
- Candidate 选择和 Credential 选择分成两阶段，站内 Key 数量不改变站间流量比例。

## 18. 模型发现契约

- 模型发现按 Endpoint + Credential 执行和保存，不能只保存 Upstream 全局并集。
- 静态模型与已接受的发现模型并存，并保留来源。
- 单次发现失败只记录失败 Run，不覆盖最后成功 CatalogSnapshot。
- 新发现模型默认进入待审核状态，不自动创建 Public Model 或 Route。
- 上游列表一次缺失只标记 `suspected_removed`；达到连续成功缺失次数和最短隔离期后才移除。
- 人工模型、Alias 源和 Mapping 源不会被自动发现任务删除。
- Fresh、Stale、Expired 必须有明确时间；静态回退不能标记成实时发现。
- 模型 ID 必须限长、去空、去重并保留上游原始大小写；匹配规范化与实际下发名称分开。
- 模型发现与推理使用同一 Endpoint 鉴权、Header Override 和 EgressPolicy。

## 19. Public Model 与模型列表契约

解析优先级：

```text
exact PublicModel > exact ModelAlias
```

第一版不在热路径支持正则 Alias。以下配置必须拒绝发布：

- Public Model 重名。
- Alias 与任意 Public Model 名冲突。
- Alias 指向 Alias、循环或悬空。
- 完全相同的 Route Candidate 重复。
- Candidate 引用不存在的 Endpoint、Credential Scope 或模型。
- Access Group 引用不可发布 Route。

`/v1/models` 只从 RouteSnapshot 生成：

- 先按 Client Key 的 Access Group 过滤。
- 至少存在一个 `hard-eligible` Candidate 才显示 Public Model。
- 协议特定视图还需至少一个 Candidate 能原生处理或无损转换该协议。
- 短期 Runtime 状态不立即隐藏模型；长期 Unauthorized、Forbidden、目录过期或人为禁用会触发重编译。
- 公开响应只显示 Public Model，不泄露 Upstream、Endpoint、Credential 或 upstream model。

## 20. RouteSnapshot 与 Client Key 契约

- Config Version 通过完整校验并成功编译后才可设为 Active。
- `ArcSwap<RouteSnapshot>` 原子发布；请求从开始到结束固定使用同一版本。
- 请求热路径不查询 SQLite、YAML、模型接口或管理服务。
- 编译失败时保留上一 Active Version，不发布部分变更。
- Client Key 创建时只返回一次完整 Key；数据库保存 Prefix 和带服务端 Pepper 的单向摘要。
- Client Key 第一版绑定一个 Access Group；模型列表与推理鉴权读取同一个预编译权限视图。
- 上游 Secret 使用 AEAD；主密钥与数据库分离，并带 Key Version 支持轮换。
- 自定义 Base URL 必须经过 EgressPolicy；本机和私网地址只能通过显式 Allowlist 放行。

完整设计和验收示例见 [上游聚合、统一模型与自有 API 设计](05-upstream-aggregation-design.md)。
