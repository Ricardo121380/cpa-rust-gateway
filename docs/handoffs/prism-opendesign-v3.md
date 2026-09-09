# Prism Liquid Glass V3 设计交接

日期：2026-09-09。状态：**完整栏目交互原型已交付，真实前端尚未实施**。

后续细化：用户继续要求优化整体排版与配色，当前视觉以 [V4 交接](prism-opendesign-v4.md)为准；本文件保留完整栏目、交互及契约清单，V3 原型未覆盖。

用户明确要求 Apple Liquid Glass、完整栏目、OpenDesign MCP 与本会话模型，禁止模型 CLI。V2 的三页与 Linear 方向偏离要求，本稿更正这些问题。正式应用目录仍是 `web/prism/`；本次没有改动该目录、Rust 或 API 契约。

## 交付

- [完整交互原型](../design/prism-liquid-glass-v3.html)：现有 12 栏目 + 新增账号池、模型目录 = 14 栏目，另有解锁页。
- [更新后的开发方案](prism-development-plan-2026-09-09.md)：保留 F0 问题与后端接口依赖，把全部栏目纳入完成条件。
- OpenDesign 项目：`Prism Gateway Console — Redesign`，ID `prism-gateway-console-redesign-a735`；文件 `prism-liquid-glass-v3.html`。
- 原型 SHA-256：`42b7c7fa766c2615b6b1ed30d2279007393fc4e52d27e6e58a9ec65ebe477e57`。
- 当前 daemon 会话预览：[账号池](http://127.0.0.1:60076/api/projects/prism-gateway-console-redesign-a735/raw/prism-liquid-glass-v3.html#/accounts)。daemon 重启后请从项目重新取得地址；仓库单文件副本可独立打开。

## 工具与模型来源，准确记录

实际调用 OpenDesign 的 `resources/read`、`get_project`、`create_artifact`、`write_file`、`get_artifact`。读取 `od://skills/frontend-design/SKILL.md`、Apple 设计参考与 `apple-hig` 目录项。后者仅是上游 skill 的介绍，不声称已安装或执行完整上游 HIG workflow。

设计、内容编排与源码均由本会话模型生成；MCP 用于资源、项目、文件版本与预览。**没有调用 `start_run`，没有运行 OpenDesign 内置生成器或其模型 Agent。** MCP 返回版本记录的 source 为 `manual`，符合外部会话编辑路径。没有模型 CLI、Cloud、BYOK、全局模型配置变化。MCP stdio 桥接进程只是传输服务，不是模型执行器。

不要把本稿称为“OpenDesign 内部模型生成”。这是按用户指定模型来源完成的设计，由 OpenDesign MCP 保存与展示。原有 `index.html`、V2 HTML 保留，不以覆盖旧文件的方式伪装生成。

## 视觉约束

以 `web/prism/DESIGN.md` 为权威，直接复用仓库 `tokens.css`、`glass.css` 的材质配方；把 `PrismLens.tsx` 中几何、滤镜更新与换态函数移植为独立脚本。固定彩色环境、棱镜光带和 grain 位于玻璃背后；顶栏 / 侧栏 / 草稿坞最多三面 chrome 玻璃。数据内容实底，表面延伸到导航下方，滚动时经过顶栏遮罩。

保留渐变 tint、高光环、边缘辉光、分层阴影与短边受限的 SVG 位移图。支持 SVG 的预览浏览器实际计算为 `backdrop-filter: url("#prism-lens-topbar")`；不支持时使用原有 blur 回退。浅色 / 深色沿用 Apple 蓝与系统 SF/PingFang 字体；独立原型调整壳体几何为更宽内容边距，保留材质预算。它是 Web 材质实现，不是原生 SwiftUI 组件。

浅色 good / warn 徽章前景以原语义 token 76% + `--ink` 24% 混合，保留语义底色。对原型实底的计算对比度约 5.47:1 / 4.67:1；旧未混合值约 3.93:1 / 3.26:1。正式落地应记录这项文字对比度改进，并重新测实际玻璃叠层，不套用旧截图的数值。

任务编排参考 [CPAMP 固定版本](https://github.com/seakee/CPA-Manager-Plus/tree/1ae656c82990c480f3f104326a08c6e0001eeb4c)，材质原则参考 [Apple Liquid Glass](https://developer.apple.com/documentation/technologyoverviews/liquid-glass)。OpenDesign 的普通 Apple 网页资源偏营销与零售，不能用来覆盖本项目已确认的 Liquid Glass 规范。

## 栏目覆盖

| 导航 | 原型路由 | 原有组件 / 来源 | 设计内容与主要交互 |
|---|---|---|---|
| 总览 | `#/overview`（正式保留 `#/`） | OverviewPage | 进程累计 attempts、异常入口、空账本、目录与配置导航。 |
| 请求与失败 | `#/monitoring` | MonitoringPage | 失败归因 / 计费账本；搜索、Request ID 定位、尝试详情、账号与 Explain 深链。 |
| 用量分析 | `#/usage` | UsagePage | 平台 / 账号 / 模型分组，六类 token、各自置信度、分布和聚合明细。 |
| 计费与价格 | `#/billing` | BillingPage | 全局价格目录 / 版本价格策略；条目详情、示例导入、草稿绑定。 |
| 账号池 | `#/accounts` | 从 RuntimePage 提取；复用池投影与凭据组件 | 平台、关键词、状态过滤；运行状态 / 权益证据 / 配置与失败三类详情；接入步骤。 |
| 上游 | `#/upstreams` | UpstreamsPage、SubresourcePanel、CredentialSheet | 上游卡片、连接详情、端点和凭据、创建与编辑草稿。 |
| 模型目录 | `#/catalog` | 从 RuntimePage 目录视图提取 | Fresh / Stale / Expired / Missing 筛选；精确 target、snapshot、refresh_due 与失败证据。 |
| 模型与路由 | `#/models` | ModelsPage、RouteWorkbench | 公开模型 / 路由工作台；模型编辑、按 ID 定位路由、别名与候选创建的提交前预览。 |
| 访问控制 | `#/access` | AccessPage | 访问组 / Client Keys；路由授权详情、示例签发、启停确认、前缀显示。 |
| 运行诊断 | `#/runtime` | RuntimePage | 可用性矩阵 / Route Explain / Channel Pin；异常过滤、恢复确认和分项验证结果样本。 |
| 出口策略 | `#/egress` | EgressPage、CompatibleProxyPanel、Provider egress | 策略 / 兼容代理 / Provider 出口三域；exact host 与重定向编辑，代理节点与绑定详情。 |
| 配置版本 | `#/versions` | VersionsPage、DraftDock | 活动 / 草稿 / 归档；选择、差异样本、校验、发布确认、回滚确认及审计样本。 |
| 审计与备份 | `#/audit` | AuditBackupPage | 生命周期审计 / 备份预检；搜索、事件详情、源库预检样本；保留部署侧恢复边界。 |
| 设置 | `#/settings` | SettingsPage | 深浅主题、透明度/对比度/动态效果开关、栏目搜索、锁定、状态样本。 |
| 解锁 | `#/unlock` | UnlockPage | 解锁布局、进入演示；原型不接收真实密钥。 |

没有禁用的主导航项。原型路由与筛选可分享；原型的草稿编辑与外观仅存于内存。手机通过玻璃菜单访问全部栏目，账号表改为卡片，其他表格在自身容器内横向滚动。

## 交互完成的范围

平台/关键词/状态过滤、主导航与子视图、账号三类详情、目录 target、请求 attempts、价格条目、路由定位、运行解释、审计事件、辅助外观与解锁均可操作。公开模型、访问组、Client Key、上游与出口策略的部分编辑会更新当前页面内的合成数据；草稿校验与发布演示切换版本和玻璃材质。

差异、Explain、恢复、备份和 Channel Pin 结果是固定设计样本，界面明确标识。别名、候选、绑定、代理等流程展示提交前核对，不能当成完整持久化 CRUD。原型不接收真实秘密，不写浏览器存储，不发 API 或 Provider 请求。正式 API 所有权、revision、409、会话失效与权限回归仍按开发方案执行。

## 数据与契约边界

- 账号认证、调度、权益、quota 与目录独立；不计算 overall health。`null` 保留未观测。权益来源使用当前枚举 `provider_subscription` / `signed_token` / `imported_metadata`。
- 目录模型数不是授权后可调用模型数。Fresh / Stale / Expired / Missing 与是否需要刷新分别呈现。完整有效模型枚举依赖 BE-FE-01。
- 配置公开模型与 exact ID 是示例，不是前端白名单；完整 Route / Candidate / Alias 枚举与候选维护依赖 BE-FE-02。
- 用量页展示范围聚合与六类 token 的观测状态，不捏造时间趋势、P95、RPM 或请求级成功率。账本为空不能推断没有消费。相关 worker/规模问题与 BE-FE-03 继续保留。
- 计费价格目录全局可见，价格策略属于配置版本；账号池读取跨版本，失败归因与诊断按各自版本范围读取。

## 已验证与交付限制

- `node --check` 通过；OpenDesign MCP 创建、迭代和读回核对。原有项目 HTML 与版本保留。
- 已在 OpenDesign 原生应用打开 V3 文件并复测总览 → 账号池导航。修正了内嵌预览注入 base URL 后相对锚点跳到错误地址的问题；导航显式更新当前文档 hash，筛选 replaceState 使用当前完整 URL。
- 桌面逐项点击 14 个栏目，每页有独立内容；1280×720 下全部导航可见。查看了宽屏浅色账号与深色设置画面。
- 390×844 逐项点击 14 个栏目，页面画布没有横向溢出；桌面长表由自身容器处理，手机账号卡和权益详情已视觉检查。
- 已检查平台筛选、过滤空态恢复、null 权益、Escape 关闭、创建公开模型到草稿、校验与发布后从 3 面到 2 面的材质换态。
- 减少透明度与增强对比度开关的计算滤镜为 `none`，增强对比度保留 1px 边界；减少动态效果使 `--dur-anneal` 变为 `0ms`。系统媒体查询规则保留，但未在所有平台逐一模拟系统偏好。
- 运行诊断的 Explain 与 Channel Pin 确认/分项结果样本可达。浏览器检查未捕获 error/warn。

没有执行真实管理 API、Provider 验证、生产部署、全套前端回归、所有浏览器或屏幕阅读器认证，也没有完成全场景玻璃背景逐像素对比度评测。以上原型检查不能替代正式实现的验收。

## 实施交接

先执行开发方案 F0 的契约、版本归属与会话修复。再把 V3 的全部栏目映射到现有组件，保留旧功能、URL 和双语言键。不能把独立 HTML 的内联脚本/样式直接拷入生产入口；生产仍要求 Vite 四文件、CSP、无存储、统一 generated client 与真实管理同源验证。前端工作归 `web/prism/**`，实际跨边界修改遵守根目录日志与提交 trailer。
