# CPA / CPAMP 渠道接入与管理行为研究

日期：2026-09-29。状态：参考源码研究完成；未进行真实运行验收。

对应研究工单：[核对 CPA/CPAMP 的现有渠道接入与管理行为](https://github.com/Ricardo121380/cpa-rust-gateway/issues/2)。

## 结论

已核对的 CPA / CPAMP 接入路径中，操作者在一次业务流程内提交 API 配置、完成 OAuth 或上传认证文件；系统负责后续配置重载、凭据登记及模型调度更新。没有要求操作者先到另一个资源页面建立独立 Upstream / Endpoint，再回来导入凭据。[API 表单][ui-openai]、[OAuth][ui-oauth]、[上传][ui-upload]、[运行登记][cpa-register]分别支撑这一结论；结论限于下述入口，不覆盖插件或所有上游扩展。

这与用户要求减少 Kimi API 等渠道的前置操作一致。但参考流程仍要求必要业务信息：例如通用兼容 API 的名称、地址和 Key。保存、模型读取、调用测试、运行可用性也有不同证据，不能合并成一个无条件的成功状态。[表单提交][ui-openai]、[独立测试][ui-test]、[异步生效][cpa-persist]

本研究提供事实和差异，不决定 CPAR 应自动发布配置、复用哪个已有连接或接受哪些协议转换损失。这些取舍继续由接入、管理状态和协议兼容工单与用户确认。

## 范围与冻结版本

| 对象 | 冻结版本 | 证据边界 |
| --- | --- | --- |
| CPA：router-for-me/CLIProxyAPI | `acdace936fa7df2905500c7f5e0a97d683138dea` | 管理 API、配置合成、OAuth、凭据登记、调度刷新与有限管理状态源码 |
| CPAMP：seakee/CPA-Manager-Plus | `29d676f8eebfbc11eedc96573f3a059360163a3e` | Provider 表单、OAuth、认证文件导入、相关 API service 和状态入口源码 |
| CPAR 对照 | 本地主工作区 `9fc4ceb4e12aab9c60658869706828a7d5d8312f` | 现有接入入口的定向源码点验，不是全渠道能力审计 |

CPAMP 指上述 seakee 仓库，不是 CPA 官方 Management Center。报告保存于独立 research 分支；该分支以已公开的 `8932bf1e91297e30a83330af57038fa0f3ee21be` 为文档存储基线，仅新增本报告。该存储基线与 CPAR 对照版本不同，不能用研究分支的旧业务代码替代主工作区现状。

所有来源为固定 commit 的公开源码。研究未使用真实凭据，未运行浏览器、服务、OAuth 交换或 Provider 请求。

## 现有渠道与参考入口的关系

映射表示可参考的接入类别，不表示两个项目的渠道隔离、账号或协议能力可以互换。

| CPAR 入口 | 已核实的参考对象 | 适用边界 |
| --- | --- | --- |
| `openai-compatible` | CPAMP OpenAI 兼容表单与 CPA `/openai-compatibility` | 同一提交包含名称、地址及 Key 条目；模型和高级参数可随配置提交。[UI][ui-openai] [合成][cpa-compatible] |
| `anthropic-compatible` | CPAMP Claude API Key 表单、CPA `claude` API Key 路径 | UI 新建要求 Key 和地址；不能因 executor 有默认地址而断言 UI 无需填写。[UI][ui-claude] [配置类型][cpa-claude-config] |
| `codex` | Codex API 配置及 Codex OAuth | API Key 表单与 OAuth 是不同接入方式；OAuth 处理 account 身份，不能笼统当成一个文本 Key。[表单][ui-codex] [OAuth][cpa-codex] |
| `claude` | Anthropic OAuth、Claude 认证文件与相关管理入口 | OAuth 完成与 API Key 保存分别观察。[OAuth][cpa-claude-oauth] [UI 状态][ui-oauth-status] |
| `grok.official` | CPA/CPAMP `xai` API Key 配置 | CPAMP xAI 表单预填官方地址；API Key 与 OAuth 不能混为相同授权。[默认值][ui-xai-default] |
| `grok.build` | CPA `xai` device OAuth 与 CLI chat proxy 路径 | 这是行为参照；CPA 把多种 xAI 路径放在一个 provider 下，不能据此合并 CPAR 渠道。[OAuth][cpa-xai-oauth] [地址选择][cpa-xai-route] |
| `kimi-coding` | CPA `kimi` device OAuth，CPAMP Kimi OAuth 入口 | CPA 保存设备及 token 信息，并区分 kimi.com / kimi.ai 的 coding 地址。[OAuth][cpa-kimi] [常量][cpa-kimi-constants] |
| `kimi-api` | 可对照通用兼容 API 的表单组织方式 | 所检原生枚举中没有同名专用入口；未验证 Moonshot API Key 经通用配置的真实请求。不能把 Kimi OAuth 当成其验收证据。[原生注册][cpa-auth-registry] [表单分派][ui-dispatch] |
| `grok.console` | 所检原生枚举没有直接对应入口 | 不能根据 xAI API Key/OAuth 推断 SSO Console 的生命周期。[原生注册][cpa-auth-registry] [执行器分派][cpa-executors] |
| `grok.web` | 所检原生枚举没有直接对应入口 | 不排除插件或通用接口，但本研究不声称已经核实。[执行器分派][cpa-executors] |
| `kiro` | 所检原生枚举没有直接对应入口 | 沿用 CPAR 的实际能力边界，后续只讨论一致的操作原则，不凭参考项目补造能力。[原生注册][cpa-auth-registry] |

否定结论的检索范围是 CPA 管理路由、原生认证注册、执行器枚举/分派及 tree 路径名称；并非所有文件内容和所有插件的穷尽检索。CPAMP 也只在 Provider 表单分派、OAuth 类型与所读 service 中核对对应项。[路由表][cpa-routes] [UI 分派][ui-dispatch]

## 用户操作与保存后的实际行为

### API Key 配置

CPAMP OpenAI 兼容新建表单收集名称、Base URL、至少一个 Key；允许在同一表单配置多 Key、权重、代理、headers、prefix、模型及别名，最终一次调用创建 API。已有记录可能通过 authIndex 保留密钥引用。[ui-openai]

Claude 表单同样提交 Key 和地址；保存后对请求指纹的回读验证可以单独警告，不能将“写入已完成但验证未确认”显示为完全没有保存。Codex/xAI 共用按 providerKind 分派的表单，xAI 有官方地址默认值。[ui-claude] [ui-codex] [ui-xai-default]

CPA 管理保存链先持久化配置、回复成功，再异步触发配置重载。默认 builder 连接重载 hook，watcher 随后更新 clients 并刷新 auth 状态。因此用户不必手动创建低层资源，但一次保存响应不证明后续运行和上游连通已完成。[cpa-persist] [cpa-reload-hook] [cpa-reload-clients]

输入路径存在差异：CPAMP UI 对 Key 和地址有验证；CPA 通用兼容配置合成代码能构造无 Key 的 Auth。此差异不能被写成“所有 API 渠道都可以不提供 Key”。[ui-openai] [cpa-compatible]

### OAuth

CPAMP 发起对应 provider 的授权，展示 URL，device flow 可显示用户码，并轮询授权状态；成功、错误和缺失 state 分别处理。可粘贴 callback 的 provider 集合与 device flow 集合不同。[ui-oauth] [ui-oauth-status]

CPA Kimi device flow 由系统选择 coding 地址并保存 access/refresh token、device_id；Codex/Anthropic 则有各自的授权及身份字段。它们不是 API Key 表单的简单换名。[cpa-kimi] [cpa-kimi-constants] [cpa-codex] [cpa-claude-oauth]

OAuth 保存经 token record 与落盘记录重新合成，再调用运行同步 hook。hook 会处理 executor/Auth，注册模型并重新整理模型状态与 scheduler entry，使新凭据进入调度视图。[cpa-save] [cpa-sync-hook] [cpa-register]

### 认证文件上传

CPAMP 对合法文件逐个上传，允许一批中部分成功、部分失败；成功部分回读列表，失败部分给出文件级信息。文件格式/大小问题和上传结果分别处理。[ui-upload] [ui-upload-service]

CPA 解析 JSON、写文件，再 Register/Update 并调用运行同步 hook。缺失 type 可以变成 unknown；写盘之后的登记或 hook 失败也会报错。因此“文件保存”“格式可识别”“运行已登记”和“上游真正可用”需要独立理解。[cpa-upload] [cpa-upsert] [cpa-sync-hook]

## 日常管理中可参考的状态模式

| 场景 | 静态源码观察 | 对后续决策的输入 |
| --- | --- | --- |
| 保存、模型读取、Key 测试 | CPAMP 有独立测试状态；模型及定义请求可部分失败或不支持。[ui-test] [ui-models] | 为不同操作保留不同完成证据，不把保存成功扩展为调用成功。 |
| 账号可用性与额度 | 列表存在 available、unconfirmed、low、exhausted、disabled、problem 等筛选；支持额度读取的类型集合不证明当前额度可用。[ui-account-states] [ui-quota-types] | 正常空态、未知、失败及停用需要明确语义。 |
| 停用 | CPAMP 的 OpenAI 配置使用 disabled；部分 Key 类路径使用排除模型规则；CPA 文件账号有独立 Disabled/Status。[ui-actions] [cpa-status] | 参考操作结果，不复制不同渠道的底层实现手段。 |
| 删除 | 有确认和共享物理文件检查。[ui-delete] | CPAR 要独立说明账号、连接和历史数据的影响。 |
| 刷新 | CPAMP 区分完成、待处理与失败；CPA 根据有效 token、错误类型及退避处理。[ui-refresh] [cpa-refresh] | 刷新失败不必然等于立即停用，也不能一律显示为临时网络问题。 |
| 重新授权 | Codex 有专用核验；其他已列 provider 跳相应 OAuth。Codex 会核对身份和凭据变化，允许未确认或歧义结果。[ui-reauth] [ui-reauth-verification] | 返回授权页面不能自动冒充原账号恢复成功。 |

## CPAR 定向对照与后续工单

本地主工作区 `9fc4ceb` 中，`web/prism/src/features/accounts/AddAccountDialog.tsx`：

- 第 26 行定义 API 渠道集合：OpenAI 兼容、Anthropic 兼容、Grok Official、Kimi API。
- 第 41–43 行要求操作者明确选择服务，单一服务也不自动选择。
- 第 230、244–247 行在导入路径中要求服务；接口可以稍后连接，所以准确的阻断点是必须先有服务，而非两个对象在所有情况下一律必填。
- 第 58–63、77–82 行对 Codex、Claude、Kimi Coding、Kiro 已有 prepare target 路径。

这些源码事实说明问题涉及共用接入组织方式，而不只是 Kimi 的文案。这里没有进行浏览器复现，也没有判定所有渠道都具有相同的自动准备条件。

研究结果已经能够输入现有工单，无需凭空新增实现任务：

1. [接入及生效流程](https://github.com/Ricardo121380/cpa-rust-gateway/issues/4)：决定必要字段、自动资源组织、已有连接选择、保存/生效确认及部分失败恢复。
2. [日常管理状态与恢复](https://github.com/Ricardo121380/cpa-rust-gateway/issues/6)：决定状态解释、下一步动作及历史数据影响。
3. [三协议兼容与可靠性](https://github.com/Ricardo121380/cpa-rust-gateway/issues/5)：以上研究没有证明任一渠道完整兼容三个协议，仍需独立确定和验证。

## 验证与限制

- PASS：固定版本公开源码查阅；关键保存/运行登记、部分上传失败及表单验证链由主代理点验；报告来源链接、结构与 diff 静态检查。
- NOT_RUN：浏览器交互、真实 OAuth、真实推理、账号权益/数值额度、刷新成功率、性能与生产验收。
- 未确认：Moonshot/Kimi API Key 的具体通用端点适配、第三方导入格式的完整兼容性、插件扩展，以及未读源码中的更多渠道特例。
- 本研究关闭的是“参考行为证据”问题；CPAR 的现状能力矩阵和产品取舍仍由开放子工单推进。

## 固定来源

[ui-openai]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/components/providers/ProviderEditDrawer/OpenAIEditDrawer.tsx#L651-L725
[ui-test]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/components/providers/ProviderEditDrawer/OpenAIEditDrawer.tsx#L485-L569
[ui-claude]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/components/providers/ProviderEditDrawer/ClaudeEditDrawer.tsx#L590-L690
[ui-codex]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/components/providers/ProviderEditDrawer/CodexEditDrawer.tsx#L623-L717
[ui-xai-default]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/components/providers/ProviderEditDrawer/CodexEditDrawer.tsx#L61-L79
[ui-oauth]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/oauth/OAuthPage.tsx#L748-L785
[ui-oauth-status]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/oauth/OAuthPage.tsx#L601-L706
[ui-upload]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/authFiles/hooks/useAuthFilesData.ts#L1000-L1078
[ui-upload-service]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/services/api/authFiles.ts#L1378-L1404
[ui-models]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/authFiles/hooks/useAuthFilesModels.ts#L156-L218
[ui-account-states]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/accounts/AccountsPage.tsx#L7805-L7823
[ui-quota-types]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/authFiles/constants.ts#L30-L41
[ui-actions]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/aiProviders/AiProvidersPage.tsx#L629-L809
[ui-delete]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/authFiles/hooks/useAuthFilesData.ts#L1191-L1258
[ui-refresh]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/authFiles/hooks/useAuthFilesData.ts#L1405-L1437
[ui-reauth]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/accounts/model/accountReauth.ts#L10-L38
[ui-reauth-verification]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/accounts/model/accountDirectReauth.ts#L360-L411
[ui-dispatch]: https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/aiProviders/AiProvidersPage.tsx#L1711-L1769
[cpa-compatible]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/watcher/synthesizer/config.go#L290-L401
[cpa-claude-config]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/config/config_types.go#L471-L536
[cpa-codex]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/handlers/management/auth_files_provider_oauth.go#L293-L343
[cpa-claude-oauth]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/handlers/management/auth_files_provider_oauth.go#L37-L195
[cpa-xai-oauth]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/handlers/management/auth_files_provider_oauth.go#L513-L623
[cpa-xai-route]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/runtime/executor/xai_executor_request.go#L203-L269
[cpa-kimi]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/handlers/management/auth_files_provider_oauth.go#L769-L899
[cpa-kimi-constants]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/auth/kimi/kimi.go#L26-L57
[cpa-auth-registry]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/sdk/cliproxy/service_auth.go#L19-L31
[cpa-executors]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/sdk/cliproxy/service_executors.go#L201-L327
[cpa-routes]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/server_management.go#L135-L200
[cpa-persist]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/handlers/management/handler.go#L409-L421
[cpa-reload-hook]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/sdk/cliproxy/builder.go#L298-L303
[cpa-reload-clients]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/watcher/clients.go#L144-L149
[cpa-save]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/handlers/management/auth_files_fields.go#L945-L1006
[cpa-sync-hook]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/sdk/cliproxy/builder.go#L308-L335
[cpa-register]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/sdk/cliproxy/service_auth.go#L498-L515
[cpa-upload]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/handlers/management/auth_files_crud.go#L261-L283
[cpa-upsert]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/handlers/management/auth_files_crud.go#L480-L558
[cpa-status]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/internal/api/handlers/management/auth_files_fields.go#L97-L146
[cpa-refresh]: https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/sdk/cliproxy/auth/conductor_refresh.go#L574-L677
