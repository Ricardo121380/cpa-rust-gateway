# Grok 与 Kiro 渠道参考实现分析

本文把 Grok、Kiro 的参考实现转化为新 Rust 网关的设计输入。目标不是拼接现有项目，也不是逐文件翻译，而是提取已经被真实流量验证过的行为、边界条件和失败语义。

## 1. 冻结的参考快照

| 项目 | 本次参考快照 | License | 使用方式 |
|---|---|---|---|
| CLIProxyAPI（CPA） | `v7.2.80` | MIT | Grok Responses/Anthropic 兼容、Thinking、Reasoning Replay、模型 Alias、凭据状态行为 |
| chenyme/grok2api | `v3.0.0`，commit `ec6cddca7d2454996540adbf994f3c3d4ed2d2a1` | MIT | Grok Build/Web 双 Provider、账号/额度/会话/出口池、安全存储 |
| 服务器定制 Kiro-RS | commit `c49c75eb4f35af74714081505841cceb68b33d9d` | MIT | Kiro CLI/IDE、Anthropic 兼容、AWS EventStream、服务器补丁行为 |
| looplj/axonhub | `v1.0.0-beta4`，commit `7122f32994d9131e63a4217be3d58d33f187c350` | 主体 Apache-2.0；`llm/` LGPL-3.0 | 上游聚合、多 Endpoint、模型关联和 API Key Profile 的 clean-room 行为参考 |
| QuantumNous/new-api | `main` commit `923a17ca8a3f08878d583ab4203190d2bee12c93` | AGPL-3.0 | 只参考公开接口行为、模型后缀与 xAI 官方 API 适配，不复制代码 |
| Wei-Shaw/sub2api | `main` commit `57914967cbb127ff715719c3879d881c10d75274` | LGPL-3.0 | 只参考公开行为与架构思想，不复制实现代码 |

CPA 的行为基线继续冻结在 `v7.2.80`。服务器上运行版本以后即使继续升级，也不自动改变本项目的验收口径；需要单独做差分分析后才更新基线。

如果以后直接移植 CPA、grok2api 或 Kiro-RS 的 MIT 代码，必须保留对应版权和 License。AxonHub、New API 与 Sub2API 默认只做 clean-room 行为重实现，避免无意引入 Apache/LGPL/AGPL 代码边界。

AxonHub、New API 与 Sub2API 的聚合专项结论不在本文重复展开，见 [上游聚合、统一模型与自有 API 设计](05-upstream-aggregation-design.md)。

## 2. 核心结论

### 2.1 Grok 不是一个单一 Provider

“Grok”只是面向用户的模型家族。其背后至少有三种完全不同的上游：

| Provider ID | 上游 | 凭据 | 主要状态 | 主要风险 |
|---|---|---|---|---|
| `grok.official` | xAI 官方 API | API Key | Quota Header、官方 Response ID | 成本、模型能力差异 |
| `grok.build` | Grok Build/CLI Responses | Device OAuth、OAuth JSON | Token Rotation、Billing、Response Ownership、Prompt Cache | OAuth 失效、免费额度、模型能力变化 |
| `grok.web` | grok.com Web/Console | SSO/Cookie | Conversation/Parent ID、网页额度、Statsig、出口会话 | 反机器人、网页协议漂移、出口指纹 |

三者必须使用独立的：

- 凭据池和状态机。
- 模型目录和能力矩阵。
- Quota/Billing 解析器。
- 错误分类器。
- Continuity/Cache 策略。
- 出站 HTTP/TLS 配置。

同一个公开模型名可以显式路由到多个来源，但不能因为都叫 Grok 就默认跨来源降级。只有路由规则明确列出候选来源时，Router 才能在首个语义事件前切换。

### 2.2 Kiro 是独立 Provider，不是 Anthropic-compatible URL

Kiro 的完整链路包含四层：

```text
Anthropic / Claude Code Ingress
  -> Canonical Request
  -> Kiro Conversation Request
  -> CLI or IDE Endpoint Transform
  -> AWS EventStream Decoder
  -> Canonical Event Stream
  -> Anthropic Egress
```

因此 `provider-kiro` 不能只实现一个 HTTP POST。它还必须负责凭据刷新、Region/Profile、端点指纹、Kiro 工具约束和 AWS EventStream 解码。

## 3. 各参考项目应吸收的长处

