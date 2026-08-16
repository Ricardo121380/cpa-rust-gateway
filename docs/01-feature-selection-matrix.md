# 功能筛选矩阵

## 使用方式

- “CPA 状态”说明参考版本中是否存在该能力。
- “建议”是当前基于高性能精简网关目标给出的初始意见，不代表用户已经确认。
- “最终决定”默认保持 `待定`；用户已经确认的范围以及开发计划 `v1.0` 冻结的核心决策直接写入 `Keep/Later/Replace/New`。
- `Replace` 表示保留业务目标，但重新设计实现和行为。
- 第一版范围锁定后，本文件将成为 Rust Workspace 的需求来源。

当前共拆分出 313 个功能点，初始建议分布如下：

| 建议 | 数量 |
|---|---:|
| Keep | 81 |
| Later | 41 |
| Drop | 44 |
| Replace | 36 |
| New | 111 |

截至开发计划 `v1.0`，最终决定分布如下；尚未冻结的项目继续保留为 `待定`，不得在开发中顺手实现：

| 最终决定 | 数量 |
|---|---:|
| Keep | 11 |
| Later | 6 |
| New | 72 |
| Replace | 7 |
| Drop | 0 |
| 待定 | 217 |
| **总计** | **313** |

已锁定的第一阶段渠道方向：

```text
Grok Provider Family（Official + Build + Web）
Kiro Provider（IDE + CLI）
OpenAI-compatible Provider
Upstream Aggregation（多中转站 + 多 Endpoint + 公开模型 + 自有 API Key）
```

## A. 接入接口与服务器能力

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| A01 | `GET /healthz` | 已有 | Keep | 待定 | 部署和负载均衡健康检查 |
| A02 | `GET /v1/models` | 已有 | Keep | 待定 | 依赖模型注册表和凭据可用性 |
| A03 | `POST /v1/responses` | 已有 | Keep | Keep | 第一核心入口 |
| A04 | `GET /v1/responses` WebSocket | 已有 | Later | Keep | `P13-10A`：OpenAI Responses WebSocket；复用 Canonical/认证/存储/连续性，不等同 Realtime API |
| A05 | `POST /v1/responses/compact` | 已有 | Later | 待定 | Codex 上下文压缩 |
| A06 | `POST /v1/alpha/search` | 已有 | Later | 待定 | Codex 专用搜索能力 |
| A07 | `POST /v1/messages` | 已有 | Keep | Keep | Claude Code 核心入口 |
| A08 | `POST /v1/messages/count_tokens` | 已有 | Keep | Keep | Claude Code 客户端兼容；仅在可证明准确时返回 |
| A09 | `POST /v1/chat/completions` | 已有 | New | New | `CR-P12-COMPAT-001` 前移到 Release 1；普通 OpenAI 客户端兼容 |
| A10 | `POST /v1/completions` | 已有 | Drop | 待定 | 旧式 Completion 协议 |
| A11 | Codex `/backend-api/codex/*` 别名 | 已有 | Later | 待定 | Codex CLI 原生路径兼容 |
| A12 | Gemini `/v1beta/models/*action` | 已有 | Drop | 待定 | 第一版不支持 Gemini 协议 |
| A13 | Gemini Interactions | 已有 | Drop | 待定 | 第一版不支持 Interactions |
| A14 | 图片生成/编辑 | 已有 | Drop | 待定 | 与文本代理主目标无关 |
| A15 | xAI/OpenAI 视频接口 | 已有 | Drop | 待定 | 大幅增加状态和文件处理范围 |
| A16 | 客户端 API Key 鉴权 | 已有 | Keep | 待定 | 数据面入口鉴权 |
| A17 | TLS 终止 | 已有 | Later | 待定 | 通常交给 Caddy；保留内建能力可选 |
| A18 | CORS | 已有 | Keep | 待定 | 管理 UI 和浏览器客户端需要 |
| A19 | 安全响应 Header 透传 | 已有 | Keep | 待定 | 必须采用白名单而非全量透传 |
| A20 | 非流式 Keepalive 空行 | 已有 | Drop | 待定 | 优先由反向代理超时配置处理 |
| A21 | SSE Keepalive | 已有 | Keep | 待定 | 长推理防止中间层空闲超时 |
| A22 | 首字节前流式 Bootstrap Retry | 已有 | Keep | 待定 | 与透明失败切换契约绑定 |
| A23 | 示例 API Key 安全模式 | 已有 | Replace | 待定 | 改成启动时拒绝不安全配置 |
| A24 | 根路径服务说明 | 已有 | Later | 待定 | 非核心 |

