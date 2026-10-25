# 最新 CPA 与 CPAR 功能差异调研报告

- 调研日期：2026-10-23。
- 对照对象：CPA / CLIProxyAPI v8.0.40 与 CPAR 当前实际工作树。
- 任务范围：网关功能、协议、Provider、账号授权、客户端兼容、路由、配置、管理与运维。统计衍生项目只作边界说明。
- 结论性质：源码功能调查与开发建议；不是运行验收报告，也不修改现有产品决策。

## 1. 结论

**CPAR 已经有可继续开发的核心网关和管理后端，但还不能称为最新 CPA 的完整 Rust 等价实现。主要差距集中在协议与渠道覆盖、现成客户端专项处理、请求配置便利性以及扩展生态。**

这次对照必须同时承认三件事：

1. **已经完成的工作很多。** 三类主协议、JSON/SSE、工具续轮、入站 Responses WebSocket、多上游账号池、Claude/Codex/Kimi 授权、Grok/Kiro 原生渠道、模型路由、配置发布回滚、持久化事件及账本，都有当前代码。不能继续用早期功能矩阵的“待定”判断它们尚未开发。
2. **最新 CPA 的功能面已经更大。** 当前源码包含 Gemini/Interactions、图片、语音、视频、实时会话、更多原生 Provider、插件、Home、多种存储后端及客户端专项适配。CPAR 尚未覆盖其中不少功能。
3. **CPAR 的一部分差异是已选择的产品行为。** SQLite 版本化配置、Client Key 权限组、Request/Attempt/Usage 分离、受归属保护的 Responses 持久化，以及加密恢复等，应视为现有成果。旧 Prism 前端已按用户决定退役；本报告不建议恢复它。

最值得先做的不是重新规划整个架构，而是**按最新 CPA 的实际代码，补齐当前三类协议和常用客户端的具体行为差异**。例如仅有 `output_config.effort` 时的处理、Messages token counting、Claude 缓存/采样处理、常用请求头与代理设置。这些比先加入插件平台或实时音视频更直接地改善现有使用路径。

这里的“已有实现”不等于“本轮已通过真实 Provider 验收”。尤其 CPAR 某些原生工具/历史能力虽有 codec 和受控测试，生产组合层仍明确阻止启用；必须单列，不能算作已可用，也不能说成完全没写。

## 2. 版本、来源与判断方法

