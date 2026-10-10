# Grok Build 首批授权承接与 VPS 核对（2026-10-10）

**用户范围已确认：Grok Build、既有 Build 账号逐一试用、4.5／4.6／4.7 均可、费用不设金额上限；来源为既有生产 VPS。** 当前阻断已经是实际运行版本与原生账号执行入口，不再是模型、账号范围、VPS位置或USD1.00批准缺失。本轮已完成只读现场核对和执行器最小修复，真实推理仍0。

[原授权语句及范围](assets/cpar-build-acceptance-20261010/user-authorization.json) 保留用户三次补充。它不授权生产服务停启、current切换、配置／绑定／权限改变、重新授权、push或发布。原Official候选／有限金额提案及其回执作为历史保留，当前由本清单承接。

## 实际 VPS 和逐账号结果

2026-10-10 16:40 CST，通过既有SSH认证执行正式 `inspect --host new-vps --service cpa-rust-gateway.service --sudo`；仅management loopback GET和/proc只读，管理凭据留在主机。命令PASS、配置稳定、目录完整。现场实例 `instance-20260726-2136`、PID499070，源码 `b0cf36e387b774439acc23830221389eb3ac74d0`，binary SHA256 `0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b`。这不是当前c1b产品源码，不能绑定为新构建。

生产源state是 `/var/lib/cpa-rust-gateway`，data／management为 `127.0.0.1:18180/18181`。16:44 CST再次只读核对仍active/running、同PID、NRestarts=0。[现场Build投影](assets/cpar-build-acceptance-20261010/build-vps-observation.json) 与 [执行回执](assets/cpar-build-acceptance-20261010/new-vps-inspect-run.json) 保留时刻／源码／hash。完整非秘密投影仅在本机output；公开附件只保留本渠道必要信息。以下状态限定观察时刻，后台刷新可继续推进原账号revision。

| 脱敏账号ID前缀 | 保存状态 | 运行池状态 | 本轮结果 | 推理数 |
| --- | --- | --- | --- | ---: |
| `fc78df8341bd` | enabled，active，native revision0 | expired／expired，无可观测lease revision | BLOCKED_ACCOUNT_EXPIRED | 0 |
| `50d8e3f4fc86` | enabled，reauth_required，native revision98 | expired／expired，无可观测lease revision | BLOCKED_REAUTH_REQUIRED_AND_EXPIRED | 0 |
| `5beff1b67bd3` | enabled，active，native revision132 | active／available，无可观测lease revision | BLOCKED_DEPLOYED_NATIVE_PIN_AND_EXECUTION_EVIDENCE_UNSUPPORTED | 0 |

三个账号均已纳入清单。过期／需重新授权不被“每个都试试”改成可调度，不自动refresh、启用、解除隔离、换材料或重新授权。第三个available是调度观察，不是推理成功；native revision132不能替代实际lease revision。

当前Build的exact endpoint／route／candidate分别为 `p12-12-production-grok-build-endpoint`／`p12-12-production-grok-build-route`／`p12-12-production-grok-build-candidate`；max_attempts=1，单enabled candidate、model `grok-4.5`、scope all_active，池中有上述三个身份。已有普通credential binding为空；有效模型投影没有本轮所需的六项serving_snapshot capability evidence。

旧源码 `crates/gateway-http-actix/src/management_resources.rs:5996–6029` 的Channel Pin先读取普通credential并要求普通binding；现有Build账号是native，无法满足该入口。这是**已部署旧版本的前置缺口**，本轮当前c1b源码已经补了native Pin和实际execution sidecar，不能为测试而给旧生产账号伪造普通binding。没有发送已知不适用的POST，也没有把GET当成账号调用测试。新正式入口的单账号池与actual execution／声明检查保持不变，不能为了旧实例通过而关闭。

## 不设货币上限的最小执行器修复

原 `verify_execution_policy` 只能表达有限正数USD预算及PROVEN费用上界；定向RED复现为 `live_cost_bound_unproven`。现在新增明确的 `cost_policy=UNLIMITED_USER_EXPLICIT`，要求：

