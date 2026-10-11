# CPAR-38 / #47：Chat JSON 502 诊断、失败观测与保真边界

本批完成了证据支持的产品修复与公共入口本地回归。新的真实诊断在启动准备阶段被本批驱动的错误环境变量锚点阻断，实例从未启动，Pin、Chat 和新推理发送均为 0；该 epoch 已永久停止。启动驱动已随后本地修复，未上传重启、未重新获得发送资格。旧 Chat JSON 502 的具体根因仍为 **UNKNOWN**，没有新原生响应样本，也没有声称修复了一个已确认的真实响应字段。

关联票为 #47（CPAR-38）、#57（CPAR-48）和父规格 #9。三票继续 OPEN；本批局部 Spec 审查不等于三票全部 AC 或父规格全范围完成。范围、停止与交付要求见[批次契约](assets/cpar-build-json-502-20261011/batch-contract.md)。本批未修改 `web/prism/**`，未 push、正式发布、部署或改变生产服务。

实际开始基线为 `codex/prism-v4-delivery` / `5a7826e806a04b3d403b1b1c0eef00a2b686ff32`。用户修改的 AGENTS.md SHA256 为 `810c5eac53766af3011d8e6b5ce0a6b783ec1cd0b291805547a063790345d002`，保留不暂存。历史 source `abd454313f6a3707d6af5a5d653b731e2e54b08b`、驱动 `5847fb224ff8f0f5ff532633c765ba08864015fd` 和 HEAD 检查点均为证据，不是回退指令。旧候选、epoch 和失败材料保留。

| 证据层 | 本批结论 | 实际范围 |
|---|---|---|
| 产品修复及公共 HTTP 回归 | PASS | 安全具体拒绝、已建立 Request-ID、已解码 Usage / 去重 / 账本、严格保真投影 |
| 新候选二进制 | LOCAL PASS | 产品 source `bf822b14a62c21cf582a7539e3635f69bd8b7677`，Linux ARM64，unsigned 两载荷 manifest |
| 发送前驱动与审查 | COMPLETE PASS | 驱动 `38504a1eabeaacfcb548e23de7a61a545351b35b`；完整审查 target `c23b82dbb8d0299c72cbd22253ccaa3727507770` |
| 修复后的最终驱动门槛 | Task42/42、Phase45/45 PASS | 冻结 source `c23d2c356ec0253d93e41ca94aac821ad167fcd6`，产品 / build-release 字节未变 |
| 最终 Standards / 本批 Spec | Standards / 本批 Spec COMPLETE PASS | 原完整范围与新增完整增量分区组合，准确覆盖 141 个实际变更路径 |
| 本批真实诊断 | BLOCKED_BEFORE_LAUNCH | 新 Pin NOT_RUN，Chat NOT_RUN；实际新进程、lease、Pin 资格未建立 |
| 完整六组合基础矩阵 | NOT_RUN | 本批 0/6 完整组合，无新增基础请求 |
| 历史通过记录 | 保留原范围 | 旧 source `1b6023f5` 的 Responses JSON/SSE 各 9/9，不迁移为当前候选通过 |
| 远端 CI / release / deployment / production acceptance | NOT_RUN | 本地 Full 不代表这些层通过 |
| GitHub 追加及独立读回 | PASS；三票 OPEN | 仅既有三条 progress，保留正文、AC、状态、元数据及依赖 |

沿实际公共路径确认并修复的缺陷如下。Provider 的 JSON 解码在返回事件源之前完成验证，原事件源没有把已解码 Usage 向后续投影 / HTTP 失败处理转发；投影器原 `Err(_)` 抹掉具体拒绝类型；HTTP 若在建立请求身份后的执行或收集阶段失败，错误响应未统一附加同一 Request-ID。这些是源码和合成公共入口回归证明的软件缺陷，不能倒推旧真实响应具有某个字段。

1. `GatewayError` 仅新增可选闭集 `ResponseProjectionRejection`，保留既有 18 个错误类别和 HTTP 分类。六个原因是 reasoning、未知扩展、stop reason、stop sequence、canonical lifecycle、工具约束拒绝；公开消息均为固定安全文字，普通日志仅记录固定阶段 / 原因 / 已建立的 Request-ID，不接受原生内容或调用方诊断字符串。见 [ADR-0105](../adr/ADR-0105-projection-failure-observation.md)。
2. 三个公共协议在已通过身份 / 模型 / 请求准入边界后，把同一服务端 Request-ID 用于响应和 Request/Attempt 事件，包括非流式收集失败与流式错误帧。准入前拒绝不伪造身份；调用方伪造的 Request-ID 不作为事件身份。流式响应开始后保持 HTTP200 并发具体安全错误帧，不能冒称已发送头之后改成 HTTP502。
3. Grok buffered / streaming 事件源只暴露已经完整验证的 Usage 快照，runtime / egress wrapper 保持来源，HTTP 失败或取消沿既有 exact Attempt 来源处理。快照合并不累加重复计数，存储与账本保持 idempotent；缺少的计数仍为 null，已知计数保持 measured / inclusive。未完成解码的数据块不建立用量事实。保留 Attempt succeeded 与最终 Request failed 的区别，不估算价格、不伪造账本、不改写旧缺少来源的 Usage。