## B. 协议、内容和流式转换

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| B01 | 统一 Canonical Request | 无 | New | New | 替代协议对协议转换矩阵 |
| B02 | 统一 Canonical Event Stream | 无 | New | New | 文本、Reasoning、Tool、Usage、Error |
| B03 | OpenAI Responses 入站 Adapter | 已有直转 | Replace | Replace | 转换到 Canonical 模型 |
| B04 | Anthropic Messages 入站 Adapter | 已有直转 | Replace | Replace | Claude Code 兼容重点 |
| B05 | OpenAI Chat 入站 Adapter | 已有直转 | Replace | Replace | `CR-P12-COMPAT-001`；严格进入 Canonical 模型 |
| B06 | Gemini 入站 Adapter | 已有直转 | Drop | 待定 | 第一版不做 |
| B07 | Interactions 入站 Adapter | 已有直转 | Drop | 待定 | 第一版不做 |
| B08 | 非流式响应编码 | 已有 | Keep | Keep | Chat、Responses、Messages 三种输出 |
| B09 | SSE 响应编码 | 已有 | Keep | Keep | 必须保持事件次序和终止语义 |
| B10 | WebSocket 响应编码 | 已有 | Later | Replace | `P13-10A`：同一 Responses Canonical lifecycle 逐 JSON text frame 投影；有界背压/关闭/取消 |
| B11 | Function/Tool 定义转换 | 已有 | Keep | Keep | JSON Schema 兼容 |
| B12 | Tool Use 事件转换 | 已有 | Keep | Keep | Claude Code 核心 |
| B13 | Tool Result 转换 | 已有 | Keep | 待定 | 保留关联 ID 和错误语义 |
| B14 | 并行 Tool Call | 已有 | Keep | 待定 | 多个工具状态机并行维护 |
| B15 | 空参数 Tool 自动补 `{}` | 局部修复 | Replace | Replace | 对所有无参数工具统一处理 |
| B16 | Tool 参数跨 Chunk JSON 拼接 | 已有但有边界问题 | Replace | Replace | 需要增量解析和随机切片测试 |
| B17 | 文本输入输出 | 已有 | Keep | 待定 | 基础能力 |
| B18 | 图片输入 | 已有 | Later | 待定 | 与多模态需求绑定 |
| B19 | 音频/视频输入 | 部分 Provider | Drop | 待定 | 第一版不做 |
| B20 | Structured Output/JSON Schema | 已有 | Later | 待定 | 可在 Provider 能力层声明 |
| B21 | Web Search Tool | 已有 | Later | 待定 | Provider 能力差异较大 |
| B22 | Token Count 转换 | 已有 | Keep | 待定 | Anthropic count_tokens |
| B23 | Usage 字段转换 | 已有 | Keep | 待定 | 输入、输出、推理、缓存细分 |
| B24 | Stop Reason 转换 | 已有 | Keep | 待定 | Tool/length/end_turn 等 |
| B25 | 错误协议转换 | 已有 | Replace | 待定 | 统一内部错误分类后编码 |
| B26 | 响应模型名回写 | 已有 | Keep | 待定 | Alias force mapping |
| B27 | 未识别 JSON 字段保留 | 部分 | New | 待定 | 使用 `RawValue` 避免无意删除字段 |
| B28 | Chunk 边界无关性 | 无显式契约 | New | 待定 | 任意网络切片必须产生同一语义 |
| B29 | 客户端取消向上传播 | 已有 Context | Keep | 待定 | 及时释放上游连接和任务 |
| B30 | 有界流缓冲与背压 | 不明确 | New | 待定 | 禁止慢客户端导致无限内存增长 |

