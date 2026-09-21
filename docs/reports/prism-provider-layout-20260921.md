# 提供商工作区布局改进 — 2026-09-21

## 本批结果

桌面缩窄提供商选择区、加宽接口与账号详情，选中项使用 Apple 蓝边标记。合并两个同目标链接为“目录与模型开放”。手机打开详情后将详情置于列表前并转移焦点，关闭回到原按钮；新增展开状态和关联面板语义。

真实本地网关检查发现成功读取仍展示红色 `null` 错误：TanStack Query 无错误时返回 null，原条件只排除 undefined。现同时排除两者，真实异常仍走既有错误及重读处理。

## 验证与证据

- TypeScript 通过；新增三个尺寸的键盘打开、焦点返回、选中态、移动布局及无错误提示回归。
- provider-layout 与 subresource-crud 浏览器回归共 22/22 通过，包含写入冲突、响应丢失、发布回执和账号操作。日志 `/tmp/prism-provider-layout-e2e.log`。
- gateway 构建、管理 SPA 门禁（确定性双构建、四文件）、Rust 嵌入测试 3/3 通过。日志 `/tmp/prism-provider-layout-build.log`、`/tmp/prism-provider-layout-gate.log`、`/tmp/prism-provider-layout-embedded.log`。
- EgoLite 使用同一隔离合成数据库的前后副本，真实嵌入 gateway 55340 / 55350，未模拟管理接口。实际检查 1440×900、1280×720、390×844，手机深色；无横向溢出，打开焦点为 provider-detail，关闭返回“接口与账号”，成功状态无 alert。未执行真实上游推理或资源写入。
- [桌面修改前](evidence/prism-provider-layout-20260921/before-1440.png) / [桌面修改后](evidence/prism-provider-layout-20260921/after-1440.png)
- [手机修改前](evidence/prism-provider-layout-20260921/before-390.png) / [手机修改后](evidence/prism-provider-layout-20260921/after-390.png)
- [1280尺寸](evidence/prism-provider-layout-20260921/after-1280.png) / [手机深色](evidence/prism-provider-layout-20260921/after-dark-390.png)

## 边界与下一步

本批未部署，未改后端契约。截图中缺少身份的账号来自本地合成样本，不构成真实生产账号身份验收。仅验证本批提供商布局与相关生命周期，不宣称全站视觉或 Safari 验收完成。下一批继续整理接口与凭据编辑表单及弹窗层级，减少嵌套边框和操作拥挤，保留现有写入归属及冲突保护。
