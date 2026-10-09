# 剩余规范与验收证据收口：本地批次报告

日期：2026-10-09；项目：cpa-rust-gateway；任务等级：L。

本批完成 S-01、E-01～E-04、O-01 的本地规范、实现及证据工作，并完成实际管理监听器嵌入页面的受控验收。E-05 仍为 **AWAITING_USER_DECISION**；真实设备授权交换丢包的整条嵌入页面路径为 **NOT_RUN**。本批不标为全范围 DONE，不改变原始 13 项 P2 的 `RESOLVED_LOCAL` 统计，也不关闭 #57 或发布门槛。

固定 base 为 `27ee36174f54bac059694a0870e452f9ac632ce0`，实际初始分支为 `codex/prism-v4-delivery`，初始 HEAD 与 base 一致。上一批接受运行源码为 `a98bfd5e01ea51d3894cce4afcbc0324e9142702`。本批最终运行及验收脚本源码为 **`47b19545f6dd102a4cb15a8af2990b76e4a359db`**；后续提交只收录报告、证据及评审记录。最终源码、执行命令、clean 状态及二进制的对应关系见 [源码绑定](assets/cpar-seo-closeout-20261009/source-binding.json)。

直接依据包括 [上一批报告](cpar-p2-eight-local-20261008.md)、[上一批 #57 草稿](cpar-p2-eight-issue57-supplement-20261008.md)、[历史双轴评审](assets/cpar-release-readiness-20261005/review-results.md)、[O-01 独立观察](assets/cpar-b01-20261007/additional-observation.md)、[前端完成定义](../08-management-frontend-development-plan.md)、ADR-0100/0103、BC-RESP-003，以及本批只读取得的 GitHub 工单快照。原任务、任务预算、初始缺口及逐项证据均由 [证据索引](assets/cpar-seo-closeout-20261009/evidence-index.json) 汇总。

## 逐项裁定

| 项目 | 当前事实、复用证据与真正缺口 | 本轮动作与验收 | 最终本地状态 |
|---|---|---|---|
| S-01 | ADR-0100 已有严格协议、Usage、所有权语义；缺 ADR 模板结构及明确回滚边界 | 原八段正文语义与引用保持；补 Context/Decision/Consequences/Alternatives/Validation and rollback，登记任务和契约；静态比较、文档门槛 | **PASS：规范结构缺口关闭** |
| E-01 | 旧 A-02～A-04 保存/生效证据未区分实际预设；产品有四渠道、五个选项 | 五个选项逐一验证；fixture 15 场景覆盖无匹配/唯一匹配/多匹配，单测覆盖非法材料和禁用；真实嵌入页面五次复用、保存、绑定、发布并读回准确配置 | **PASS_LOCAL：预设证据补齐；Grok 推理 NOT_RUN** |
| E-02 | 原 unknown-before-save seam 不等于提交后丢包；复用原 session/task 恢复逻辑 | Kiro/Kimi 各一真实 HTTP/SQLite/TCP EOF 场景，提交在断开前可见；原 session 回执读取两次、同账号/修订、exchange=1、重放 409；fixture 关闭并返回原任务后完成连接/发布 | **PASS_HANDLER_AND_FIXTURE；实际 serve 设备丢包 UI NOT_RUN** |
| E-03 | fixture bearer/null 不能证明生产 Kiro 分类错误 | 保留 OAuth marker，纠正 API Key 的共享投影；两次导入、两清单/目录分区和重新授权入口定向 RED/GREEN；独立 fixture 恢复和实际 Kiro OAuth/API Key 页面证据 | **PASS：fixture 缺陷关闭；未确认生产分类缺陷** |
| E-04 | 原 410 项公开 HTTP 不能自动覆盖缺失连接/模型/共享 Upstream Chat 单元 | 保留并重跑原 410；新增 14 行、117 检查，公开请求/terminal/Attempt/执行身份/配置/候选/账号修订/出口及全接收计数一致 | **PASS_PUBLIC_HTTP：527 检查** |
| O-01 | 历史静态观察需当前公开 JSON/SSE 复现 | JSON commentary phase-only 误拒绝与 SSE 非法私有 envelope 准入得到 RED；最小修复；合法 phase、paired metadata、16 个负例、存储/下一轮历史及 B-01～B-03 回归 | **PASS_LOCAL：限定的普通 Responses 准入修复** |
| E-05 | #36 的全部 revision 变化拒绝与 BC-RESP-003 的严格同授权 Build 刷新例外冲突 | 仅发送一次选择卡；保留既有行为和冲突，未改规范、实现或测试预期 | **AWAITING_USER_DECISION：批准规则和对应实现/验证均 NOT_RUN** |

S-01 只改变规范表达。[ADR-0100](../adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md) 的回滚说明与 [ADR-0103](../adr/ADR-0103-failed-bootstrap-usage-attribution.md) 一致：旧 binary 不识别 B-04 新 Usage 身份，切回旧程序不能保证新数据安全可用。需要兼容代码，或另外获准、验证相容性的状态恢复及历史保全；多密钥版本、owner/AAD/TTL 条件同样不能由回退 binary 消除。本批没有执行生产恢复。

## 四渠道与实际五选项

名称及映射取自现有 `CONNECTION_PRESETS` 和后端契约，未人为合并 Responses 与 Chat。

| UI 选项 | 账号导入 channel / Upstream kind | Adapter / API format | 推理路径 |
|---|---|---|---|
| Responses | `openai-compatible` / `openai-compatible` | `openai-compatible.responses` / `openai/responses` | `/responses` |
| Chat Completions | `openai-compatible` / `openai-compatible` | `openai-compatible.chat-completions` / `openai/chat-completions` | `/chat/completions` |
| Messages | `anthropic-compatible` / `anthropic-compatible` | `anthropic-compatible.messages` / `anthropic/messages` | `/v1/messages` |
| Kimi API | `kimi-api` / `kimi` | `openai-compatible.chat-completions` / `openai/chat-completions` | `/chat/completions` |
| Grok API | `grok.official` / `grok.official` | `grok.official.responses` / `openai/responses` | `/v1/responses` |

fixture 的五选项 × 三种匹配数共 15 场景，记录真正提交的 channel、owner、adapter、format、path、account、binding 和 publish。多匹配时先拒绝写入，再显式选择第二个连接；既有对象及禁用值保持。四渠道非法 JSON 材料不创建账号。类型/单测并非实际页面替代品。

实际页面五次复用预置合法 HTTPS 连接：每次一账号保存、正确 binding、一次 publish、零 Upstream/Endpoint 创建；读回目标的协议、路径、owner 和 enabled 值与预置一致。Responses、Chat、Messages、Kimi 的显式 Channel Pin 各成功一次，打开模型清单不产生上游请求，点击测试后全接收计数恰好增加一，并匹配实际账号、连接及 wire model。Grok 使用固定官方地址的禁用 owner/Endpoint，保存和发布保持禁用；没有执行 Grok 推理。

### 本轮额外确认的生产缺陷 F-01

真实管理 Endpoint 的 `transport` 为 `https`。前端 `matchingApiConnections` 误用运行观测的 `http/sse`，因此实际 `/admin-ui/` 对合法已有连接显示“将新建”；fixture 又把 HTTPS 写入/读回转换成 HTTP，掩盖了该缺陷。

独立实际页面 RED、模型单测 RED（1 passed/3 failed）、fixture create/read RED（1 failed/12 skipped）均保留。最小修复仅将匹配条件改为 `https`，并校正配置 fixture 的 transport；运营 pool 的 HTTP 投影保持。28 项定向 GREEN、最终真实五选项复用和 Full 支持 **F-01 RESOLVED_LOCAL**。这是本批新发现，独立于原 13 项 P2；跨界路径、原因和 FYI 已在同提交 [跨边界日志](../cross-boundary-log.md) 及 trailer 登记。

## E-02/E-03 的证据层与实际页面

设备丢包后端测试启动真实 Actix HTTP server，使用生产鉴权、handler 和临时 SQLite；只替换设备授权上游 transport 为本地合成结果。loopback proxy 读到真实 201 和 ETag，并从独立数据库连接确认 `rows=1, credential_revision=0` 后才断开原客户端，原客户端得到 EOF。两个原 session 各只读回两次，账号、配置、owner、committed revision 不变，token exchange 始终为 1；poll 重放 409、无鉴权回读 404，不新增账号。原 `5701e0e5` 定向回执记录本轮真实 session/端口和交换次数；最终源码的后端及验收脚本逐字相同，Full 也重新覆盖这两个测试。

EgoLite 的开发 fixture 另外证明关闭对话框、切页、返回原任务、只读核对回执和继续连接/应用的完整顺序；保存前 unknown 测试仍保留。两层不能拼成“实际 stock serve 设备丢包整条 UI 已通过”。stock serve 的设备 transport 固定指向真实服务，当前没有安全的本地注入入口，因此该层明确 **NOT_RUN**；回执还是进程内、有限容量与 TTL 的事实，不宣称跨进程重启恢复。

Kiro fixture 完成的 bearer envelope 带 OAuth marker，纠正后两个清单共享与生产一致的材料分类，OAuth 有重新授权入口，API Key 没有。首轮 Spec 独立发现非 OAuth Kiro 仍返回 null；导入回归 RED 为 14 passed/1 failed，修复后的 15 项生成客户端测试检查 OAuth/API Key 两清单、目录分区、状态和操作入口，全部通过。实际页面分别导入合成 social OAuth JSON 与合成 `ksk_` API 材料，经真实 handler 保存并发布；生产清单分别为 `oauth` 和 `api_key`，OAuth 重授权入口打开后取消，未开始交换。账号目录使用 `entry_type=account` 展示 OAuth；AI 提供商目录使用 `entry_type=api` 展示 API Key。两页的真实 UI、响应和操作入口另有截图及读回补证。最初截图期待同页两行是错误验收预期，单独登记，未据此认定生产分类错误。

实际页面验收时网关 PID 为 `56948`，管理源为 `http://127.0.0.1:54641/admin-ui/`，二进制 SHA256 为 `794bf6c34b5d193ae320d879ba92ef5f15b4a0b6da3b4da4eb68fee18807cf2f`。启动和源码 clean 状态由 descriptor 记录；进程专用 macOS outbound sandbox 只允许 localhost，预检证明保留地址的出站 connect 被拒绝。API 测试另用仅允许本任务 loopback TLS 地址/端口的 egress policy。临时密钥、密码及原始材料不进入证据归档。

| 实际嵌入页面场景 | 浏览器、handler/storage 与观察 | 判定 |
|---|---|---|
| 首次管理员改密及重新登录 | 真正登录/强制改密页面；真实管理会话 | PASS |
| E-01 五选项复用、保存、绑定、应用 | 精确请求与配置读回；真实 handler/SQLite；Grok 保持禁用 | PASS（配置范围） |
| 鉴权、Origin、CSRF | 同源写操作携带 session/CSRF；无鉴权、无 CSRF 写、异源 GET/写均 404 | PASS |
| A-02 保存成功响应在浏览器丢弃、回执后一次读取失败 | 真实 201 提交；browser seam 丢弃响应和注入后续 503；原回执/账号保留，无重导入 | PASS（浏览器故障层） |
| A-02/A-04 明确核对后继续连接/应用 | 原保存事实、checkbox 和键盘确认；import 总数仍为一 | PASS |
| E-03/A-06 Kiro OAuth/API Key | 真正材料导入、发布、production inventory 和分别对应的页面/操作 | PASS（本地材料） |
| A-05/A-06 原账号重授权入口 | 打开并取消；`kiro-authorization/start` 请求为零 | PASS（入口）；交换 NOT_RUN |
| A-04 已有非空草稿 | 保留 `ui-prior-group`，无自动 publish；完整差异核对后一次 publish | PASS |
| A-07 四次显式单调用 | 模型读取增量零，执行后增量一；账号/连接/模型与回执一致 | PASS（本地合成上游） |
| A-08 自定义映射拒绝 | 不写 candidate/grant、不 publish；active candidate 和 grant 前后读回相同 | PASS |
| E-02 真实 serve 设备交换提交后丢包 | 当前缺安全的受控 transport/网络故障注入 | NOT_RUN |

主整轮实际页面 **18 场景**；fixture **17 场景**作为历史补证独立列计，保留 b0710eb4 的原执行源码。纠正后的 fixture 分类在最终源码的 15 项定向及 496 项整组单测中复核，没有把旧 fixture 浏览器运行改写成最终源码执行。最终 clean checkout 重建的生产嵌入页面四个静态文件，以及实际网关二进制，均与已验收对象逐字节相同；后端与验收脚本无增量。[等值复核](assets/cpar-seo-closeout-20261009/reviewed-runtime-equivalence.json) 支持实际页面及公开 HTTP 的复用，原源码、时间和 PID 保持。确认清单的部分自动指针点击被视口/固定栏遮挡，改用真正的 focus/Enter 后通过；指针滚动与布局关系仍是未确认观察，不标作生产 CSS 缺陷或全站视觉通过。非空草稿的差异清单也包含因配置 AAD 变化而重新加密的 ciphertext 差异，不能硬断言只有一个条目；验收明确检查先前 group 在完整清单中及无自动发布。

## E-04 与 O-01

公开隔离矩阵的新配置在本构建 binding concurrency 预算内，使用三 owner、六合法连接和临时客户端身份；没有将同一 Upstream 下两条相同 API format 的非法连接当正例。14 行分别证明：同账号相同 wire model 的两种合法连接隔离；404 模型失败后同 binding 的其他模型可用且原模型可再调用；429 是 binding-wide 并有意阻断同 binding 两模型；共享 owner 的 Responses/Message 失败后 Chat JSON/SSE 可用；无 grant/未知 key 在全上游接收层增量为零。每个 fault/probe 之间没有重启、重新发布或恢复操作。

每行记录 public request/terminal、原始公开响应摘要、实际 Attempt、配置与 credential revision、candidate、Endpoint、owner、wire model、direct egress，以及 Attempt payload 与执行证据摘要的对应关系。早期失败没有 correlation header 时，以唯一串行请求的 append-only event ordinal 关联；不把“缺 header”当作没有请求。404 没有 retained runtime block，因此本批不宣称触发了公开 model circuit。最终 527 = 原 410 + 新 117；早期 targeted 339 = 共同 setup 222 + 新 117，不能误加成 749。

O-01 在普通 `openai-compatible.responses` 公开路径分别测试 JSON/SSE。当前 JSON commentary phase-only 被额外配对校验误拒绝；SSE phase-only 正例原本可通过，但私有 envelope 在校验前被剥离，使缺失 paired metadata 的负例错误通过。修复在 `wire_item_metadata` 剥离私有字段前校验 message/function_call envelope，只有私有 metadata 存在时才要求严格证明，保留既有闭集、成对 ID/所有权/顺序及永久拒绝语义。

合法 `commentary` / `final_answer` 在公开 output、存储 GET 和下一轮真实受控上游历史中保留；合法 paired-turn metadata 正例保留，缺失/冲突/非法字段的 8 类 × JSON/SSE 共 16 负例不保存成功、不产生可续接结果。最初 JSON 正例测试在 commentary 处先失败，因此没有声称 `final_answer` 另有独立执行 RED。定向 GREEN 3 tests、B-01～B-03 所在 batch C 回归 35 tests，最终 Full 再覆盖这些案例。

2026-10-09 已核对 [OpenAI 官方 phase 说明](https://developers.openai.com/cookbook/examples/gpt-5/codex_prompting_guide)。官方说明还列出 null；CPAR 保持已有已审查的两个字符串子集，没有借本批扩展 null 或其他字段。[来源回执](assets/cpar-seo-closeout-20261009/official-phase-sources.json) 明确区分已取得的官方页面和因大小限制未取得的 API reference。公开 phase 不等于上游私有 paired-turn metadata。

## 验证、失败记录与评审

| 门槛 | 实际范围与结果 |
|---|---|
| S-01 static/docs | 8 段原正文语义保持，必要段落/引用和安全回滚检查；docs gate PASS |
| 定向 RED/GREEN | O-01 1 pass/2 fail → 3 pass；E-03 分类 RED → GREEN；E-01 fixture 非法材料 RED → GREEN；F-01 两类 RED → 28 pass |
| E-02 真实 handler/TCP | 2 passed、0 failed、0 ignored；原 `5701e0e5` 定向回执及最终 Full 重新覆盖，原 session、commit、EOF、exchange=1 |
| 前端类型/单测 | 干净 checkout，type-check PASS；68 files / 496 tests PASS |
| 前端 `check:full` | 批次 Full 包含契约/generated freshness、source policy 和双构建字节一致性 |
| EgoLite | 实际页面 18 PASS（最终重建二进制字节相同）；development fixture 历史 17 PASS（独立来源，不改执行标签） |
| 公开 HTTP | 527 PASS 原运行完整保留，最终后端/验收脚本与重建二进制等值；新 14 行/117 检查，原 410 不重新命名为新证据 |
| Full | 最终 `47b19545` clean checkout：45/45 PASS，Rust 1493 passed / 0 failed / 12 既有 ignored，269.612 s；[最终 Full](assets/cpar-seo-closeout-20261009/accepted-full-closeout-receipt.json)，逐步骤日志独立归档 |
| Standards / Spec | 首轮两个 complete FAIL 已保留并修正；纠正后固定 base 与完整提交变更集合分别复审，结论及范围见 [双轴评审](assets/cpar-seo-closeout-20261009/review-results.md) |
| 收口文档增量 | 内容/链接/secret/whitespace/docs gate 与证据哈希复核；不将之前 Full 重标为最后的文档提交执行 |

首轮 clean Full 在新增测试 doc-markdown 的 SQLite 反引号上失败；修正后 `b0710eb4` Full 45/45、1493 passed/0 failed/12 ignored。随后真实 UI 的异源 GET 断言失败；源头是验收预期/前端文档与真实鉴权不符，改为携带异源 Origin 的读写都拒绝并复核 `check_origin/check_browser_boundary`，最终页面通过。首轮独立评审前收口的 Full 绑定 `5701e0e5`，该版本的源码、命令和日志保留。首轮双轴独立评审均 FAIL：Standards 指出九个本地标题缺 Task ID 前缀，Spec 指出 E-03 API Key fixture 投影缺陷。标题以保留旧历史引用、逐提交 tree/body/trailer/作者字段一致的方式纠正；报告的原正文段落计数校正为八。E-03 定向修复后，新 clean Full 在 `9517ac06` 完成 1493 项 Rust 测试，却被测试合成材料的长凭据形状字面值触发 secret scan，449.293 s 的 FAIL 单独保留；中间 staged scan 也保留其 FAIL。测试材料改为短非空合成 OAuth 值与合成片段组装的 API Key，扫描规则不变，定向 15 PASS、staged scan PASS。最终 `47b19545` Full 45/45 PASS，269.612 s，1493/0/12，前端 496 PASS；没有改写任何早期结果。

另外保留 fixture 设备文字/页面等待错误、真实 UI stale embedded build、无效 observer/CDP registration、关闭/确认按钮定位、错误 `#/config-versions` 路由、非空草稿差异数量预期、HTTP matrix 的非法相同 format、总 concurrency 超额、correlation header 假设、E-02 编译及 ETag 引号预期等独立失败。两轮宽范围 transport 探索被中断，只作 PARTIAL 线索；后续窄范围来源定位由主代理点验，不作为双轴最终 PASS。

完整命令和 UTC 时间在每份 receipt，Rust 计数、Vitest 计数、HTTP checks 和浏览器 scenes 分开。早期 receipt 的 Rust `test_totals=0` 不代表前端没有执行；其 Vitest 实数在原始日志中，索引另外解析，不改写原 receipt。索引明确标记原始日志字节、gzip 字节及 JSON 字节各自的 SHA256；报告不保存临时口令、token、Cookie、Authorization Header 或 credential material。

## E-05、权限与下一批

已只读核对当前 [GitHub #36](https://github.com/Ricardo121380/cpa-rust-gateway/issues/36) 和 [BC-RESP-003](../contracts/BC-RESP-003-exact-continuity-and-compaction.md)：前者要求 credential revision 变化拒绝续接，后者允许严格同授权证明的 Grok Build 正常刷新跨 revision。一次选择卡推荐保留当前严格限定例外，依据为已有同授权证明与正常刷新连续性；用户未答复。**user_choice=null, approved_rule=null**。既有行为保持不代表获得批准；S-01、O-01 和测试没有先行消除冲突，远端同步内容仍待裁定。

本批只进行了获准的本地修改、合成材料、loopback 服务、EgoLite、只读 GitHub 和本地提交。基线含 8199 个文件对象，其中 3804 个已跟踪、4395 个未跟踪；用户 AGENTS.md、原 4395 个未跟踪文件与无关已跟踪文件按 baseline/hash 复核不变，未混入提交。只有本批获准的原文件改变。首轮保全断言错误地将混合基线视为全未跟踪并把获准修改列为失败，该失败与校正后的逐对象结果分别保留。实际验证使用 clean 隔离 checkout；只关闭本任务进程和 TaskSpace，清理本任务合成临时材料，保留脱敏证据。没有读取或更改生产凭据/密钥/数据/服务，没有真实 Provider/真实账号推理、收费调用、push、远端写工单、部署或发布。

下一批的具体缺口为：

1. E-05 得到明确裁定后，将获批规则、兼容性与影响范围记入本地，实施对应必要修正并验收 exact revision / strict same-grant / 手动重新授权 / 其他 lineage；GitHub #36 同步仍需另有外部写入授权。
2. 为真实 stock serve 设备授权建立经审查、安全的本地合成 transport 与提交后响应中断方式，补 Kiro/Kimi 的真实嵌入页面关闭返回验收；保留当前 handler/fixture 证据作为基线。
3. 建立 #57 剩余页面、动作及证据矩阵，再验证尚未覆盖的真实 UI 主流程和指针滚动观察。本轮五预设实际验证的是已有连接复用，fixture 的创建/多匹配不得扩大为这些分支的真实页面证明。
4. 真实渠道、远端 CI、新候选制品/签名、受控部署、生产验收和发布各保留独立门槛，只有具体授权和新证据到位后推进。

当前不建议冻结新的发布候选。[本地 #57 补充草稿](cpar-seo-closeout-issue57-supplement-20261009.md) 可供审查，未发送到 GitHub。
