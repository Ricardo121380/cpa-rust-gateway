# CPAR #47：Grok Build 新版单次真实复验与基础验收停止记录

新版单次 Responses JSON Pin **PASS**；随后首个公共基础请求实际成功返回，但正式验收入口错误拒绝 Responses 的 `error=null`，触发全批停止。基础批次原执行 **FAIL**，只发送 **1/54**，完整基础场景 **0/18**、完整组合 **0/6**。最小修复及本地验证已完成，未再次真实重跑。#47／#57／#9 继续 **OPEN**，整体规格 **PARTIAL**。

## 当前基线与运行身份

开始时实际 HEAD 为 `910cad6713444a35fb8f7b11bcb416a5fe6f1177`，分支 `codex/prism-v4-delivery`，与用户历史检查点相同；最近产品源码为 `1b6023f52f4c00ea6d8296524d306c141e4edae7`。没有回退、覆盖用户 `AGENTS.md` 或无关 tracked／untracked 内容。只提交本任务的验收代码、测试及新证据路径；最终 HEAD 和工作区保留检查见本批收口回执。

已读取项目规则、适用目录规则、跨边界日志、issue tracker、必读授权／旧失败报告以及 GitHub #47／#57／#9 的正文、既有评论和相关依赖。新改动不涉及 Rust 产品、Web／Console 页面、OpenAPI 或前后端共享契约；不新增跨边界产品改动。T12、E-05、已补 Web／Console 和历史审查保持各自来源及边界，未重跑无关验收。

实际运行的 Linux ARM64 临时候选：

| 身份 | 本批实际值 |
| --- | --- |
| 产品 source SHA | `1b6023f52f4c00ea6d8296524d306c141e4edae7` |
| binary SHA256 | `ea89d5c72628a871f7683f32d0b51a76dc9c588f7172c86345d1c989f1bd9063` |
| binary 大小／平台 | 19,500,176 bytes，ELF Linux AArch64 |
| manifest 文件 SHA256 | `20ef11961ca40d4dd659ae278c1e7ee629f9f70319f4a4b006d2d8685a5c9777` |
| manifest 规范化对象 SHA256 | `ffaea62e61fa3c974d2e4a212556249ac2db10963657db9d078eb7e151de7d14` |
| build metadata SHA256 | `a1e47a1ad7c132e223d5692bdee95b613d3f6465107c38867b9c58b9506c4895` |
| 执行进程 | PID `2212026`，`/proc` start `656883401`；结束 PID=0 |
| 临时服务／端口 | `cpar-build-acceptance-20261010.service`，loopback `19180／19181` |
| 配置 | `cpar-build-config-20261010`，serving revision `9`；管理表示 `rev-9` |
| channel／provider | `grok.build`／原生 Build |
| endpoint／route／candidate | `cpar-build-endpoint`／`cpar-build-route`／`cpar-build-candidate` |
| 模型／首组合参数 | public／upstream／response 均 `grok-4.5`；`max_output_tokens=4096` |
| 账号脱敏身份 | `5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54` |
| 凭据版本 | 原生持久 revision `133`；实际 Pin 及公共 Attempt 的运行 lease revision `134` |
| egress | direct；revision 来源 `not_applicable`，未配置代理或替换出口 |
| serving 声明范围 | 同一 Client Key evidence `ff7ef703ed7ad99438aa143c89b8f17704d93746574f3e557875440274181025`；projection `76b46919f61a1174ea8bab32a9ba08174812e6d068e97c80f0eb9b6070802e25` |

native revision 与 lease revision 表示不同对象；没有用 `133` 代替实际执行的 `134`。已核对上轮同一账号、同一已授权来源，源与隔离副本的 ciphertext／账号字段匹配（只排除任务自有 import batch）；key version=1。未推定外部个人身份：来源记录中的 external account identity 为 null，归属依据为已批准材料来源、不可混淆的账号 ID 及相同材料证明。发送前账号 enabled／active／available，worker claim 空，expiry `1791647802393`（2026-10-10 23:56:42.393 CST），两次发送均早于 expiry；没有刷新或重新授权。

启动回执证明单 enabled candidate、单原生账号池、`max_attempts=1`，background provider tasks disabled，guard 映射及环境有效、无代理，65 秒观察内新增 egress 事件 0，配置／账号未变、无其他任务状态进程或 DB 文件持有者。同 serving snapshot 的有效模型投影证明该 Client Key 权限及能力声明。原始 inspection、凭据和会话留在远端保护目录，公开只有允许字段与哈希。

## 历史承接与新版 Pin

