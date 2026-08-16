# CR-P12-AUTOREG-SEPARATION-001 · Autoreg 与 CPAR 的项目边界分离

| 项目 | 值 |
|---|---|
| 状态 | **Approved（用户于 2026-08-16 明确批准；运行时账号池健康语义由 CR-P12-AUTOREG-SEPARATION-002 澄清）** |
| 适用范围 | P12 当前计划解释、P12-10 Grok 账号相关验收、后续 P13 任务归属 |
| 核心决定 | Autoreg 与 CPAR 是两个完全独立的项目；Autoreg 负责账号源生命周期，CPAR 负责已导入凭证在自身运行时账号池中的健康、调度和反代；详见 CR-P12-AUTOREG-SEPARATION-002 |
| 不改变 | 历史 receipt、历史失败分类、CPAR Provider adapter、现有生产路由、现有 Autoreg 部署和回滚材料 |

## 1. 用户批准的边界

P12 不负责解决账号源生命周期问题。下列行为属于 Autoreg 项目，不属于 CPAR/P12 的注册、登录或账号
修复职责：

- 注册新账号；
- 浏览器登录、SSO、Device Code 或 OAuth 授权；
- refresh token / SSO refresh；
- 账号额度、套餐、封禁和上游账号权益恢复；
- 账号池 replenishment、注册调度和自动 reauth；
- Autoreg 数据库、任务队列、浏览器运行时和密码/SSO 存储；
- Autoreg 服务的 Jakarta/Oracle 单活切换。

Autoreg 可以把一个已完成、已验证、符合形状和 expiry 要求的凭证包交给 CPAR；CPAR 不依赖
Autoreg 的数据库、HTTP 服务、浏览器或 scheduler 才能运行，也不把 Autoreg 作为上游 Provider。
CPAR 仍然独立维护该凭证进入自身 pool 后的 available/cooling/circuit/quota/unauthorized/expired/
disabled 等运行时健康状态；这些状态不要求 CPAR 连接 Autoreg，也不等于 Autoreg 的账号源健康。

## 2. CPAR/P12 的职责

当外部已经提供有效凭证时，CPAR 只验收以下内容：

1. CPA/Sub2API/OAuth/Provider-specific envelope 的严格解析和脱敏导入；
2. credential 与正确 Provider、Channel、Endpoint、Route 的显式绑定；
3. AEAD 加密、revision、expiry、Health/Quota、lease 和 account isolation；
4. 通过 CPAR Base URL + client key 的真实 JSON/SSE/协议验证；
5. 上游错误的 value-free 分类，例如 `CredentialUnauthorized`、`EgressRejected`、`ProviderRateLimited`；
6. rollback、无明文落盘、无跨 Provider fallback 和生产不变性。

其中 Health/Quota/lease、cooldown、Circuit、failure feedback、pool rotation、capacity 和受控恢复
是 CPAR 的运行时账号池职责。CPAR 可以参考 grok2api 的行为语义，但不调用 grok2api 或 Autoreg 的
数据库、HTTP、浏览器或 worker。

如果外部凭证无效、过期、未授权或上游拒绝，CPAR 必须记录运行时 pool 状态、停止继续租用并 fail
closed；账号重新登录、refresh、权益修复和新凭证生成必须回到 Autoreg。CPAR 负责“如何隔离和控制
失效凭证”，但不负责“如何修复账号本身”。

## 3. 对历史 P12-10 记录的解释

历史 P12-10I receipt 保留不变，作为当时发生过的事实证据。自本 CR 起，每条历史记录拆成两部分：

- **Autoreg 外部部分**：注册、SSO→OAuth 转换、refresh、账号权益、expiry 来源、补给和 scheduler；不计入
  CPAR/P12 未完成开发任务；
- **CPAR 部分**：收到凭证后的 shape 校验、加密导入、provider binding、route/lease、运行时 pool
  Health/Quota/Circuit、failure feedback、公共 HTTP 反代、错误分类和回滚；继续计入 CPAR/P12 验收。

因此，P12-10H 的历史 `BLOCKED_EXTERNAL_CONSOLE_403` 表示“历史全池/外部账号样本没有形成统一成功
证据”，不表示 CPAR Console adapter 或单账号 Console 反代路径未实现。后续 Oracle Console 单账号
`6/6` 公共矩阵仍按 CPAR 验收结果保留。

## 4. 后续计划调整

- 不在 P12 新增 Autoreg 注册、refresh、SSO 转换或自动 replenishment 任务；
- 保留并继续维护 CPAR 自己的 credential pool Health/Quota/Circuit、lease、rotation、cooldown、
  recovery 和管理状态；这不是 Autoreg 账号生命周期工作；
- P13-12 的“自动 refresh/reauth/replenishment”改为 Autoreg-owned external dependency，除非未来
  单独批准一个 CPAR↔Autoreg credential handoff contract；
- P13-11 仍只处理 CPAR 的 Provider-specific egress/proxy policy，不负责注册或修复账号；
- 任何真实 CPAR 渠道验收仍必须使用 CPAR Base URL + client key，Autoreg 的本地 probe 不能替代 CPAR
  公共数据面证据；
- 该边界变更不修改管理 OpenAPI/Prism，不需要前端改造。

## 5. 完成语义

本 CR 只改变计划归属和状态解释，不把历史 `BLOCKED_WITH_EVIDENCE` 改写为成功，也不把外部账号
失败隐藏为 CPAR 成功。未来报告必须分别标注：

```text
AUTOREG_ACCOUNT_SOURCE_RESULT
CPAR_RUNTIME_POOL_STATE
CPAR_IMPORT_RESULT
CPAR_PUBLIC_PROXY_RESULT
```
