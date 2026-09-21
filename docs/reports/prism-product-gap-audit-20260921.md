# Prism 产品差距复核 — 2026-09-21

## 结论与证据边界

当前不能声明“全面对齐完成”。已有渠道授权、模型来源、权限和事务保护应保留；本轮优先修复的是操作组织与主次，而不是再次重写这些能力。

检查基线：本地 HEAD `f781617`，产品代码 `b518d92`。本轮不修改产品代码、不部署、不调用真实推理。使用 frontend-ui-craft；未使用 Codex with ChatGPT。以下是源码复核及局部真实网关观察，不是全站重新验收。

参考材料为仓库内已有源码快照，不代表参考项目最新 HEAD：

- CPAMP：`output/prism-cpa-alignment-20260912/references/CPA-Manager-Plus/apps/web/src/features/oauth/OAuthPage.tsx:318` 按渠道构建授权入口；`apps/web/src/services/api/oauth.ts:22` 按渠道发起授权并读取状态。
- CPAMP：`apps/web/src/features/aiProviders/AiProvidersOpenAIEditPage.tsx:343,611` 从提供商编辑进入模型发现，并维护多模型输入。
- 官方 Management Center：`output/prism-model-passthrough-20260913/references/Cli-Proxy-API-Management-Center/src/pages/OAuthPage.tsx:381` 为指定渠道建立独立授权尝试，迟到结果通过 attempt 检查隔离。
- CLIProxyAPI：`output/prism-cpa-alignment-20260912/references/CLIProxyAPI/internal/api/server_management.go:178-182` 分别提供 Codex、Kimi 授权和状态入口。它用于核对后端职责，不当成第三套前端。

## 已确认差距

| 编号 | 证据 | 判断 | 下一批行为标准 |
|---|---|---|---|
| G01 | `AddAccountDialog.tsx:194-208`；本轮 Kimi 桌面/手机截图 | Kimi 专用授权已实现，也不再选择 Codex/Krill 接口。但“授权登录”与“导入账号”同时成为蓝色主操作，导入文本框始终抢占主体 | 渠道选择后明确选择授权或导入；授权态不展示凭据导入字段；每态只保留一个主要提交动作 |
| G02 | `ModelsPage.tsx:183-226`、`CatalogPage.tsx:53-68` | 已开放模型和上游目录仍通过独立页面跳转；不能据此说目录能力缺失 | 同一模型工作区切换目录/已开放，保留筛选与来源；旧链接继续可用 |
| G03 | `UpstreamsPage.tsx:249-255` | 提供商目录链接保留 upstream_id，但“开放模型”使用无来源参数入口，用户需重新选来源 | 从提供商发起开放保留来源上下文；不自动选择账号权限或自动开放模型 |
| G04 | `UpstreamsPage.tsx:243-258`；本轮1440截图 | DOM叫card，但实际桌面已为紧凑横行，不能重新报告为“巨型卡片尚未改”。需核对多提供商与详情展开密度 | 多资源数据下检查列表/详情，不凭单资源截图认定整体密度达标 |
| G05 | 计划/台账追加记录与热修复报告 | 局部检查点、上线、整站产品完成混在历史状态里；之前完成结论已更正 | 按功能实现、浏览器验证、人工授权、视觉验收分列，缺证据不记通过 |

上述特性代码路径均以 `web/prism/src/features/` 为前缀。G01、G02属于交互；G03属于流程连续性；G04、G05属于待验证/证据管理，不虚构成功率。

## 八个工作区复核队列

| 工作区 | 已有源码证据 | 当前结论与剩余核对 |
|---|---|---|
| 仪表盘 | `overview/OverviewPage.tsx:304-338` 请求、费用、资源、折叠遥测 | 保留真实指标；尚未补本轮同数据前后视觉比较 |
| 账号 | `accounts/AddAccountDialog.tsx:194-225` | 首先处理G01；真实官方授权不因本轮打开弹窗而重新记通过 |
| 提供商 | `upstreams/UpstreamsPage.tsx:241-260` | 保留现有紧凑行；处理G03，核对多资源与详情工作区 |
| 模型 | `models/ModelsPage.tsx:183-226` | 处理G02；保持exact ID、多来源和手动开放语义 |
| API密钥 | 既有台账的签发、私有组、一次性秘密证据 | 本轮未重新操作，不能升级验收结论；与模型开放流程一同复核 |
| 请求日志 | `monitoring/MonitoringPage.tsx:89-111`及既有终态记录 | 本轮未重跑详情/导出链；需检查筛选到详情的连续性 |
| 用量与价格 | `usage/UsagePage.tsx:316-352`及既有价格内联工作区 | 保留价格容量与历史保护；需统一时间筛选和详情层级 |
| 设置与高级维护 | `settings/SettingsPage.tsx:54-59` | 高级维护已经收起，不重新列为未实现；核对返回路径与会话动作 |

## 本轮浏览器证据

EgoLite，真实本地嵌入 gateway `127.0.0.1:55130`，合成临时状态。仅登录、浏览和选择Kimi渠道，没有发起外部授权、导入或写入。

- [提供商1440×900](evidence/prism-product-gap-20260921/providers-1440.png)：实际为紧凑横行。
- [Kimi1440×900](evidence/prism-product-gap-20260921/kimi-1440.png)：双主操作与始终可见的JSON输入。
- [Kimi390×844](evidence/prism-product-gap-20260921/kimi-390.png)：同一操作竞争延续到手机。

账号页一次宽泛 `waitForSelector('button')` 超时；后续快照确认页面正常出现。本次不把该自动化等待失败归为产品崩溃。浏览器TaskSpace已关闭。

本轮没有生成旧版本同尺寸、同数据截图，因此这些是**当前状态证据**，不是视觉改善前后证明。旧截图与现图不能混用宣称对照验收完成。全站逐操作参考矩阵和全尺寸视觉对照仍未完成。

## 实施顺序

1. G01：账号接入拆分为渠道 → 接入方式 → 专用授权/导入 → 结果。复用现有任务、取消、秘密清理与回执；默认按渠道支持能力展示，禁止将API渠道伪装OAuth。
2. G02/G03：把提供商 → 目录 → 选择开放 → 创建受限密钥串成保留上下文的工作流，后台配置主键不进入日常步骤。
3. 将上述布局与动作层级应用到余下工作区，逐项补参考源码行为映射。不要先全站批量改CSS。
4. 相同合成数据重建旧/新截图，三个尺寸、深浅主题、错误/空/长内容和键盘；补已有标签跨发布刷新。通过后再按既有授权发布。

不为这次局部复核计算“功能完成百分比”；缺少逐操作分母，数字会误导。既有M4/M5技术证据保留，产品对齐结论继续开放。