[旧真实执行报告](cpar-build-live-20261010.md) 保留：目录 GET 1 次成功；首个管理 Pin 发送前拒绝，上游 0；修复后的 `f346a03f` Pin 真发送 1 次，上游 HTTP426／Unrecognized，基础 0/54、retry／fallback／rerun=0。此次仅按用户新增授权执行一次新版 Pin，不新增目录请求。

发送前发现正式入口直接比较管理 `rev-9` 与 serving 数字 `9`，形成可复现的 `execution_configuration_changed`、发送 0。后继修复精确比较数字与唯一 `rev-N` 表示；拒绝不同值、引号、前导零、浮点、错误配置 ID，不改变账号 revision 比较。独立审查另指出授权 binding 与 Pin 后声明、既有 Client Key／projection 再冻结风险，后继修复逐项保留范围并重新绑定，均在真实发送前完成。

新版 Pin 于 `2026-10-10T14:37:14.523138Z` 完成：管理 HTTP200，保护回执 `outcome=succeeded`、`attempt_count=1`、`upstream_sent=true`、`response_started=true`、stage `http_status`；实际 target、lease 134、配置 9、direct egress 与运行候选相同。判定依据是保护回执及实际执行，不是管理 HTTP 状态。

结论只到“**当前候选、当前账号、当前 Responses JSON 路径通过**”。版本头／UA 从 0.2.111 对齐到 1.0.50 是已验证的本地漂移修复，新版未再出现旧 Pin 的 426；尚不能将其写成 426 的唯一根因。旧失败与旧私有材料不改写。

## 六组合基础矩阵与实际首轮

复用正式 `cpar-batch-c-live.py --new-build` 所委托的 `cpar_new_build_acceptance.py`、既有 3／2／4 场景和原 `REMOTE_READ／REMOTE_COLLECT`。远端 wrappers 只实施同任务身份、预算、独占锁和请求停止控制；无替代场景、无参数降级、无模拟工具提示词。

| 协议／模式 | 对应 Pin | 文本多轮 3 次 | 工具及结果 2 次 | 连续工具 4 次 | 组合结果 |
| --- | --- | --- | --- | --- | --- |
| Chat JSON | NOT_RUN | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |
| Chat SSE | NOT_RUN | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |
| Messages JSON | NOT_RUN | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |
| Messages SSE | NOT_RUN | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |
| Responses JSON | PASS | FAIL，首轮停止，发送 1/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | FAIL |
| Responses SSE | NOT_RUN | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |

这里文本 FAIL 指实际执行器在第一轮报错并停止；完整三轮并未完成。原正式回执自身仍保留 `text_multi_turn=NOT_RUN／execution_stopped／executed_turn_count=1` 和 `basic_status=NOT_RUN`，同时有 `execution=FAIL／response_error`、`formal_audit_status=FAIL`。本批总结按实际执行失败记 FAIL，未覆写该旧回执。

唯一公共请求返回 HTTP200、`status=completed`、`error=null`、`incomplete_details=null`，原生 output 有 reasoning 和 message，工具调用数 0。正式 `observe()` 先拒绝任何存在 error key 的对象，因此在它的后续严格终态校验前误判正常 Responses 成功对象。公共产品编码本来输出 `error=null`；这是验收判定缺陷，不能归因为 Provider 失败。

停止后只读补采：Request／Attempt／Usage／request_finished 各 1，账本 1 行；唯一 Attempt outcome succeeded、retry decision completed。原 payload SHA256 与执行 sidecar 精确匹配，channel、账号 lease、配置、candidate、模型、egress、Client Key、协议与 stream 均匹配冻结身份；Usage 原响应、事件与账本按真实 source event 关联，逐字段相等。Pin 不持久化这些公共事件，没有要求它虚构 Usage／账本。

| 首个公共请求的 Usage 事实 | 值 |
| --- | --- |
| input／output／total | 227／136／363 |
| reasoning_tokens | 135，属于已有 output 语义，不再重复加总 |
| cached_tokens | 128 |
| cache_read_tokens／cache_creation_tokens | null／null，未填零 |
| provenance／input_accounting | measured／inclusive |
| 账本金额／状态 | `cost_microunits=null`，`cost_confidence=unpriced`，`billing_status=unpriced` |

未验证另外两轮历史、任何工具 call_id／结果闭环或连续两轮工具交互。53 个基础请求及另外 5 个协议／模式 Pin 立即停止，NOT_RUN；unused request budget 不构成停止后的继续授权。

## 修复、本地门槛及审查

本地提交顺序：`f9412895` 精确管理 revision 表示；`a6aa31df` 绑定 Pin 后声明；`3bf9acd3` 保留冻结 Client Key／projection 范围；本轮最后的 nullable error 与执行失败汇总修复见收口提交。

