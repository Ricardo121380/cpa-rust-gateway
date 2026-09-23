# V2 文字边距与弹窗精修（2026-09-23）

本轮针对用户反馈的“文字靠边、显示不全”修正实际生产前端代码，并在本地真实嵌入网关验收。沿用 V2 设计、现有 React 组件与业务流程；没有部署生产，也没有触发真实渠道授权或推理。

## 发现与修改

| 问题 | 原因与处理 | 代码 |
|---|---|---|
| 多处区块左侧留白被清零 | 删除已经退出 V2 布局的 rail-underlap 全局规则；各面板拥有自己的内边距，避免所有 section 子元素被重置 | `web/prism/src/app/app.css` |
| 账号运行状态筛选框、分组标题与底栏贴边 | 有边框面板的工具栏/底栏恢复 18px 水平边距，桌面分组标题对齐表格；移动账号区保留 16px | `web/prism/src/app/v6.css` |
| 手机指标数字紧贴圆角面板 | 移除奇数指标项 `padding-left:0` 覆盖，保留每格 14px 留白 | 同上 |
| 提供商选中行文字贴蓝色边线 | 所有提供商行使用一致的 12px 水平内距，选中时不跳位 | 同上 |
| 长身份/连接/状态可能挤出固定列 | 账号单元格允许换行；去掉运行摘要 120px 最小宽度；手机值列使用 `minmax(0,1fr)` | 同上 |
| 弹窗表单长标签、下拉框、操作区空间不足 | 正文最小宽度归零，标签/legend 受容器约束并可换行，下拉箭头预留空间，操作按钮可在边界内换行 | `web/prism/src/design/modal.css` |
| 手机授权渠道名称碎行 | 420px 及以下改为紧凑单列，正文可滚动，取消按钮固定在页脚；桌面仍双列 | `web/prism/src/components/workflow-forms.css` |
| 账号两种视图的标题/页签顺序不同 | `AccountsPage` 传递同一个导航节点，运行状态页将它放在标题之后 | `features/accounts/{AccountsPage,AccountRuntimePanel}.tsx` |

使用 `frontend-design`、`ui-ux-pro-max`、`frontend-ui-craft` 约束视觉精修；React 改动按 `vercel-react-best-practices` 保持组件组合，不增加请求、订阅或依赖。没有改造真实邮箱、模型 ID 或内部关联。

## 本次验证

- `npm --prefix web/prism run sync-contract`：契约无变化。
- `npm --prefix web/prism run check`、`type-check`：通过。
- `node scripts/check-management-spa.mjs`：最终修改后通过双构建、生成客户端及四文件门禁，152 个生成操作。
- 账号相关现有回归：`AddAccountDialog.test.ts`、`AccountRuntimeSummary.test.ts`、`presentation.test.ts`，**3 文件 / 12 测试通过**。
- `cargo build -p gateway --bin gateway`：通过，重启同一个临时状态目录的本地网关。
- 实际 HTTP 返回的四个文件逐字节等于最终 `web/prism/dist`；SHA-256 前缀：入口 `69ff3ab26b5e8496`、main `e7ae7fdb379d7ee7`、vendor `29a0a30425bd5cae`、CSS `04a848d2c3092fec`。入口由 `/admin-ui/` 提供。
- `git diff --check`：通过。

## EgoLite 实际浏览器证据

通过 Computer Use 连接 EgoLite 操作真实本地应用，非静态 HTML。验收地址 `http://127.0.0.1:62992/admin-ui/`，合成账号与 loopback mock 环境。浏览器连接层显示 Chrome，实际应用为 `com.citrolabs.ego.lite`。

| 检查 | 结果与截图 |
|---|---|
| 1280×720 账号运行状态 | 工具栏 `16px 18px`，分组标题左右 18px，标题位于页签上方；[截图](assets/prism-v2-spacing-20260923/runtime-1280.png) |
| 1280×720 提供商选中详情 | 蓝色选中边与头像、文字、操作分离；[截图](assets/prism-v2-spacing-20260923/provider-selected-1280.png) |
| 1440×900 账号表格 | 当前数据的单元格及 main 无水平溢出；[截图](assets/prism-v2-spacing-20260923/accounts-1440.png) |
| 390×844 全部账号与运行状态 | 卡片和操作完整显示；运行分组 16px、指标 14px 留白，main 无水平溢出；[全部账号](assets/prism-v2-spacing-20260923/accounts-390.png)、[最终运行状态](assets/prism-v2-spacing-20260923/runtime-390.png) |
| 390×844 渠道选择 | 长渠道名称完整阅读；滚动后可选最底部 Kiro 并进入授权说明，未点击授权登录；[截图](assets/prism-v2-spacing-20260923/channel-chooser-390.png) |
| 390×844 Kimi 说明 | 标题、说明、底部按钮均在边界内；[截图](assets/prism-v2-spacing-20260923/kimi-dialog-390.png) |
| 1440×900、390×844 密钥表单 | 输入较长中英文名称，Tab 移至有效期出现可见焦点；手机弹窗 x=12、宽366，正文与操作按钮无水平溢出；取消→未保存确认→放弃正常，未创建密钥；[截图](assets/prism-v2-spacing-20260923/key-form-long-390.png) |
| 1280×720 深色模型页与接入表单 | 文字/边框留白、下拉框及页脚完整，弹窗正文无水平溢出；[模型页](assets/prism-v2-spacing-20260923/models-dark-1280.png)、[表单](assets/prism-v2-spacing-20260923/model-dialog-dark-1280.png) |
| 390×844 设置辅助偏好 | 减少透明度、增强对比度、减少动态效果开启后内容和开关正常；[截图](assets/prism-v2-spacing-20260923/settings-accessible-390.png) |
| 1440×900 总览、请求、用量 | 截图人工复核，main 无水平溢出，检查的按钮/标签/段落/三级标题未发现被 hidden/clip 横向裁切；[总览](assets/prism-v2-spacing-20260923/overview-1440.png)、[请求](assets/prism-v2-spacing-20260923/requests-1440.png)、[用量](assets/prism-v2-spacing-20260923/usage-1440.png)、[DOM 检查](assets/prism-v2-spacing-20260923/page-checks.json) |

浏览器本次读取的 error 日志为空。最后一次修改仅修正手机运行面板的两个间距覆盖，重新构建、字节比对和手机截图已完成。之前其他页面截图中的样式未再变动。

## 范围与限制

- 这是本次间距与裁切修复的针对性验收，不代表整个 V2 全部状态重新跑完，也不替代真实账号授权验收。
- 单行输入框的长名称仍按原生输入框左右滚动编辑，不改成多行账户名称。此次没有为验收添加生产账号或更改正式配置。
- 没有跑完整 Rust 测试或全量业务 E2E：本轮是 CSS 与账号导航位置调整，使用类型/构建门禁、相关单测和实际页面交互验证。
- 本地网关保持运行并保留登录预览；生产域名仍使用此前部署版本。
