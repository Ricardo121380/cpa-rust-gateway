# CPAR #47：Grok Build 六组合真实基础验收及失败收口

本批真实基础验收 **FAIL，已停止**。新 Responses JSON Pin **PASS**；文本多轮 **3/3 PASS**；单工具场景发送 **2/2**，结果回传轮触发 `history_or_result_not_used`，场景 **FAIL**。完整场景 **1/18**、完整组合 **0/6**。新增真实推理 **6 次（Pin 1＋基础 5）**，连同历史 3 次累计 **9/63**。其余请求和全部扩展未发送。修复后没有真实重跑，不能把本地通过改写为真实六组合通过。

批次沿用 `20261010` 标识，真实执行时间为 **2026-10-11 00:04—00:07 CST**（UTC 2026-10-10 16:04—16:07）。#47／#57／#9 继续 OPEN；本报告与扩展清单是本轮交付，失败停止后的额度不自动续用。

## 基线、范围与继承证据

开始 HEAD 为 `27f8349fff2e98e8d10700ae78c856f27503d52c`，分支 `codex/prism-v4-delivery`，相对用户历史检查点没有新增提交。未回退。产品 source 为 `1b6023f52f4c00ea6d8296524d306c141e4edae7`。根 AGENTS.md 的用户修改及无关 tracked／untracked 内容保留。

已读取项目规则、跨边界日志、issue tracker、[上轮复验](cpar-build-revalidation-20261010.md)、[旧真实失败](cpar-build-live-20261010.md)、[授权记录](cpar-build-acceptance-authorization-20261010.md) 和 GitHub 三票的完整正文／既有进度评论／依赖。当前规格未缩减；旧 HTTP426、旧 `error=null` 误判执行 FAIL、旧 `runtime_observation_stale` 和旧计数仍各自保留。

发送前核对的正式入口及 tests 与 `68158c012ce44dd3e6aeafcd342c857e10ed37af` 字节相等，见 [byte-verification.json](assets/cpar-build-six-combinations-20261010/byte-verification.json)：

- `cpar_new_build_acceptance.py`：`2107227d545acb898ea6b036bdcef6ec5fc030e266b880d63f876b62746a9f48`。
- `cpar-batch-c-live.py`：`7d4187759fb86210d5791c37a397f385e4c3254c69bf1f95319bdd55ee5f992f`。
- 本批真实执行仍使用以上正式入口；后继本地修复未替换远端执行代码或旧回执。

`apps/`、`crates/`、`web/`、Cargo 文件、Dockerfile、SPA 构建脚本与 check.sh 相对产品 source 均无产品改动。按已核对来源复用 clean Full **45/45**、Rust **1501 passed／0 failed／12 ignored**、Build 定向 **19 PASS**；它们不验证本轮后增 Python 修复。新改动只涉及验收脚本、对应测试和本任务报告／资产，无前后端共享边界改动。

## 同账号正常续期与新批次绑定

23:48 首次只读检查时，旧副本 native revision=133，expiry 为 **2026-10-10 23:56:42.393 CST**；不足启动安全余量，未发送。23:55 再次只读检查发现已授权来源已正常续期为 native revision=134，expiry 为 **2026-10-11 05:49:42.392 CST**；池 available、active leases=0、worker claim 空。

已确认相同授权来源、相同账号脱敏身份 `5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54`、identity digest 不变，正常刷新记录前进。只复制该账号当前材料及匹配 key version=1 至自有新 epoch。来源读库 schema31，自有候选 schema34；副本账号字段除本任务 import batch 外逐字段与来源相等。任务发起 OAuth／refresh／重新授权 **0 次**。

新 epoch 为 `new-vps:/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010/private/six-combinations-20261010/`。从原自有隔离状态 backup 建立新副本；继承 Request／Attempt／Usage／terminal 各 1 和 ledger 1，均明确排除在本批增量外。新预算账独立保存历史 Pin2＋基础1=推理3、历史目录GET1，未复用旧“停止后剩余额度”。

新材料没有套入旧 Pin／旧进程／旧会话。实际新 Pin 和公共 Attempt lease revision 均为 **135**，与 native134 表示不同对象。旧会话仍须满足 [E-05 精确连续性契约](../contracts/BC-RESP-003-exact-continuity-and-compaction.md)；本批未调用旧根或宣称新副本更新自动满足其例外。

