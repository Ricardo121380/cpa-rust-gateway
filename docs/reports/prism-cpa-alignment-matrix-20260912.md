# CPA / CPAR 日常管理流程差异

2026-09-12，实施中的源代码对照，不是上线完成报告。
范围与剩余工作见 [本轮执行记录](../handoffs/prism-cpa-alignment-execution-20260912.md)。

## 来源与阅读覆盖

- [CPA-Manager-Plus](https://github.com/seakee/CPA-Manager-Plus/tree/e19d8267a52ca146c43bdb86d185bb7baed7ff38)：
  `apps/web/src` 的 440 个非测试 TypeScript 模块已建立导入、处理函数、API 调用、文案键索引。
  阅读路由、布局、账号/OAuth 手册和主工作区，沿保存、导入、状态、模型选择、配置合并和系统读取追踪关键实现。
- [官方 Management Center](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/tree/ed5f1c48e11ba7335f1e8f676f228c280196af85)：
  `src` 的 283 个模块同样建立完整结构索引；阅读路由、认证文件管理 hook、OAuth/API、提供商工作台与表单、配置文档保存和系统页。
- [CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI/tree/5b2785617d1e7de84a9f4dee599d275a4ccd8999)：
  `internal/managementasset/updater.go` 默认加载上述官方 Center 的 `management.html`。
  核对 `internal/api/handlers/management` 的账号身份、配置/认证接口，以及 `sdk/cliproxy` 的配置和认证更新连接。
  它不是第三套独立 Web SPA。

CPAMP 的演示站通过 EgoLite 核对账号、提供商、OAuth、配置、请求监控、用量、日志、系统和总览。
演示数据只用于观察交互，不是 CPAR 的联调证据。首次仅改 hash 的部分捕获出现旧内容，已废弃，
采用目标路由重新加载后的捕获。源码固定版本、结构索引和浏览器记录存于
`output/prism-cpa-alignment-20260912/`。上述覆盖不声称逐行完成三个项目的安全审计。

## 对齐矩阵

| 用户任务 | 参考实现与来源路径 | CPAR 本轮开始时 | 本轮落实方式 |
|---|---|---|---|
| 添加账号 | CPAMP `features/accounts/AccountsPage.tsx`；Center `features/authFiles/hooks/useAuthFilesData.ts`：上传、粘贴、类型/状态筛选、批量操作 | 已按真实身份和渠道分组；原生导入仍被配置草稿阻挡，必须手填导入标记 | 原生账号直接接入；普通账号按需准备配置；自动内部标记；有界文件预览、逐项结果、常用批量维护 |
| 登录授权与更新凭据 | 两项目 OAuth API/页面；CPA `auth_files.go::authEmail` 从 metadata/attributes 获取身份 | 授权/导入已采集身份，但列表增加“读取身份”；SSO 缺少更新入口 | 删除身份补救按钮；身份取得仍属于授权、导入、凭据更新。保留真正重新授权；SSO 更新凭据，失败资料如实表达 |
| 启用、停用、移除 | Center 账号 hook 的单项和批量状态/删除；CPAMP 统一账号操作 | 普通账号要先手动进入草稿；原生账号缺少等价操作 | 统一操作格式与确认；账号级 revision 冲突；运行配置同步；保留历史请求与费用 |
| 账号状态、额度与模型 | CPAMP 账号的健康/额度/模型详情；Center `features/quota/providers/*` | 已有权益、目录、认证/调度、失败诊断；不同事实曾混合展示 | 身份与渠道贯穿列表和运行视图；认证、调度、并发、额度、目录分别表示。已有未观测值保留未知 |
| 配置提供商 | CPAMP `features/aiProviders/*Edit*`；Center `features/providers/sheets/forms/BaseProviderForm.tsx`：类型、地址、凭据、模型、保存 | UI 分为上游、端点、绑定、出口对象；新用户无法从空状态直接完成 | 提供商表单组织名称、渠道、接口地址、账号和模型；内部资源自动生成，高级对象编辑继续可用 |
| 模型映射/开放模型 | 两项目提供商的 model entries、discovery 选择和别名；Center `ModelDiscoveryPanel.tsx` | 有有效模型/provenance、完整 Route/Candidate/Alias，但接入动作分散 | 复用权威目录或输入 exact ID，选择连接并开放模型；默认路由与候选由流程建立；保留显式授权范围与高级路由 |
| 客户端密钥 | 两项目 `services/api/apiKeys.ts`；配置页新增、修改、停用密钥 | CPAR 组/路由/密钥分开，需要先手组权限关系 | 创建密钥时选择开放模型和有效期，自动形成对应授权组；秘密仅签发时显示一次，后续不返回明文 |
| 保存生效 | CPAMP provider save 后重读；Center `useConfigDocument.ts` 重读、冲突预览、保存；CPA watcher/auth manager 更新运行对象 | 只发布 RouteSnapshot，执行器与账号池还绑定启动配置，需要重启 | 先准备完整运行代际，再提交精确版本与原生账号时钟，统一切换鉴权/执行/管理投影；失败保留当前配置；前端提供保存并应用 |
| 总览 | CPAMP `features/dashboard/DashboardPage.tsx`；Center dashboard：连接、资源数、请求与用量、快捷入口 | 首页仍较多底层版本/尝试计数，空安装没有明确接入链 | 强调服务、账号、开放模型、真实请求/费用和下一步；累计 attempt 不作为请求成功率 |
| 请求、失败、日志 | CPAMP monitoring/logs；Center logs：筛选、详情、分页、刷新 | 已有持久请求、失败、attempt 深链，诊断以内部对象呈现 | 账号/模型/状态可读筛选与详情；保留失败到诊断链；日志与请求事实分开，不添加无来源日志开关 |
| 用量与价格 | CPAMP usage analytics；Center quota/usage | CPAR 有真实物化账本、六类 token 及置信度、unpriced 和处理状态 | 把来源、区间、账号/模型筛选与费用详情放在日常任务中。未定价不计作零费用，额度不冒充消费 |
| 设置与系统 | CPAMP/Center SystemPage：连接、版本、构建、模型和配置；config 的路由/重试/网络项 | 设置主要是浏览器偏好，服务信息缺少入口 | 增加安全服务信息和现有运行配置入口；提供真正可保存的选项，进程级设置明确只读；高级配置/审计留在设置内 |
| 会话与外观 | 两项目主题/语言/布局及认证状态 | CPAR 管理员密码与内存会话、V6 Liquid Glass 已有 | 保持管理员登录、辅助偏好、桌面/手机统一布局、四文件/CSP；不复制参考项目的明文密钥读取或浏览器秘密持久化 |

## 保留的差异

CPAR 管理面与 Rust 数据面使用自己的权威 OpenAPI、加密存储、revision、授权组和 serving snapshot。
对齐用户能完成的任务，不把 CPA YAML 字段、明文 API Key 列表、任意 API-call 转发直接搬进 CPAR。
参考项目的插件、供应商赞助接入、Autoreg、备份恢复、密钥导出和未支持的 OAuth/discovery/媒体协议
不自动成为本次支持能力。Grok Web、Console、Build 继续各自独立；未知身份不会生成假邮箱。

所有实施项的完成证据应写入后续交付报告；此矩阵不代表上述目标已经全部实现或已经部署。