## C. Provider 和渠道

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| C01 | Grok Official API Key | 已有 | Keep | Keep | `grok.official` 独立凭据池 |
| C02 | Grok Build OAuth/Device Code | 已有 | Keep | Keep | `grok.build` 独立凭据池 |
| C03 | Grok Official Responses HTTP | 已有 | Keep | Keep | 官方 API Key 垂直链路 |
| C04 | Grok Official 上游 WebSocket | 已有 | Later | 待定 | 先验证 HTTP 性能和语义 |
| C05 | Codex OAuth | 已有 | Later | 待定 | 第一阶段之后的渠道候选 |
| C06 | Codex Device Code | 已有 | Later | 待定 | 依赖 C05 |
| C07 | Codex API Key/兼容上游 | 已有 | Later | 待定 | 依赖 Codex Provider |
| C08 | Claude OAuth | 已有 | Later | 待定 | 是否保留由用户决定 |
| C09 | Claude API Key | 已有 | Later | 待定 | Anthropic 原生上游 |
| C10 | Gemini API Key | 已有 | Drop | 待定 | 第一版不做 |
| C11 | Gemini Interactions Key | 已有 | Drop | 待定 | 第一版不做 |
| C12 | Vertex 凭据 | 已有 | Drop | 待定 | 第一版不做 |
| C13 | AI Studio 账号 | 已有 | Drop | 待定 | 第一版不做 |
| C14 | Antigravity | 已有 | Drop | 待定 | 第一版不做 |
| C15 | Kimi OAuth | 已有 | Drop | 待定 | 第一版不做 |
| C16 | 通用 OpenAI-compatible Provider | 已有 | Keep | Keep | 已确认进入第一阶段渠道包 |
| C17 | 编译期 Provider Trait | 无 | New | 待定 | 替代动态 Executor 插件 |
| C18 | Provider 能力描述 | 部分模型元数据 | Replace | 待定 | 协议、工具、模态、Thinking、流式 |
| C19 | Provider 自定义 Header | 已有 | Keep | 待定 | 渠道适配需要 |
| C20 | 全局出站代理 | 已有 | Keep | 待定 | HTTP/HTTPS/SOCKS |
| C21 | 单凭据出站代理 | 已有 | Keep | 待定 | 账号隔离需要 |
| C22 | 显式 Direct 连接 | 已有 | Keep | 待定 | 覆盖环境代理 |
| C23 | Provider Header/客户端指纹 | 已有 | Replace | 待定 | 按 Provider Adapter 隔离 |
| C24 | Claude Cloak | 已有 | Drop | 待定 | 除非确认需要非官方客户端伪装 |
| C25 | Claude CCH Signing | 已有实验能力 | Drop | 待定 | 高维护成本 |
| C26 | Provider 独立超时配置 | 部分 | New | 待定 | connect/first-byte/idle/total 分离 |
| C27 | Provider 健康与熔断 | 部分冷却 | Replace | 待定 | 凭据冷却和 Provider 熔断分开 |
| C28 | Grok Build Responses HTTP | 已有 | Keep | Keep | CPA 协议兼容 + grok2api 账号/Billing 能力 |
| C29 | Grok Web/Console SSO 凭据 | 无（grok2api 有） | New | New | `grok.web` 独立账号池 |
| C30 | Grok Web App Chat Adapter | 无（grok2api 有） | New | New | Web Conversation/Parent ID 状态 |
| C31 | Grok Official/Build/Web Provider 隔离 | 无 | New | New | 禁止共享凭据、Quota 和故障状态 |
| C32 | Grok Web SSO 转 Build OAuth | 无（grok2api/Sub2API 有） | Later | Later | 便利导入能力，不进入首条推理链路 |
| C33 | Grok 分来源模型与 Quota 同步 | 部分 | Replace | Replace | 按 Adapter、凭据和能力保存 |
| C34 | Grok Web REST/gRPC-Web Quota | 无（grok2api 有） | New | New | 周期、Tier、Breakdown 和来源可信度 |
| C35 | Kiro Provider | 无 | New | New | 已确认进入第一阶段渠道包 |
| C36 | Kiro Social/Builder ID OAuth | 无 | New | New | Social Token 刷新 |
| C37 | Kiro IdC/Enterprise 凭据 | 无 | New | New | OIDC、Client Secret、Profile |
| C38 | Kiro `ksk_` API Key | 无 | New | New | Headless/CLI 凭据 |
| C39 | Kiro IDE Endpoint | 无 | New | New | `q.{region}.amazonaws.com` |
| C40 | Kiro CLI Endpoint | 无 | New | New | `runtime.{region}.kiro.dev` |
| C41 | Kiro AWS EventStream 解码 | 无 | New | New | Frame、Header、CRC 和增量事件 |
| C42 | Kiro 动态模型列表 | 无 | New | New | 启用凭据真实模型并集 + 最后成功快照 |
| C43 | Kiro `profileArn` 生命周期 | 无 | New | New | 查询、回退、注入、持久化、审计 |
| C44 | Kiro 凭据级 Region/Machine/Client Version | 无 | New | New | Auth/API Region 和端点指纹分离 |
| C45 | Kiro 原生 MCP/WebSearch 调用 | 无 | Later | Later | 架构预留，核心 Claude Code Tool 先完成 |
| C46 | Kiro Endpoint Transform Policy | 无 | New | New | CLI/IDE Header、Origin、Thinking 差异 |
| C47 | Kiro 每凭据模型/订阅能力 | 无 | New | New | 替代按模型名猜账号能力 |

