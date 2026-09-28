# Prism 前端优化与本地验收

日期：2026-09-28。基线：`5497f92`，开始实施时无已跟踪文件改动。

**本轮优化已在本地完成。** 浏览器验收使用 EgoLite TaskSpace 23，以及生产前端构建 + 本地合成 API。真实 Rust 网关、真实账号与生产环境未参与本次验收。

## 交付内容

| 项目 | 实际修改 |
| --- | --- |
| 按钮层级 | 默认按钮恢复珠光次操作；主操作显式使用 `primary`。43 个业务界面文件补充层级，删除/回滚确认保留危险操作语义。 |
| 待应用任务岛 | 860px 上限、琥珀标记、两行草稿状态、单一“查看变更”入口。进入核对页后显示禁用的“正在核对”。窄屏保留“当前服务未改变”。 |
| 徽章 | 7px 圆角、轻边框；只有 `StatusBadge` 保留状态圆点。警告与严重状态的文字颜色通过对比度复算后调整。 |
| 总览 | 保留当前代码已存在的四项指标、真实请求趋势和关注面板；改善说明文字字号、移动端分隔线，并明确时间范围、当前运行快照和部分加载的统计边界。 |
| 动效 | 新增独立选择底板组件，覆盖侧栏、请求时间范围和趋势指标。读取 `--motion-select`，更新 SVG 几何属性；处理中断、隐藏、滚动、尺寸变化和卸载。 |
| 辅助偏好 | 系统与会话减少动效均立即归位；强制色模式隐藏装饰底板，使用系统色边框保留选中态。 |

资源工作区各有一个主要入口；草稿任务岛作为全局核对入口独立存在。筛选、刷新、翻页、关闭、普通行操作和完成回执保持次级层级。

草稿任务岛没有使用不完整分页结果生成变更总数。完整净变化计数仍在核对工作区展示；本次 fixture 流程读取到 23 项。

总览差异报告对应较早快照。当前基线已有请求终态分桶序列与完整总览结构，因此本轮保留这些能力，修正呈现与统计说明。

### 主要文件

- [app.css](../../web/prism/src/app/app.css)、[v6.css](../../web/prism/src/app/v6.css)：按钮、徽章、任务岛、状态文字对比度。
- [DraftDock.tsx](../../web/prism/src/app/DraftDock.tsx)：核对入口及当前工作区状态。
- [SelectionIndicator.tsx](../../web/prism/src/components/motion/SelectionIndicator.tsx)、[selection-indicator.css](../../web/prism/src/components/motion/selection-indicator.css)：装饰动效与降级。
- [RequestHistory.tsx](../../web/prism/src/features/monitoring/RequestHistory.tsx)、[requests.css](../../web/prism/src/features/monitoring/requests.css)、[OverviewPage.tsx](../../web/prism/src/features/overview/OverviewPage.tsx)：总览呈现与数据范围说明。

43 个按钮分类文件经 TypeScript AST 对比：移除 `className` 属性后，与基线一致。路由、事件处理、禁用条件和业务数据逻辑没有随分类调整而改变。[核对结果](assets/prism-frontend-polish-20260928/button-ast-review.json)

Sheet、OperationBoundary 和配置发布机制保持现有实现。动效不写行内样式、不新增依赖，继续输出既有四个固定文件。跨边界修改已登记于 [cross-boundary-log.md](../cross-boundary-log.md)。

## 验收结果

| 检查 | 结果 | 证据/范围 |
| --- | --- | --- |
| TypeScript | PASS | `npm run type-check` |
| 相关单元测试 | PASS | 9 文件、58 测试；配置核对与生命周期、请求记录、账号授权、模型连接、修订边界 |
| 完整构建门槛 | PASS | `npm run check:full`；契约、浏览器存储、请求入口、源码行内样式、严格 CSP、资源修订、四文件输出、两次构建字节一致。[日志](assets/prism-frontend-polish-20260928/build.txt) |
| 页面矩阵 | PASS | 8 个主要页面 × 3 个尺寸 × 2 个主题，共 48 个状态；无可见内容裁切或文档横向溢出。 |
| 工作区矩阵 | PASS | 6 个尺寸/主题组合；左右内边距均等，桌面 24px/窄屏 16px，页脚按钮 44px，窄屏输入 16px。 |
| 稳定状态文字对比度 | PASS | 374 个按钮、徽章及链接样本，最低 4.862:1；不包含禁用控件。按浏览器解析颜色、渐变端点和祖先背景复算。[结果](assets/prism-frontend-polish-20260928/contrast.json) |
| 严格 CSP 运行时 | PASS | 最终生产构建在网关同款 CSP 下加载、导航、核对、确认与完成，无运行异常或 CSP 违规。API 为本地 fixture。 |
| 选择动效算法 | PASS | 受控 16ms 帧时钟：时间范围、趋势模式、导航与中断导航均经过 13 个不同位置，最终误差 ≤0.005px。 |
| 动效辅助偏好 | PASS | 系统和会话减少动态效果均为 `0ms`，立即归位；强制色三个底板隐藏、选中态使用一致的系统色边框。 |
| 实际动画帧率/流畅度 | NOT_RUN | 本次 EgoLite 原生帧回调约 1000ms，不能用该环境认证真实显示帧率。受控时钟结果只证明插值和中断逻辑。 |
| Playwright E2E 套件 | NOT_RUN | 按用户规则使用 EgoLite，覆盖 smoke/glass/contrast/narrow/workspace-visual 的相关场景；未声称完整旧套件已执行。 |
| 真实网关/Provider/生产发布 | NOT_RUN | 本次为本地前端验收；没有部署或真实账号写入。 |
| 最终 diff | PASS | `git diff --check`；无无关已跟踪文件修改。 |

