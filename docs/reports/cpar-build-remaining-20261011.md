# Grok Build 剩余基础组合与验收工具收口（2026-10-11）

验收工具的限定实施切片已取得独立 **Standards PASS／Spec PASS**。本批真实基础验收 **FAIL，已停批并退出**：Responses SSE 新 Pin＋9/9 完整通过；Chat JSON 新 Pin 通过，首个基础请求 HTTP400 `ClientRequestError`；Chat SSE、Messages JSON、Messages SSE 均 NOT_RUN。保留历史 Responses JSON 9/9 后，完整场景 **6/18**、完整组合 **2/6**。扩展真实调用仍为0，#47、#57、#9 继续 **OPEN／PARTIAL**。

本批新增预算意图 **12/50（2 Pin＋10 基础 HTTP）**，加历史21为 **33/71**；网关及 Pin 回执确认实际推理发送 **11（2 Pin＋9基础 Attempt）**，历史累计 **32**。HTTP400对应本地 Request失败、没有 Attempt／Usage；它仍占用一次预算，不释放或续用余量。独立上游接收计数和实际费用 **UNKNOWN**。

## 基线、范围与审查

开始核对的分支为 `codex/prism-v4-delivery`，实际 HEAD=`9a9dc029c42ee7fd3b476cb6d290052a88fbfb74`。用户给出的衔接信息没有作为回退指令。当前产品源码仍为 `1b6023f52f4c00ea6d8296524d306c141e4edae7`，本批未修改 Rust、前端、共享契约或生产配置。初始已修改 AGENTS.md、无关 tracked/untracked 文件均保留，未 reset、清理或宽范围暂存。

固定审查起点为 `2d66f892a0e507becbf9d2bd1f0142a64d16fd4a`。核心最终审查 HEAD=`bb30f04839abb566aa8d11dd950280f7deaa70d1`，92路径；新包装及退出计数最终审查 HEAD=`dbfb977ac24e825a43c73f6317576a8604b8bd86`，108路径。完整提交列表、文件清单及 SHA256 见 [核心冻结](assets/cpar-build-remaining-20261011/review-freeze-execution.json)、[本轮最终冻结](assets/cpar-build-remaining-20261011/wrapper-review-freeze-final.json)。代码实施与后续文档提交分别记录；该范围不包含全仓产品审查。

| 轴 | 核心终态 | 新驱动终态 | 证据与限制 |
|---|---|---|---|
| Standards | complete PASS，硬问题0、smell0 | complete PASS，硬问题0；继承非阻断 heuristic1 | [核心回执](assets/cpar-build-remaining-20261011/review-final.json)、[新驱动最终回执](assets/cpar-build-remaining-20261011/wrapper-review-final.json)；两独立只读探子分别核验 |
| Spec | complete PASS，未闭合问题0 | complete PASS，未闭合问题0 | 同上；108/108磁盘与Git blob一致，92核心路径未变，复用其原完整结论 |

本批实施提交：`be9df161`（资源原始操作链与成功WS Origin）、`4790be8d`（DELETE／compact／WS精确契约）、`bb30f048`（compact消费前止步、严格布尔删除及空终态）、`1758c70f`（六个新批次远端驱动）、`dbfb977a`（未知Pin发送数保留null、资源与计数结论分开）。`9fcb4fcc`仅修正文档中的合成接收计数。后续审查及真实回执、此报告和GitHub读回均属文档证据提交。

初始及中间 FAIL/PARTIAL 审查、红绿复现全部保留；没有用测试全绿替代未完成审查。最终闭合项涵盖公开受约束投影、发送失败完整请求及已收到原字节、根→parent→summary→consume→final同链与固定计数／controls、真实资源状态、WS精确路径／Origin／nonce／原握手字节及终态后额外帧、DELETE精确对象和JSON布尔类型、compact唯一opaque item与完整成功终态、坏summary在消费前停止。SSE仅补契约允许的空annotations／空或null logprobs；文本变化、非空新增、字段丢失及终态矛盾继续拒绝。

