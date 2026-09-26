# CPAR M2：渠道生命周期与续接本地交付

对应 [可靠性计划](../handoffs/cpar-reliability-alignment-plan-20260926.md) M2；实施基线 `1c94c5b83316d197004463450ec649068a3f3495`。
状态：**M2 实现与本地验收完成**。实现提交 `1a2fdb86f7e6be42f9f437d74797dc14e89f8353`。M3、M4 未启动；未部署、未读取或修改生产数据，新增真实推理额度仍为 **0/12**。

## 本次实现

1. 普通 OAuth 刷新统一接入启动及每分钟 worker，覆盖 Codex、Kimi Coding、Claude、Kiro Social/Enterprise。新增 schema30 持久 claim、凭据 revision CAS、退避与永久失效分类；成功密文、revision 和安全审计同事务提交。API Key 与 SSO 不制造 refresh grant。
2. 普通刷新单轮最多8次交换，30秒后不启动下一次；单次网络15秒超时、响应64KiB、不跟随重定向。停止与 generation 退役在交换前复查，已发出的刷新完成 CAS 后退出。Grok 继续复用原生有界 coordinator，完成时间按实际经过时间核对 claim 截止，防止慢响应越过租期提交。
3. Claude 刷新保留 email/account_id/plan；上游明确返回不同 account UUID 时拒绝覆盖。Kiro 刷新使用真实 camelCase 请求，保持 kind/client/region 和原绑定。临时网络、HTML拒绝、429/5xx、解析失败不会误判为撤销。普通 OAuth 到期时间进入租赁池，过期材料不参与新请求选择。
4. Grok Build 仅对同一 subject、client、相同 scope 集的正常刷新保存连续 revision 证明。旧历史可租当前材料，其它 lineage、账号、健康、额度、过期和并发限制不变；重新授权、人工替换、未知主体和撤销拒绝继承证明。在途租约继续使用取得时的不可变材料。
5. Kiro IDE OAuth 目录接入真实运行时 worker 与手动刷新，固定区域、精确凭据、分页读取；完整成功后才发布。上限100页、每页1MiB、10,000个 exact ID，重复 cursor/部分失败不冒充完整目录。复用已有持久来源/时间/过期/权限隔离，发现不会创建服务模型或密钥授权。
6. 账号渠道名按实际类型/协议判断，不再把所有 oauth_json 都归为 Codex；Grok Official 保留 API 类别和实际提供商名称。管理对象形状、权威 OpenAPI、Prism 及四文件构建约束不变。

实现入口：[刷新 worker](../../apps/gateway/src/credential_refresh.rs)、[普通刷新](../../apps/gateway/src/credential_refresh/ordinary.rs)、[持久状态](../../crates/gateway-store/src/credential_refresh.rs)、[停机装配](../../apps/gateway/src/runtime/reload.rs)、[目录运行时](../../apps/gateway/src/runtime/catalog_refresh.rs)、[Kiro 元数据边界](../../crates/provider-kiro/src/catalog.rs)、[Build 持久证明](../../crates/provider-grok/src/account_worker.rs)、[精确租赁](../../crates/gateway-upstream/src/credential_pool.rs)。

## M2 验收映射

| 编号 | 主要用例/证据 | 验证边界 |
| --- | --- | --- |
| M2-01 | managed_resource_inventory 的 Codex/Claude/Kimi/Kiro 首次授权、替换、错误账号、重复导入、未自动绑定；原生 Grok 管理回归 | 本地合成交换；不宣称真实官方登录 |
| M2-02 | ordinary::tests 到期恢复、kind/身份保留、退避/撤销、8次上限、损坏 AEAD 隔离、停机后不启动下一条；refresh_scope 精确渠道和启停；credential_refresh store 跨连接/重启 claim、过期/旧 revision/归档 CAS；native worker 慢响应截止 | 真实 SQLite、loopback HTTP 分类和注入式交换；无真实刷新请求 |
| M2-03 | account_presentation 分类；Claude 刷新身份冲突；managed_resource_inventory 的身份/协议/账号视图；既有 entitlement/quota 来源和未知值回归 | 后端投影验收；浏览器操作与进一步诊断呈现属 M3，生产元数据属 M4 |
| M2-04 | catalog_refresh::tests Kiro 两页、exact ID、游标、错误和账号头隔离；provider-kiro catalog 请求目的地/过期边界；durable catalog、route_snapshot 既有时效/隔离/发现不授权；管理目录刷新/缓存回归 | 新分页算法直接用于生产分支，测试注入 HTTP 字节；固定 HTTPS/egress 不放宽，不冒充真实 Kiro 目录验证 |
| M2-05 | m2_continuity::native_build_http_history_survives_proven_rotation_in_json_and_sse；p12_10d_native_account_workers 的持久证明、重载、主体变化与截止；精确池的当前有效性/容量/在途材料；既有 router 禁 fallback、HTTP 取消及原生 reasoning/tools 回归 | 新组合测试走真实 Actix HTTP、加密历史库、精确池与原生 Build 编解码；传输为合成，路由/取消另由各层测试覆盖 |