账号更新及准备见 [prepare-receipt.json](assets/cpar-build-six-combinations-20261010/prepare-receipt.json)。报告不公开密钥、ciphertext 或原始账号 ID；来源 external identity 为 null，归属依据为既有批准材料来源及账号 ID 的一致性，不凭空补个人身份。

## 启动准备、发送前资格及实际运行身份

操作前确认 owner.task=`cpar-build-20261010`，临时 unit／19180／19181 空闲，没有其他任务占用该目录或状态文件。仅操作指定临时 service、保护根及 loopback 19180／19181；生产保持原状态。

首次启动在任何真实发送前退出：必要 `management-csrf` 材料缺失。此时 MainPID=0、审计0、事件增量0、Pin0／基础0。修复仅补齐原自有保护区已有的 `management-csrf`、`backup-key`、`client-key-pepper`、`grok-build-cache-key`；不生成或修改生产授权。旧失败日志、旧 prepare／start bytes 与失败回执均保留。修正后的 prepare 文件是后继准备修复参考，实际首执行 prepare 另以 `remote-prepare-v1.py` 原字节归档。

第二次启动 PASS；发送前连续 **65.61 秒**审计不增，后台 OAuth／refresh／自动 catalog disabled，单 enabled candidate、单账号池、`max_attempts=1`，worker claim 空，保护目录0700／文件0600，guard 已映射，管理审计不变，无并发状态占用。账户剩余有效期约5小时；每次新发送还核对 expiry 至少剩60秒。见 [start-receipt.json](assets/cpar-build-six-combinations-20261010/start-receipt.json) 与 [归档工具说明](assets/cpar-build-six-combinations-20261010/tools/README.md)。

| 冻结项 | 新批次实际值 |
| --- | --- |
| source | `1b6023f52f4c00ea6d8296524d306c141e4edae7` |
| 实际 binary SHA256 | `ea89d5c72628a871f7683f32d0b51a76dc9c588f7172c86345d1c989f1bd9063` |
| manifest 文件 SHA256 | `20ef11961ca40d4dd659ae278c1e7ee629f9f70319f4a4b006d2d8685a5c9777` |
| manifest 规范化对象 SHA256 | `ffaea62e61fa3c974d2e4a212556249ac2db10963657db9d078eb7e151de7d14` |
| binary／平台 | 19,500,176 bytes，Linux AArch64；复用已核对候选，未新构建 |
| 临时进程 | PID2318163，`/proc` start=`657486820` |
| config | `cpar-build-config-20261010`，revision9（管理 `rev-9`） |
| channel／provider | `grok.build`／`cpar-build-provider`，原生 Build |
| endpoint／route／candidate | `cpar-build-endpoint`／`cpar-build-route`／`cpar-build-candidate` |
| 模型 | public／upstream／response 均为 `grok-4.5` |
| 首组合 | Responses JSON；`max_output_tokens=4096` |
| 账号材料／实际 lease | native134／runtime135，同一账号，无替换或 fallback |
| egress | direct，脱敏身份 `69aa38e91dc761ae26632216b5fc04e7362ce1437b2116cc15340c6c77e830b6`；revision=`not_applicable` |
| Client Key／projection | `ff7ef703ed7ad99438aa143c89b8f17704d93746574f3e557875440274181025`／`76b46919f61a1174ea8bab32a9ba08174812e6d068e97c80f0eb9b6070802e25` |
| serving 声明 | reasoning／stored／continuation／compact／websocket=true；parallel=false |

新采运行观察及 Pin 后声明均绑定该 PID／config／candidate／Client Key／projection。600秒有效窗未延长，旧时间戳未修改。实际资料和最终 hash 见各回执；当前已停止进程，归档观察不能直接用于后继发送。

## 六组合实际矩阵

正式 `cpar-batch-c-live.py --new-build`、执行器、collector 和 3／2／4 场景均复用；wrapper 只增加授权预算、独占锁、每次身份检查及 fail-stop。首组合选择 Responses JSON。

| 协议／模式 | 精确 Pin | 文本多轮 | 单工具及回传 | 连续两轮工具 | 组合 |
| --- | --- | --- | --- | --- | --- |
| Responses JSON | PASS，1次 | **PASS 3/3** | **FAIL，发送2/2** | NOT_RUN 0/4 | **FAIL** |
| Responses SSE | NOT_RUN 0/1 | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |
| Chat JSON | NOT_RUN 0/1 | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |
| Chat SSE | NOT_RUN 0/1 | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |
| Messages JSON | NOT_RUN 0/1 | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |
| Messages SSE | NOT_RUN 0/1 | NOT_RUN 0/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | BLOCKED |

