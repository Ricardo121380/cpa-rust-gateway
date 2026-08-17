# CR-P13-11-PROVIDER-SPECIFIC-EGRESS-001 · Provider-specific egress, health, and recovery boundary

| 项目 | 值 |
|---|---|
| 状态 | **Approved（用户于 2026-08-17 明确批准开始 E0 计划/CR 与 review）** |
| 适用范围 | `P13-11E`：CPAR 内已导入凭证的 Provider/Channel egress、健康投影和受控恢复设计 |
| 当前切片 | `E2`：Grok Build/Console 的 CPAR imported-account、exact lease 与 adapter-local synthetic/loopback seam；不接 Autoreg 或真实网络 |
| 下一切片 | `E3`：Grok Web sticky egress/session/clearance 的 fake-only seam；真实代理、FlareSolverr、DNS 和公网请求仍需独立授权 |
| 不包含 | Autoreg 注册/登录/SSO/refresh/replenishment、真实 Provider/代理/DNS 探针、服务器/staging/production、默认公开 API 变化 |

## 1. 用户澄清与目标

P13-11A/B/C/D 已建立通用 compatible endpoint 的 profile、runtime registry、serving handoff、
Config-Version-owned proxy pool 和管理面。下一步不是把所有渠道强行套进一个“代理池”，而是冻结
每个 Provider/Channel 的出口、账号健康、会话/clearance 和恢复边界，避免一个渠道的失败污染另一个渠道。

本 CR 采用以下原则：

1. CPAR 只管理已经导入 CPAR 的凭证在自身运行时中的 Health/Quota/Circuit、lease、cooldown、失败
   反馈和调度；Autoreg 仍独立负责注册、浏览器登录、SSO/OAuth、refresh、套餐/权益和 replenishment。
2. 凭证格式不是出口策略。CPA JSON、Sub2API JSON、官方 OAuth、API key 和自定义 `base_url + api_key`
   只决定凭证解析和 endpoint binding；不会隐式选择代理、clearance 或其他 Provider 的 fallback。
3. Provider、Channel、Endpoint、Credential、Egress Node、Session/Clearance 是可分别归属和审计的
   failure domain；任何跨域合并都必须有显式、可验证的 capability。
4. E0-E3 只冻结类型和合成证据。真实网络或 Provider 请求必须另立明确授权的 canary CR，不由本 CR
   推断为已验证。

## 2. 渠道矩阵

| 渠道/适配器 | 出口边界 | 账号/会话边界 | 允许的恢复方向 | 明确禁止 |
|---|---|---|---|---|
| Generic compatible（OpenAI Chat/Responses、Anthropic Messages、Codex/ChatGPT OAuth、CPA/Sub2API、Krill/custom relay） | 沿用 P13-11D 的 exact Upstream/Endpoint-Credential egress profile；direct/fixed/pool 只属于该 Upstream | Credential Health/Quota/Circuit 与 Egress Node Health 分离 | 只允许同一 exact binding 内的有界 pre-submit transport recovery；默认不自动换 Provider | 不使用 Grok browser/clearance/Statsig/DPoP 状态，不把“Krill”当全局渠道，不跨 endpoint/credential fallback |
| Grok Build native | Build 专属 direct/fixed/pool egress scope；出口状态不得与 Console/Web 共用 | Build native account、Credential revision、quota 和 egress 各自独立 | 仅 CPAR 运行时对 exact egress/transport 进行分类和有限恢复；账号 refresh/SSO 由 Autoreg 外部交付新凭证 | 不从 Autoreg 读取数据库，不用 Console/Web session，不跨 Grok channel 或 generic relay fallback |
| Grok Console native | Console 专属 egress/session scope；DPoP、bootstrap 等辅助请求必须归入 Console adapter 的请求账本 | Console account、session/DPoP、credential revision、quota 与 egress 分离 | 只在 Console capability 明确允许时恢复 exact session/egress；一次诊断不得暗含第二次 inference | 不复用 Web clearance、Build OAuth、Statsig cache 或其他 Provider 的 egress；不把隐藏辅助请求伪装成“零请求” |
| Grok Web native | sticky browser egress 与 clearance/session 绑定；egress、clearance、session、account 分开记账 | Web account/SSO、session、clearance、credential revision 和 quota 独立 | 对明确的 egress challenge 才允许一次 bounded provider-specific recovery；需单独 capability 和证据 | 不把未知 403 直接标成账号封禁，不跨 Web/Console/Build fallback，不在 E0 启动 FlareSolverr/代理池 |
| Grok Official API key（P8 deferred） | generic direct/fixed/pool profile；不得套用浏览器出口 | API key credential 与 quota/egress 分离 | 待 P8 官方 API-key E2E 定义 | 不使用 Web clearance、Console DPoP 或 native Grok Web recovery |
| Kiro / Claude-compatible / 其他 Provider | 由各自 adapter 声明 endpoint/region/transport profile；没有声明时只能走 generic compatible seam | 各自 Credential/Account/Quota/Session namespace | 各自 capability 内恢复；未冻结前只 fail closed | 不借用 Grok 状态、代理池、凭证或 fallback；P7/Kiro 认证边界保持延期 |