| 领域 | 主要参考 | 新项目取舍 |
|---|---|---|
| Responses/Anthropic 语义转换 | CPA | 吸收边界行为，但落入 Canonical Request/Event，不复制全连接 Translator 矩阵 |
| Grok Build OAuth 与 Responses | CPA + grok2api | CPA 的兼容修复与 Reasoning Replay，加上 grok2api 的独立账号池、模型同步和 Billing |
| Grok Web/Console | grok2api | 采用独立 Web Adapter、会话状态、Statsig、浏览器 TLS/出口池和反机器人分类 |
| 账号调度 | grok2api | 并发租约、Priority、Tier、Quota 门控、粘性和恢复探测；改成 Rust 分片状态与不可变候选快照 |
| OAuth 并发刷新 | grok2api + Sub2API | 每凭据 Singleflight、Token Version、条件更新，防止旧请求覆盖新 Token |
| 凭据故障分类 | CPA + Sub2API + Kiro-RS | 分离 Request、Credential、Account、Egress、Provider 五个故障作用域 |
| Prompt Cache | CPA + grok2api + Sub2API | 分离客户端 Cache Key、上游 Cache Identity、凭据亲和与 Reasoning Replay |
| Response/Conversation 连续性 | grok2api + CPA | Response Ownership 与 Web Conversation State 持久化，不能靠普通 Round-robin 猜测 |
| Kiro 端点 | 服务器定制 Kiro-RS | 保留 CLI/IDE 差异、Region、机器 ID、客户端版本和 `profileArn` 行为 |
| Kiro Tool/Thinking 修复 | 服务器定制 Kiro-RS | 纳入通用流状态机和 Kiro Adapter 回归测试，不保留 `-thinking` 重复模型展示 |
| xAI 官方 API 基础适配 | New API | 模型后缀、Search、Reasoning Effort 等简单兼容作为测试输入，不把其简单转发器当核心架构 |
| Chat 到 Responses 的安全桥接 | Sub2API | 只有字段可无损映射时才桥接；未知字段或工具语义不确定时拒绝静默转换 |
| Secret 与出口安全 | grok2api | AEAD 加密、密钥哈希、日志脱敏、SSRF 域名限制、出口会话隔离 |

## 4. Provider 能力拆分

grok2api 的“小能力接口”比一个包含所有方法的巨型 Provider Trait 更适合新项目。建议抽象为：

```rust
pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> ProviderId;
    fn capabilities(&self) -> ProviderCapabilities;
}

pub trait InferenceAdapter: ProviderAdapter { /* execute / execute_stream */ }
pub trait ModelCatalogSource: ProviderAdapter { /* list per credential */ }
pub trait CredentialRefresher: ProviderAdapter { /* refresh / revoke */ }
pub trait QuotaSource: ProviderAdapter { /* sync windows / billing */ }
pub trait CredentialImporter: ProviderAdapter { /* parse/export */ }
pub trait CredentialConverter: ProviderAdapter { /* e.g. SSO -> Build */ }
pub trait ContinuityPolicy: ProviderAdapter { /* affinity/ownership/replay */ }
```

一个 Adapter 只实现真实具备的能力。例如 `grok.web` 实现 Web Quota 与 Conversation State，但不伪装成支持 `/responses/compact`；`kiro` 实现 AWS EventStream，但不实现 xAI Billing。

## 5. Grok Provider Family 目标行为

### 5.1 路由与模型

Route Candidate 的主键至少包含：

```text
(public_model, provider_id, upstream_model, credential_id, capability)
```

选择顺序：

1. 公开模型和 Alias 解析。
2. 显式 Provider/账号池限制。
3. 模型、工具、模态、Reasoning、协议能力过滤。
4. Credential、Account、Egress、Quota 状态过滤。
5. Response Ownership 或 Conversation State 强绑定。
6. Cache/Session Affinity。
7. Tier、Priority、并发占用、剩余额度和权重排序。
8. 获取并发租约后执行。

模型同步必须按凭据保存结果，不能只保存一个 Provider 全局并集。对外 `/v1/models` 可以显示并集，但调度必须知道每个账号实际支持哪些模型。

### 5.2 四种连续性状态必须分开

| 状态 | Key | Value | 用途 |
|---|---|---|---|
| Cache Affinity | tenant + provider + model + cache identity | credential + egress | 提高服务端 Prompt Cache 命中率 |
| Response Ownership | tenant + response ID | provider + credential | `previous_response_id`、GET/DELETE Response |
| Reasoning Replay | tenant + provider + model + session | 加密 Reasoning/Tool 状态 | 无状态入口保持多轮推理连续性 |
| Web Conversation State | tenant + local response ID | account + conversation ID + parent ID | 继续 grok.com Web 会话 |

这四类记录不能共用一个模糊的 `session_id -> account` Map。

客户端传入的 `prompt_cache_key` 保留在 Canonical Request 中。Provider Adapter 可以生成稳定、租户隔离的上游 Cache Identity，但必须满足：