新包装的非阻断 `Duplicated Code` 为 standalone Pin/basic 原子 `save_budget` 的相同形状。没有文档硬性要求提取，保留独立可执行脚本，未为风格重构。最后一项计数 P2 的 [红证据](assets/cpar-build-remaining-20261011/review-pin-count-red.json)、[四案例绿证据](assets/cpar-build-remaining-20261011/review-pin-count-green.json) 与独立两轴核验均保留；未知Pin实际发送数不再投影为0或计数PASS。

## 本地验证与真实执行资格

最终 [50/50定向回归](assets/cpar-build-remaining-20261011/unit-execution-tests.json)、[354/354公共边界检查](assets/cpar-build-remaining-20261011/local-execution-public-receipt.json)通过。40次正式执行CLI＝19 PASS＋10 BLOCKED＋11预期FAIL；19成功bundle另经正式audit复核。52合成接收＝44 CLI＋5 Pin＋3独立演练，全部 **LOCAL_SIMULATED**。包装的 [5项transport控制检查](assets/cpar-build-remaining-20261011/wrapper-local-checks.json)与退出4案例也仅属本地。未变化源码及一致hash的既有验证被复用；旧产品 Full 不是新增Python验收器证明。

新授权见 [user-authorization](assets/cpar-build-remaining-20261011/user-authorization.json)：剩余五组合各最多1新Pin＋9基础，新上限50、历史起点21、累计上限71；继承明确批准费用政策，不继承旧失败批次余量。没有重跑 Responses JSON，没有增加账号、模型、Client Key、扩展调用、目录查询或额度查询。

[只读资格](assets/cpar-build-remaining-20261011/readiness.json)核对原批准来源的正常续期：同账号、同非空身份摘要及同授权来源，连续区间floor77，材料从原134正常变为135。没有主动refresh、重新授权或账号替换。严格比较原授权与新来源字段、密文／key版本及既有连续性后，仅在既有保护根建立新的自有 `remaining-five-20261011` epoch；[准备回执](assets/cpar-build-remaining-20261011/prepare-receipt.json)的136是预期值，[启动回执](assets/cpar-build-remaining-20261011/start-receipt.json)及本批两个新Pin将实际运行pool lease revision **136** 独立确认。原生产pool的null revision没有用作lease证明。

| 身份 | 本批冻结值 |
|---|---|
| 环境／unit／端口 | new-vps，`cpar-build-acceptance-20261010.service`，loopback19180／19181 |
| product source／candidate binary SHA256 | `1b6023f52f4c00ea6d8296524d306c141e4edae7`／`ea89d5c72628a871f7683f32d0b51a76dc9c588f7172c86345d1c989f1bd9063` |
| manifest文件／规范化对象SHA256 | `20ef11961ca40d4dd659ae278c1e7ee629f9f70319f4a4b006d2d8685a5c9777`／`ffaea62e61fa3c974d2e4a212556249ac2db10963657db9d078eb7e151de7d14` |
| 实际PID／process start | 2992034／661359016 |
| config／endpoint／route／candidate | `cpar-build-config-20261010/rev-9`／`cpar-build-endpoint`／`cpar-build-route`／`cpar-build-candidate` |
| model／材料revision／实际lease | grok-4.5／135／136 |
| 账号／Client Key evidence ID | `5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54`／`ff7ef703ed7ad99438aa143c89b8f17704d93746574f3e557875440274181025` |
| egress | direct，revision not_applicable；guard SHA256 `91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06` |
| 请求控制 | Responses SSE `max_output_tokens=4096`；Chat JSON `max_completion_tokens=4096`；单账号池／单enabled candidate／max_attempts1／无fallback |
| 能力声明 | reasoning/stored/continuation/compact/websocket=true，parallel=false；未削减声明 |

[manifest口径核对](assets/cpar-build-remaining-20261011/manifest-hash-reconciliation.json)对同一远端manifest同时重算：启动记录使用文件原始字节SHA256，正式 `bind_manifest` 使用排序、紧凑JSON对象SHA256，两者不是相同输入。对象revision与实际binary均匹配。初次独立证据复核因限定receipt缺口径说明记为 [PARTIAL](assets/cpar-build-remaining-20261011/real-evidence-initial-review.json)，补充结果单独保留，不改写原结论。