“Krill”因此只是一个 generic compatible endpoint 实例；同一规则也适用于任何提供 `base_url + api_key`
的 relay。`openai-compatible.responses` 不能仅凭名称决定它是官方 Codex、Krill 还是其他 relay，必须
由 selected Config Version 的 explicit Upstream/Endpoint binding 区分。

## 3. 冻结的状态和失败归属

E1 之后的 typed model 必须至少保留以下相互独立的状态域：

- `CredentialRuntime`: `available`、`cooling`、`circuit_open`、`quota_blocked`、`unauthorized`、
  `expired`、`disabled`、`recovery_in_flight`；
- `EgressRuntime`: `available`、`cooling`、`circuit_open`、`probe_due`、`probe_in_flight`、
  `disabled`；
- `ProviderSessionRuntime`（仅需要会话的 Provider）：`absent`、`active`、`expired`、
  `challenge_required`、`invalid`；
- `ClearanceRuntime`（仅 Web 或明确声明的 adapter）：`absent`、`fresh`、`expired`、
  `refresh_required`、`refresh_in_flight`、`invalid`。

故障归属按首次可证明的边界确定：

| 观测 | 首选归属 | 处理 |
|---|---|---|
| DNS、连接、TLS、proxy handshake 或提交前 egress 拒绝 | exact Egress Node/egress profile | 只更新 egress state；是否换节点由同一 Provider/explicit pool policy 决定 |
| Credential 解析、401、明确 credential unauthorized 或 expiry | exact Credential/Account revision | credential fail closed；不得改写 egress 为账号故障 |
| 429、quota window 或明确 rate limit | exact Credential/Endpoint/Model quota | 更新 quota/cooldown；不得跨 Provider 借用额度 |
| 未证实归属的 403 | Provider-specific ambiguous state | 保留证据分类；不得直接标记账号 forbidden，也不得直接重试 |
| 明确账号/套餐禁用证据 | exact Credential/Account | 标记账号状态；不污染 egress sibling |
| 协议转换、解码或 Canonical lifecycle 失败 | adapter/protocol | 不把协议错误伪装成网络故障 |
| 首个语义事件之后的失败 | request/Provider outcome | 不进行 transport replay 或透明 egress recovery |

默认状态优先级只能用于投影，不得抹平来源：`recovery_in_flight` > `unauthorized/challenge_required` >
`expired/invalid` > `quota_blocked` > `circuit_open` > `cooling` > `available`。管理投影必须仍能区分
credential、egress、session 和 clearance 的原始状态。

## 4. 运行和恢复边界

1. 所有状态键至少包含 Config Version/Upstream/Endpoint（或 Channel）和 exact identity；Provider
   native 状态还需包含 account/revision，Web clearance 还需包含 clearance/session lineage。
2. Egress lease 与 Credential lease 继续由现有 P13-11B/C owner 管理；E1/E2 不创建第二个 scheduler，
   不推进普通 routing cursor，也不读取 Store/Provider。