- 使用版本化 HMAC/Hash，不向上游泄露客户端原始标识。
- 至少隔离客户端 API Key、Provider 和上游模型。
- 相同输入稳定复现，禁止每轮随机生成。
- 记录“客户端 Key 指纹 -> 上游 Identity 指纹”的可诊断映射，不记录明文 Key。
- Provider 不支持 Cache Key 时不得伪造缓存命中。

### 5.3 凭据调度

从 grok2api 吸收：

- Build 与 Web 账号池完全分离。
- 每账号并发租约。
- Priority、Tier Order、Quota/Billing 门控。
- `prompt_cache_key` 粘性。
- 免费/付费额度耗尽后的不同恢复探测。
- 候选快照短 TTL + 状态变更主动失效。
- Response Ownership 强制命中原账号。

需要改进：

- 候选排序和并发计数放入分片 Runtime State，不在热路径逐账号查询数据库。
- 所有粘性记录增加租户隔离和原因字段。
- 路由失败返回结构化排除原因，而不是只有“没有可用账号”。
- 不允许同一公开模型无提示地从 Build 切到 Web；来源变化必须是显式 Route Policy。

### 5.4 错误分类

| 示例 | 作用域 | 默认动作 |
|---|---|---|
| 请求 JSON、工具 Schema 不合法 | Request | 立即返回，不重试 |
| OAuth `invalid_grant`、缺失 Refresh Token | Credential | 退出调度，等待重新授权 |
| 账号无模型权限、订阅被封 | Account/Model | 只禁用对应账号或模型能力 |
| Grok Web 403/反机器人 | Egress Session | 重建浏览器会话或切出口，不直接封账号 |
| 免费额度耗尽 | Account/Quota Window | 冷却到已知 Reset；无 Reset 时受控探测 |
| 普通 429/high traffic | Provider/Transient | 尊重 Retry-After；首事件前有限重试 |
| 连接、TLS、首字节前中断 | Egress/Provider | 首事件前切换候选 |
| 流已经输出后中断 | Stream | 输出目标协议错误并终止，不透明重放 |

错误分类必须使用结构化状态码、Header 和受限长度 Body 特征；不能只靠一个宽泛子串把 403 全部判成账号封禁。

### 5.5 Grok Web 特殊约束

`grok.web` 需要独立的浏览器出站会话：

- SSO、Cloudflare Cookie、User-Agent、TLS Profile 和出口节点共同形成会话指纹。
- 同一 Web Conversation 默认绑定账号与出口会话。
- Statsig 签名缓存按 method/path/环境版本隔离，403 后只失效相关项。
- Signer URL 必须 HTTPS、受信域名或显式 Allowlist，并阻止内网/重定向 SSRF。
- Web 额度的 REST 与 gRPC-Web 来源分别记录，附带 `source`、`observed_at` 和 `confidence`。
- Web 协议漂移时只熔断 `grok.web`，不能拖垮 `grok.build` 或 `grok.official`。

Web Function Tool 目前依赖 Prompt 注入和输出解析，不等同于原生 Tool Calling。对外能力元数据必须标记为 `emulated`；有严格 Tool Use 需求时优先路由 Build/Official。

## 6. Kiro Provider 目标行为

### 6.1 凭据类型

| 类型 | 必需字段 | 刷新 | 典型端点 |
|---|---|---|---|
| Social/Builder ID | access token、refresh token、expiry | Social OAuth Refresh | IDE |
| IdC/Enterprise | access token、refresh token、client id/secret、region | AWS SSO OIDC Refresh | IDE |
| Kiro API Key | `ksk_...` | 不刷新 | CLI |

每个凭据独立配置 `auth_region`、`api_region`、endpoint、machine ID、客户端版本和代理。Region 不是展示字段，它会改变 Token 刷新和 API Host。

### 6.2 CLI/IDE 端点

| 行为 | IDE | CLI |
|---|---|---|
| URL | `q.{region}.amazonaws.com/generateAssistantResponse` | `runtime.{region}.kiro.dev/` |
| Content-Type | JSON | `application/x-amz-json-1.0` |
| Target Header | 无 | `AmazonCodeWhispererStreamingService.GenerateAssistantResponse` |
| Origin | `AI_EDITOR` | `KIRO_CLI` |
| Thinking 包装 | 对支持模型可加入 IDE `thinking` wrapper | 仅保留真实 CLI `output_config.effort` |
| API Key Header | `tokentype: API_KEY` | `tokentype: API_KEY` |

端点差异由 `KiroEndpointPolicy` 处理；通用 Kiro Provider 不应出现散落的 `if endpoint == ...`。

### 6.3 `profileArn`