保真转换继续采用既有严格语义边界：plain reasoning 独立保留，符合条件的 native item / delta 可等价桥接；native summary、citation、phase、signature、ciphertext 或其他不可等价表达内容保留具体拒绝。没有删除 native items、工具结果、call_id、顺序或历史，没有关闭已声明能力、改变 4096 上限或放宽断言。上述 rich 字段仅是旧故障的候选解释；没有旧原始响应，不能指认为旧 502 根因。本批没有足够证据支持新增具体字段转换或产品规则例外。

默认关闭的诊断捕获同时绑定候选 source 和选定公共请求字节 SHA，限定一个合成 Chat JSON 非流式请求。目录 / 文件使用 0700/0600、同 owner、无符号链接及独占创建。捕获遵循原读取过程与已有 1 MiB 上限，不额外发请求；认证头、Client Key、管理密钥及凭据不进入捕获侧带、公开响应或普通日志。真实原生内容不得入 Git、GitHub 或外部模型。本批没有启动该功能的真实实例，原生捕获目录为空；只有本地合成捕获验证。

公共入口回归直接调用真实 HTTP handler、runtime factory、Build 解码、canonical/native 事件、projector、SQLite 异步事件写入与 billing worker，仅注入合成上游 transport。五个测试覆盖：Chat/Messages JSON/SSE 的安全具体拒绝及 Request-ID；三协议 × 两模式 × full/partial/absent Usage 的 18 个用量 / 账本场景；重复事件保持去重；private capture 来源 / 请求 / 模式 / 一次发送边界；三协议准入前无身份；目录、source、hash、symlink 拒绝。Responses 保留 rich 内容成功，Chat/Messages 不可保真时明确拒绝。它们不是旧真实响应的精确重放，亦不计入真实六组合验收。

完整本地门槛与诊断单测分别封存。发送前驱动 source `38504a1e` 的 Task42/42、Phase45/45 PASS，完整日志及双 hash 见 [quality-gates-38504a1e.json](assets/cpar-build-json-502-20261011/quality-gates-38504a1e.json)。最终冻结驱动 source 的 Task42/42、Phase45/45 PASS，完整原文 gzip 与双 hash 见 [quality-gates-c23d2c35.json](assets/cpar-build-json-502-20261011/quality-gates-c23d2c35.json)。最终诊断单测 18/18、既有 oracle54/54 PASS，既有 Rust workspace / all-features、Chat/Messages/Responses JSON/SSE、契约、秘密扫描与供应链检查仍按项目门槛执行。

初次本地 Task 失败来自 runner 错误设置 release revision，使 development 断言不成立；断言未修改。重跑时本批误覆盖了该次完整 FAIL 日志，只保留工具输出摘录，不能冒称全文保存或重建。最终完整 PASS 日志均保留；限制见 [gate-initial-diagnosis.json](assets/cpar-build-json-502-20261011/gate-initial-diagnosis.json)。此外，启动测试夹具的事务 / SQL 错误与真正行为 RED 分开保存；文件清单触发秘密扫描的前缀误报通过不可变清单 hash 消除，扫描器与原断言未放宽。

发送前独立两轴完整结论见 [review-pre-diagnostic.json](assets/cpar-build-json-502-20261011/review-pre-diagnostic.json)：Standards 完整覆盖初始范围及两轮完整增量；Spec 分区覆盖 product、driver、contract、GitHub baseline、build/gates 和 local proof，再独立关闭辅助核验缺口。共 105 个实际变更路径，分区并集及复用范围核对一致。此前 P1/P2 失败、中断与 PARTIAL 记录保留，不能单独当 PASS。已经修复的驱动问题还包括：停止只绑定自有 PID/start/binary/state；native Usage 与账本 source_event_id 精确核验；实际 Client Key、候选、配置、出口、lease 与原 Attempt payload-bound sidecar 精确核验后才能标匹配。此次完整审查只证明本批变更符合批次契约，不授予父票全范围完成或生产权限。

