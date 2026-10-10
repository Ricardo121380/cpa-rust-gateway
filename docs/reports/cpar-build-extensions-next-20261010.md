# CPAR #47：已声明扩展的下一批可执行清单

本文件仅准备后续执行；本批新增扩展请求 **0**。实际 serving 观察来自 PID2318163／config9／lease135／同一 Client Key／projection，见 [新批次 Pin 目标与声明](assets/cpar-build-six-combinations-20261010/responses-json-pin.json) 及 [Pin 后冻结目标](assets/cpar-build-six-combinations-20261010/responses-json-basic.json)。reasoning、stored、continuation、compact、websocket=true；parallel=false。实例已停止，观察不能用于下一批发送，旧根／旧 Pin／旧 revision 不替代新的冻结。

当前所有17类的真实结果均 **NOT_RUN**，执行状态均 **BLOCKED**：本批已真实失败停止，后续扩展发送、故障操作、第二 Client Key、会话根删除及重启还没有新的具体授权。以下是在这些门槛满足后可逐步执行的操作卡，不是17次请求承诺或现有额度的续用许可。

## 共用执行门槛、证据及清理规则

1. 新批次先确认自有 unit／epoch 无他人占用；同一授权 Build 账号仍有效。只读确认正常续期材料，禁止自发 refresh／重新授权。重新冻结 source、binary、manifest、进程、config、provider、endpoint、route、candidate、model、实际 lease、egress、同一 Client Key／projection／serving声明。资格观察最多600秒，临近到期不启动请求，后台任务禁用，`max_attempts=1`、单candidate／单账号池、无 retry／fallback。
2. 每个真实正例保留完整 JSON 或 SSE 原字节、唯一完整终态、原生 Responses items、有序历史、真实 call_id／执行器结果；使用现有正式 collector 核对 Request／唯一 Attempt／Usage／ledger source-event、实际发送与释放。不把 helper、合成输入、旧 Full 或声明本身当 Build 真实正例。
3. 零发送负例必须有有效认证、精确 operation、实际 HTTP／事件及 `upstream_send_count=0`；401、错误路径、空资源或无效模型不替代指定边界。负例所需真实根单独计推理；GET／DELETE／Upgrade 与上游推理分开计操作次数。
4. 复用 `cpar_new_build_acceptance.py` 的 `EXTENSION_CHECKS`／`extension_check()`，把原始 sample 导入正式 audit。当前自动 `run` 只驱动基础3／2／4，**不会自动执行扩展**。真实扩展 executor／采样步骤、完整请求控制绑定、审计样本格式必须先完成本地公共入口验证和审查；未具备就保持 BLOCKED，不用替代 harness 放宽标准。
5. 首个真实发送后的 FAIL／未知结果／归属漂移／证据矛盾，立即停止所有剩余正例和可能发送的操作，不修复后真实重跑。离线定位、只读核对可继续。每个对象操作前核对 owner；停止自有 unit、断开socket、释放 lease，保留原始材料及失败账。删除仅限下一批已明确授权删除的自有根；禁止改生产、他人根、旧失败和旧证据。

共性本地证据及其范围：

| 标记 | 可复用的已实现边界 | 来源与限制 |
| --- | --- | --- |
| R | reasoning编解码、历史、参数拒绝 | `crates/gateway-http-actix/src/lib.rs` reasoning JSON／SSE tests；`crates/gateway-router/src/protocol_transform.rs` 精确thinking转换。是本地／受控证据 |
| S | 加密保存、owner、删除、expiry、重启读取 | [BC-RESP-001](../contracts/BC-RESP-001-client-owned-stored-response-foundation.md)；HTTP stored tests；`scripts/acceptance/cpar-batch-b-http.py` 隔离合成fixture，expiry写DB只属于合成 |
| C | exact continuation、E-05、compact lineage | [BC-RESP-003](../contracts/BC-RESP-003-exact-continuity-and-compaction.md)；HTTP exact／compact tests；[原生会话本地报告](cpar-native-session-batch-local-20261009.md)。不代表本次Build真实通过 |
| W | WebSocket握手、多轮、取消与失败非持久化 | HTTP WS tests及 batch-b 合成socket／进程fixture；不代表Build真实socket证据 |
| O | 17类formal判定及零发送归属 | `scripts/acceptance/cpar_new_build_acceptance.py`；[本批local-verification](assets/cpar-build-six-combinations-20261010/local-verification.json)的38项覆盖基础oracle等，**不宣称17类真实样本全覆盖** |