真实停止后的最小修复只改变两处 formal 判定：Responses 接受 absent／null error，任意非 null error（含 `{}`、false、0、空串）继续拒绝，其他协议规则不变；`basic_status` 将 execution FAIL／BLOCKED 纳入原优先级。没有删请求字段、降参数、改变所有权或允许失败后的重发。

新增测试先在原代码得到 38 tests／4 个 subtest FAIL（JSON、SSE nullable 以及 FAIL／BLOCKED 汇总），修复后 **38/38 PASS**。此前本批 revision、float、client scope 的 RED 和后继 35/35 GREEN 均保留；docs 正式入口 **7/7 PASS**。完整原日志以 gzip 保存，不修改其字节来通过 whitespace。

产品 source 未变，直接复用已核对的干净 `1b6023f5` Full **45/45**、Rust **1501 passed／0 failed／12 ignored**、Build 定向协议 **19 PASS** 及候选 manifest／binary。未机械重跑 Rust／前端全量；旧 Full 不能冒称验证本批新增 Python 修复，后者由定向测试和本批 docs 门槛覆盖。

Standards **PASS（独立限定范围）**；Spec **PASS（独立修复 diff 范围）**；#47 整票／全项目 Spec **PARTIAL**。历次 Spec FAIL、修复与独立覆盖限制见 [review.md](assets/cpar-build-revalidation-20261010/review.md)。后增离线／seal 及报告由主代理复核，没有独立全范围审查结论。

对已经收到的首轮离线重读，修复后响应和请求控制、精确归属及 Usage lineage 均 PASS；因原进程观察超过 formal 600 秒有效窗，当前完整 runtime audit **BLOCKED／runtime_observation_stale**。没有更改时间、延长窗口或改成“完整基础 PASS”，没有新网络推理。原执行 FAIL 保持。

## 请求预算、网络观察和费用

| 类别 | 本批新增 | 旧批保留 | 历史累计 |
| --- | --- | --- | --- |
| 真发送 Pin | 1，PASS | 1，HTTP426 失败 | 2 |
| 公共基础推理 | 1 | 0 | 1 |
| 推理 Attempt 总数 | 2/60 | 1 | 3/61 |
| 目录 GET | 0 | 1 | 1 |
| 管理 Pin 发送前拒绝 | 0 | 1 | 1 |
| retry／fallback／失败后真实 rerun | 0／0／0 | 0／0／0 | 0／0／0 |
| OAuth／refresh／自动 catalog／额度查询／扩展／故障探测 | 全部 0 | 保留旧批 0 | 本批无新增 |

management GET 与真实推理分开：[request-counts.json](assets/cpar-build-revalidation-20261010/request-counts.json) 记录可从归档程序核对的 **76 个 GET intents**（发送前检查 1、启动 6、正式预检 22、两次 Pin inspection 44、退出 2、finalize 1）及 Pin POST 1。更早临时诊断未统一埋点，完整 GET 总数 UNKNOWN；GET intent 亦不能冒称管理服务接收数，启动 readiness 可能在连接前失败。

网络审计只有 DNS allowed 2／TCP connect allowed 1，连接复用导致其不能等同两个 HTTP 调用。gateway 已发 Pin 和公共 Attempt 的计数是 2；**Provider 独立 HTTP 接收数 UNKNOWN**。实际响应事实保留，但不伪造上游独立计数。

货币授权为 UNLIMITED_USER_EXPLICIT；新增推理消耗 2/60、累计 3/61，58 个未用额度因 stop 不能自动使用。未做额度／余额查询；Provider 实际扣费 **UNKNOWN／NOT_PROVEN**，公共账本为 unpriced，不能写成 0 元或精确费用。Pin 无账本是其本来行为。

## 退出、保护材料与生产不变量

已停止临时实例，PID=0、原 PID 消失、19180／19181 无 listener；停止后审计 5 秒保持不增，最终 seal 再次确认。候选、旧失败和新版原始材料保留，不做全局清理。

首退出程序在完成停服及生产核对后，对单个 `presend-inspection-private.json` 的 0600 断言失败；它原为 0644，但一直处于 0700 根／epoch 中。后继仅收紧该自有文件到 0600、内容 SHA 不变，并验证所有 epoch 文件 0600、目录 0700。失败 stderr、原哈希和后继回执都保留，不将该断言失败解释为未停服。

旧 gateway log 当前 6178 bytes，与旧归档 5308 bytes 的精确 prefix 相等；新增 870 bytes／6 行是原批优雅停机尾部。旧 Pin、预算 v1/v2、audit 哈希保持，未把合法 append 写成覆写旧失败。