最终独立两轴结论见 [review-final.json](assets/cpar-build-json-502-20261011/review-final.json)：Standards 和本批 Spec 均 COMPLETE PASS。前轮完整105路径 + 本轮39路径有5个重叠，为139；随后3份证据文档有1个已在并集中，最终 target `74de66f3ff95ec78364c01d231b9617f482d5353` 精确覆盖141。代码5、本地证据16、远端证据18分区互斥；完整10份gzip共7110行，Task/Phase每轮Rust1510通过、0失败、12忽略，Phase14条依赖重复告警保留。新增证据的规范化JSON摘要与归档文件字节摘要现在明确分列；辅助检查的路径拼写和字段匹配错误保留PARTIAL，并由独立27项字段绑定、5项辅助范围核验闭合，未将原PARTIAL改称PASS。规则、断言与扫描器未放宽。独立 [preservation-final.json](assets/cpar-build-json-502-20261011/preservation-final.json) 在其明确记录的98857ba目标重新核验48185基线路径，0改变/缺失/错误；后续只改本批证据文档，用户AGENTS保持不暂存。

候选二进制 SHA256 为 `cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e`，19,565,744 bytes，嵌入产品 source `bf822b14a62c21cf582a7539e3635f69bd8b7677` / Rust1.97.1 / aarch64 Linux。manifest SHA256 为 `655c037df68c5657b250105bbb07bed665210a7f6b20ea8d8b88aab488157a56`。隔离构建沿用已通过的 image / offline recipe，没有重复依赖准备；旧候选及两次已完成 builder 均保留。产品源码和 build-release 到最终驱动 source 字节未变，因此后续仅驱动修复不重新构建产品，也不借用一个不同产品源码 binary 的资格。

本次唯一执行冻结仍是 [freeze.json](assets/cpar-build-json-502-20261011/freeze.json) 所记 `38504a1e` 驱动：freeze SHA256 `daf161acb2caaeee15e82c2018db4576ac2adc4ecbf0dc58e8524c3224a2d5af`，tar SHA256 `98e504d482799324d5f6060771bb174fa6ade08ff7f4abbbd97cf5cd7ed7143f`，首 freeze 加 14 个载荷。上传字节逐项核对，未覆盖历史文件。最终修复后的驱动不是此次上传包，未上传或创建新的可执行 epoch，不能复用停止批次的资格。

真实诊断准备沿用 new-vps、同一批准 Build 账号与授权来源、grok-4.5，仅在保护根 `private/chat-json-502-20261011` 和对应新 bin 目录操作。选定请求的字节 SHA256 为 `245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062`；POST Chat JSON、stream=false、max_completion_tokens=4096，原合成输入语义保持。源账号 native revision137、连续性 floor77/current138、相同非空 identity digest 与允许的正常刷新字段已核对，没有本批主动刷新、重新授权或新增例外。预期 runtime revision138 不等于实际 lease 证据：新实例从未启动，实际 pool、serving declaration、lease 和新 Pin 均未建立，继续 UNKNOWN / NOT_ESTABLISHED。

具体启动失败为发送前驱动 `remote-start.py:98` 对不存在的 `--setenv=LD_PRELOAD` 求 index。只读脱敏结构证据确认既有批准 launch 使用单个 `--property=Environment=`，其中 guard/audit 两值匹配；没有所假定的 setenv 参数。见 [remote-start-failure-location.json](assets/cpar-build-json-502-20261011/remote-start-failure-location.json)、[remote-launch-environment-structure.json](assets/cpar-build-json-502-20261011/remote-launch-environment-structure.json)。原 stderr 留在本地 0600 private output，公开材料仅保留位置、固定异常类型和摘要；远端只读代码在原地比较必要 launch 值，仅输出闭集结构和比较结果，原文未进入主代理模型或外部模型。

依据失败停止规则立即永久锁存 budget，不重启 / 重试 / fallback / 主动刷新 / reauth / 换账号，不继续 Pin 或 Chat。随后对实际 approved Environment 形状做 network-free AST 回归：旧注入真实失败 16 tests / 1 failure / 0 errors；最小修复只给已有 Environment 属性追加三个诊断选择字段，guard/audit 和其他 argv 保持。另一个真实 AST 回归先证明缺少启动前 stopped-epoch guard，再补齐拒绝已停止 epoch 的检查；最终 18/18 PASS。该修复的实际 systemd / 远端实例执行仍 NOT_RUN；没有借用初次审查资格为修复后的驱动重新发送。

