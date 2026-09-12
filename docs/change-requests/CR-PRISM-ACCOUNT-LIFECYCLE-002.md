# CR-PRISM-ACCOUNT-LIFECYCLE-002

2026-09-12 · 本轮 Codex 负责前后端定稿与实施 · 状态：已接受，实施中。
依据：[CPA 工作流差异](../reports/prism-cpa-alignment-matrix-20260912.md)。

## 需求

原生 Grok Web/Console/Build 账号已可导入、授权和读取，但缺少日常启停、移除与 SSO 凭据更新。
这些账号独立于配置版本；不得为操作它们创建空草稿。“读取身份”不再作为界面操作，身份采集属于
授权、导入及更新凭据。必须同时更新已启动的真实账号池，并明确持久保存与运行时应用的结果。

## 权威接口定稿

所有接口位于既有同源 `/admin` 管理鉴权、CSRF 和 no-store 边界，不接收 Client Key secret。

| 方法/路径 | 请求 | 结果 |
|---|---|---|
| PATCH `/admin/native-accounts/{account_id}` | `revision`、`enabled` | 精确 CAS 启停、下一 revision、`runtime_applied` |
| PUT `/admin/native-accounts/{account_id}/credential` | `revision`、`secret`（最多 64 KiB） | 仅 Web/Console SSO；格式校验、身份采集、CAS 替换及应用结果 |
| DELETE `/admin/native-accounts/{account_id}` | `revision` query | CAS 删除授权、保留历史请求和账本；返回移除与应用结果 |
| GET `/admin/native-accounts/{account_id}/audit` | 无 | 最近最多 100 条安全维护记录；删除账号的记录仍可读 |
| POST `/admin/operations/runtime/apply` | 无 | 重建已保存的 active 运行代际，不发布草稿、不请求 Provider，返回应用结果 |

新增维护结果使用 `account_id`、`revision`、`removed`、`runtime_applied`；凭据更新另返回
`identity_state`（`observed` / `unavailable`）。409 不自动重试，404 表示账号不存在，400 表示格式无效。
如果持久保存完成但装配失败，结果明确 `runtime_applied=false`；暂停新的数据面准入，保留在途请求，
允许通过已认证的运行应用操作重试。不能将已经保存的修改报告成完全未发生。
已有 native import 和 Build device 完成结果补充运行应用结果，旧 API 路径保持兼容。

## 数据与运行装配

schema 26 新增独立、追加式 `native_account_management_events`，记录账号、渠道、动作、revision、
管理主体和时间，不记录秘密或 Provider 原始响应。启停、SSO 替换和删除与审计在同一事务内完成。
请求/账本不随账号删除；本次上线不自动删除生产账号。现有 device 审计表保留。

凭据由服务器校验并 AEAD 加密；身份 observations 与新 ciphertext 绑定，不能把旧邮箱直接附到
新秘密上。若新旧渠道明确返回不同邮箱/电话，拒绝覆盖，提示作为新账号添加；身份未返回时如实标记未知。
SSO 更新无法更换为 Build；Build 仍使用既有官方重新授权和 subject 校验。

运行应用复用完整代际装配和发布互斥，保留事件队列、在途快照、并发计数及最新凭据保护。
配置为零、渠道尚无账号、最后一份账号被停用或移除都应是合法的可管理状态，实际请求无可用账号时失败关闭。
普通账号、提供商/接口停用保留候选，但其 hard-eligible binding count 为零；停用模型保留存储中的授权，
从运行授权视图中省略。悬空引用、没有任何候选的启用路由、格式和能力越权仍拒绝。原生渠道不再使用
虚拟的单个绑定冒充容量，由 `serve` 将实际账号池数量投影到同一 serving snapshot，空池的有效模型目录为空。
schema 26 上线前需独立验证迁移及回滚到 25；回滚前保存新维护审计，不能直接套用旧的同 schema 发布脚本。

## 验收

合成账号覆盖三类原生渠道；启停、SSO 变更、并发 409、移除、删除后审计、身份冲突、秘密不回显；
真实 gateway 中修改后重读运行池、最后账号停用、在途请求和空安装；模拟身份端点不连接真实 Provider。
前端提供统一动作和有界批量结果，导入无需手填内部 ID，不能用 fixture 单独证明服务已更新。
后端与权威 OpenAPI 定稿后运行 `npm --prefix web/prism run sync-contract`，再连接界面和回归。

## 2026-09-13 配置接入与目录一致性补充

普通 `importChannelAccount` 新增 200（匹配已有授权）响应；201 仍表示新建。服务端按同一提供商、
规范化凭据类型与材料去重，不以浏览器生成的导入标记作为身份。重复导入保留已有授权 ID、启停状态、
credential revision 和接口连接，并通过 draft revision CAS 记录一次 matched 审计。原生管理导入采用
同样的重复资料保留策略；旧迁移工具的严格导入语义不变。Kimi 增加独立 API 凭据入口，不宣称原生 OAuth。

普通配置操作由私有副本完成校验和发布；中间写入不切换浏览器页面。明确选择的草稿只保存，不自动发布
其他未完成修改；历史配置不能将按钮动作投射到另一份现行配置。表单不自动重放 409。批量最多 20 份授权，
文件每份最多 64 KiB、总量最多 1 MiB，逐项展示结果；连接失败与凭据保存成功分别表达。

配置副本原样继承三张目录表的观测、过期时间及模型移除证据，不延长目录寿命。发布和启动从该版本的
真实目录读取模型状态；已知硬过期模型保留为不可准入的候选，不使整个管理服务无法启动，未知模型仍需
显式手动配置例外。运行层继续按精确账号及快照截止时间限制选择和租约。修改接口地址/实现或替换凭据材料
会清除当前副本中对应的旧目录证据；仅启停不清除，原始版本证据保留。回归覆盖继承期限、状态保留、
凭据替换失效及每次编译重新读取过期状态。

停用或移除目录拥有者后，运行装配保留该接口的否定观测边界，不把目录过滤为空后恢复为所有账号可用。
第二个尚无自身目录证据的账号不能继承前者的模型；取得自身有效观测后才能恢复准入。
真实运行装配回归先重现了停用 A 后 B 仍看到模型的问题，再验证停用/移除 A、B 的独立观测及最终空池。
