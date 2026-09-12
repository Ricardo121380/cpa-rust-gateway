# 账号管理展示优化（2026-09-11）

本轮针对“全部账号”实现六个分组：API、Codex / ChatGPT、Claude、Kimi、Kiro、Grok。
Grok 下 Web、Console、Build 独立子组。统一 AccountList 列表，手机使用同一 DOM 的纵向行；
没有单独 Grok 卡片。沿用已确认 V6 Liquid Glass 材质，保留辅助偏好、焦点、详情和操作。
本轮没有重新调用 OpenDesign 外部模型，也没有更换原设计系统。

## 信息规则

- 主身份优先邮箱、电话、用户名，不再以人工备注兜底。阶段 ID、长哈希、测试或 Autoreg
  批次不再被包装成身份；来源独立标注。完整 ID 仍用于 API / query / 缓存 / 技术详情。
- 后端从已保存 JSON / JWT 的允许字段投影身份；JWT 信息仅作显示，不成为鉴权或权益证据。
  原生身份读取结束后再次检查 generation，普通账号在同一 SQLite snapshot 内投影。
  每次最多 100 份凭据；解密失败或缺字段保留 null，不猜邮箱、不访问 Provider。
- 渠道显示实际类型名称。Kimi 以明确 upstream kind 或已配置的 Kimi/Moonshot 主机识别，
  无法识别的兼容服务仍归 API，不能从随机 ID 猜真实厂商。
- “接口连接”显示实际 Responses / Chat Completions / Messages 用途，详情列出主机和
  配置启用状态；连接数包含已配置绑定，不冒充健康或当前运行状态。Grok 明确为渠道账号池。
- 搜索按已加载身份/渠道工作，分页和已加载计数可见，不把部分枚举称为全量搜索结果。

## 验证与限制

当前已经验证：267 项前端单测、既有账号管理/原生授权 7 项 E2E、新增三尺寸及深色/辅助偏好 4 项 E2E，
8 项真实 SQLite/HTTP 账号回归、2 项身份字段单测；类型、相关 Clippy、122 operation
权威契约/四文件双构建、依赖边界、文档链接与契约引用检查通过。

截图检查发现 shell 历史 underlap 规则清掉分组标题左边距，已修正并加入实际几何断言。
截图位于本地 `output/accounts-directory-{1440,1280,390}.png`，均为合成 fixture。

本地真实 gateway 通过管理 API 创建合成的五类普通账号与三种原生 Grok 账号，确认分类、普通账号邮箱、Grok Build JWT 邮箱投影及连接用途。API、CLI、SSO 都走真实存储与加密；未调用外部 Provider。预览运行信息
在 `output/prism-account-directory-20260911/preview.json`，私有状态和随机登录密码只在临时目录。
本地 Chrome 已打开预览，但扩展弹窗阻止自动化，尚未完成 Chrome 登录后的人工交互核对。

SSO/API Key 可能从未保存人类身份。随机 digest 不能还原邮箱或手机号，故此类账号仍显示
身份缺失。用户已明确应从授权自动获取；后续修复见[授权身份报告](prism-authorization-identity-20260912.md)，不采用手工补录方案。
本轮未迁移/重命名生产账号，未删除历史数据，也未重新部署生产服务。

[真实网关脱敏收据](evidence/prism-account-directory-20260911.json)只记录数量与检查结果，不含凭据。