金额统一为 **UNKNOWN／NOT_PROVEN**，账本若无价格保持 unpriced／null。下列“追加推理”仅为条件下限，不含资格 Pin、未确定的覆盖复用或失败。现有“金额不限”授权针对本批基础范围，不能自动扩张为扩展费用授权。新的实际覆盖表、去重根清单、最大请求数及操作范围批准前，不填写一个合计预算。

## reasoning：2类

### 1. `visible_reasoning`

- 适用：Chat JSON／SSE，参数 `reasoning_effort:"high"`；Responses JSON／SSE，`reasoning:{"effort":"high"}`。发送带明确推理任务的真实请求，冻结既有上限控制，观察公开 reasoning 与完整成功终态／Usage。不能把本批偶然存在的 reasoning_tokens 或空 reasoning item 当显式开启通过。
- 正例：各获准协议模式公开非空 reasoning，模型／实际目标／Usage语义匹配。负例：先在本地公共入口证明非法effort或不兼容thinking显式拒绝且零发送；真实零发送操作若获准则保留认证与operation。
- 共性 R／O。资格缺口：formal plan 的 controls白名单目前不接受 Chat `reasoning_effort`；必须先最小补齐完整参数冻结／本地验证／审查，不能省略参数绑定。Messages 原生 `thinking:{"type":"enabled","budget_tokens":2048}` 与 Build effort没有无损转换，不能直接套用；Messages JSON／SSE 正例仍 BLOCKED，声明不关闭。
- 条件计数：每个实际可执行协议模式至少1次推理。相同账号、不含旧Pin复用；与下一类同协议模式可以共用第一轮。通过后仅保留证据并释放资源；无须新增远端工具副作用。

### 2. `reasoning_history`

- 适用与参数同上一类。以已合格首轮为父，把完整 reasoning／assistant或Responses output原样加入有序历史，再发一轮要求使用首轮事实；保持精确控制，不将reasoning拼成普通提示词或丢弃原生items。
- 正例：后轮仍有非空reasoning、连续性和严格任务答案，同时 `check_history`、终态、归属及Usage PASS。受控负例：缺失／重排reasoning或变更控制的样本由本地formal拒绝；不能发不明恢复请求补证。
- 共性 R／O；账号、参数qualification和Messages缺口同上。条件计数：有合格首轮追加至少1；无首轮与visible一起至少2／协议模式。Chat／Responses四个模式都获资格且每对共用时，reasoning两类的条件下限为8；这是条件推算，非已授权总预算。清理同共用规则。

## stored：6类

### 3. `store_read`

- 适用：Responses JSON／SSE。POST `/v1/responses`，同模型、`store:true`、明确 `stream:false/true`；获得完整成功根后，以同一Client Key GET `/v1/responses/{id}`。
- 正例：GET200，持久读取的完整对象与原成功响应相等；保存用实际source-event及唯一Attempt验证。零发送负例可与下面owner／delete边界配套，不能用不存在的任意ID冒充真实根。
- 共性 S／O。需要单账号有效材料、自有加密DB／keyring与根索引。当前基础样本没有显式 `store:true` 资格，不能默认复用。条件计数：建根每模式1次；已有同一冻结身份且显式store的合格根时，读取追加0推理／1 GET。结束保留原wire／读取回执；删除要另获根删除授权。

### 4. `foreign_owner`