[远端12文件字节核对](assets/cpar-build-remaining-20261011/remote-byte-verification.json)与最终审查hash一致，包括正式入口、core、extensions及六个驱动。65.60秒启动静默观察无网络事件，后台OAuth／refresh／自动catalog禁用，账号worker claim为空，无竞争状态进程；每个真实请求前后仍检查绑定进程、binary、材料revision、审计及有效期。完整请求、原生items、call_id、有序历史、真实工具结果及raw字节留在0700/0600的自有私有目录，未进入提交或GitHub。

## 六组合矩阵与首个失败

| 组合 | Pin | 文本多轮3 | 单工具闭环2 | 连续两轮工具4 | 基础结论 |
|---|---|---|---|---|---|
| Responses JSON | 历史 PASS | 历史 PASS 3/3 | 历史 PASS 2/2 | 历史 PASS 4/4 | 历史 PASS 9/9；本批未重跑 |
| Responses SSE | 本批 PASS | PASS 3/3 | PASS 2/2 | PASS 4/4 | **PASS 9/9** |
| Chat JSON | 本批 PASS | **FAIL**：首轮400；其余2 NOT_RUN | NOT_RUN 0/2 | NOT_RUN 0/4 | **FAIL**：仅1/9 HTTP请求，0通过 |
| Chat SSE | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 停批后未执行 |
| Messages JSON | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 停批后未执行 |
| Messages SSE | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 停批后未执行 |

历史 Responses JSON 来源是 [原批报告](cpar-build-batch-20261011.md)及其 [Pin](assets/cpar-build-batch-20261011/responses-json-pin.json)、[正式基础回执](assets/cpar-build-batch-20261011/responses-json-basic.json)。产品source/binary/manifest、配置和能力未变；同授权来源的严格连续性已核对，旧结果作为明确标记的历史证据保留，不冒充新Pin或新资格。

本批 Responses SSE [Pin](assets/cpar-build-remaining-20261011/responses-sse-pin.json)、[摘要](assets/cpar-build-remaining-20261011/responses-sse-basic-summary.json)、[正式audit](assets/cpar-build-remaining-20261011/responses-sse-audit.json)全部完成；每轮唯一 Request／Attempt及1 Usage／1 ledger，精确response关联、原生Usage及source-event一致。3/2/4场景均严格核对答案；原 `changed_part_snapshot` FAIL及旧epoch保留。本次正式整体audit仍为 **NOT_RUN**，因为声明扩展及受控故障场景未运行；仅basic_status为PASS。

Chat JSON [新Pin](assets/cpar-build-remaining-20261011/chat-json-pin.json)PASS，随后 [首请求摘要](assets/cpar-build-remaining-20261011/chat-json-basic-summary.json)与 [audit](assets/cpar-build-remaining-20261011/chat-json-audit.json)均FAIL。失败发生在 `2026-10-11T02:51:35.976790+00:00`，保全117字节HTTP400错误及请求参数，见 [capture摘要](assets/cpar-build-remaining-20261011/chat-json-failure-capture.json)。错误为gateway_error／ClientRequestError／“the client request is invalid”。响应没有x-request-id；[补充只读取证](assets/cpar-build-remaining-20261011/production-ports-after-and-failed-request.json)仅通过唯一发送时间窗口找到同model、非流式 `openai_chat_completions` Request与failed request_finished，4ms、Attempt0、Usage0。该归属不是精确wire Request-ID绑定；成功9轮的精确关联独立保留。

[静态代码定位](assets/cpar-build-remaining-20261011/chat-json-code-trace.json)核对三个实际文件与1b产品Git blob一致：Chat `decode_request`保留 `openai.chat.max_completion_tokens`，`project_root_extensions`只认 `openai.chat.max_tokens`，其余字段返回 `UnknownRequestExtensions`；runtime在选择凭据前拒绝候选，并将全候选不兼容映射为ClientRequestError。该链与实测400一致；没有凭此声称已完成产品规格裁决或Rust离线动态复现。最小复现对象为正式 `text_multi_turn` 首轮、冻结4096及JSON控制，完整body在原私有capture。后续产品范围为Chat输出上限别名映射及碰撞规则，本批不修改Rust、不改参数规避失败、不自动真实重跑。

## 计数、退出与分层状态