## D. 模型注册与路由

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| D01 | 全局模型注册表 | 已有 | Keep | 待定 | 模型到 Provider/凭据索引 |
| D02 | 按凭据动态注册模型 | 已有 | Keep | 待定 | 只暴露当前可调用模型 |
| D03 | 静态远程模型目录更新 | 已有 | Drop | 待定 | 首版使用配置和 Provider 查询 |
| D04 | 全局模型 Alias | 已有 | Keep | 待定 | 客户端统一模型名 |
| D05 | 单凭据模型 Alias | 已有 | Later | 待定 | 高级覆盖 |
| D06 | 模型 Prefix | 已有 | Later | 待定 | 显式指定 Provider/账号池 |
| D07 | 强制 Prefix 模式 | 已有 | Drop | 待定 | 可由明确路由规则替代 |
| D08 | Alias 响应回写 | 已有 | Keep | 待定 | 保持客户端模型视图稳定 |
| D09 | Excluded Models 通配符 | 已有 | Later | 待定 | 可用显式 allowlist 简化 |
| D10 | 相同 Alias 的上游模型池 | 已有 | Keep | 待定 | 同名模型轮询/回退 |
| D11 | 同一模型跨 Provider 路由 | 已有 | Keep | 待定 | 核心聚合能力 |
| D12 | 凭据 Round-robin | 已有 | Keep | 待定 | 基础策略 |
| D13 | Fill-first | 已有 | Later | 待定 | 订阅额度耗尽型场景 |
| D14 | 凭据 Priority | 已有 | Keep | 待定 | 公益/付费/免费分层 |
| D15 | Session Affinity | 已有 | Replace | 待定 | 为缓存与会话连续性重新设计 |
| D16 | 多种 Session ID 提取 | 已有 | Replace | 待定 | 变成入口 Adapter 明确字段 |
| D17 | 绑定凭据不可用时 Failover | 已有 | Keep | 待定 | 只能在首字节前透明发生 |
| D18 | 插件 Scheduler | 已有 | Drop | 待定 | 首版使用内置策略 Trait |
| D19 | 插件 Model Router | 已有 | Drop | 待定 | 首版使用静态编译路由规则 |
| D20 | Capability-aware Routing | 部分 | New | 待定 | 根据工具、模态、Thinking 选择渠道 |
| D21 | Weighted Routing | 无内置核心策略 | New | 待定 | 渠道权重和成本控制 |
| D22 | Least-loaded Routing | 无 | Later | 待定 | 需要并发和排队指标 |
| D23 | Cache-affinity Routing | 不完整 | New | 待定 | `prompt_cache_key` 到凭据稳定绑定 |
| D24 | 路由决策可解释输出 | 无稳定接口 | New | 待定 | 管理端显示为何选择/跳过账号 |
| D25 | 原子路由快照 | Go 锁/调度结构 | Replace | 待定 | Rust 使用不可变快照和 `ArcSwap` |
| D26 | Provider-specific Continuity Policy | 无 | New | New | Cache、Response、Replay、Web Conversation 分开 |
| D27 | Response Ownership 强绑定 | 局部 | New | New | `previous_response_id` 固定 Provider/凭据/租户 |
| D28 | Web Conversation 强绑定 | 无（grok2api 有） | New | New | 账号、出口、Conversation/Parent ID |
| D29 | Route 显式记录上游来源 | 部分 | New | New | `grok.official/build/web` 不靠模型名猜测 |
| D30 | 禁止 Grok 跨来源隐式 Failover | 无明确契约 | New | New | 只有 Route Policy 明确允许才切换 |
| D31 | 每凭据 Capability Candidate Index | 部分 | New | New | 模型、工具、模态、Reasoning、端点能力 |

## E. 凭据生命周期、错误和重试

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| E01 | JSON auth 文件加载 | 已有 | Keep | 待定 | 兼容现有导入数据 |
| E02 | 配置内 API Key 条目 | 已有 | Keep | 待定 | 静态渠道配置 |
| E03 | 凭据上传/下载/删除 | 已有 | Keep | 待定 | 管理 API |
| E04 | 凭据启用/禁用 | 已有 | Keep | 待定 | 必须立即影响调度 |
| E05 | 凭据字段修改 | 已有 | Keep | 待定 | Label、Proxy、Priority 等 |
| E06 | OAuth Token 自动刷新 | 已有 | Keep | 待定 | 后台任务 |
| E07 | 401 请求内刷新再重试 | 已有 | Keep | 待定 | 同一凭据串行刷新 |
| E08 | 凭据级状态机 | 已有 | Replace | 待定 | 明确 Active/Cooling/Unauthorized/Disabled |
| E09 | 凭据-模型级状态机 | 已有 | Keep | 待定 | 某账号可能仅某模型不可用 |
| E10 | 402/403 分类 | 已有 | Replace | 待定 | 区分额度、封禁、权限、上游 WAF |
| E11 | 429 Backoff | 已有 | Keep | 待定 | 优先使用 Retry-After/Reset |
| E12 | 408/5xx 临时冷却 | 已有 | Keep | 待定 | 可配置退避 |
| E13 | 冷却状态持久化 | 已有可选 | Later | 待定 | 重启后是否延续冷却 |
| E14 | 全局/Provider/凭据禁用冷却 | 已有 | Drop | 待定 | 容易破坏调度正确性 |
| E15 | 请求重试次数 | 已有 | Keep | 待定 | 与首字节状态绑定 |
| E16 | 最大尝试凭据数 | 已有 | Keep | 待定 | 防止一次请求扫完整账号池 |
| E17 | 最大冷却等待时间 | 已有 | Later | 待定 | 建议默认不等待，直接返回可重试错误 |
| E18 | 最近成功/失败时间桶 | 已有 | Replace | 待定 | 改为统一 Metrics/Event |
| E19 | 手动重置 Quota/Cooldown | 已有 | Keep | 待定 | 管理操作需审计 |
| E20 | 凭据固定 ID | 已有 | Keep | 待定 | 重启、导入、统计保持稳定 |
| E21 | 凭据变更事件 | Hook/Watcher | New | 待定 | UI 和统计实时消费 |
| E22 | 403/封禁账号明确可见 | 外置面板不完整 | New | 待定 | 显示状态、原因、首次/最近发生时间 |
| E23 | 凭据并发上限 | 无统一能力 | New | 待定 | 防止单账号过载 |
| E24 | 凭据请求排队上限 | 无统一能力 | New | 待定 | 与背压和拒绝策略绑定 |
| E25 | Provider-specific Credential Schema | JSON/Metadata 混用 | Replace | Replace | Grok 三来源和 Kiro 三认证类型显式建模 |
| E26 | 每凭据 Token Refresh Singleflight | Provider 实现不一致 | Replace | Replace | 防刷新风暴和旧 Token 覆盖 |
| E27 | Web/SSO 浏览器会话生命周期 | 无 | New | New | 与 OAuth Access Token 生命周期分离 |
| E28 | 关联/转换凭据血缘 | 无 | New | New | 记录 SSO -> Build 等来源，不共享状态 |
| E29 | Token Version + 条件状态更新 | 无统一能力 | New | New | CAS 防止并发刷新后被旧请求封禁 |

