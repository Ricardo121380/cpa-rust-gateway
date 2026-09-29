# 第三版操作流程与 CPA / CPAMP 对照

用户反馈：第一版未完全测完，操作逻辑感觉别扭，要求整套逻辑参考 CPA 和 CPAMP。第二版按实际提交和管理链路调整；随后用户确认上游 API Key 配置与授权账号应按 CPA / CPAMP 分开。第三版修正混列，保留已有 ChatGPT / Kimi 视觉资产。

本记录是冻结源码对照，不是上游页面的浏览器验收。三个来源分别固定为：

| 来源 | revision | 用途 |
| --- | --- | --- |
| router-for-me/CLIProxyAPI | `acdace936fa7df2905500c7f5e0a97d683138dea` | CPA 后端及官方管理 UI 来源关系 |
| router-for-me/Cli-Proxy-API-Management-Center | `4530da271ba2e89810d4dccebc57f3091afa590a` | CPA 管理 UI 的配置抽屉、授权和认证文件操作 |
| seakee/CPA-Manager-Plus | `29d676f8eebfbc11eedc96573f3a059360163a3e` | CPAMP 的 API 配置、账号列表和 OAuth 操作 |

CPA 的 [panel-github-repository 配置](https://github.com/router-for-me/CLIProxyAPI/blob/acdace936fa7df2905500c7f5e0a97d683138dea/config.example.yaml#L70-L82) 指向上述管理 UI；UI 独立更新，因此后端 revision 不能证明某个部署使用的 UI revision。CPAMP 是另一项目，不能称为 CPA 官方 UI。

## 操作对照

| 操作 | 核实的参考行为 | 第三版处理 |
| --- | --- | --- |
| 页面分工 | CPA 使用 AI 提供商、认证文件和 OAuth 三个入口；CPAMP 使用 AI 提供商、账号管理和 OAuth 三个入口。 | API Key 配置仅进入 AI 提供商；授权 / 导入凭据仅进入账号管理；OAuth 授权负责发起登录，列表、筛选和数量不再混用。 |
| API 配置 | CPAMP 从列表打开类型对应的编辑抽屉；一个表单包含名称、地址、上游密钥和模型，提交后刷新列表并关闭。CPA 管理 UI 同样区分创建 / 编辑 / 详情抽屉。 | API 配置列表打开单张表单，直接保存返回列表；取消三步向导。 |
| 模型发现与测试 | CPAMP 分别提供读取和测试操作。保存条件不要求测试成功或模型发现成功，正在测试时阻止同时保存。 | 模型读取及测试为可选操作；原型内均为模拟。保存与生效不依赖推理成功。 |
| OAuth 完成 | 两个 UI 均有授权完成后的账号 / 认证文件入口；核对的成功链路没有要求再做一次账号保存。 | 授权成功自动登记，直接查看账号。 |
| 账号列表 | CPAMP 桌面默认表格，可切换布局；紧凑屏幕使用卡片。行上提供模型、详情、启停、删除及必要时重新授权等入口。 | 日常账号列表为主，移动端用卡片；状态详情放入账号抽屉。 |
| 认证文件导入 | CPA 管理 UI 和 CPAMP 都支持直接上传文件并刷新列表；CPAMP 支持粘贴 JSON，部分失败分别反馈。 | 同页载入合成 JSON，成功项保留，失败项修正后重试；存在旧草稿时仍按 CPAR 的合并核对规则执行。 |
| 重新授权与状态 | CPAMP 的重新授权入口按渠道路由；列表区分失效、额度、停用及未知等状态。停用并不禁止一切维护操作。 | 同一账号更新授权保留设置，停用状态持续；新增另一可靠身份与覆盖目标账号的身份冲突分开处理。 |
| 客户端模型权限 | 核对的配置保存及授权成功链路没有强制逐客户端 Key 选择模型权限；上游 apiKeyEntries 不能当作客户端权限。 | 用户已确认接入与权限分开：已有权限保持，新模型默认不开放，随后从“模型”入口独立设置。 |

## 可定位的主要源码

- [CPAMP 导航分组](https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/components/layout/MainLayout.tsx#L589-L610)明确分开 `/ai-providers`、`/accounts`、`/oauth`；[CPA 管理 UI 路由](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/4530da271ba2e89810d4dccebc57f3091afa590a/src/router/MainRoutes.tsx#L24-L29)分别为 `/ai-providers`、`/auth-files`、`/oauth`。这是页面职责的参照，不要求修改 CPAR 的底层数据模型。

- CPAMP [API 配置列表与抽屉装配](https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/aiProviders/AiProvidersPage.tsx#L1556-L1768)、[保存条件](https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/components/providers/ProviderEditDrawer/OpenAIEditDrawer.tsx#L295-L305)及[提交后关闭](https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/components/providers/ProviderEditDrawer/OpenAIEditDrawer.tsx#L651-L725)。
- CPAMP [OAuth 发起与状态处理](https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/oauth/OAuthPage.tsx#L601-L785)、[成功后的账号入口](https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/oauth/OAuthPage.tsx#L1250-L1270)。
- CPAMP [账号行操作](https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/accounts/AccountsPage.tsx#L8304-L8463)、[上传与粘贴入口](https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/accounts/AccountsPage.tsx#L10209-L10246)、[默认列表状态](https://github.com/seakee/CPA-Manager-Plus/blob/29d676f8eebfbc11eedc96573f3a059360163a3e/apps/web/src/features/accounts/model/accountsWorkspaceUiState.ts#L44-L54)。
- CPA 管理 UI [配置抽屉](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/4530da271ba2e89810d4dccebc57f3091afa590a/src/features/providers/sheets/ProviderSheet.tsx#L110-L224)、[创建 / 更新配置链路](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/4530da271ba2e89810d4dccebc57f3091afa590a/src/features/providers/useProviderWorkbench.ts#L637-L761)、[OAuth 完成后入口](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/4530da271ba2e89810d4dccebc57f3091afa590a/src/pages/OAuthPage.tsx#L375-L402)、[认证文件行操作](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/4530da271ba2e89810d4dccebc57f3091afa590a/src/features/authFiles/AuthFilesPage.tsx#L701-L781)。

## CPAR 仍须遵守的决定

参考项目的顺手操作不替代 CPAR 的配置并发、权限及保存回执要求。旧草稿必须合并核对，应用失败必须准确反馈；不能照搬上游实现后误报已生效。删除仍采用用户已确认的“预览影响后一次确认”，不为了模仿参考代码增加重复确认。CPA 无直接对应渠道时，仅复用适用的交互原则，不虚构其能力。

[权限顺序修订](https://github.com/Ricardo121380/cpa-rust-gateway/issues/4#issuecomment-5886103785)已由用户确认；完整第三版仍待[实际操作走查](https://github.com/Ricardo121380/cpa-rust-gateway/issues/7)。未确定新的后端资源模型或接口，也未修复生产代码。