- 原 `status=APPROVED`、`basis=user_explicit`、source/process/artifact/target/URL/scenario精确绑定仍满足；每命令仍9次、max_attempts=1、fallback=false。
- `cost_authorization_evidence_sha256` 必须指向操作者保留并审核的用户授权材料，`budget_limit=null`；缺少／不合法hash、冲突数值预算及unknown policy均拒绝。
- 只跳过**货币费用上界门槛**。费用资料未知仍保留 `cost_bound.status=NOT_PROVEN`，不伪造金额、计费范围或PROVEN。账号／协议／参数／原生cap语义／终态／归属／Usage／账本／失败停止不豁免。
- 未填写cost_policy的旧方案继续使用BOUNDED，仍需要完整六类费用覆盖和有限预算；null金额不会自动变成不限额。授权hash字段只是审核材料引用，脚本不能替用户或操作者判断授权真伪。

改动仅在acceptance Python、对应测试和入口说明，不改产品Rust／UI／OpenAPI／依赖／服务。原RED原字节、阅读副本与GREEN分别归档。定向全oracle当前34 tests PASS；六protocol/mode分别验证通常受控正例和带不限额live策略的受控正例，共108次正向loopback接收；48个首响应故障各只发送1次，15类发送前阻断×6接收0。后续固定commit的干净重验和独立Standards／Spec终态另附，不把unit中的模拟manifest当真实制品。

## 逐账号执行顺序与计数

先对目录中每个账号完成状态／expiry／归属核对。每个eligible账号在独占副本中用一个确定模型执行一次Responses JSON精确Pin，无fallback／重试；先采用当前已配置且用户允许的 `grok-4.5`，不同时扫描三个模型。失败停止该账号，继续下一账号只作为独立case，不作为原请求fallback。软件协议／归属／制品／证据错误停止整个批次，保留原响应；任何unknown结果都不重放。

从成功且绑定充分的账号中取第一个作为代表，串行完成Chat／Messages／Responses×JSON/SSE六组合，每组合3／2／4共9次，总54次基础；该账号已有Responses JSON Pin可复用时只再取得5个Pin。其他成功账号不自动展开54次矩阵。若eligible账号为N且首Pin均无合格历史证据，推理计划最多N+59；当前观察N=1时最多60。两个过期账号当前为0，而非计入三次成功。

六份后继 [JSON草案](assets/cpar-build-acceptance-20261010/plans/grok.build-responses-json.plan.json) 已切换Build/4.5及不限金额策略，授权scope已确认但actual runtime/artifact、target、lease、配置、出口、声明未绑定，execution status保持PENDING_RUNTIME_BINDING，不能直接执行。目录最多每eligible账号1次，OAuth／refresh／quota／扩展／故障探测／验证重跑均为0，不因金额不限而扩张动作。

Build的正常serve会执行startup refresh、每分钟refresh和立即／每小时catalog；既有生产后台不是本轮新调用，不纳入“本轮0”的断言。新隔离进程启动可能产生额外请求，必须先阻止未批准的刷新／自动目录请求并逐类计数；单独manual目录最多1次不能掩盖后台重复。到期停止账号，不从副本续期同一生产grant；本批不改变E-05已批准的严格同授权证明规则。

## 可审查的下一步运行对象

[隔离位置只读预检](assets/cpar-build-acceptance-20261010/vps-isolation-preflight.json) 确认VPS为aarch64，候选unit不存在、候选根目录不存在、19180／19181在观察时刻没有监听；主机没有cargo／rustc。本机Docker daemon／Colima也未运行。没有为凑环境启动共享VM、安装工具链或调用旧签名候选冒充当前源码。

建议的专用对象：

| 对象 | 拟操作及限制 |
| --- | --- |
| 制品 | 从干净 `c1b304d02e61fe1c75653a32fba920093462add6` 构建Linux ARM64 binary、manifest及hash，实际hash在构建后冻结；尚未构建，不用旧source80签名制品或本机Mach-O冒充 |
| 临时service | `cpar-build-acceptance-20261010.service`，与 `cpa-rust-gateway.service` 分离；启动／停止仅作用该临时unit |
| 私有目录 | `/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010`，0700；独立state／credential／binary／receipt子目录。先核对root未存在，存在则停止，不覆盖 |
| listeners | `127.0.0.1:19180/19181`，只用loopback与SSH既有认证，不修改Caddy、DNS、生产18180／18181或公网暴露 |
| 材料 | 只在保护区取得单个已获授权Build账号必要的密文及匹配key；其他账号／渠道／生产业务数据库不复制；密钥和wire不进入聊天／GitHub |
| 控制 | 副本每次只有一个Build身份、精确route/candidate/model和专用Client Key；六声明从实际serving snapshot冻结。不扩大生产Key权限、不修改生产bindings |
| 网络 | 启动前落实候选进程的OAuth／自动catalog阻断和请求审计；只批准目录及exact inference动作。token到期或上限语义／终态失配立即停止 |
| 退出 | 保存任务证据后仅停止本批临时实例；不删除未知/新增生产数据，不修改current或生产unit；不以恢复备份替代原生产运行 |