- 适用：Responses stored read，JSON资源；根可以来自JSON／SSE创建。owner先GET200，再用**第二个有效、不同归属**的Client Key GET同一个精确ID。
- 正例／负例：owner完整读回为正控制；异主404，`operation=stored_read`、authenticated=true、上游0，两个opaque Client Key不同，根存在事实可证。401不通过。
- 共性 S／O。需下一批批准创建／使用第二个自有Client Key及配置projection，不复用未知凭据。已有根追加0推理、2 GET；无根另计创建1。核对后停止两个key的测试使用，是否撤销／删除需覆盖具体对象；原证据保留。

### 5. `deleted_read`

- 适用：Responses JSON资源，根可以来自JSON／SSE。owner先证明根存在，再DELETE精确自有 `/v1/responses/{id}`（200／204），再GET同ID。
- 正例／负例：删除操作有真实回执，之后404、`operation=stored_read`、authenticated=true、上游0；deleted_response_id与read_response_id相同。
- 共性 S／O。根删除是状态操作，需要下一批明确自有根范围。已有根追加0推理，至少DELETE1＋GET1，存在性读控另计GET；无根创建另1。只能删除清单内根，先归档，禁止删除本批旧证据或其他任务状态。

### 6. `expired_read`

- 适用：Responses JSON资源；按真实创建时间及固定TTL到期后GET同ID。TTL为 **30天（2,592,000,000ms）**，公共调用不能缩短，观察时间必须 `now_ms>=expires_at_ms`。
- 正例／负例：到期前owner读回成功、到期后404／上游0，时间及根身份真实可审计。合成expiry测试可复用为本地边界，不能修改真实DB时间或回写旧timestamp当真实过期。
- 共性 S／O。缺少已授权自有且达到真实TTL的合格根，当前额外 BLOCKED；若已有根到期则追加0推理、前后至少2 GET，否则新建1并等待30天、另安排明确后续执行。费用／期间账户变化未知。只读测试后保留证据，GC／删除不自动授权。

### 7. `restart_read`

- 适用：Responses JSON资源，根创建覆盖JSON／SSE。显式store成功、重启前GET200；停止并启动**下一批自有隔离unit**，同source／artifact／DB／keyring／Client Key，重新采集进程，再GET同ID。
- 正例：两个新鲜 runtime 均合格，process_start不同、source相同，响应完整相等；读取上游0。受控负例：错误keyring／损坏状态只在合成fixture，不影响真实根。
- 共性 S／O。必须授权隔离restart并留住加密keyring；变更进程后更新600秒观察。已有根追加0推理、2 GET；无根创建1，重启次数另计1。结束停自有unit、证明端口和资源释放；保留root及失败日志。

### 8. `persistence_failure`

- 适用：Responses JSON／SSE，`store:true`。只在单独隔离故障环境中，在一次真实Attempt成功路径的持久提交边界触发已验证的受控失败。
- 正例对照可复用合格store成功根；受控负例必须唯一失败终态、无completed、failed root GET404、恰1 Attempt、终态后0发送、资源释放，不能由非法请求代替持久化失败。
- 共性 S／O。本项目当前未证明可安全执行且可审计的真实故障设施，额外 **BLOCKED**；禁止破坏本批／生产DB或换权限制造不受控结果。额外真实请求数／费用 **UNKNOWN**，不能凭formal恰1Attempt要求直接填预算1；先提交故障点、正控制、触发时序和清理演练，再批准精确上限。退出恢复故障环境并停unit，保留全部失败材料。

## continuation：2类

### 9. `exact_continuation`

- 适用：Responses JSON／SSE。同一Client Key、model、实际冻结目标，用 `previous_response_id` 引用合格自有根，POST新input；所有root关联及新请求原字节保留。
- 正例：parent_target与target精确相同、任务连续性及成功终态／归属／Usage PASS，普通重试／fallback0。受控负例与下一类配对。本批根没有store／continuation资格，不能拿来直接续接。
- 共性 C／O。E-05默认revision相等；仅Provider固定issuer worker证明同非空subject／client／scope且durableCAS连续区间时允许已有批准的窄例外。材料副本更新、手工替换或普通OAuth不能证明。条件计数：有合格根追加1／模式；无根共2。结束记录新根及释放，不自动删除旧根或刷新账号。

