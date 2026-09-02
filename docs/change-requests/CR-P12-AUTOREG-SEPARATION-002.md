# CR-P12-AUTOREG-SEPARATION-002 · CPAR 运行时账号池健康边界澄清

| 项目 | 值 |
|---|---|
| 状态 | **Approved（用户于 2026-08-16 明确补充；OAuth runtime refresh 由 CR-P13-RUNTIME-OAUTH-REFRESH-001 修正）** |
| 前置 CR | [`CR-P12-AUTOREG-SEPARATION-001`](CR-P12-AUTOREG-SEPARATION-001.md) |
| 适用范围 | P12/P13 账号池、Grok Console/Build/Web 运行时、管理状态和失败反馈 |
| 核心决定 | Autoreg 与 CPAR 仍是两个完全独立的项目；Autoreg 管理账号生命周期，CPAR 管理已导入凭证在 CPAR 运行时账号池中的健康、可用性和调度 |

## 1. 澄清“账号健康”的两个层次

此前“账号健康”一词过于宽泛。本 CR 将它拆成两个互不替代的层次：

### 1.1 Autoreg 的账号源生命周期健康

> 2026-09-02 修正：Autoreg 不负责 CPAR 已保存、可刷新 OAuth 的日常 token rotation。
> [`CR-P13-RUNTIME-OAUTH-REFRESH-001`](CR-P13-RUNTIME-OAUTH-REFRESH-001.md) 规定 CPAR 自己执行
> exact-channel refresh、CAS 持久化和运行时替换；Autoreg 只在首次授权或 refresh grant 被撤销后
> 提供新的交互授权材料。

Autoreg 负责账号是否能够被创建、登录和继续获得新的认证材料：

- 注册、浏览器登录、SSO/Device OAuth；
- 首次取得 refresh token、SSO cookie 更新和 refresh grant 已失效后的交互重新认证；
- 账号套餐、额度、封禁、上游权益和登录资格；
- 账号来源的密码/SSO/浏览器运行时、任务队列、replenishment 和 scheduler；
- 生成或修复后，把一个符合 envelope、expiry 和脱敏要求的凭证包交给 CPAR。

这些是账号源的生命周期事实。CPAR 不读取 Autoreg 数据库，不启动 Autoreg scheduler，也不在 CPAR
内注册、登录或修复账号。

### 1.2 CPAR 的运行时账号池健康

CPAR 必须控制已经导入的 credential 在自身数据面中的运行时状态。该状态不等于 Autoreg 的账号源
生命周期，也不能由 Autoreg 的本地 probe 替代。CPAR 负责：

- 按 Provider、Channel、Endpoint、Model、Credential 的精确边界维护 `available`、`cooling`、
  `circuit_open`、`quota_blocked`、`recovery_in_flight`、`unauthorized`、`expired` 和 `disabled` 等
 运行时投影；
- 记录请求失败反馈并更新同一 Provider/Channel 内的 cooldown、Circuit、Quota 和恢复状态；
- 进行 pool inventory、priority/weight/concurrency、lease、capacity、轮询/选择和账号隔离；
- 在管理面提供脱敏状态、failure feedback、`cool_down`/`request_recovery` 等受控 operator action；
- 对失效或未授权 credential fail closed，停止继续向该 credential 发送请求，等待外部替换凭证或受控
  runtime recovery；
- 对 CPAR 已保存且 exact-channel executor 支持的 OAuth 在 expiry 前主动 refresh，以加密 CAS 保存完整
  envelope，并只向后续请求原子发布新 runtime revision；
- 通过 CPAR Base URL + client key 反映真实 JSON/SSE 结果，并将 `CredentialUnauthorized`、
  `ProviderRateLimited`、`EgressRejected`、超时和 5xx 归类为可审计的运行时结果。

CPAR 可以因此“控制账号池健康”，并负责已导入 OAuth 的常规 token 续期；但它不负责让被撤销或失去
权益的账号重新获得资格。Autoreg 交付新授权凭证后，CPAR 以新的 credential revision/CAS 批次导入、
替换或回滚；两个项目之间只交换受控凭证包，不共享数据库或调度器。

## 2. grok2api 参考范围