这项**VPS新候选实例的构建、私有材料副本和启动**尚未获得原任务禁止部署边界之外的明确批准，实际NOT_RUN。账号／费用／VPS选择已经接受，不重复要求这些授权；只需对该具体临时运行操作作一次决定。旧生产升级或停服不是本方案的默认下一步。

## 最终证据与边界

只读inspection和三个账号状态核对已完成；本地执行器修复、固定提交验证、独立两轴review、报告归档及GitHub写后读回均已完成。真实推理0，新候选构建／启动／生产部署／push均NOT_RUN。#47、#57、#9不以费用授权或目录available而关闭，旧186基础／49未知声明后继台账不回写PASS。

| 范围 | 终态与具体来源 |
| --- | --- |
| 最小修复 | 本地提交 `dfe17b39920c51670a42fea28dce1e2880df9bda`；只改验收Python／测试／说明和报告资产，未push。产品Rust／UI／OpenAPI无变化 |
| 干净来源oracle | dfe17b39独立worktree前后clean，34 tests PASS；[运行回执](assets/cpar-build-acceptance-20261010/clean-dfe17b39-oracle-receipt.json)、[阅读log](assets/cpar-build-acceptance-20261010/clean-dfe17b39-oracle.log)及原字节gzip分别保存。正向108个受控loopback接收、48个首响应故障用例各1次、15类发送前拒绝×6均0；不是真实Provider |
| 文档门槛 | 同一干净dfe17b39 `scripts/check.sh docs` 7/7 PASS，包含文档链接、contract、plan、canary、Caddy、tracked secret及whitespace；[回执](assets/cpar-build-acceptance-20261010/docs-dfe17b39-receipt.json)、[门槛表](assets/cpar-build-acceptance-20261010/docs-dfe17b39.md) |
| 独立两轴review | 固定 `d4accdd8..dfe17b39` 的[Standards PASS](assets/cpar-build-acceptance-20261010/review-standards-dfe17b39.md)及[Spec PASS](assets/cpar-build-acceptance-20261010/review-spec-dfe17b39.md)，hard0；限本diff静态核验，不含真实运行或全项目规格 |
| 六草案预检 | 同一dfe17b39入口、显式空的未绑定runtime、没有execute；6/6 BLOCKED `runtime_unconfirmed`，sent_requests=0；[逐次命令／hash／时间](assets/cpar-build-acceptance-20261010/draft-preflight/summary.json)。这是预期阻断验证，不是live PASS |
| GitHub写后读回 | 17:07 CST独立GET核对 [#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)的既有评论追加后继段；正文／历史评论前缀／OPEN状态／labels／assignees／评论数／native dependencies逐项PASS。48实施票35 CLOSED／13 OPEN；[读回断言](assets/cpar-build-acceptance-20261010/github-sync/summary.json) |
| 既有工作保护 | 25,815个preexisting git-visible路径的lstat size／mtime_ns／mode及AGENTS字节SHA一致；旧两批tracked资产无diff。[保存核对](assets/cpar-build-acceptance-20261010/preservation-closeout.json)。不声称全文件内容hash或ignored nested覆盖 |
| 真实运行 | 当前BLOCKED：旧部署native Pin／actual execution evidence不足；新候选仍NOT_RUN。候选启动前的后台OAuth／refresh／目录阻断及逐类请求审计尚未实装／验证，不将其准备度写成PASS |

原c1b产品Full 45/45、Rust1500／0／12及前端70文件513测试仅按产品源码无diff复用；没有重跑Full或以旧Full证明新增Python行为。原RED／FAIL、旧Official有限预算方案及旧源回执均保留。证据资产由独立hash索引绑定；本文件和终态附件单独形成后置本地提交，最终HEAD／资产跟踪状态由本机 `output/cpar-build-acceptance-20261010/final-delivery-attestation.json` 记录，以避免在被证明文件中循环写自己的提交hash。

**本批可独立执行的本地工作已完成；首批真实验收仍PARTIAL／BLOCKED。** 账号、模型、费用及VPS输入已接受；剩余用户决定仅是上述具体临时候选构建、私有副本和启动操作。后台网络控制、实际Linux制品、exact binding和有效账号必须先落实，才进入已授权请求范围。