| 推理与费用事实 | 历史 | 本批新增 | 本批后累计 |
|---|---:|---:|---:|
| 推理意图 | 35 | 0（Pin0、Chat0） | 35 |
| 网关 / Pin 确认发送 | 34 | 0 | 34 |
| Provider 独立接收数 | UNKNOWN | UNKNOWN | UNKNOWN |
| 实际 Provider 费用 | UNKNOWN | UNKNOWN | UNKNOWN |

新上限原为 Pin1 + Chat1、最多 2 次，与费用政策独立。旧剩余58已终止；本批未用2也随 epoch 终止，不能续用。管理只读 GET 不作为推理意图。新 Request / Attempt / RequestFinished / Usage 和账本均无新增；这是实例未启动、未调用推理的已核事实，不是从旧 Usage0/账本0推导“免费”。

退出回执 [remote-prelaunch-exit.json](assets/cpar-build-json-502-20261011/remote-prelaunch-exit.json) 确认 `NOT_STARTED_VERIFIED`，没有冒称停了一个未启动的进程。临时 unit MainPID0 / not-found，19180/19181 无监听，无竞争 state 进程，audit 文件不存在且 5 秒继续不变，原生捕获为空；保护根 334 个已冻结文件 SHA 全保留，private 目录 / 文件为 0700/0600，无 symlink。生产 PID499070、restart0、进程开始时间、unit / binary / current / Caddy 和活动配置保持初始基线。退出只锁存本批 budget、写本批私有回执与读取管理状态，没有生产写入。后续 [remote-final-readback.json](assets/cpar-build-json-502-20261011/remote-final-readback.json) 再独立只读确认退出状态、334 个历史文件及 14 个上传载荷，包含退出回执在内的 29 个私有文件与 4 个子目录权限合格，修复后的驱动未上传。活动配置在退出时读取，最终读回复核运行身份。Colima 的 default 和 acceptance 两 profile 都已恢复 Stopped，最终 [colima-final-readback.json](assets/cpar-build-json-502-20261011/colima-final-readback.json) 与初始六项配置精确相同；旧对象与新 builder 保留。

交付前又一次只读复核于2026-10-11T13:19:55Z完成，见 [remote-delivery-readback.json](assets/cpar-build-json-502-20261011/remote-delivery-readback.json)：停止budget、never-started服务、无监听/竞争进程、audit不存在且5秒不新增、空原生捕获、334历史文件和14上传载荷、29私有文件/4目录权限，以及生产运行身份均仍符合退出基线；没有上传修复驱动或重启。随后 [colima-delivery-readback.json](assets/cpar-build-json-502-20261011/colima-delivery-readback.json) 确认两个profile仍Stopped、六字段与初始状态相同。

GitHub 交付已向 [#47 progress](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57 progress](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9 progress](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490) 原评论追加脱敏结果。独立新GET与写入响应分开，逐字确认原历史前缀 + 已封存追加内容；正文/AC、state、labels、assignee及已读取的保护元数据、分页8/23/0条依赖精确保持，三票继续OPEN。原文及原前缀保留在0700/0600本地档案，Git中只保存本批脱敏追加与必要摘要；见 [github-write-receipt.json](assets/cpar-build-json-502-20261011/github-write-receipt.json)、[github-final-readback.json](assets/cpar-build-json-502-20261011/github-final-readback.json)。本地提交不push，GitHub所记核心冻结及退出证据提交 `65c1ab124ccada3773d6353cc20989e11b703723`、追加草案提交 `f06ee0b86713019899c4d9433bea0a2430aeea6e` 都仅在本地，未伪装为远端代码或CI证据。

重新开展基础矩阵需要先解决诊断资格与旧故障的证据缺口：采用修复后的已冻结驱动和新的自有 epoch；重新核对实际 launch / 环境 / guard / 背景出口 / ports，并在产品 source 变化时重新生成候选；完成对应 Task/Phase 与独立完整 Standards/本批 Spec；发送前重新冻结账号材料、实际新进程、pool / lease、路由 / 模型、Client Key、serving declaration、出口与有效期，绝不沿用旧 Pin / lease / 会话资格。若新原生证据显示不可等价表达，先保留拒绝并对具体产品规则给出聚焦决策，不能丢字段、降低上限或关闭能力取得通过。

建议先**另行明确授权新 epoch 的最多 2 个诊断意图**（新 Pin1，PASS 后原 Chat JSON1，任意结果停止）；本批已停止的2不可复用。诊断及保真规则阻碍消除、完整资格成立后，再另行授权基础矩阵最多 **60 个新意图**（6 个 fresh Pin + 54 个基础请求），扩展族另批。此为请求预算建议，不是费用估算或本次执行授权；本批结束不自动进入下一轮真实调用。