[Pin 回执](assets/cpar-build-six-combinations-20261010/responses-json-pin.json)：management200，outcome=succeeded，attempt_count=1，upstream_sent=true，response_started=true。该 Pin 只证明自身 Responses JSON，不能替代其他五个 Pin。

[原基础回执](assets/cpar-build-six-combinations-20261010/responses-json-basic.json)：前三轮完整文本历史及严格答案检查 PASS，每轮唯一 Request／Attempt、Usage 与 ledger source-event 关联均 PASS。第四轮返回真实 `lookup({"value":1})` 调用，原生 assistant items 和 call_id 留存。第五轮 HTTP200／completed／error=null／incomplete=null，但最终文本未符合 expected_text，执行器立即 FAIL 并锁定停止；工具闭环不能记 PASS。

第五轮尚未进入原 collector，停止后用**相同正式 REMOTE_READ／REMOTE_COLLECT**只读补采。归属及 Usage PASS，唯一 Attempt succeeded、ledger1、usage1，实际 config9／lease135／同 Client Key／模型等均精确匹配。补采使用当时活进程的新观察，不塞回原 wire 或抹除其 execution_failure。见 [posthoc-receipt.json](assets/cpar-build-six-combinations-20261010/posthoc-receipt.json)。

| 新公共请求 | input | output | total | reasoning | cached | 场景结果 |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 文本1 | 227 | 110 | 337 | 109 | 128 | 完整三轮中的第1轮 |
| 文本2 | 242 | 108 | 350 | 92 | 128 | 完整三轮中的第2轮 |
| 文本3 | 272 | 177 | 449 | 161 | 128 | 文本场景 PASS |
| 工具调用 | 303 | 157 | 460 | 148 | 128 | 真实 call_id／参数已核对 |
| 工具回传 | 346 | 614 | 960 | 602 | 256 | 严格最终答案 FAIL |

以上均 measured／inclusive；cache_read／cache_creation 缺失为 null，未补0。reasoning 属于 output，不重复计费或加到 total。事件和账本保留同一 source usage，金额均 null／unpriced，Provider 实际扣费 UNKNOWN。

## 失败定位、最小修复和验证边界

只读诊断确认正式脚本的确定性缺陷：`conversation_step()` 调用原执行器得到正确值 `"1"` 后，将工具结果覆盖为整段 `"Return exactly ACCEPT:..."` 指令；下一请求没有普通用户回放指令，却严格期待裸标记。原生 items／call_id 配对虽然保留，执行器结果契约已经改变。此缺陷能在本地公共入口复现；它不证明 Provider 最终回复的唯一因果，也不把产品判为完整通过。

先强化既有真实 loopback HTTP fixture，再在未修复正式入口上运行：六个协议／模式均于第五请求停止，**1 test／6 subtest FAIL**。随后最小修复：

1. 保留既有确定性执行器 `lookup(value=1)` 的结果 `"1"`，原 call_id／原生 items／有序历史全部保留。
2. 下一步指令放普通 user 消息；Messages 同一 user content 在 tool_result 后增加 text block。
3. 工具最终答案严格期待随机标记加 `:1`，用户要求标记加“实际 lookup 结果”，不把答案值写进回放指令。原严格 equality、3／2／4、请求控制及归属检查不放宽。

修复后正式 oracle **38/38 PASS**。六模式每组合9个 loopback 请求完整通过；错误工具结果返回 `:2` 时仍于第五请求 FAIL，后续停止。零真实 Provider 调用。见 [local-verification.json](assets/cpar-build-six-combinations-20261010/local-verification.json)、其中 RED／GREEN 完整 gzip 日志及 [六模式本地回执](assets/cpar-build-six-combinations-20261010/local-http-receipts/responses-json.json)。修复后 formal SHA256 为 `3dee3525132871d4049a3d8b05c5481eb902425380babcdaa4e5a4a656d7c4fa`；真实执行的旧 SHA256 保持原值。

首轮固定 `7fb544b6` 的 docs **7/7 PASS**，独立 Standards／Spec 均无已证实finding，见 [首轮审查](assets/cpar-build-six-combinations-20261010/review-7fb544b6.md)。Standards另指出历史兼容性提示：原无版本号v2的7请求历史经新函数重审变FAIL，旧测试用新函数重建历史未覆盖原encoding。

