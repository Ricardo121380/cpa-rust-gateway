# 全渠道账号闭环检查 — 2026-09-27

检查基线：b696083。范围：普通账号与原生 Grok 的授权/导入、接口绑定、身份、目录、额度及前端消费。方法：两个独立只读代码检索后按关键出处复核；未执行真实授权、生产变更、元数据调用或推理。结论为实现审计，不是线上逐渠道验收。

## 必须先修的共同缺陷

1. **高：Codex/Claude/Kiro/Kimi Coding 首次导入仍可能零绑定。** `web/prism/src/features/accounts/AddAccountDialog.tsx:34` 非 API 导入返回空接口；`:130` prepare 只保留 upstream_id；`:146` 仅在接口非空时绑定。后端 `crates/gateway-http-actix/src/management_resources/account_channels.rs:1028` 只保存 credential。应使用本次 prepare 返回的明确 endpoint_id，不能从历史列表随便挑接口。Kimi b696083 仅修复设备授权组件，不覆盖此导入分支。
2. **高：验收断言没有覆盖业务终点。** `AddAccountDialog.test.ts:11` 断言命名渠道不使用观察到的端点，这条安全约束可以保留，但缺少“prepare 返回精确端点→导入→绑定→发布→运行重读”的集成验证。此前单测通过不能证明完成接入。
3. **中：额度不是跨渠道已闭环能力。** `AccountEvidenceTabs.tsx:30` 通用额度正文仍为“未提供数值观测”；Kimi 是独立新分支。套餐标签不能作为实时余额。

## 渠道矩阵

| 渠道 | 绑定/身份 | 模型目录 | 额度/套餐 |
| --- | --- | --- | --- |
| Codex/ChatGPT | 首次授权组件会绑；导入存在上述缺陷。身份依赖 OAuth/导入/JWT 内容 | CodexCatalogAdapter 真实请求专用 models；实现不等于本轮实测成功 | 标签/令牌套餐投影，非实时剩余额度 |
| Claude | 首次授权会绑，授权实现有 email/profile 提取；导入有缺陷 | 官方准备目标 models_path=None，而通用发现要求 models_path，因此默认授权后没有自动目录目标 | 导入套餐标签，未见数值余额闭环 |
| Kiro | 设备授权会绑；导入有缺陷。保存 envelope 无单独身份字段，JWT 是否含身份本轮未验证 | IDE OAuth 有分页 ListAvailableModels；不能推广到 CLI/API Key | getUsageLimits 订阅解析代码存在，已查运行层没有接线证据 |
| API compatible/Krill | API 流程显式选择接口；不能假定任意 API Key 都能查询邮箱 | models_path 配置后走通用真实目录请求 | 未见通用实时余额抓取；需供应商能力核实 |
| Grok Build | 原生账号按 provider 建池；身份依赖指纹关联观测 | Build 真实专用 models 适配器 | 有套餐观测，生产刷新入口仅 Refresh，未见 Quota worker 接线 |
| Grok Console/Web | 导入尝试读取身份，但失败可仍保存；需显示独立身份结果 | 当前 catalog worker 无该两类目标，不能称完整上游动态目录 | Console 有解析器但未见生产调用；Web quota decoder 明确仅用于 synthetic fixtures |
| Kimi Coding | b696083 修设备授权绑定及元数据；导入分支仍有缺口 | 绑定后可进入兼容目录；真实模型协议仍待核对 | 本地已实现官方 me/usages，尚未部署及真实账号验收 |

## 证据定位

- 首次授权/续授权：`web/prism/src/features/accounts/AuthorizationCodeDialog.tsx:37`、`KiroDeviceDialog.tsx:33`；账号页续授权传原 owner/credential、空 endpoint，避免修改已有绑定。
- Claude 默认目录缺口：`crates/gateway-http-actix/src/management_resources/account_channels.rs:158`，`apps/gateway/src/runtime.rs:3643`。
- Kiro 身份 envelope：`crates/gateway-http-actix/src/management_resources/kiro_device.rs:337`；usage 解析 `crates/provider-kiro/src/dynamic_catalog.rs:123`。
- Grok 生产 worker：`apps/gateway/src/credential_refresh.rs:276`；Web fixture-only 声明 `crates/provider-grok/src/web_quota.rs:180`。
- 目录 adapter 分支：`apps/gateway/src/runtime.rs:3631`；兼容/IDE 请求 `apps/gateway/src/runtime/catalog_refresh.rs:136`、`:206`。
- 原生账号身份：`crates/gateway-http-actix/src/management_resources/native_accounts.rs:154`、`:330`；前端原生详情复用通用额度页 `web/prism/src/features/accounts/NativeAccountDialog.tsx:62`。

“未见调用”的检索范围是 apps/gateway/src、gateway-http-actix/src、gateway-control/src 与对应 provider；不推断第三方 API 不存在，也不推断现网无历史外部观测。Codex 旧 OAuthWizard 仅确认接受现有 credentialId，未完成整个续授权路径的额外审计。

## 修复优先级及完成标准

1. 统一四渠道导入绑定，补批量重复导入、部分绑定失败、发布冲突、不重新启用停用绑定的回归；发布前重读实际运行池。
2. 逐渠道列出身份/目录/额度能力与来源。明确区分“不支持、未实现、未观测、读取失败、成功空值”，不能统称未知。
3. Claude 默认目录、Kiro 身份/usage、Grok Console/Build quota 优先核对官方真实来源后接线；Web 不得把 fixture decoder 当生产实现。兼容 API 缺统一身份/余额协议时提供真实能力说明。
4. 每条验收从空账号状态起：授权或导入→绑定→身份→目录→独立额度。mock 覆盖失败/分页/过期；生产元数据和必要官方登录另列，真实推理遵守现有额度。

本轮只有检查报告，无产品代码修改、生产发布或推理消耗。Kimi 发布确认仍未回答，不能将此次“检查其他渠道”当作发布授权。