## F. Thinking、签名与缓存

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| F01 | 模型名 Thinking suffix | 已有 | Drop | 待定 | 优先显式参数，避免重复模型名 |
| F02 | Named effort 转换 | 已有 | Keep | 待定 | low/medium/high/xhigh 等 |
| F03 | Thinking token budget | 已有 | Later | 待定 | Provider 支持时启用 |
| F04 | Provider-specific Thinking Apply | 已有 | Replace | 待定 | 放入 Provider Adapter |
| F05 | Claude/Antigravity 签名缓存 | 已有 | Drop | 待定 | 除非保留对应 Provider |
| F06 | Codex Reasoning Replay | 已有 | Later | 待定 | 依赖 Codex Provider |
| F07 | xAI Reasoning Replay | 已有 | Keep | 待定 | 多轮工具/推理兼容 |
| F08 | Antigravity Reasoning Replay | 已有 | Drop | 待定 | 第一版不支持 |
| F09 | Anthropic `cache_control` 保留 | 已有 | Keep | 待定 | 不应被协议转换静默删除 |
| F10 | `prompt_cache_key` 原样保留 | 已有但粘性不完整 | Replace | 待定 | 与 D23 绑定 |
| F11 | `prompt_cache_retention` 保留 | 已有 | Keep | 待定 | Provider 支持时透传 |
| F12 | Codex Identity Confuse | 已有 | Drop | 待定 | 非核心且影响缓存身份 |
| F13 | 准确缓存 Token 统计 | 外置统计曾有偏差 | New | 待定 | 明确 read/creation/cached 口径 |
| F14 | 缓存命中率按账号/模型/路由统计 | CPA 本体无持久报表 | New | 待定 | 不使用错误分母 |
| F15 | 缓存破坏原因诊断 | 无 | New | 待定 | system 变化、key 变化、账号切换等 |
| F16 | 租户隔离的 Canonical Cache Identity | 无统一能力 | New | New | 版本化 HMAC/Hash，不泄露原始 Key |
| F17 | Provider-specific Continuity Store | 多个内存缓存分散 | New | New | Affinity/Ownership/Replay/Web State 分表 |
| F18 | Cache Identity 版本与迁移 | 无 | New | New | 算法升级可灰度，不随机破坏命中 |

## G. Usage、日志和可观测性

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| G01 | 应用结构化日志 | 已有 Logrus | Replace | 待定 | Rust `tracing` JSON |
| G02 | 日志文件轮转 | 已有 | Later | 待定 | 容器环境优先 stdout |
| G03 | 请求/响应日志 | 已有 | Keep | 待定 | 默认脱敏、可配置 Body 采样 |
| G04 | 独立错误日志 | 已有 | Replace | 待定 | 统一 Event Store，不重复写多份 |
| G05 | Request ID | 已有 | Keep | 待定 | 跨入口、路由、Provider、Usage |
| G06 | 内存 Usage 聚合 | 已有 | Replace | 待定 | 明确实时指标与持久事件边界 |
| G07 | 短期 Usage Queue | 已有 | Replace | 待定 | 改为广播事件或订阅接口 |
| G08 | Usage Plugin Hook | 已有 | Drop | 待定 | 首版使用内部 Event Bus |
| G09 | 输入/输出 Token | 已有 | Keep | 待定 | 标准字段 |
| G10 | Reasoning Token | 已有 | Keep | 待定 | 与显式 effort 分开 |
| G11 | Cache Read/Creation Token | 已有 | Keep | 待定 | 准确缓存率基础 |
| G12 | TTFT | 已有 | Keep | 待定 | 首个有效模型事件时间 |
| G13 | 总延迟 | 已有 | Keep | 待定 | 请求完成/失败时间 |
| G14 | 请求/响应 Service Tier | 已有 | Later | 待定 | Provider 支持时记录 |
| G15 | 失败状态码和响应摘要 | 已有 | Keep | 待定 | Body 必须脱敏和限长 |
| G16 | 上游响应 Header 快照 | 已有 | Later | 待定 | 白名单保存 quota/reset 等 |
| G17 | OpenTelemetry Trace | 无核心内置 | New | 待定 | 热路径异步导出 |
| G18 | Prometheus Metrics | 无核心内置 | New | 待定 | 请求、状态、池、连接、队列 |
| G19 | SQLite 请求事件持久化 | CPAMP 外置 | New | New | 有界异步写入；SQLite 不进入请求热路径 |
| G20 | 账号健康时间线 | CPAMP 部分 | New | 待定 | 401/403/429/恢复事件 |
| G21 | 路由决策记录 | 无稳定结构 | New | 待定 | 记录候选、排除原因和最终选择 |
| G22 | Body/密钥脱敏 | 部分 | Replace | 待定 | 默认 deny-by-default |
| G23 | 性能剖析端点 | 已有 pprof | Later | 待定 | Rust 可选 Tokio Console/pprof |
| G24 | Provider Family/Adapter 维度 | 无 | New | New | 区分 `grok.official/build/web` 与 Kiro endpoint |
| G25 | Continuity 决策事件 | 无 | New | New | 命中、断裂、迁移和强绑定原因 |
| G26 | Quota Source/Confidence | 无统一字段 | New | New | Header、Billing、REST、gRPC、估算分开 |
| G27 | Upstream First Event 与 Downstream First Semantic Event | 无统一口径 | New | New | 正确判断 TTFT 和透明重试边界 |
| G28 | 模型/能力同步诊断 | 无稳定接口 | New | New | 每凭据成功、失败、快照年龄和差异 |