主代理按已存在的历史版本契约，从公开audit CLI复现六模式原v2 encoding的FAIL后，最小修复只读version2审计比较其原历史；live producer继续固定version3，输出1／普通user指令及严格答案不变。历史7请求不能晋升为新9请求资格，任何真实旧失败／wire／时间戳均未重写。后继 **38/38 PASS**，详见 [local-verification-v2.json](assets/cpar-build-six-combinations-20261010/local-verification-v2.json)；当前formal SHA256=`b33e551ef018df71a4bb0d3e301249b75eaf603d46590b20529e649ee1ff9d8a`。首轮3dee字节／回执另留；后继diff需新的独立审查，不能套用首轮结论。

最终项目门槛及独立审查在收口资产记录。审查只验证本地修复与证据表达，不能替代尚缺的真实五组合／工具闭环／扩展正例；#47整票Spec继续PARTIAL。

## 计数、停止与退出

| 类别 | 本批新增 | 历史保留 | 累计 |
| --- | ---: | ---: | ---: |
| 上游已发 Pin | 1 | 2 | 3 |
| 公共基础 Attempt | 5 | 1 | 6 |
| 推理总数 | **6/60** | **3** | **9/63** |
| 模型目录 GET | 0 | 1 | 1 |
| 额度／OAuth／refresh／重新授权／扩展／故障探测 | 0 | 按旧批原证据保留 | 本批0新增 |
| retry／fallback／失败后真实 rerun | 0／0／0 | 0／0／0 | 0／0／0 |

新公共事件增量 Request／Attempt／Usage／request_finished 各5，ledger5；继承各1另列。实际 Pin1＋Attempt5 与预记发送意图6相等。网络审计有 DNS／TCP allow 事件，连接复用使其不能等同 HTTP 数；**Provider 独立 HTTP 接收计数 UNKNOWN**。

明确埋点的 management GET intents 为启动6、Pin前后44、退出生产配置只读1，共51；更早 readiness／诊断是否有额外管理 GET 不由此推定，完整管理 GET 总数 UNKNOWN。这些不是模型目录／额度查询，也不是推理预算。停止后仅本地／只读诊断。

临时 unit 已停止，MainPID0、原 PID 消失、19180／19181 无 listener，停止后5秒审计不增。预算 `stopped=true`；剩余 Pin5／基础49 不构成重跑许可。所有自有 epoch 目录0700、文件0600；旧失败及旧证据 hash 保持。见 [final-receipt.json](assets/cpar-build-six-combinations-20261010/final-receipt.json)。

生产前后 PID499070／NRestarts0／active-running，相同 unit／binary／Caddy SHA，current 指向既有 `b0cf36e387b774439acc23830221389eb3ac74d0`；active config=`kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75／rev-1`。未修改生产 service／配置／权限／current／Caddy／18180／18181。没有 push、正式发布或生产切换。

## 扩展准备、GitHub 与交付状态

由本批实际 serving 声明建立 [17类扩展后续清单](cpar-build-extensions-next-20261010.md)：reasoning2、stored6、continuation2、compact3、websocket4。parallel=false 另保留显式拒绝资格，未关闭任何 true 声明规避验收。全部扩展真实 NOT_RUN；未证明安全执行的项目 BLOCKED。17类不等于17次请求，清单提供带前提的计数与授权缺口，不编造合计预算。

GitHub 更新范围限定 [#47 原进度评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57 原进度评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9 原进度评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)。追加本批事实并完整保留旧 prefix；正文／AC／状态／title／labels／assignees／native dependencies 不改。本轮回读和完成提交在收口资产中记录，不能以准备完成关闭 #47。

真实目标的缺口仍是单工具闭环、连续工具、其余五模式各自 Pin 与基础，以及已声明扩展。按照用户失败停止要求，本批不能用未消耗额度继续发送。后继任何真实批次都须重新明确请求授权、累计计数上限和当前资格；本报告不安排后台继续。

证据层：产品 Full 复用 PASS（原范围）；新 Python 本地 PASS；新真实 Pin PASS；真实基础批次 FAIL／未完成；扩展 NOT_RUN／准备清单 BLOCKED；remote CI／签名正式制品／正式发布／生产部署 NOT_RUN。