### 10. `lineage_rejection`

- 适用：Responses JSON／SSE，真实自有根的 `previous_response_id`。在下一批批准的独立隔离配置／账号副本中改变一个精确lineage维度，复核不属于E-05例外，再尝试同根续接。
- 正控制：改变前同一目标续接可复用上一类证据。拒绝：400／404／409，`operation=responses_continue`、有效认证、上游0，精确差异可证；错误账号／无效根不能替代受控漂移。
- 共性 C／O。需要明确授权该隔离配置变更与恢复，不改生产，不发refresh，当前额外BLOCKED。合格根和正控制已有时拒绝追加0推理／1 POST；否则创建及正控制另计1／2次。恢复自有配置、重新核对hash并停unit，保留差异与原根。

## compact：3类

### 11. `compact_roundtrip`

- 适用：Responses；根覆盖JSON／SSE，compact本身JSON。POST `/v1/responses/compact`，`model:grok-4.5`、`previous_response_id:<owned qualified root>`、`stream:false`。压缩会发一次真实summary推理，产品固定 `max_output_tokens=2048`，必须原样记录；再POST Responses JSON／SSE，把返回的完整compaction item放入input，**不同时带 previous_response_id**。
- 正例：compact200、item.type=compaction、created_by=cpar、encrypted_content前缀 `cpar_compact_v1.`；消费后连续性与成功终态／Usage／归属PASS，summary与消费各自唯一Attempt及账本。零发送反控使用后两类。
- 共性 C／O；需要合格自有stored根、compact声明及独立summary/消费采样，不打印opaque内容。已有根追加2／消费模式；无根共3。两个消费模式是否共用同一item需先本地证明，未证明不扣预算。结束保留摘要／消费证据，清理只限批准的根范围。

### 12. `compact_foreign_owner`

- 适用：Responses compact JSON；owner GET同一个真实根200，以第二有效Client Key POST compact该根，`stream:false`、同model。
- 正控制是owner存根／读取；异主404，`operation=responses_compact`、有效认证、上游0，两key不同，compact_request.previous_response_id精确等于已证存在的read_response_id。
- 共性 C／S／O。需要第二key授权及根；已有根拒绝追加0推理、POST1＋owner GET1；无根创建另1。不能意外发summary；任何上游发送立即FAIL-stop。清理key及根按明确对象授权，证据保留。

### 13. `compact_lineage_rejection`

- 适用：Responses compact JSON，`model`、`previous_response_id`、`stream:false` 同上。隔离环境内受控改变一个精确lineage维度，确认不属于批准刷新区间。
- 正控制可复用成功compact；拒绝400／404／409、`operation=responses_compact`、认证且上游0、无summaryAttempt／无新账本。不能用任意不存在ID制造404。
- 共性 C／O。依赖合法根、隔离配置变更及恢复授权，未获则额外BLOCKED。正控制已有时拒绝追加0推理／1 POST；需新建根或compact正控制时分别另计1，不隐去summary。恢复自有配置并停unit，不修改旧根。

## websocket：4类

### 14. `ws_multiturn`

- 适用：Responses WebSocket；formal target使用Responses SSE的冻结身份，Upgrade及frame另附证据，不构造未支持的target.mode。GET Upgrade `/v1/responses`，Bearer同一Client Key，无Origin；101及真实nonce／accept匹配。frame为 `{"type":"response.create","model":"grok-4.5","input":...}`，隐式stream=true；第二frame同socket使用合格首轮 `previous_response_id`。
- 正例：至少两轮，各有完整Responses事件终态、连续历史、精确target、唯一Attempt／Usage／ledger；无重复成功终态。受控反控握手拒绝见下一类，不以普通HTTP SSE代替socket。
- 共性 W／C／O。需稳定socket采样、当轮root、冻结控件及有序transcript的本地演练。条件计数至少2推理／会话，Upgrade另计1，不能用基础文本回执抵扣。关闭socket后确认leases释放、无后继send，停unit并保留transcript。