### 页面矩阵

页面：运行概览、账号、提供商、模型、客户端密钥、请求日志、用量与费用、设置。

| 尺寸 | 浅色 | 深色 |
| --- | --- | --- |
| 1440 × 900 | [8 状态 PASS](assets/prism-frontend-polish-20260928/matrix-1440-light.json) | [8 状态 PASS](assets/prism-frontend-polish-20260928/matrix-1440-dark.json) |
| 1280 × 720 | [8 状态 PASS](assets/prism-frontend-polish-20260928/matrix-1280-light.json) | [8 状态 PASS](assets/prism-frontend-polish-20260928/matrix-1280-dark.json) |
| 390 × 844 | [8 状态 PASS](assets/prism-frontend-polish-20260928/matrix-390-light.json) | [8 状态 PASS](assets/prism-frontend-polish-20260928/matrix-390-dark.json) |

测量排除折叠内容与可水平滚动的宽表格。最初的用量页裁切提示来自关闭的 `details` 子节点，`checkVisibility` 与截图确认其未显示；没有据此修改布局。

主代理人工查看了桌面/窄屏、深浅总览、账号、用量、工作区及强制色代表截图。48 个页面截图保存在各矩阵 JSON 的 `screenshot` 字段路径。

### 工作流与配置边界

- 登录失败提示与 fixture 登录成功可用。
- 干净表单按 Esc 关闭，焦点恢复到“添加提供商”。未保存表单拦截导航；“继续编辑”保留输入，路由和底板停在提供商页。[工作区结果](assets/prism-frontend-polish-20260928/workspace-flow.json)
- 减少动态效果开关支持键盘 Space；38 × 22px 视觉轨道由整行 `label` 提供大于 44px 的命中区域。[辅助偏好结果](assets/prism-frontend-polish-20260928/accessibility.json)
- 点击任务岛只发出读取请求。核对完整差异后“校验并应用”发出 `/validate`；只有点击“确认应用”才发出 `/publish`。处理中控件禁用，完成后上下文变为 active，任务岛和确认窗消失。[完整流程](assets/prism-frontend-polish-20260928/configuration-flow.json)
- 三个玻璃外壳均有真实 displacement map；草稿 frost/saturation 为 20/1.2，应用完成后侧栏为 9/1.85。减少透明度与增强对比度正确关闭 backdrop-filter。

本地验收脚本曾把发布路径误写为 `/apply`、读取了错误的 SVG 色彩矩阵节点；检查实际请求和滤镜后修正为现有 `/publish` 与 `type="saturate"`，重置合成数据并重跑通过。业务代码与 API 未因测试修正而变化。

## 实施范围与后续

本轮动效覆盖导航与总览两个分段触点。Toast 编排、面板入场和异步按钮宽度冻结尚未实施；后续应分别明确需求，再进入各自的交互验收。

实际帧率仍需要在不受节流的显示环境核对。生产验收需要单独的发布授权；当前交付为未提交、未部署的本地改动。验收专用浏览器空间与本次启动的两个本地服务在交付前关闭。

### 代表截图

- [桌面浅色总览](assets/prism-frontend-polish-20260928/matrix-overview-1440-light.png)
- [桌面深色总览](assets/prism-frontend-polish-20260928/matrix-overview-1440-dark.png)
- [窄屏浅色总览](assets/prism-frontend-polish-20260928/matrix-overview-390-light.png)
- [窄屏编辑工作区](assets/prism-frontend-polish-20260928/workspace-390-light.png)
- [确认应用](assets/prism-frontend-polish-20260928/confirm-apply.png)
- [强制色模式](assets/prism-frontend-polish-20260928/forced-colors.png)
- [动效受控时钟结果](assets/prism-frontend-polish-20260928/motion-controlled-clock.json)