新组合测试源码：[m2_continuity.rs](../../apps/gateway/src/runtime/m2_continuity.rs)。它验证首次存储→证明轮转→previous_response_id＋工具结果→JSON/SSE存储当前revision，以及跨客户端404、人工替换后拒绝、无额外上游调用、租约归零。不能称为真实 Provider 或完整生产运行装配验收。

## 渠道能力与限制

| 渠道 | M2 后刷新/恢复 | 身份、目录与额度边界 |
| --- | --- | --- |
| API/兼容（Krill、Kimi API、Grok Official） | 无通用 refresh；沿用导入/更新、启停 | 按 models_path 读取；API key 本身不保证邮箱，额度未知保持未知 |
| Codex/ChatGPT | 持久普通 OAuth worker；重新授权按原入口 | 保留原生身份/Responses/目录与权益，旧 revision 续接仍严格拒绝 |
| Claude | 新增持久 worker，真实 refresh 协议 | 保留已知 UUID/邮箱/套餐；Messages；按已配置 models_path，未知额度不补零 |
| Kimi Coding | 持久普通 worker，与 API Key 分开 | 保留 device/身份元数据与 Coding 协议；按兼容目录，旧 revision 续接不扩权 |
| Kiro | Social/Enterprise 持久 worker；API Key 只更新 | IDE OAuth 新目录；CLI/API Key 元数据明确不支持自动发现，允许原有手工 exact ID。token 响应不保证邮箱/主体；未知不造。订阅纯解析器不冒充运行时额度观测 |
| Grok Web | SSO 导入/替换，不声明 refresh | 可靠身份才展示；不套 Build 目录；旧 revision 续接严格 |
| Grok Console | SSO 导入/替换，不声明 refresh | 保留实际额度类型/观测来源及身份缺失；不造 OAuth 或完整目录 |
| Grok Build | 原生 claim/CAS worker，完成时再次核对租期 | 同 grant 刷新证明允许历史续接；真实目录/权益逻辑保留，账号权限独立 |

Kiro 的未知身份授权替换是明确管理操作，不是同主体证明；不能据此跨 revision 续接。无证据的 CLI 元数据、Provider 登录与运行时真实额度列为能力/验收限制，不以其它账号、模型样例或套餐推断通过。来源、请求界限和回退要求见 [CR](../change-requests/CR-20260926-channel-refresh-lifecycle.md)。

## 迁移、验证与下一步

schema30 新增普通刷新状态表和 Build 证明列，不改写历史、账本、配置权限。合成29→30→29→30回归断言已轮转密文/revision和审计保留；证明/退避可以丢弃，凭据不能倒退。生产整体升级/回退演练属 M4，schema28 不能仅换旧二进制回滚 M1 事件。

首轮门禁的 Rust、嵌入、真实网关均通过，密钥扫描因两个文件中的长变量引用被误识别而失败；已改为局部变量引用，未放宽扫描器。[首轮回执](evidence/cpar-reliability-m2-20260926/fast-check-first.md) 保留失败记录。

最终 `./scripts/check.sh fast` 全部通过：[逐步回执](evidence/cpar-reliability-m2-20260926/fast-check.md)、[测试摘要与真实网关结果](evidence/cpar-reliability-m2-20260926/test-summary.json)、[相关前端回归](evidence/cpar-reliability-m2-20260926/prism-tests.txt)。

- 本次 Rust **1,350通过、0失败、12项显式忽略**；忽略项包括专用负载、真实授权、历史副本输入与子进程入口，名称/原因保存在摘要，不计入通过。
- 相关前端 **12文件、33测试通过**。SPA检查含类型检查、契约一致性、CSP限制和四文件两次确定性构建；未改UI，因此未额外进行视觉验收。
- schema30真实本地 gateway 启动、listener隔离及管理鉴权通过。loopback TLS mock 完成JSON/SSE显式历史与stored history四组三轮，12次成功均物化；另外覆盖协议解码失败、断流、取消和两页目录不扩权。
- 格式、严格Clippy、源代码政策、crate边界、文档/行为契约、密钥扫描与diff检查通过。没有新增依赖；签名供应链、生产副本及实际部署留给M4。[收口文档门禁](evidence/cpar-reliability-m2-20260926/docs-check.md) 亦通过。

本轮只做本地实现/验证，没有前端视觉改造、官方登录、真实推理、生产切换或 C2C 调用。下一步 M3：路由渲染兜底、分页/后台失败保留记录、会话/并发恢复、账号与请求安全诊断深链，以及 V2 三尺寸实际操作验收。