## H. Management API 与管理界面

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| H01 | Management Key | 已有 | Keep | 待定 | 与客户端 API Key 分离 |
| H02 | 仅本机/允许远程管理 | 已有 | Keep | 待定 | 默认仅本机或私网 |
| H03 | 完整配置读取 | 已有 | Keep | 待定 | 返回脱敏视图 |
| H04 | 完整 YAML 覆盖 | 已有 | Drop | 待定 | 风险高，改为结构化事务更新 |
| H05 | 结构化配置修改 | 大量细粒度接口 | Replace | 待定 | 带版本号和校验的配置事务 |
| H06 | 客户端 API Key CRUD | 已有 | Keep | 待定 | 密钥只显示一次或掩码 |
| H07 | Provider/API Key CRUD | 已有 | Keep | 待定 | 统一 Credential API |
| H08 | Auth 文件 CRUD | 已有 | Keep | 待定 | 兼容导入，内部不暴露文件路径 |
| H09 | OAuth 发起/状态/取消 | 已有 | Keep | 待定 | Grok Build 与 Kiro OAuth |
| H10 | 凭据模型查询 | 已有 | Keep | 待定 | 验证账号能力 |
| H11 | 静态模型定义查询 | 已有 | Later | 待定 | 可由 Provider Capability API 替代 |
| H12 | 日志查询和下载 | 已有 | Replace | 待定 | 使用结构化事件查询 |
| H13 | Quota/Cooldown Reset | 已有 | Keep | 待定 | 必须审计 |
| H14 | 任意管理端 API Call 代理 | 已有 | Drop | 待定 | SSRF 和密钥泄露风险 |
| H15 | 最新版本检查 | 已有 | Later | 待定 | 不自动更新二进制 |
| H16 | 面板资源自动下载更新 | 已有 | Drop | 待定 | 构建时嵌入静态资源 |
| H17 | TUI | 已有 | Drop | 待定 | 使用 Web 管理端 |
| H18 | 新 Web 管理面板 | CPA 面板外置 | New | 待定 | 第一版可先提供 API，再做 UI |
| H19 | 凭据状态/403 可视化 | 不完整 | New | 待定 | 依赖 E22/G20 |
| H20 | 实时请求和状态推送 | CPAMP 订阅 | New | 待定 | SSE/WebSocket 管理事件流 |
| H21 | 管理操作审计日志 | 不完整 | New | 待定 | 谁在何时改了什么 |
| H22 | 管理角色/权限 | 无核心 RBAC | Later | 待定 | 单用户首版可不做 |

## I. 插件与扩展机制

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| I01 | 原生动态库插件 | 已有 | Drop | 待定 | ABI、崩溃和热更新复杂度高 |
| I02 | 插件商店 Registry | 已有 | Drop | 待定 | 第一版不做 |
| I03 | 插件在线安装/删除 | 已有 | Drop | 待定 | 供应链风险 |
| I04 | 插件 Executor | 已有 | Replace | 待定 | 使用编译期 Provider Trait |
| I05 | 插件 Translator Hook | 已有 | Replace | 待定 | 使用 Protocol/Provider Adapter |
| I06 | 插件 Scheduler/Router | 已有 | Replace | 待定 | 使用编译期 RoutingStrategy Trait |
| I07 | 插件前端鉴权 | 已有 | Drop | 待定 | 首版固定 API Key/OIDC 接口 |
| I08 | 插件 Usage Consumer | 已有 | Replace | 待定 | 使用稳定事件订阅协议 |
| I09 | 插件 CLI Flags | 已有 | Drop | 待定 | 固定 CLI 配置 |
| I10 | 插件 Management Routes | 已有 | Drop | 待定 | 管理 API 由核心注册 |
| I11 | 编译期 Feature Flags | 无对应核心设计 | New | 待定 | 按构建裁剪 Provider/协议 |
| I12 | WASM 沙箱插件 | 无 | Later | 待定 | 若未来需要第三方扩展再设计 |