3. `PreSubmit` 的有限重试只能在同一 explicit Provider/Channel scope 内，并且必须证明请求尚未提交；
   语义事件之后一律禁止重放。Provider 内部隐藏请求（如 Console DPoP/bootstrap、Web Statsig）必须
   纳入 adapter 的 bounded transport ledger，不能只计数主 inference。
4. E0/E1/E2/E3 的 probe/recovery 使用 injected fake transport、deterministic clock 和 synthetic state。
   任何真实 endpoint、真实 proxy、DNS、FlareSolverr、SSO 或 CPAR 公网 Base URL 验证都需要新的
   canary CR、目标、次数/预算、receipt 和 rollback 规则。
5. CPAR 可以把外部 Autoreg 交付的新 credential 作为新的 revision/CAS batch 导入并重新编译；CPAR
   不启动 Autoreg worker，不修复账号，不读取 Autoreg 数据库。

## 5. 计划切片与交付门槛

| 切片 | 内容 | 本 CR 是否授权 |
|---|---|---|
| E0 | 计划、ADR、Contract、渠道/状态/失败矩阵和独立 review | 是，本轮完成 |
| E1 | typed provider-aware egress/session state、capability registry、合成状态转换测试 | 已完成本地 slice；无网络 |
| E2 | Build/Console adapter 接线：复用 CPAR 账号池与现有 transport，验证 exact failure isolation；隐藏辅助请求必须有计数/one-shot 语义 | 仅本地合成/loopback；不是真实 Provider |
| E3 | Web sticky egress/clearance/session 状态 seam，注入式 challenge/recovery 测试 | 仅 fake transport；真实代理/FlareSolverr 另立 CR |
| E4 | 如需管理状态或 operator action，先另行确认 OpenAPI/Prism 变更与 Claude Code handoff | 不由 E0 自动授权 |
| E5 | 真实 Provider/代理/DNS canary | 明确不授权；必须新 CR |

E1/E2/E3 完成后仍只能宣称 local/synthetic evidence，不能宣称某个 Grok 账号、节点或公网出口可用。
正式 Phase Gate 仍按“每个 P 一次”规则执行，不为每个子切片重复运行昂贵 Delivery Gate。

## 6. 非目标和回滚

- 不改变 P7 Kiro、P8 Official API-key、P12 Autoreg separation 或 P13-10 WebSocket 的既有状态。
- 不新增公开 inference 字段，不把 egress/node/session/clearance 交给客户端选择。
- 不修改 `docs/openapi/management-v1.json`、`web/prism/**` 或管理 HTTP；E0 没有前后端契约变化，
  因此本轮没有 Claude Code action-required handoff。
- 若 E1/E2/E3 的状态编译或 capability 注入失败，必须以 rejecting/fail-closed facade 处理，不能阻断
  已有 Direct serving graph，也不能静默降级为跨 Provider fallback。
- 所有新状态和 registry 均应可在 feature/Config Version 级别回滚；不改生产配置，不启动服务器任务。

## 7. E0 review checklist

- [x] Generic compatible、Grok Build、Grok Console、Grok Web、Official、Kiro/其他 Provider 均有明确
      owner 和 no-fallback 规则。
- [x] Credential、Quota、Egress、Session、Clearance、Protocol failure 没有被一个枚举合并。
- [x] Autoreg 仍是外部项目；没有 DB/HTTP/browser/scheduler 依赖。
- [x] 隐藏辅助请求被明确纳入后续 one-shot、attempt、audit 和成本边界要求；实现验证留给 E2/E3。
- [x] Egress 状态不会跨 Upstream/Provider/Channel 污染；sticky 不可用时必须 fail closed。
- [x] E0 没有真实网络副作用；E1/E2 已有本地实现证据，E3 仍未开始；E5 明确需要新授权。
- [x] 没有 OpenAPI/Prism 变更；若 E4 改管理契约，先更新权威 OpenAPI、同步 Prism 并写
      `docs/cross-boundary-log.md`。

E0/E1/E2 的 checklist 是设计与本地实现边界 review，不是 E3/E5 或真实网络验收；未勾选的实现证据不会被本 CR
伪装成已完成。