### 15. `ws_handshake_rejection`

- 适用：Responses WebSocket Upgrade；有效Bearer Client Key，正确路径／Upgrade／nonce，额外设置明确Origin。
- 正控制可复用上一类101；负例必须403、`operation=responses_ws_upgrade`、authenticated=true、上游0，不能用401／404或缺Upgrade头替代Origin边界。
- 共性 W／O。同账号／key及隔离unit，纯拒绝追加0推理／1 Upgrade；若另建正控制，其至少两轮费用另计。关闭所有socket或拒绝连接，确认上游0／资源释放并归档原握手头的脱敏证据。

### 16. `ws_cancel`

- 适用：Responses WebSocket，先成功Upgrade，发送真实response.create。必须实际观察首个语义输出后close；不可在未发送时取消冒充after-output。
- 正控制可复用完整socket多轮；受控取消负例要无成功终态／无重复terminal、恰1Attempt、终态后0发送、lease释放、failed root GET404，保留精确取消时序。
- 共性 W／O。当前只有合成socket取消证据，真实稳定触发、观测方法及采样预算未证明，额外BLOCKED。额外真实请求数／费用UNKNOWN；“一次取消Attempt”不能确定是否还需新的正控制或预演发送。先零真实本地演练、给出有上限且不靠失败重跑的方案，再获授权。清理关闭socket／停unit，保留未完成wire与账。

### 17. `ws_failed_root`

- 适用：Responses WebSocket；对已验证的独立隔离故障点发一次response.create，使根以真实失败结束。
- 正控制使用成功socket根；受控失败须唯一失败终态、无completed、恰1Attempt、后续0发送、失败根读404且不能成为可引用根、资源释放。后续“不可引用”验证需明确零发送操作与证据，不能再发真实恢复请求。
- 共性 W／C／O。安全且可归因的Build真实故障设施未证明，额外BLOCKED，不用实际账号／额度／Provider故障探测凑样本。额外真实请求数／费用UNKNOWN；先证明故障位置、rootID取得和拒绝操作的零发送性，再提出精确新预算。恢复故障环境／关socket／停unit，保存失败根证据，不把失败根写成成功根。

## parallel=false 的额外拒绝资格

parallel不是本次17类true能力之一。声明仍false，formal不能把 `explicitly_not_declared` 当真实拒绝已验证。

- Chat／Responses 显式 `parallel_tool_calls:true`、实际工具定义；Messages `tool_choice:{"type":"auto","disable_parallel_tool_use":false}` 表达并行要求。各JSON／SSE实际适用入口先在本地公共边界验证，再在明确批准的零发送范围调用：须明确不支持、HTTP400、上游0，账号资格／归属不漂移。
- `parallel_tool_calls:false` 是串行控制，不是并行拒绝负例；没有两调用交错正例支持材料，不能把声明改true。来源为 `crates/gateway-router/src/tool_controls.rs` 和 `protocol_transform.rs` 的明确拒绝。
- 这些操作目前NOT_RUN／BLOCKED，不含在17类计数，也不自动增加本批真实请求。认证拒绝本身的目标预算为0推理，HTTP请求数、模式覆盖及新资格Pin仍需单独批准。

## 下一批批准前必须补齐的对象与预算

先交付并审查扩展executor／样本schema与参数绑定、Messages reasoning的精确支持方案、第二Client Key边界、可复用根清单、restart对象、30天expiry对象、安全故障／取消采样。未具备的条目继续BLOCKED，不能以删参数、缩TTL、关闭声明或合成结果替代。

随后按选定协议模式列出每一个Pin、根创建、continuation、summary、消费、socket帧及可能发送的正控制，并给出共享依赖的去重证明。列出另计且要求0上游的GET／DELETE／拒绝POST／Upgrade，以及隔离配置修改／restart／删除根范围。金额未知或不限都不取消请求上限。新总预算与累计上限须结合当时历史推理累计 **9** 重新批准；本次未给出假总额或自动安排下一批。