CPAR 可以参考 grok2api 的账号池行为模型，例如按账号轮换、401/403/429 分类、失败后冷却、恢复探针、
账号隔离和 provider-specific pool，但参考的是可验证的行为语义，不是项目运行时耦合：

- CPAR 使用自己的 Rust pool、Health/Quota/Circuit、lease、AEAD 和 management facade；
- CPAR 不把 grok2api 作为上游，不反向调用 grok2api 的数据库、HTTP 或 worker；
- 不把 grok2api 的注册、浏览器 OAuth 或账号 replenishment 移植成 CPAR 职责；只复用 CPAR 已导入
  OAuth 的 Provider-specific refresh 语义，不调用 grok2api worker；
- 任何 provider-specific 重试、egress、Statsig/DPoP 或 session 行为仍必须经过 CPAR 的 Provider
  adapter 和显式 capability/失败域，禁止隐式跨 Provider fallback。

## 3. 失败归因与控制动作

| 现象 | CPAR 的动作 | 根因修复归属 |
|---|---|---|
| `CredentialUnauthorized`、上游 401/403 | CPAR 标记精确 credential 为 `unauthorized`，停止租约并保留脱敏反馈；不得把普通 inference 401 当作无限 refresh/replay 许可 | CPAR 可在独立后台 refresh 窗口续期；若 grant 被撤销，重新登录/权益修复属于 Autoreg |
| credential 即将 expiry | CPAR 对已保存且 Provider executor 支持的 OAuth 主动 refresh、CAS 保存并原子替换；静态 key/cookie 不伪刷新 | refresh grant 无效时由 Autoreg/operator 重新授权；CPAR 保持 `reauth_required` |
| 429、窗口耗尽或模型额度阻断 | CPAR 标记精确 Endpoint/Credential/Model 的 `quota_blocked`，按策略恢复 | 账号套餐/真实额度来源属于外部账号源；CPAR 不猜测或伪造额度 |
| timeout、5xx、暂时性连接失败 | CPAR 更新对应目标的 `cooling`/`circuit_open` 并按同 Provider/Channel 规则恢复 | Provider 出口或上游故障；必要的代理池属于 CPAR 的 P13-11 |
| `EgressRejected` | CPAR 标记 Provider-specific egress 失败，不把它误报为账号注册失败 | 对应 Provider 出口/代理策略；不由 Autoreg 解决 |
| shape、binding、lease、协议投影、Secret、rollback 错误 | CPAR fail closed 并登记实现缺陷 | CPAR/P12/P13 实现责任 |

因此，“账号无效”可以是 Autoreg 的源生命周期问题；“CPAR 如何隔离、冷却、熔断和恢复这个无效
credential”则始终是 CPAR 的运行时职责。两者必须同时记录，但不能混为一个项目或一个状态字段。

## 4. 对计划的影响

- P12-10B/C 以及 P12-10D 中的 CPAR pool、Health/Quota、scheduling、lease、backoff 和 restart 行为继续
  属于 CPAR 证据；P12-10D 中的 `refresh_due` 由 P13-16 exact-channel worker 执行，`reauth_required`
  表示当前 refresh grant 已不能自动恢复，才等待 Autoreg/operator 交付新授权材料。
- P13-06A/B/C 已实现的 provider account-pool inventory、运行时状态投影和 operator failure feedback
  继续属于 CPAR 管理控制面。
- P13-11 只补 CPAR 自身的 Provider-specific egress/proxy pool；不负责注册或账号修复，OAuth refresh
  仍必须沿同一 channel 的受控 egress 执行。
- P13-12 已改作 Provider/channel entitlement；P13-16 负责 CPAR 已保存 OAuth 的自动 refresh，仍不把
  Autoreg 的注册、交互 reauth 或 replenishment scheduler 搬进 CPAR。
- 历史 receipt 不改写；后续报告同时区分 `AUTOREG_ACCOUNT_SOURCE_RESULT`、`CPAR_RUNTIME_POOL_STATE`、
  `CPAR_IMPORT_RESULT` 和 `CPAR_PUBLIC_PROXY_RESULT`。

本澄清不新增前端、OpenAPI、Provider 真实请求或生产配置变更；它修正的是职责边界和验收归属。
