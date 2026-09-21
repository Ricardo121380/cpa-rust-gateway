# 接口与凭据编辑表单 — 2026-09-21

## 交付范围

接口表单分为连接设置、实现参数、目录与启用状态。地址独占一行，协议与路径在桌面并排、手机单列；保留真实原始协议和适配器值，不推测兼容关系。凭据编辑分为账号与认证、授权资料两个无卡片边框分区，状态选项中文化但请求值不变。沿用既有 Sheet 的底部操作区、关闭、焦点与未保存保护。

没有更改 DTO、请求体构造、秘密清理、CAS、冲突或回执处理；没有新增依赖、玻璃面或持久化。本批未部署。

## 验证

- TypeScript、契约同步通过；契约没有变化。
- 现有 subresource-crud 浏览器回归 19/19 通过，覆盖创建、编辑、重复/丢失响应和发布结果；日志 `/tmp/prism-resource-editor-e2e.log`。随后仅重排地址位置及调整分组标题，最终构建与实际浏览器检查通过。
- gateway 构建、管理 SPA 四文件与确定性双构建门禁通过；Rust 嵌入回归 3/3 通过。日志 `/tmp/prism-resource-editor-build.log`、`/tmp/prism-resource-editor-gate.log`、`/tmp/prism-resource-editor-embedded.log`。
- EgoLite 使用真实嵌入 gateway：旧版55350，新版55360；相同隔离合成数据的副本。接口弹窗检查1440×900、1280×720、390×844；凭据检查1440×900、390×844及手机深色、减少动效设置。未提交资源或凭据修改，未调用真实上游。
- 手机弹窗无横向溢出；接口路径修改后 Escape 显示放弃确认，“继续编辑”保留输入且焦点回到请求路径；随后明确放弃。凭据保存按钮按 Tab 返回关闭按钮，Escape 可关闭。

## 视觉证据

- [桌面修改前](evidence/prism-resource-editor-20260921/before-endpoint-1440.png) / [桌面修改后](evidence/prism-resource-editor-20260921/after-endpoint-1440.png)
- [手机修改前](evidence/prism-resource-editor-20260921/before-endpoint-390.png) / [手机修改后](evidence/prism-resource-editor-20260921/after-endpoint-390.png)
- [1280接口](evidence/prism-resource-editor-20260921/after-endpoint-1280.png)
- [桌面凭据](evidence/prism-resource-editor-20260921/after-credential-1440.png) / [手机凭据](evidence/prism-resource-editor-20260921/after-credential-390.png) / [手机深色](evidence/prism-resource-editor-20260921/after-credential-dark-390.png)

截图中的无身份账号来自合成样本。本批不代表全站验收或 Safari 验收。下一步继续账号连接与绑定表单的操作层级，之后汇总近几批进行跨工作区整体验证；正式发布仍需执行发布门禁。
