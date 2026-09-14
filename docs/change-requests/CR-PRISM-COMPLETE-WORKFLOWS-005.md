# CR-005 完整日常流程：真实目录、账号投影与请求观测

用户确认完整目录/手动开放，显式密钥权限，删除6个旧别名；新请求趋势和时延纳入。范围见prism-complete-alignment-execution.md。

第一批增加 POST /admin/catalog/refresh：管理鉴权/同源CSRF、当前配置上下文，body只有endpoint_id和credential_id；服务端使用当前运行代际的凭据和出口读取完整目录，不接受秘密/自定义URL，不开放模型。35秒外部上限、最多4个并发；不支持源501、忙429、代际变化409、上游失败502。返回完整model_count和observed_at_ms，失败保留上次成功目录。授权/导入/模型开放是独立动作。

旧discover-preview/apply为固定零响应的无实际发现入口，前端替换为真实目录工作区；停止伪成功。权威契约同步后生成客户端，不改生成文件。

第二批增加 `GET /admin/requests` 与 `GET /admin/requests/summary`。按已受理的外部推理请求统计，不把上游 attempts 或账本条数当作请求数。时间筛选使用终态时间；显式 `include_unknown` 可包含历史缺失终态的请求，历史耗时不会由 attempt 或账本补造。列表游标绑定筛选、事件水位与账本水位；聚合在同一事件快照内完成，P50/P95 采用 nearest-rank。成功率分母排除 unknown、包含 failed/cancelled。

新增必需事件 `RequestFinished`，保存处理开始/结束、单调时钟耗时、首内容交付耗时、最终结果和安全错误码。成功由实际 JSON/SSE/WebSocket 交付方记录，不能由上游流结束记录；首内容仅在编码内容交给服务端传输层后计时，不把响应头或 keepalive 当内容。此指标不是客户端收包确认。SSE 编码失败、截断和客户端取消分别保留终态。

Schema27 保留既有事件序号与历史，增加终态类型、时间索引与请求账本索引。当前降级脚本在已有终态记录时拒绝有损降级；最终发布前仍须完成生产副本与适用回滚演练，不能直接换回只认识 schema26 的旧二进制。

后续批次仍包括统一普通/原生账号展示与全量筛选、全部渠道日常操作、价格工作流和整体验收，未移出原计划。


## 统一账号查询补充

`GET /admin/accounts/inventory`：版本与普通资源审计、原生账号代际共同约束游标。安全身份全量搜索、类别/状态/提供商筛选、名称/提供商排序、每页最多100，完整查询最多10000条，超过返回503而非伪造完整结果。复用加密身份投影，不把邮箱索引明文写入数据库，不发起Provider请求。返回普通/原生来源及真实现有动作；套餐/运行健康仍使用单独观测，不由启用状态推断。


## 别名维护补充

在既有 `/admin/public-models/{public_model_id}/aliases` 增加 DELETE，复用 AliasInput JSON（保留含斜杠的 exact alias）与 ConfigVersion/IfMatch。只删除属于指定模型的那一条别名，审计为 `model_alias_deleted`；模型、候选和权限不受影响。先补齐通用操作，再用于已授权的六个遗留别名清理。

## First Codex enrollment (2026-09-14)

Add revision-bound POST start/cancel/callback operations under
`/admin/upstreams/{upstream_id}/codex-authorization/`. Start requires an owned draft
and unused proposed credential ID, creates only a bounded transient PKCE session,
and returns the existing value-limited OAuth operation. Session identity binds the
configuration revision, upstream and proposed ID. Callback uses the existing validated
Codex token exchange off the Actix thread, imports encrypted material under CAS and
returns Credential plus ETag; identical stored material can reuse an existing credential.
Cancel never creates a placeholder account. Tokens are never returned to Prism. This is
Codex enrollment only, not a claim that Claude/Kiro enrollment is implemented. Existing
Codex reauthorization remains its own revision-bound operation. Browser mutation failures
are not automatically replayed; saved drafts remain inspectable after partial connection
or publication failure. First authorization is advertised only with an injected exchange.

## Claude authorization and stable OAuth identity (2026-09-14)

Add parallel Claude start/cancel/callback operations and optional `replace_existing` on
code-authorization inputs. First authorization writes no placeholder; explicit replacement
retains the credential ID, bindings and disabled state, checks the prior credential revision,
and refuses a changed observed account binding/email. Shared code-authorization sessions
use provider-specific client, redirect, scope and exchange; Claude uses the pinned reference's
`platform.claude.com/v1/oauth/token` and advisory OAuth profile read. Responses and profile
reads are bounded; redirects and ambient proxies are disabled. Normalized Claude material
accepts the observed email so inventory can display it.

Ordinary OAuth import now rotates the existing record only when normalized provider account
binding AND observed email match within the same upstream and credential kind. Identical
bytes remain idempotent. Different organization members are not merged on a shared account
binding alone; disabled status remains disabled. This changes no model grants.
