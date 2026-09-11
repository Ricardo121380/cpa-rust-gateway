# Prism 功能重构实施进度


2026-09-11 deployment update: current implemented channel/account improvements are live at
`https://cpar.142857142.xyz/admin-ui/`, code `1cbc20d`, schema 24.
[Production release evidence](prism-channel-production-release-20260911.md) records signed builds,
formal gates, rollback rehearsal and two passing Grok canaries. Remaining functional work below
is unchanged; deployment is not completion of the whole redesign.

依据：`docs/handoffs/prism-cpamp-functional-redesign.md`。用户于 2026-09-11 授权实施，
并要求原定渠道全部有入口。以下是当前状态，不以历史测试数量代替本次验收。

## 已落实

- 正确性：真实 oauth_json／bearer 类型、Codex 重授权判定；完整普通凭据／端点库存，
  未绑定对象可管理；普通凭据无需重填秘密的状态修改。
- 导航：8 个一级工作区，14 个旧路由经子页保留。
- 配置编辑底层：完整 fork、版本绑定秘密重新加密、OAuth 轮换及发布／回滚 ABA 保护。
  它仍不是运行时“保存并应用”。
- 渠道入口：OpenAI／Anthropic 兼容、Codex、Claude、Grok Official、Kiro 的校验导入，
  以及 Grok Build／Console／Web 原生导入；九类均有真实凭据接入路径。
- 原生 Grok：完整有界库存；Build 首次 Device OAuth、原账号 ID／revision 保护的重新授权。
  schema 24 支持与账号变化绑定的分页和重授权审计。
- 真实授权验收：Chrome → 官方 Grok → 本地实际 gateway → 加密入库／重授权 → 重启重读，
  两项授权均通过。账号数量保持 1、ID 不变、revision 0→1；没有对新账号发送推理请求。
  见 [真实授权收据](prism-grok-authorization-acceptance-20260911.md)。

## 仍需完成

- 第二批：Codex／Claude／Kiro 首次 OAuth 的管理接线，SSO 更新与原生账号状态维护，
  批量导入预览／提交与批量操作，资料编辑，提供商向导，真正的运行配置切换。
- 第三批：模型与客户端 Key 的完整任务流程。
- 第四批：请求级事实／统计和网关设置。
- 功能重构完整本地验收及后续发布；当前代码没有部署生产。

本轮原生接入验证：7 项 SQLite/HTTP、2 项授权工作流、7 项既有迁移回归、7 项浏览器检查、
迁移往返及 122 操作四文件门禁通过。实际 Chrome 授权与重启验证另有独立收据。
此前线上已有账号的 Grok JSON／SSE 小测只证明原版本调用／计费路径，不替代本轮授权测试。

## 浏览器与环境

后续验收使用 Chrome，不使用 Codex 侧边栏。独立本地入口为
`http://127.0.0.1:61700/admin-ui/#/accounts`，已经在 Chrome 保留。
侧边栏旧授权会话已取消，Chrome 的首次及再次授权均已结束；当前无待用户确认的授权。
未改写生产 CPAR 部署、配置和存储记录，未清理历史请求或账本。