| 口径 | 历史起点 | 本批新增 | 累计 |
|---|---:|---:|---:|
| 预算Pin意图 | 5 | 2 | 7 |
| 基础HTTP意图 | 16 | 10 | 26 |
| 预算推理总数 | 21 | **12/50** | **33/71** |
| 网关／Pin确认的推理发送 | 21 | **11**（Pin2＋Attempt9） | **32** |
| 独立Provider接收／实际费用 | UNKNOWN | UNKNOWN | UNKNOWN |

退出数据库增量 Request10／request_finished10／Attempt9／Usage9／ledger9；继承的1组旧事件和账本独立扣除。所有新基础Attempt succeeded/completed，无retry或fallback，Pin实际发送完整计数为2。账本cost=null、unpriced保留，不能解释为免费或零成本。余量38不续用。扩展、上游catalog、额度查询、主动OAuth refresh、重新授权、第二Client Key、失败后重跑均0。[管理证据独立记账](assets/cpar-build-remaining-20261011/management-accounting.json)记录99次只读管理GET及2次新Pin POST；读取本地配置／有效模型不是上游目录查询。

[正式退出](assets/cpar-build-remaining-20261011/real-exit-receipt.json)资源与计数均PASS：只停止绑定PID2992034的自有临时unit，MainPID0，原PID不存在，19180/19181无listener，停止后5秒审计不增。139旧历史文件SHA256均不变，旧失败／旧epoch／原始证据未覆盖或删除。[后续读回](assets/cpar-build-remaining-20261011/post-exit-readback.json)再次确认PID0、budget.stopped、审计hash不变，并证实本epoch没有 Responses JSON、Chat SSE、Messages JSON/SSE 目录。

生产保持PID499070、NRestarts0、active/running，unit、binary、current、Caddy和active config均与开始基线一致；[端口前](assets/cpar-build-remaining-20261011/production-ports-before.json)／[端口后](assets/cpar-build-remaining-20261011/production-ports-after-and-failed-request.json)确认18180／18181仍属于同一生产PID及process start。未改变生产service、配置、current、Caddy或端口。

| 证据层 | 本批结论 |
|---|---|
| 本地验收工具／双轴 | PASS；50/50、354/354及包装控制核验；全部LOCAL_SIMULATED，10类local入口、7类BLOCKED/NOT_IMPLEMENTED |
| 真实Provider基础 | FAIL并停批；历史与本批合计2/6完整组合、6/18完整场景；扩展真实0 |
| 远端CI | NOT_RUN；没有push或触发CI |
| 候选制品 | 既有1b候选source／binary／manifest匹配；本批没有重新构建或正式发布 |
| 临时隔离运行／退出 | PASS，绑定运行和退出证据完成 |
| 正式部署／生产验收 | NOT_RUN；只读保持核对PASS不等于部署或生产通过 |
| 项目整体／#47/#57/#9 | PARTIAL／OPEN；未缩减能力声明、AC或依赖关系 |

GitHub已更新#47、#57、#9的既有进度评论，主代理及独立只读探子均当前读回 **complete PASS**，见 [GitHub回执](assets/cpar-build-remaining-20261011/github-sync.json)。提交body SHA256与两次读回一致、历史body完整前缀保留，每票仍1条评论；原body／AC／title／labels／assignees及OPEN状态不变。完整分页依赖身份集合不变：#47 blocked_by8／blocking1，#57 23／0，#9 0／0。评论分别为 [#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)。

报告及真实证据本地提交为 `aae3b393dccd4d96bf56b566013d0807c877bc9b`，未push；GitHub读回与最终门槛另归档为文档证据提交。[交付docs 7/7](assets/cpar-build-remaining-20261011/docs-delivery-check.json)已PASS，最终记录见 [收口docs门槛](assets/cpar-build-remaining-20261011/docs-closeout-check.json)。本批真实证据的独立 [最终一致性复核](assets/cpar-build-remaining-20261011/real-evidence-final-review.json)为complete PASS，仅覆盖回执一致性；未把真实FAIL或整体PARTIAL改为PASS。修改后的报告只补GitHub读回及文档门槛，前一份22文件冻结保留原hash。

本批交付闭环完成，真实基础批次仍FAIL，#47/#57/#9继续OPEN/PARTIAL；到此停止。无push、发布、生产部署或扩展真实批次。