- Builder ID 占位 ARN 不进入不需要它的查询 Header。
- 上游生成请求明确要求时，按凭据类型在请求体注入有效 ARN。
- Enterprise 优先查询真实 Profile；失败时使用可识别的 Region-aware 回退值，并记录来源。
- `profileArn` 的查询、回退、注入和持久化必须产生审计事件。
- 不把“存在任意 ARN”误判为“已验证有效”。

### 6.4 模型与 Thinking

- `/v1/models` 动态合并启用凭据的上游 `ListAvailableModels`。
- 保存每凭据模型能力，不只保存并集。
- 单凭据查询失败不拖垮整个模型列表。
- 全部查询失败时可使用带时间戳的最后成功快照；静态内置快照只是最后回退。
- 不暴露重复 `-thinking` 模型；Thinking/effort 是请求能力。
- 显式 `output_config.effort` 优先，Anthropic budget 映射必须受模型能力约束。
- CLI 与 IDE 对 Thinking 字段的差异由 Endpoint Policy 决定。

### 6.5 Tool 与流式

必须把服务器定制 Kiro-RS 的补丁变成稳定回归契约：

- `EnterPlanMode`、`ExitPlanMode` 和任意无必填字段工具的空输入补 `{}`。
- 非空但未闭合 JSON 仍然报错，不能伪造成合法 Tool Call。
- 缺少必填参数的零字节 Tool Call 不执行；按兼容策略转为文本或明确错误。
- `AskUserQuestion` 输入规范化并满足 Claude Code Schema。
- Tool 名过长时可逆映射，Tool ID 跨协议稳定。
- 历史 `tool_use` / `tool_result` 成对验证。
- AWS EventStream 按帧长、Prelude CRC、Message CRC 和 Header 边界校验。
- 任意网络 Chunk 切分产生相同 Canonical Event。
- Reasoning、Text、Tool、Usage 的 Start/Delta/End 顺序保持一致。

### 6.6 相比 Kiro-RS 的改进

- 不再用“模型名包含 opus”作为唯一账号能力判断；使用每凭据动态模型能力。
- Balanced 不使用累计成功次数代替实时负载；改用并发租约、近期成功率和权重。
- 网络错误、账号 429、普通 429、额度耗尽和鉴权失败分别建状态。
- 冷却与禁用状态可查询、可持久化、带恢复原因，不依赖进程重启自愈。
- Token Refresh 使用每凭据 Singleflight，而不是所有凭据共享一把刷新锁。

## 7. 第一阶段渠道包与实现顺序

第一阶段最终目标锁定为：

```text
Grok Provider Family
  ├── grok.build
  ├── grok.official
  └── grok.web

Kiro Provider
  ├── IDE endpoint
  └── CLI endpoint

OpenAI-compatible Provider
```

实现顺序按风险拆成可独立验收的垂直切片：

1. `M0 Core + Aggregation`：Canonical Request/Event、Responses/Anthropic 入站、Upstream/Endpoint、Public Model/Route、Access Group/Client Key、Route Snapshot、Credential Lease、Attempt/Usage 事件；先用两个 OpenAI-compatible Responses Endpoint 验证聚合底座。
2. `M1 Grok Build`：OAuth/API 导入、Responses HTTP、模型/Billing、Reasoning Replay、Cache Affinity。
3. `M2 Kiro`：Social/IdC/API Key、CLI/IDE、AWS EventStream、Claude Code Tool/Thinking 回归。
4. `M3 Grok Official`：API Key、Responses、官方 Quota Header、Reasoning/Tool 能力。
5. `M4 Grok Web`：SSO、Web Conversation、Statsig、浏览器出口池、Web Quota。
6. `M5 Compatible Relay Expansion`：增加 Chat、更多模型发现格式和迁移 Import Adapter，不稀释前述 Provider 特有状态。

`M1` 到 `M4` 都属于第一阶段渠道包；顺序只表示开发和验收风险，不表示删除任何已确认渠道。

## 8. 第一阶段验收门槛

每个 Provider 切片至少通过：

- 非流式与流式最小请求。
- Claude Code 无参数 Tool、普通 Tool、并行 Tool 回归。
- 客户端取消和上游中断。
- 401 Refresh、403 分类、429/Quota、5xx 重试。
- 两凭据轮询、并发租约、Affinity 和首事件前 Failover。
- 动态模型列表与单凭据能力过滤。
- 不泄露 Token、Cookie、SSO、API Key 或原始 Cache Key。
- Attempt、TTFT、Usage、Cache、Quota 与路由原因可查询。
- 与 CPA/grok2api/Kiro-RS 的固定夹具差分测试。

在这些契约通过前，不接管服务器现有生产流量。