## J. 配置、存储和部署

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| J01 | YAML 主配置 | 已有 | Replace | 待定 | 静态配置与运行时数据分离 |
| J02 | 本地凭据文件存储 | 已有 | Keep | 待定 | 导入兼容和备份 |
| J03 | SQLite 运行时状态 | CPAMP 外置 | New | 待定 | 凭据、事件、配置版本 |
| J04 | PostgreSQL Store | 已有 | Drop | 待定 | 第一版不做 |
| J05 | S3/Object Store | 已有 | Drop | 待定 | 第一版不做 |
| J06 | Git Store | 已有 | Drop | 待定 | 第一版不做 |
| J07 | Home 集中控制平面 | 已有 | Drop | 待定 | 单节点首版不做 |
| J08 | 配置热更新 | 已有 | Replace | 待定 | 校验后原子替换不可变快照 |
| J09 | Auth 文件热更新 | 已有 | Keep | 待定 | 导入和外部自动化需要 |
| J10 | 模型目录远程更新 | 已有 | Drop | 待定 | Provider 查询/配置驱动 |
| J11 | 单二进制部署 | 已有 | Keep | 待定 | 内嵌管理静态资源 |
| J12 | Docker 镜像 | 已有 | Keep | 待定 | 固定版本标签 |
| J13 | systemd 部署 | 当前服务器使用 | Keep | 待定 | 与现有服务运维一致 |
| J14 | 优雅停机和流 Drain | 已有 | Keep | 待定 | 停止接新请求并等待活动流 |
| J15 | 独立数据面/管理面端口 | 无固定分离 | New | 待定 | 降低攻击面和资源干扰 |
| J16 | Commercial Mode | 已有 | Drop | 待定 | 新项目无此历史需求 |
| J17 | Redis RESP 协议复用 | 内部存在 | Drop | 待定 | 使用普通事件接口 |
| J18 | 配置版本和回滚 | 不完整 | New | 待定 | 每次管理修改可恢复 |
| J19 | Secret 加密落盘 | 存储后端各异 | New | 待定 | 主密钥与数据分离 |
| J20 | 启动前配置验证 | 已有部分 | Replace | 待定 | 无效路由/别名/密钥直接拒绝启动 |

## K. 建议新增的性能能力

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| K01 | `Bytes` 零拷贝流管道 | 无 Rust 对应 | New | 待定 | 避免重复复制 SSE Chunk |
| K02 | 有界 Channel | 无显式统一约束 | New | 待定 | 每流内存上限 |
| K03 | 上游连接池全局共享 | Executor 各自实现 | Replace | 待定 | 提高 TLS/TCP 复用率 |
| K04 | 连接池指标 | 无统一接口 | New | 待定 | 连接复用、等待、握手次数 |
| K05 | 分离 connect/TTFB/idle/total timeout | 部分 | New | 待定 | 长流不能套普通总超时 |
| K06 | 路由热路径无全局读锁 | 调度器有锁 | New | 待定 | 不可变快照、分片可变状态 |
| K07 | JSON RawValue 保留/局部改写 | 部分 gjson/sjson | New | 待定 | 减少全量反序列化 |
| K08 | 日志异步批量写入 | 插件队列 | Replace | 待定 | 有界、可丢弃低优先级事件 |
| K09 | 慢客户端检测 | 无明确能力 | New | 待定 | 超过背压阈值主动终止 |
| K10 | Mock Provider 压测模式 | 有测试工具但非产品能力 | New | 待定 | 稳定测量网关自身开销 |
| K11 | 差分请求镜像 | 无 | New | 待定 | 灰度期同时对比 CPA 和新网关 |
| K12 | 性能回归门禁 | 无 | New | 待定 | CI 检查 P99、吞吐和 RSS 变化 |

## L. 上游聚合、统一模型和自有 API