生产 `cpa-rust-gateway.service` 前后保持：PID499070、NRestarts0、active/running；unit SHA `fe8e5fb9329a478c17fac3b23a8a3c29bc6048aab7f6af6db82e14589bc49213`，binary SHA `0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b`；current 仍指向 `b0cf36e387b774439acc23830221389eb3ac74d0` release；Caddy SHA `bfede78bae3c2bb4d120bcbd08924c0ba564fa3a1a02d8edda37a9004f9b945e`；active configuration 仍为 `kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75／rev-1`。18180／18181、生产配置和服务未更改。

真实材料所在：`new-vps:/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010/private/revalidation-20261010/`；历史 `private/phase2/` 与自有候选 `bin/1b6023f5/` 保留。公开 [final-receipt.json](assets/cpar-build-revalidation-20261010/final-receipt.json)、[seal.json](assets/cpar-build-revalidation-20261010/seal.json) 和 [evidence-index.json](assets/cpar-build-revalidation-20261010/evidence-index.json) 只包含脱敏事实、hash 和范围；本机任务 output 是保护工作底稿，不提交原始响应或凭据。

## GitHub、交付边界与下一项

已更新 [#47 既有进度评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57 既有进度评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9 既有进度评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)。主代理在 15:04Z 使用新 GET 回读，独立只读代理在 15:06Z 再次读取并完成核验，均 PASS：comment SHA256 匹配、历史正文逐字保留为完整 prefix、marker 各一次、评论各仍 1 条、三票 OPEN；正文／AC／title／标签／负责人／native dependencies 均不变。见 [主代理 fresh readback](assets/cpar-build-revalidation-20261010/github-readback.json) 和 [独立 readback](assets/cpar-build-revalidation-20261010/github-independent-readback.json)。

本地修复／真实证据归档提交为 `68158c012ce44dd3e6aeafcd342c857e10ed37af`；此前本批提交 `f9412895`、`a6aa31df`、`3bf9acd3` 均保留。固定 `68158c01` 的正式 docs 再核对 **7/7 PASS**，见 [固定提交门槛回执](assets/cpar-build-revalidation-20261010/fixed-commit-docs.json)。后继只归档 GitHub 回读及文档收口，实际 source／binary／真实执行结果不变。本批没有 push，不提供不存在的远端 commit／blob 链接。

#47 当前共性前置 #16／#27／#29–#34 均 CLOSED，扩展关联 #35–#38 也以各自本地范围 CLOSED；它们不替代新版 Build 的真实证据。#57 仍有 11 渠道票及全承诺证据／发布候选边界未收口；#9 整体规格未满足。没有扩张到 Grok Web／Kiro 实施。

当前 serving 声明和下一批需要的证据：

| 声明 | 当前值 | 仍需验收，本批均 NOT_RUN |
| --- | --- | --- |
| reasoning | true | visible_reasoning、reasoning_history；本轮偶然输出 reasoning 不代替显式参数／历史验收 |
| parallel | false | 保持未声明及显式拒绝，不能改为支持或关闭其他能力来通过 |
| stored | true | store_read、foreign_owner、deleted_read、expired_read、restart_read、persistence_failure |
| continuation | true | exact_continuation、lineage_rejection；保持既有 E-05 精确连续性边界 |
| compact | true | compact_roundtrip、compact_foreign_owner、compact_lineage_rejection |
| websocket | true | ws_multiturn、ws_handshake_rejection、ws_cancel、ws_failed_root |

后五项 true 对应 formal 17 类检查，场景检查数不等于真实请求预算；协议适用性、受控零发送负例与真实正例需另行分开计算。基础全部通过也不能关闭这些扩展承诺。

下一项是**在修复后的正式入口上，另行明确授权一个新的真实基础验收批次**，重新核对届时相同账号的资格／expiry、source／artifact／配置／lease／声明，逐协议模式 Pin 后串行 54 次基础，失败即停。若重新完整执行六 Pin＋54 基础，需要明确最多新增 60、历史累计最多 63（目前累计 3），不能沿用本批 61 的累计上限或停止后的剩余额度。当前账号有明确 expiry；若已过期或需要刷新／重新授权，须先提供该动作的明确授权，不能沿用本批禁止刷新条款绕过。此报告完成准备及本轮收口，不安排未授权后台继续。

当前证据层：本地／受控上游及产品 Full 复用 **PASS（各自范围）**；真实 Provider Pin **PASS**，真实基础批次 **FAIL／未完成**；临时候选制品 **PASS**；当前源码 remote CI、签名正式制品、正式发布及生产部署 **NOT_RUN**。