| 对象 | 本轮固定身份 | 说明 |
|---|---|---|
| CPA 最新正式发布 | [v8.0.40](https://github.com/router-for-me/CLIProxyAPI/releases/tag/v8.0.40) | 发布时间 2026-10-23 00:10:37 UTC，即北京时间 08:10:37 |
| CPA release tag 对应提交 | `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683` | 已通过 GitHub 当前元数据核对 |
| CPA main | 同上 | 2026-10-23 14:48:53 UTC 核对时与 tag 相同；本文所有 CPA 源码链接固定到这个 SHA |
| CPA 本地源码归档 | archive SHA-256 `6182c0e7ab64dea342253da9d6aaf66214e7c8e68885f779eda6414cc6deaa82` | 从该提交归档读取，未把浮动 main 当不可变证据 |
| CPAR | `feature/62-client-metadata`，HEAD `ab78c65ca68978bf46c7cf530ea795417d8431f3` **加当前未提交修改** | 包含已实施的后端整理、前端退役及 ADR-0125 请求兼容调整 |
| CPAR 当前主要 Go 行为参考 | v7.2.158；更早基线文档还保留 v7.2.80 | 这是历史/实施来源身份，不能当作最新 CPA 功能清单；本报告不自动改写它们 |

CPAR 检查目录为 `worktrees/cpar62-client-metadata-20261020`。本轮开始时记录了 7,491 个 tracked/untracked 路径的内容、权限或链接状态。参考资料和核验输出位于工作区外层 `maintenance/2026-10-23-cpa-functional-comparison/`；CPA 归档复用 `maintenance/2026-10-23-cpa-ecosystem-research/references/CLIProxyAPI/`。

判断顺序是：**实际注册入口 → 处理器/执行器 → 生产装配与能力限制 → 测试存在情况**。README、计划和历史验收用于解释背景，不覆盖当前源码事实。CPA 的个别文档仍保留旧 SDK 版本或已经不存在的 Amp 模块描述，因此没有照搬宣传清单。

本文使用以下状态：

- **已有**：相关代码已实现并接入相应入口；仍可能有配置、模型和渠道条件。
- **部分**：路由、codec、库或测试存在，但生产装配、覆盖范围或启用条件不完整。
- **未发现**：在本报告列明的源码/注册范围未找到；不推断外部私有插件或未调研服务。
- **行为不同**：双方都有实现，但输入、选择策略、持久化或配置语义不等价。
- **未运行**：本轮没有实际客户端、Provider、构建或部署执行，不能从静态阅读推断成功率。

CPA/CPAMP/Keeper 的统计关系与长期数据差异见[同日生态调研报告](cpa-ecosystem-comparison-20261023.md)。本报告独立比较 CPA 本体，不把衍生项目的数据库和界面算成 CPA 内建功能。

## 3. 公共协议和 API

### 3.1 核心推理与续接

| 功能 | CPA v8.0.40 | CPAR 当前实现 | 差异意义 |
|---|---|---|---|
| OpenAI Chat Completions | JSON/SSE、工具及协议转换 | 已有 JSON/SSE、工具及协议转换 | 共同核心；不能据接口名称推断所有扩展字段等价 |
| Anthropic Messages | JSON/SSE、工具、thinking、原生 Claude 专项适配 | 已有；近期放宽了 adaptive/effort/cache 等转换 | 主要问题已转向具体字段和客户端行为 |
| OpenAI Responses | JSON/SSE、工具、多个上游 executor | 已有；含原生字段、历史与续轮处理 | CPAR 部分高级原生字段仍被生产资格门限制 |
| Legacy `/v1/completions` | 有处理器 | 未发现对应公共路由 | 旧客户端不能直接迁移 |
| `GET /v1/models` | 有；另有模型详情路径 | 有列表，未发现对应模型详情路径 | 列表支持相同不代表模型 API 完全相同 |
| `/backend-api/codex/responses` 等别名 | 有；含 compact/alpha search 别名 | 未发现；主要暴露标准 `/v1` 路径 | 使用硬编码 Codex 路径的客户端需要适配 |
| 入站 Responses WebSocket | 有 | 有握手、事件和 session continuation | CPAR 的前端 WS 入口已开发 |
| 原生上游 WebSocket | Codex、xAI 有真正的 WS 执行分支，受下游/凭据配置条件约束 | 当前 Responses 路径进入 HTTP/SSE 出站 | 两端 WS 与 WS→HTTP/SSE 桥接不是同一能力 |
| `POST /responses/compact` | 有，但依上游；例如 Gemini executor 返回不支持 | 有，生产能力表限定 Grok Build | 双方都不能宣称所有 Provider 支持 compact |
| `GET/DELETE /responses/{id}` | 公共路由中未发现 | 有；按 Client Key 归属读取/删除，受 adapter 支持范围限制 | CPAR 的明确扩展，不能从 CPA 的 WS 续接反推其存在相同存储 API |
| Messages `count_tokens` | 有执行链；Claude 第一方走上游，第三方可用本地估算 | **部分**：路由、codec、可注入计数 executor 存在；生产装配仍使用 UnsupportedCountTokensExecutor | 这是一项明确的可用性缺口，存在路由并不代表能计数 |

入口证据：[CPA 公共路由](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/api/server_routes.go#L60)、[CPAR 公共路由](../../crates/gateway-http-actix/src/lib.rs#L1061)。细节证据：[CPA compact](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/api/handlers/openai/openai_responses_handlers.go#L654)、[Gemini compact 拒绝](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/gemini_executor.go#L129)、[CPAR adapter 能力表](../../apps/gateway/src/runtime.rs#L591)、[CPAR 归属读取](../../crates/gateway-http-actix/src/lib.rs#L2428)。

`count_tokens` 的差异有直接装配证据：[CPA CountTokens](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/claude_executor_tokens.go#L22)、[CPAR 默认 Unsupported executor](../../crates/gateway-http-actix/src/lib.rs#L475)、[生产 reload 装配](../../apps/gateway/src/runtime/reload.rs#L195)。当前测试还明确检查不能准确计数时的 422；本轮未重新执行。

上游传输证据：[CPA Codex WS 选择](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/codex_websockets_executor.go#L78)、[真实 WS DialContext](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/codex_websockets_connection.go#L38)、[CPAR 入站 WS 调用](../../crates/gateway-http-actix/src/lib.rs#L1496)、[CPAR HTTP execute](../../crates/gateway-upstream/src/upstream_client.rs#L467)。

### 3.2 Gemini、多模态和实时能力

| 功能 | CPA v8.0.40 | CPAR 当前实现 |
|---|---|---|
| Gemini 原生协议 | models、generateContent、streamGenerateContent、countTokens | 未发现原生协议 codec/公共路由 |
| Gemini Interactions | `/v1beta/interactions`，有 JSON/SSE 执行 | 未发现 |
| 多模态输入 | 有按来源/目标格式处理图片、音频、视频的转换；支持形态依具体转换器 | **部分**：Responses/Messages 能保留部分 opaque/native 内容；Chat 当前内容形态较窄；生产能力表未声明 Vision |
| 图片生成/编辑 | 有，分 Codex、xAI、OpenAI-compatible 路径 | 未发现独立图片 API |
| 语音合成 | audio/speech、tts；当前实际路径包含 xAI TTS 和模型名映射 | 未发现独立音频 API |
| 视频生成/编辑/扩展/查询/下载 | 有 xAI 执行路径及部分 OpenAI 风格别名 | 未发现独立视频 API |
| Realtime / live | 有 WebRTC SDP bootstrap、WS、sideband、会话/client secrets 等 | 未发现 realtime/live；Responses WS 不等于实时音视频 |
| Realtime translation/transcription-only/SIP | **当前 handler 明确返回 501** | 未发现；不能把这些 CPA 路由算成其已完成的功能 |
| 独立 alpha search | 有 credential-aware 转发 | 未发现独立 search 路由；Responses hosted search 是另一项受限能力 |

来源：[Gemini handler](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/api/handlers/gemini/gemini_handlers.go#L166)、[Interactions](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/api/handlers/gemini/interactions_handlers.go#L113)、[图片执行](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/api/handlers/openai/openai_images_handlers.go#L738)、[语音执行](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/api/handlers/openai/openai_speech_handlers.go#L63)、[视频执行](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/api/handlers/openai/openai_videos_handlers.go#L987)、[实时能力与明确不支持项](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/client/codex/live/capabilities.go#L19)、[alpha search](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/api/server_routes.go#L463)。

多模态需要特别保留边界：CPA 的 Chat→Gemini 示例支持 base64 媒体，但该转换中的远程 image URL 不能直接 inline；CPAR 的同协议 Passthrough 已能保留未知内容，所以“CPAR 完全没有图片内容表达”也不准确。**正确表述是 CPAR 尚未形成可宣称支持的通用 Vision/多模态推理功能。** 证据：[CPA 媒体转换](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/translator/gemini/openai/chat-completions/gemini_openai_request.go#L162)、[CPAR native passthrough](../../crates/gateway-router/src/protocol_transform.rs#L384)、[CPAR Vision 能力限制](../../apps/gateway/src/runtime.rs#L591)。

## 4. Provider、账号和授权

判断 Provider 支持时必须分开：原生执行协议、账号登录/导入、刷新、以及通用协议能否有条件接入。已有通用 OpenAI adapter，不表示某个网站账号已经可用。

| Provider/渠道 | CPA v8.0.40 | CPAR 当前实现 | 判断 |
|---|---|---|---|
| 自定义 OpenAI-compatible | 原生通用 executor、API key/base URL/headers | 独立 Chat 与 Responses adapter、API key/base URL | 共同能力；配置便利性有差异 |
| Anthropic-compatible | Claude key/base URL 接入 Claude executor | 独立 Messages adapter | 共同能力 |
| Claude OAuth | 原生 executor、授权码/PKCE 登录、刷新 | 已有授权码交换、导入和刷新；推理复用 Messages adapter | 不是缺失；专项请求行为还未完全对齐 |
| Codex/ChatGPT OAuth | 原生 HTTP/WS executor；授权码与 device 登录 | 已有授权码交换、导入、刷新和官方 Responses 行为；未发现对应 Codex device 登录入口 | 已支持主要账号路径，但不是完整 native executor 等价 |
| Kimi | 原生 executor；kimi.com/kimi.ai 等身份分支及 device flow | Kimi Coding device OAuth、API key、导入/刷新，推理复用 Responses；未发现 kimi.ai 专用分支 | 部分覆盖 |
| xAI 官方 API / Grok Build | 原生 xAI auto executor；key 与 OAuth/Build 路径 | 分别有 grok.official.responses、grok.build.responses；Build device OAuth/导入/刷新 | 两边已有；实际字段和模型资格需单独核对 |
| Grok Web / Console | 本体内置注册与 xAI 路径未发现同等 SSO/cookie 推理渠道 | 独立原生 adapter、SSO 导入及账号池 | CPAR 已有的额外渠道；Web 当前能力仍偏文本流式 |
| Kiro | 本体内置 executor/authenticator 中未发现；外部插件未评估 | 原生 kiro.messages；Social/Enterprise/API-key 凭据导入；AWS IdC device flow | CPAR 已有的额外渠道；不能把导入能力扩大成全部 Social 交互登录 |
| Gemini / Interactions | 原生 executor；Gemini API key/custom URL | 未发现原生渠道 | 明确覆盖缺口 |
| Antigravity | 原生 executor 与浏览器 OAuth | 未发现原生渠道 | 明确覆盖缺口 |
| Vertex | 原生 executor、服务账号导入 | 未发现原生渠道 | 明确覆盖缺口 |
| AIStudio | 在 wsGateway 存在时注册原生 executor | 未发现原生渠道 | CPA 自身也有装配条件 |
| Devin / Meta | 原生 executor 与相应授权流程 | 未发现原生渠道 | 明确覆盖缺口 |
| Qwen / iFlow | 当前本体未发现内置实现；插件机制不能代替具体插件证据 | 未发现原生实现 | 不列为“CPA 已有、CPAR 缺失”；历史印象不作为最新证据 |

注册证据：[CPA executor 注册](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/cliproxy/service_executors.go#L278)、[CPAR 8 个 adapter 注册](../../apps/gateway/src/runtime.rs#L2352)。这些 adapter 数量不是 Provider 功能完成百分比：Claude、Codex、Kimi 会复用协议 adapter，Grok 则按不同渠道拆开。

授权和刷新证据：[CPA 登录参数](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/cmd/server/main.go#L138)、[CPA Kimi device](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/auth/kimi.go#L25)、[CPA xAI 路径选择](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/xai_executor_request.go#L230)、[CPAR Claude 授权](../../crates/gateway-http-actix/src/management_resources/claude_authorization.rs#L151)、[CPAR Codex 授权](../../crates/gateway-http-actix/src/management_resources.rs#L2293)、[CPAR Kimi 配置](../../crates/gateway-http-actix/src/management_resources/account_channels.rs#L314)、[CPAR 常规刷新 worker](../../apps/gateway/src/credential_refresh/ordinary.rs#L190)、[CPAR Build device](../../crates/gateway-http-actix/src/management_resources/grok_device.rs#L171)、[CPAR Kiro device](../../crates/gateway-http-actix/src/management_resources/kiro_device.rs#L288)。

CPAR 还已支持部分外部凭据格式，例如 Codex 的 CPAR/CPA/sub2api 导入家族；但这只是对应凭据格式的兼容，不是整份 CPA 配置、插件状态或业务数据库可直接迁移。来源：[Codex 凭据归一化](../../crates/provider-openai-compatible/src/runtime_credential.rs#L334)。

CPA 有真实插件 Executor 与 AuthProvider 接入点，支持 StartLogin/PollLogin/RefreshAuth；本轮没有逐个安装或调查外部插件包，因此对 Kiro/Qwen/iFlow 等只能作“本体未发现”的判断。来源：[插件 executor 注册](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/pluginhost/adapters_executors.go#L64)、[插件授权接口](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/pluginhost/auth_provider.go#L281)。

## 5. 常用客户端兼容：哪些已经改好，哪些仍不同

### 5.1 近期调整已经落地，不能继续重复旧结论

[ADR-0125](../adr/ADR-0125-cpa-request-compatibility.md) 对应的当前代码已经包含：

- 同协议 native Passthrough 保留未知原生字段；已知控制字段和工具/历史形态仍校验。
- 跨协议 CanonicalBridge 按 CPA 方式映射 thinking/adaptive/effort/预算。
- Messages 的 block cache 提示在转换到 OpenAI 时允许省略；原生 Messages 保留原有位置和混合 TTL。
- temperature/top_p、stop 等按目标协议做明确转换。
- 显式 LosslessBridge 保留“不能近似转换”的约定，不能把它和普通兼容模式混用。

这些事实意味着，以前“看到普通客户端附加字段就拒绝”的笼统描述已经过时。当前更准确的问题是：**仍有具体规则未追上 CPA v8，以及原生高级工具/历史的生产启用范围没有闭环。**

实现证据：[兼容转换](../../crates/gateway-router/src/protocol_transform/compatibility.rs#L38)、[native 与 bridge 分派](../../crates/gateway-router/src/protocol_transform.rs#L384)。当前支持清单也明确保留了历史受测源码与最新代码的区别：[首版候选说明](../support/first-release-candidates.md)。

### 5.2 仍需对齐的具体行为

| 场景 | 最新 CPA 行为 | CPAR 当前行为 | 对使用的影响 |
|---|---|---|---|
| Messages 仅给 output_config.effort，没有 thinking | 中央 thinking 提取器识别显式 effort | 跨协议 compatibility::thinking 提前返回 None，effort 被省略 | 请求可被接受，但用户指定的思考强度可能未生效 |
| thinking=enabled，无 budget，另有 effort | CPA 先使用显式 effort，再回退 auto | 当前跨协议固定回退 Responses=medium、Chat=auto | 默认值和显式控制优先级不一致 |
| budget=0 或 -1 | CPA 中央处理分别表示 none/auto | CPAR Messages budget 形态要求正整数 | 兼容哨兵值存在差异，不等于所有合法预算仍被拒绝 |
| 模型专用 thinking | 按模型级别/预算范围验证和调整，支持 model(value) 后缀 | 固定预算桶与通用 Reasoning 能力，未见同等每模型 registry/后缀解析 | 自定义模型控制和跨模型默认值不等价 |
| Claude 原生客户端身份 | 检查 X-App、UA、beta、metadata 等，影响 cache placement 和请求处理 | 主要依协议与账号路径处理，未发现等价身份判定和完整处理链 | 单纯转发字段无法复制 CPA 的全部 Claude Code 兼容 |
| Claude cache/sampling | 原生请求保留客户端 cache placement；其他请求可补 cache；限制断点数；按身份/thinking 清洗采样项 | 原生路径偏向保留；跨协议已实现一部分映射和省略 | 上游是否接受、缓存效果和采样组合可能不同 |
| thinking/tool 历史签名 | 按上游判定签名，可 normalize 或删除不兼容 thinking block/工具签名 | 已知历史要求角色、非空签名和限定形态；跨协议 Reasoning/Opaque 仍有拒绝 | 多轮历史的可接受范围不同 |
| service_tier / speed | 有 Claude fast/priority→Codex priority；部分路径处理 ultrafast/routing hint | 当前跨协议通用投影未建立对应映射，可能省略 | 客户端速度/服务档位偏好未完整保留 |
| Responses 原生工具及 item | 同协议 Codex 转换以 raw JSON 为基础，另有工具类型与 agent item 处理 | 工具/item 枚举较窄；function/namespace/特定 web_search 有实现，其他新 item 可能不接受 | 新版 Codex 扩展不能凭基础 Responses 支持推定兼容 |
| Codex optimize-multi-agent-v2 | 开关与客户端识别后改写 agent 描述、collaboration 模型及部分消息，并恢复响应 | 未发现对应实现 | CPA 的 Codex 专项增强未移植 |
| Codex SIWC | 有绑定及分区凭据逻辑；v8.0.40 对识别为 Codex 的请求强制用原 OAuth 分区，忽略 use_siwc | 未发现 SIWC 机制 | 属于专项账号能力；不要按旧 use_siwc 描述移植 |

核心证据：[CPA extractClaudeConfig](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/apply.go#L740)、[CPAR thinking 投影](../../crates/gateway-router/src/protocol_transform/compatibility.rs#L185)、[CPA 模型验证](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/validate.go#L56)、[Claude 客户端识别](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/helps/claude_client_detection.go#L134)、[cache 所有权](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/claude_executor_execute.go#L157)、[sampling 清洗](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/claude_executor_request.go#L941)、[历史签名处理](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/signature/claude_messages_sanitize.go#L42)、[服务档位转换](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/translator/codex/claude/codex_claude_request.go#L418)。

Codex 专项证据：[multi-agent v2](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/client/codex/optimize-multi-agent-v2/optimize_multi_agent_v2.go#L138)、[agent_message 转换](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/client/codex/optimize-multi-agent-v2/optimize_multi_agent_v2.go#L788)、[最新 SIWC 请求选择](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/cliproxy/auth/types.go#L893)。CPAR 形态边界见[工具集合 decoder](../../crates/protocol-openai-responses/src/tool_set.rs#L92)、[输入 item decoder](../../crates/protocol-openai-responses/src/lib.rs#L535)、[native 输出允许项](../../crates/protocol-openai-responses/src/native_output.rs#L74)。

第一项可用一个很小的例子说明：请求携带 `output_config: {"effort":"high"}`，但省略 `thinking`。CPA 当前源码明确写着 effort 在 thinking 省略时也适用；CPAR 当前兼容函数则写着此情形返回 None。该结论限定在**跨协议转换**；同协议 native 保留路径应独立判断，不能把差异推广成所有请求都会丢失 effort。

### 5.3 已写代码但生产尚不开放的能力

CPAR 当前 `validate_native_field_candidate` 检查 namespace、hosted search、input message IDs、input history 及 Build parallel 等请求。相关条件触发后，非测试编译明确设置 `qualified = false`；受控测试开关不在生产替代它。

因此应写成：

> 对这些受限的请求形态，CPAR 已有协议表示与受控验证代码，但当前生产组合仍拒绝。近期普通 thinking/cache 兼容调整没有解除这个独立限制。

来源：[生产资格分支](../../apps/gateway/src/runtime.rs#L4847)。这会影响实际工具/续轮客户端可用性，优先级高于重复增加同类 parser。具体启用仍须遵守现行功能契约及其服务证据条件，本报告不授权删掉资格判断。

历史 Codex/Pi/Claude D2 受控通过也不能绑定到本轮最新 dirty 源码。当前原版 Claude、固定 Build 与真实 Provider 结果应重新取得；现有“首版未 READY”不因静态比较改变。来源：[当前支持与历史证据边界](../support/first-release-candidates.md)。

## 6. 模型、路由、调度与故障恢复

| 功能 | CPA v8.0.40 | CPAR 当前实现 | 实质差异 |
|---|---|---|---|
| 模型目录/发现 | 内置及远端目录，部分按账号探测；Codex OAuth 按计划选择目录 | Endpoint/Credential 发现、手动允许列表、发现时效与移除隔离 | CPAR 已有此能力，并增加明确证据时效 |
| 模型别名 | OAuth 全局/单凭据别名 | 精确 Public Model/Alias | 共同目标，配置对象不同 |
| 模型前缀/排除 | prefix、force-model-prefix、通配排除 | 未发现同等前缀/通配排除配置 | CPA 的模型命名配置不能原样迁移 |
| RR / weighted / fill-first | 有 round-robin、weighted-round-robin、fill-first | 有 RR/SWRR/priority_failover 计划 | 同名领域不代表相同选择逻辑 |
| priority 方向 | 数值越大优先级越高 | 对应比较中数值越小越优先 | 直接复制数字可能反转顺序 |
| 普通公共模型请求实际选择 | 由所配 selector 与可用凭据决定 | 默认入口使用 ProviderScopedSelector：配额/价格证据、负载比例、priority、weight 等排序 | CPAR weight 在此处用于排序破同分，不是可直接宣称的请求份额 |
| fill-first 对等性 | 选择 available[0] | priority_failover 同档仍轮转 | CPAR 的优先级故障转移不等于 CPA fill-first |
| 会话/缓存亲和 | 可选 session-affinity；含 session、prompt_cache_key、conversation/LCP、父子会话及 rebind 策略 | 有精确 continuation pin；Build cache_affinity 库及测试存在，但未找到 serving 调用接线 | 续接固定账号和通用请求粘性需要分开 |
| 请求重试 | 凭据轮次重试、retry 配置、RetryAfter/指数冷却 | max_attempts/bootstrap budget，排除失败目标；首个语义交付后停止透明重试 | 行为和等待时间不等价 |
| 配额/故障恢复 | 429 冷却、可配置禁用、冷却状态保存 | Health/Quota 筛选、Lease、冷却、恢复票/探测 | 核心功能双方已有 |
| 并发与排队 | Home 有并发分配协议；控制器整体实现不在本体范围 | 本地原子 CredentialLease，容量不足继续找候选 | 本轮两边均未确认通用推理等待队列；usage/event 队列不算 |
| Client Key 权限 | 本体 API key 列表；插件可扩展身份/调度策略 | Client Key→Access Group→可见路由/模型 | CPAR 有原生权限分组；不能说 CPA 插件生态完全做不到 |
| 标签路由 | 插件 API 能接受 Metadata/Attributes | 未发现标签路由谓词 | 有任意属性字段不等于已有标签路由产品功能 |
| 配置热更 | 监视 YAML/auth 文件变化 | validate/publish/rollback 后切换 Snapshot | 两边都支持运行时更新，操作方式不同 |

来源：[CPA 模型别名/排除/前缀](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/cliproxy/service_models.go#L657)、[CPA selector/priority](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/cliproxy/auth/selector.go#L552)、[CPA session affinity](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/cliproxy/auth/selector.go#L1100)、[CPA 冷却](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/cliproxy/auth/conductor_cooldown.go#L1087)、[CPA Scheduler 插件](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/pluginapi/types.go#L489)。

CPAR 证据：[模型发现时效](../../crates/gateway-catalog/src/lib.rs#L217)、[路由策略计划](../../crates/gateway-router/src/route_snapshot.rs#L537)、[ProviderScopedSelector 排序](../../crates/gateway-router/src/provider_scoped_selector.rs#L529)、[实际请求入口选择](../../apps/gateway/src/runtime.rs#L2197)、[Build cache affinity 库](../../crates/provider-grok/src/continuity_state.rs#L538)、[透明重试边界](../../crates/gateway-router/src/attempt_orchestrator.rs#L1329)、[账号容量扫描](../../crates/gateway-upstream/src/credential_pool.rs#L433)。

这里最重要的迁移提醒是：**不能把 CPA 的 priority/weight/session-affinity 数值与名称抄进 CPAR 后，就认为行为已经一致。** 特别是 CPAR 普通入口先比较配额、价格证据和负载，priority 并非无条件的第一排序项；具体路由分支还包括精确模型与 continuation 路径。

## 7. 请求级配置与出站网络

| 功能 | CPA v8.0.40 | CPAR 当前实现 | 对用户的影响 |
|---|---|---|---|
| 请求字段 default / override / filter | 有模型/协议相关 payload 规则，并在 executor 中真正调用 | 未发现同等用户配置入口；当前依 typed/native builder 和协议转换 | CPA 可通过配置兼容特殊服务，CPAR 可能需要代码改动 |
| 任意自定义 headers / User-Agent | 配置转 header 属性，实际发送应用 | 主要是按协议/账号身份生成的固定头；未发现通用配置入口 | 部分第三方网关或客户端身份需求缺少直接调整方式 |
| Claude cloak | 有模式与实际请求改写调用 | 未发现同等机制 | 属于 CPA 专项行为；是否移植应按所需服务判断 |
| 模型 thinking 后缀 | 解析 model(value)，后缀优先于 body | 精确模型/Alias；未发现同等解析 | 不能直接复用此类 CPA 模型名约定 |
| 出站代理协议 | direct/none、HTTP/HTTPS、SOCKS5/SOCKS5h | 直连，或无 userinfo、由本地解析目标的 SOCKS5；明确拒绝 HTTP/HTTPS/SOCKS5h | 当前已有代理环境可能不能直接迁移 |
| 代理绑定/池 | 有按配置/凭据的代理路径 | 有 fixed/pool 出口及实际 transport 替换 | CPAR 不是没有代理功能，限制主要在协议和安全约定 |
| 超时 | executor/transport 可有不同策略；所查兼容 HTTP 路径传 timeout=0，另有握手超时配置 | 生产 runtime 定义 connect/TTFB/idle/progress/total 等上限 | 不能把一边某个 timeout 字段直接视为另一边全套超时 |
| 环境代理继承 | 取决于所选 CPA transport 配置 | CPAR 出站明确 no_proxy，按已选择路径发送 | 部署时不能假设 HTTP_PROXY 等自然生效 |

CPA 接线证据：[payload 与 headers 在发送前应用](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/openai_compat_executor.go#L168)、[default/override/filter 实现](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/helps/payload_helpers.go#L95)、[Claude cloak 调用](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/claude_executor_execute.go#L108)、[thinking 后缀](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/apply.go#L317)、[代理协议处理](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/proxyutil/proxy.go#L110)。

CPAR 证据：[出站代理明确限制](../../crates/gateway-upstream/src/upstream_client.rs#L207)、[出口配置装配](../../apps/gateway/src/runtime.rs#L3281)、[timeout profile](../../apps/gateway/src/runtime.rs#L347)、[transport 应用](../../crates/gateway-upstream/src/upstream_client.rs#L574)。

代理差异与 CPAR 现有 DNS/出站约束有关。它是值得评估的易用性缺口，但本报告不把“放宽请求参数”解释为可顺便删除出站地址约束。

## 8. 管理、扩展与部署运维

| 功能 | CPA v8.0.40 | CPAR 当前实现 | 判断 |
|---|---|---|---|
| 管理 API | /v8/management；旧 /v0 仍注册但已 deprecated | 独立监听器的 /admin | 数据模型、路径和操作语义不兼容 |
| 管理认证 | 管理密钥、Bearer/X-Management-Key、远程访问策略 | CLI key 或管理员 session、Origin/CSRF 检查 | 均已有，接入现成面板需要适配 |
| 配置权威 | YAML v8 树；兼容读取旧布局，成功写入才迁移；可选 If-Match | SQLite 版本化配置、草稿校验、publish/rollback、原子 Snapshot 与审计 | CPAR 是已完成的不同产品设计 |
| 配置/凭据备份 | 配置与 auth 文件上传下载；本轮未发现同等完整控制面加密恢复入口 | 加密 SQLite 快照、预检、空目标恢复；备份制品创建为 operator/embedding 方法 | CPAR 有较完整恢复基础，但不能宣称已有 HTTP 下载备份制品接口 |
| 当前管理 Web UI | 运行时提供 management.html，缺失时可下载独立 Management Center 发布制品 | 旧 Prism 已按决定移除；/admin API 和 OpenAPI 仍在 | 预期产品状态，新前端另行设计 |
| CLI / TUI | 服务/登录 CLI、远程 TUI、standalone TUI；TUI 仍调用 v0 | serve、本地 admin 生命周期及渠道命令；未发现对应 TUI | CLI 能力已有，操作界面覆盖不同 |
| 可嵌入 SDK | Go sdk/cliproxy Builder | Rust workspace crates，publish=false；未发现独立对外 SDK 入口 | 不能把内部 crates 自动等同稳定 SDK 产品 |
| 动态插件 | 动态库加载、Executor/Auth/FrontendAuth/Scheduler 等扩展，管理/商店安装入口 | 未发现运行时动态插件宿主 | 新产品范围，不是补几个 endpoint |
| Home | home-jwt/mTLS、远端配置/插件/发现/凭据并发通路 | 未发现对应控制面接入 | 属于分布式运行模式扩展 |
| 存储后端 | 文件、Postgres、Git、S3-compatible ObjectStore；主要用于配置/auth 资料 | SQLite 控制面/事件/账本，加密凭据与独立密钥 | 两种存储职责不等价 |
| 日志/诊断 | 管理日志、请求/错误日志、可选 pprof | JSON stdout/journald、受保护指标接口 | CPAR 有指标；CPA 本体本轮未发现 Prometheus/debugmetrics 入口 |
| OTel | 本轮未作完整 OTel 支持调查 | 接口存在，serve 装配 NoopOpenTelemetryExporter | 不能把接口存在写成已可用外部 tracing |
| 容器/服务部署 | Docker/Compose、Home compose、二进制/镜像发布 workflow | Docker/Compose、systemd、Caddy、交付/制品工作流 | 部署材料双方已有；CPAR 私有 workflow artifacts 不等于已公开发布镜像 |
| Usage/长期审计 | 用量队列/订阅与日志，长期分析主要由生态完成 | 内置 Request/Attempt/Usage、事件持久化、billing ledger 和管理审计 | CPAR 已有的重要产品差异；分析展示仍有欠缺 |

来源：[CPA v8 管理路由](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/api/server_management_v8.go#L10)、[配置写入规则](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/api/handlers/management/config_v8.go#L20)、[管理 UI 服务](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/api/server_management.go#L312)、[CLI/Home/存储分支](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/cmd/server/main.go#L160)、[SDK Builder](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/cliproxy/builder.go#L199)、[动态库加载](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/pluginhost/loader_unix.go#L43)、[Home client](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/home/client.go#L23)。

CPAR 证据：[管理监听器](../../crates/gateway-http-actix/src/lib.rs#L1094)、[管理认证](../../crates/gateway-http-actix/src/management_security.rs#L30)、[发布/回滚服务](../../crates/gateway-control/src/management_service.rs#L289)、[备份制品不是 HTTP 下载入口](../../crates/gateway-control/src/management_backup_service.rs#L98)、[指标接口](../../crates/gateway-http-actix/src/management_observability_resources.rs#L76)、[Noop OTel 装配](../../apps/gateway/src/runtime.rs#L706)、[CLI 用法](../../apps/gateway/src/main.rs#L516)、[交付制品 workflow](../../.github/workflows/release-artifact.yml#L213)。

CPA 本体没有找到当前可用 Amp 模块，保存配置的代码还会删除旧 Amp 配置键。源码根规则文件的旧描述不能推翻这一事实。未调查具体外部 Amp 插件，因而不作“已转为某插件且可用”的结论。来源：[旧集成键清理](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/config/config_yaml.go#L921)。

## 9. 已有成果怎样继续使用

从“CPA 用 Rust 实现”的目标出发，下面这些工作已经有基础，无须重新起一套框架：

| 现有成果 | 继续承接的开发 |
|---|---|
| protocol-* + protocol_transform | 对照 CPA translator/thinking/signature 规则，补具体协议行为与回归样例 |
| provider-openai-compatible / provider-anthropic-compatible | 完善 Claude/Codex/Kimi 的兼容和授权细节；不必为了名字一致另造重复 adapter |
| provider-grok / provider-kiro | 保留已经开发的额外渠道，按各自真实能力验收 |
| gateway-router / gateway-upstream | 补会话亲和、策略一致性和出站配置；先确认真实 serving 分支 |
| gateway-control / gateway-store | 继续承接账号、配置、恢复、用量及账本，不必改成 CPA 的文件状态才能对齐推理 |
| gateway-http-actix 的 /admin + OpenAPI | 作为新前端的后端基础；具体浏览器工作流后续另做 |
| apps/gateway/runtime | 完成现有实现的生产装配与能力开启；减少“库和测试已有，实际入口未用”的落差 |

模块对照遵循[当前后端架构](../03-target-architecture-draft.md)和[后端优先/前端退役决策](../adr/ADR-0124-cpa-first-backend-and-frontend-retirement.md)。这是对既有模块的工作分配，不提出新架构层。

CPAR 的“额外功能”也不必笼统视为包袱。账号归属、持久化、恢复、事件与账本已经承担实际职责。可以单独评估不需要的产品界面或历史材料，但不能仅因 CPA 不采用相同实现，就删除正在承载现有行为的代码。

## 10. 建议的对齐顺序

### 第一批：让当前主路径更接近最新 CPA

1. **更新 thinking/effort 规则。** 先做 effort-only、enabled-without-budget、0/-1 哨兵值和模型默认值的差分样例；明确跨协议与 native 路径，不扩大成全部字段无条件放行。
2. **补 Messages count_tokens 的生产实现。** 参考 CPA 第一方调用/第三方估算分支；明确哪些返回是真实上游计数、哪些为估算。该功能应接入现有账号选择与认证，不只增加测试 stub。
3. **完成 Claude/Codex 常见请求的专项适配。** 优先 cache/sampling、历史签名、service tier、当前客户端实际会发送的 Responses item；multi-agent/SIWC 根据实际需求逐项加入。
4. **检查并闭合现有高级工具/历史功能的生产可用性。** 不重复造 codec；列出当前限制、所需真实服务证据和各自验收。保留具体能力范围，不用一个“支持 Codex”覆盖全部原生工具。
5. **对最新候选重新执行实际客户端闭环。** 至少覆盖常用请求、两轮工具、流式结束、取消、失败和续接；代码对照结果与真实服务结果分别记录。

这批最直接回应“请求不要过度严格、按 CPA 的成熟逻辑实现”的方向。它不要求先引入插件框架、新前端或新数据库。

### 第二批：补迁移与日常配置便利性

- 通用 session/prompt-cache affinity，并确认与现有 continuation pin 的关系。
- priority/weight/fill-first 的对等行为或清晰的配置迁移映射。
- 模型 prefix/通配排除、思考后缀等已确有使用场景的配置。
- 有范围的 payload defaults/override/filter、自定义 headers。
- 按部署需求支持 HTTP/HTTPS/SOCKS5h/认证代理，保留明确的目标地址与凭据处理约束。
- 需要时增加 legacy completions、Codex 别名、模型详情等低层接口兼容。

这些改动应按实际使用优先级进入现有模块。没有必要为了覆盖 CPA 每个配置键，立即把全部规则做成通用 DSL。

### 第三批：明确作为功能扩展选择

- Gemini/Interactions 及 Antigravity/Vertex 等新增渠道。
- 图片、语音、视频与 Realtime。
- 原生上游 WebSocket。
- Devin/Meta 等额外 Provider。
- 动态插件、Home、Postgres/Git/ObjectStore、对外 SDK/TUI。
- 基于当前 /admin 接口的新前端。

若目标最终是“完整覆盖 v8.0.40 功能”，第三批需要纳入；若近期目标是“稳定承接 Codex/Claude/Pi 与 Grok/Kiro”，它们可以按需后置。**本报告列的是功能差异及建议顺序，没有自动把所有缺项转成已批准开发范围。**

## 11. 为什么现在容易感觉混乱

这是根据当前代码与资料的组织作出的判断：

1. **参考版本不同步。** 历史基线是 v7.2.80，当前主要移植参考是 v7.2.158，而最新 CPA 已到 v8.0.40；旧结论会遗漏新功能，也可能保留已删除的 Amp 等印象。
2. **计划状态落后于实现。** 早期矩阵中的待定/未来项，已有若干在后续账号和运行态工作中实现。按它重启规划会重复劳动。
3. **“写好了”和“生产路径可用”混在一起。** count_tokens、cache affinity、高级原生工具，分别属于未装配、未接线或未启用的不同情况。
4. **CPA 功能对齐与 CPAR 自有控制能力混在一起。** 前者应直接看 upstream 行为；后者应判断是否承载现有产品需求。二者不需要争夺同一套抽象设计。
5. **历史验收身份与当前源码容易混用。** 历史客户端/OCI 通过有价值，但不能覆盖之后的兼容性和前端退役改动。

建议后续工作始终记录四项最小信息：**参照的 CPA 提交、对应功能/请求样例、CPAR 当前生产行为、尚需完成的一项实现或验证。** 现有架构与模块已经足够承接这种逐项移植方式，不需要先做全仓重构。

## 12. 本轮验证与限制

本轮研究已完成：

- 重新查询 GitHub 最新发布，并核对 release tag 与 main 的提交一致。
- 主线程阅读当前架构、领域、兼容性和首版支持基础文档。
- 六组只读源码调查，覆盖协议、Provider/OAuth、客户端兼容、调度、管理运维与配置网络。
- 主线程抽查 effort-only、生产资格限制、token-count 装配、路由排序、实时 501、代理限制等关键结论。
- 生成本报告，执行文档检查，并核对既有工作树保存情况。检查结果以调研目录中的 docs-check-report.md 和 result.json 为准。

没有执行：CPA/CPAR 全量运行测试、真实客户端、真实 Provider、OAuth 登录、容器部署、性能比较、远端 CI、发布。没有读取生产凭据或私有请求内容。

缺失判断的主要检索范围是 CPA 的 internal/api、sdk/api、internal/runtime、sdk/cliproxy、internal/pluginhost、cmd/config，以及 CPAR 的 apps、crates、生产装配与公共/管理路由。没有检验未安装的第三方插件、Home 外部控制器或私人部署。所有“未发现”均受此范围限制。

因此本报告可以作为**当前功能盘点、下一步移植和差分测试的输入**；不能作为功能全部可用、实际成功率相同、性能更优或首版可发布的证明。