| ID | 功能 | CPA 状态 | 建议 | 最终决定 | 说明/依赖 |
|---|---|---|---|---|---|
| L01 | `Upstream` 逻辑实例 | 无统一实体 | New | New | 一个中转站、官方服务或本机网关实例 |
| L02 | 单 Upstream 多 Endpoint | 无 | New | New | 同一站可有多个 Base URL、路径和协议 |
| L03 | Endpoint 单一 API Format 绑定 | 无 | New | New | Responses、Chat、Anthropic 分成独立 Endpoint |
| L04 | Endpoint 独立 Base URL/Path/Transport | 部分 Provider 固定 | New | New | HTTP、SSE、WebSocket 和特殊路径显式建模 |
| L05 | Endpoint-Credential 多对多绑定 | 凭据池直接挂 Provider | New | New | 同 Key 可复用，多 Key 可独立状态和权重 |
| L06 | OpenAI-compatible Responses Endpoint | 已有通用兼容能力 | New | New | 聚合 MVP 的第一条上游协议 |
| L07 | OpenAI-compatible Chat Endpoint | 已有通用兼容能力 | New | New | `CR-P12-COMPAT-001`；与 A09 入站区分 |
| L08 | Anthropic-compatible Messages Endpoint | 已有转换能力 | New | New | 支持中转站原生 Anthropic 路径 |
| L09 | Endpoint 模型发现策略 | Provider 内部分散 | New | New | OpenAI/Anthropic/Gemini/Provider-native |
| L10 | 每 Endpoint+Credential 模型快照 | 无统一能力 | New | New | 不能只保存中转站全局模型并集 |
| L11 | 静态模型与发现模型并集 | 部分 | New | New | 保留 manual/discovered 来源 |
| L12 | 最后成功模型快照 | 部分静态回退 | New | New | 单次同步失败不清空模型 |
| L13 | Catalog Fresh/Stale/Expired | 无统一状态 | New | New | 防陈旧目录永久参与路由 |
| L14 | 模型移除隔离期 | 无 | New | New | 连续成功缺失后再移除，避免列表抖动 |
| L15 | 模型发现 Preview/Apply | 管理 API 部分 | New | New | 差异审核后更新 RouteSnapshot |
| L16 | 发现模型默认不自动公开 | 无明确契约 | New | New | 防止上游目录污染客户端视图 |
| L17 | `PublicModel` 独立实体 | 模型注册表可近似 | New | New | 客户端稳定模型名，不等于上游模型 |
| L18 | `ModelAlias` 与 Route 分离 | Alias 混入 Provider | New | New | Alias 只解析到 PublicModel |
| L19 | `ModelRoute` 独立实体 | 调度器隐式表达 | New | New | 一个 PublicModel 的策略与候选集合 |
| L20 | `RouteCandidate` 固定 Endpoint+上游模型 | 无显式实体 | New | New | 候选不靠模型同名推断 |
| L21 | Candidate 语义能力契约 | 部分 | New | New | Tool、Thinking、Stream、模态、JSON Schema |
| L22 | Pass-through/Canonical/Lossless Bridge 模式 | 转换器隐式决定 | New | New | 跨协议必须证明语义可保留 |
| L23 | Alias/Route/Candidate 编译期冲突检查 | 部分启动校验 | New | New | 冲突、循环、悬空引用整版拒绝发布 |
| L24 | Route Priority Tier | 已有 Priority | New | New | 低数值优先，先耗尽同层再降级 |
| L25 | Smooth Weighted Round-robin | 无统一核心策略 | New | New | 预编译有界调度序列 + 原子 Cursor |
| L26 | Candidate 与 Credential 两阶段调度 | 无 | New | New | 站间权重不受站内 Key 数量干扰 |
| L27 | `AccessGroup` | 前端 API Key 列表 | New | New | 客户端权限、模型和配额边界 |
| L28 | Client Key 绑定 Access Group | API Key 仅鉴权 | New | New | 一个 Key 第一版绑定一个生效 Group |
| L29 | Group 级 Route/Model Allowlist | 无统一能力 | New | New | `/v1/models` 与推理权限使用同一来源 |
| L30 | `hard-eligible` 与运行时暂不可用分离 | 无明确契约 | New | New | 模型列表稳定，短 429 不导致列表抖动 |
| L31 | RouteSnapshot 编译和原子发布 | 配置热更新 | New | New | SQLite 不进入请求热路径 |
| L32 | Route Explain API | 无 | New | New | 返回 Candidate 排除和最终选择原因 |
| L33 | 模型同步诊断矩阵 | 无稳定接口 | New | New | 按 Endpoint、Credential、模型展示快照年龄和错误 |
| L34 | 上游 Secret AEAD 与主密钥分离 | 存储实现各异 | New | New | Key Version、Nonce、轮换和日志脱敏 |
| L35 | Client Key Prefix + 单向摘要 | 配置明文 Key | New | New | 完整 Key 只在创建时显示一次 |
| L36 | 自定义 URL Egress/SSRF Policy | 通用代理未统一限制 | New | New | 私网默认拒绝，本机服务显式 Allowlist |
| L37 | New API/AxonHub 配置 Import Adapter | 无 | Later | Later | 迁移工具，不成为持续运行依赖 |
| L38 | Candidate 成本元数据与 Cost-aware 策略 | 无 | Later | Later | 需先统一计费口径 |
| L39 | 管理调试用单请求 Channel Pin | 部分管理能力 | Later | Later | 仅授权管理 Key，禁止普通客户端任意选上游 |
| L40 | 禁止同名模型隐式跨协议/Provider Failover | 无明确契约 | New | New | 必须由 Route 显式列出并通过能力门禁 |

## 开发计划 `v1.0` 已冻结的核心决策

Provider 方向和以下 20 个核心功能点已经由开发计划 `v1.0` 冻结，不再作为创建 Rust Workspace 前的待定项：

```text
A03 A07 A08
B01 B02 B03 B04 B09 B11 B12 B15 B16
D26 D27 D30
E26
F16 F17
G19 G27
```

L01-L36 与 L40 已随“多中转站汇总 + 自有 Base URL/API Key”需求锁定；`CR-P12-COMPAT-001`
进一步将 L07 前移到 Release 1，L37-L39 仍进入后续阶段。完整行为见
[上游聚合、统一模型与自有 API 设计](05-upstream-aggregation-design.md)。

其最终决定分别为：`Keep` 6 项、`New` 9 项、`Replace` 5 项。这些选择已经确定入口协议、流状态机、Continuity Store、透明重试边界和内建持久化观测。完整执行顺序和变更控制见 [Rust AI Gateway 详细开发计划](06-development-plan.md)，Grok/Kiro 的分层细节见 [Grok 与 Kiro 渠道参考实现分析](04-channel-reference-analysis.md)。
