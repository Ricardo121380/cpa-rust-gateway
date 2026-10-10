# CPAR #47：扩展工具本地实现与下一批操作清单

本报告与 [真实基础批次](cpar-build-batch-20261011.md) 分开：**扩展真实 Provider 请求0**。正式 `--new-build run-extension` 已实现10类本地执行／采样；另外7类明确在 I/O 前返回 `BLOCKED/extension_executor_not_implemented`。未实现入口没有被局部演练替代，#47／#57／#9继续未完成。

## 来源、接口与本地证据

产品源码仍为 `1b6023f52f4c00ea6d8296524d306c141e4edae7`，产品路径没有变化。基础实测脚本 `b33e551e…` 的完整字节保留在原冻结 epoch；基础 SSE 最小修复独立提交 `d64703c07f29d71df643b83229d57a009d610e97`。本扩展改动单独提交，不能替换已经执行的基础来源。

实现沿用 [正式入口说明](../../scripts/acceptance/cpar-new-build-acceptance.md)、正式 `REMOTE_COLLECT` 和 `extension_check`；[扩展执行模块](../../scripts/acceptance/cpar_build_extensions.py) 只支持 LOCAL_SIMULATED。新 `extension_bundle_version:1`／`extension_sample_version:1` 保留完整 request、wire、模式、source、冻结及逐操作 runtime、request binding、根、summary、原生历史、WebSocket帧及精确 Request／Attempt／Usage／ledger。异主／删除的拒绝样本与原成功建根样本分别保存，audit逐个检查真实捕获的推理样本。原始内容留在0700目录／0600文件，不提交。

请求 controls完整冻结：Chat reasoning_effort=high；Responses reasoning.effort=high；Chat SSE stream_options.include_usage=true。没有用账本倒填缺失的原生 Usage，没有丢历史或替换工具结果。compact summary单独核对固定2048控制、JSON终态和source-event；消费保留完整compaction item，不同时传previous_response_id。WebSocket实际验证Upgrade／nonce／accept、两个response.create帧和各自终态／归属，无重连、retry或fallback。

本地实际进程使用同源码构建的debug binary，不是正式ARM64候选的新发布。操作前读取PID／start、command／state、实际binary SHA256及嵌入source；当地 `/var` 与 `/private/var` 以实际路径解析对应。OS sandbox禁止非loopback出口，独立负例确认；后台 Provider tasks disabled，单enabled candidate／单账号／concurrency1／max_attempts1。LIVE／PRODUCTION的`local_subprocess`证明不可替代`/proc`及manifest。

定向验证使用 [现有 Batch B 公共HTTP／TLS／SQLite fixture](../../scripts/acceptance/cpar-batch-b-http.py) 与 [定向验证模块](../../scripts/acceptance/cpar_build_extensions_http.py)。正式CLI执行、正式audit读回、错误Client Key真实公共请求后停止、缺证据发送前阻断、错误答案和上游错误停止、实际捕获样本的归属／终态／controls／缺根负例分别有证据；不是仅构造PASS结果对象。运行结果与hash见 [扩展本地回执](assets/cpar-build-batch-20261011/extensions/local-receipt.json) 和 [验收器回归](assets/cpar-build-batch-20261011/extensions/unit-tests.json)。产品原Full45/45、Rust1501/0/12、Build19项及ARM64证据按未变来源复用，未机械重跑。

## 17类准备度

所有行的真实结果仍为NOT_RUN。表中的PASS仅属于本地范围；“演练”列不是正式执行入口资格。

| 类别 | 正式本地执行入口 | 本地证据／额外缺口 |
| --- | --- | --- |
| visible_reasoning | 已实现且本地通过 | Chat／Responses JSON／SSE；Messages语义阻断 |
| reasoning_history | 已实现且本地通过 | 4模式完整原生历史及controls；Messages语义阻断 |
| store_read | 已实现且本地通过 | Responses JSON／SSE成功根与完整owner GET相等 |
| foreign_owner | 已实现且本地通过 | 第二有效key看得到目标模型，owner200、异主404、0发送 |
| deleted_read | 已实现且本地通过 | owner200、DELETE、同根404、0发送；真实根删除需授权 |
| expired_read | 待实现，入口BLOCKED | 当前源码固定30天TTL及合成expiry拒绝演练PASS；真实需自然到期根 |
| restart_read | 待实现，入口BLOCKED | 当前源码自有进程重启／binary相同／owner GET完整相等演练PASS；真实restart需授权 |
| persistence_failure | 待实现，入口BLOCKED | 本地自有SQLite提交触发器、唯一失败终态、GET404、1Attempt／Usage／ledger演练PASS；真实故障设施未具备 |
| exact_continuation | 已实现且本地通过 | 两模式独立建根／精确续接及账本 |
| lineage_rejection | 待实现，入口BLOCKED | 禁用原账号后原根仍可owner读取，续接拒绝且0发送演练PASS；真实隔离配置操作需授权 |
| compact_roundtrip | 已实现且本地通过 | 两模式各建根／summary／消费，3次推理分别核对 |
| compact_foreign_owner | 已实现且本地通过 | owner根存在、有效异主compact404、0summary发送 |
| compact_lineage_rejection | 待实现，入口BLOCKED | 受控禁用原账号，summary及既有item消费拒绝、0发送演练PASS；真实操作需授权 |
| ws_multiturn | 已实现且本地通过 | 真实本地socket Upgrade＋2帧／终态／Usage／账本 |
| ws_handshake_rejection | 已实现且本地通过 | 正确认证／路径／Upgrade／Origin→403、0发送 |
| ws_cancel | 待实现，入口BLOCKED | 首个语义输出后close、合成上游EOF、GET404、唯一Attempt及partial Usage／ledger演练PASS；真实取消采样需授权 |
| ws_failed_root | 待实现，入口BLOCKED | 截断后的唯一失败终态、GET404、同socket引用失败根拒绝且0追加发送演练PASS；真实故障点未具备 |

Messages thinking与Build effort没有无损转换依据，保持BLOCKED并保留true声明。没有关闭其他能力以缩减验收。通用fixture的parallel=true保持原值；原生Build实际声明parallel=false保持不变。本次通用fixture不能作为Build parallel=false的公共拒绝资格，仍单独BLOCKED，后续须在原生Build本地公共入口获得0发送证明。

故障Request失败／cancelled时，原始Attempt可以是success；这是上游执行与Request完成的不同事实。当前collector保留原outcome，没有把failed Request改为成功；账本异步物化只进行有上限只读等待，绝不重发公共请求。失败观察同时要求完整source-event、同一个Client Key／配置／lease、资源释放和无后继发送。concurrency1的单账号路由在故障／取消后重新可选，证明容量已释放；退出另核对端口与进程。

既有 [20260930 Batch B](cpar-batch-b-20260930.md) 的stored／continuity／WS证据保留为历史辅助，不能替代当前源码的执行入口qualification。所有本地先前失败epoch和回执保留，见 [本地迭代历史](assets/cpar-build-batch-20261011/extensions/local-history.json)；本地合成接收数不计入真实历史累计21。

## 下一批可审查操作与准确上限

当前批次已失败停批，余下真实额度不续用。以下是新的范围卡，**没有授权执行**，没有安排后台继续。进入每批前都需重新核对同一账号资格、材料／实际lease、source／binary／manifest／进程、配置／endpoint／route／candidate／模型、egress／Client Key／serving声明；新观察保持600秒。主动refresh／OAuth／catalog／quota仍不自动包含。

基础续批可只选择尚未完整通过的5组合：Responses SSE→Chat JSON／SSE→Messages JSON／SSE，各新Pin1＋完整基础9；上限 **5Pin＋45基础＝50新增推理**，以本次累计21为基础，新的累计上限至少71。Responses JSON的完整9轮证据可保留复用；旧Pin／进程观察不能当下批资格。任何身份变化导致整组重做时须重新列范围，不能默默扩成6组合。修复后的SSE必须取得新真实闭环，不能用本次离线解码抵扣。

扩展第一候选批仅选择10类已实现入口，按当前接口的实际独立建根行为给保守上限，不假设跨调用共享根：

| 操作分组 | Pin | 建根／首轮 | 后轮／续接 | summary | compact消费 | socket帧推理 | 推理上限 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Chat／Responses JSON／SSE的visible与history各调用一次 | 4 | 8 | 4 | 0 | 0 | 0 | 16 |
| Responses JSON／SSE store_read | 复用同批Responses两个Pin | 2 | 0 | 0 | 0 | 0 | 2 |
| Responses JSON／SSE exact_continuation | 同上 | 2 | 2 | 0 | 0 | 0 | 4 |
| Responses JSON／SSE compact_roundtrip | 同上 | 2 | 0 | 2 | 2 | 0 | 6 |
| foreign_owner／deleted_read／compact_foreign_owner，JSON各自建根 | 复用同批Responses JSON Pin | 3 | 0 | 0 | 0 | 0 | 3 |
| ws_multiturn／ws_handshake_rejection | 复用同批Responses SSE资格；每操作重新观察 | 0 | 0 | 0 | 0 | 2 | 2 |
| 合计 | **4** | **17** | **6** | **2** | **2** | **2** | **33** |

表内首行的16含4Pin；其12次公共推理是4个visible调用各1、4个history调用各2。合计33＝4Pin＋29公共推理。每个模式独立冻结与建根，未证明跨模式共享compact item，没有用17类直接当17请求。history首轮可供visible判定的复用已经本地检查，但减少独立visible调用需要先形成正式覆盖表／导出审计证据；当前33上限不扣这一优化。所有Pin须仍新鲜且同一冻结身份，否则停下重新决策，不能超过4自动补Pin。

零推理操作另列：store_read的owner GET2；foreign_owner的owner GET／有效模型权限GET／异主GET共3；deleted_read的owner GET／DELETE／GET共3；compact_foreign_owner的owner GET／有效模型权限GET／拒绝POST共3；WebSocket成功Upgrade1＋Origin拒绝Upgrade1，共 **13个HTTP／Upgrade操作**；多轮socket的2个response.create帧已计2推理，成功socket关闭控制帧1另计。catalog GET2仅用于第二key模型权限证明，必须明确新增授权，不能套用本轮目录GET0规则。第二key创建／配置projection、使用及撤销对象单列授权；根删除只限deleted_read新建自有根，其他根及历史证据保留。

这些数字仅用于10类可执行正例候选；当前executor仍拒绝LIVE。先实现并审查真实执行的授权绑定／全批预算控制／远端collector适配，再申请上述范围；不能仅修改layer绕过。扩展单批若从累计21开始且采用33新增，累计上限至少54；若先完成上述50次基础，扩展批需从届时实际累计重新计算，满额路径为71＋33＝104。任何失败立即止批，不用剩额重跑。

其余7类的操作方案：自然到期读先取得创建／到期时间可证的30天根（或建根1并另安排30天后执行）；restart复用同批stored根，前后GET各1、只重启自有unit1次；lineage／compact lineage复用已归档根和正控制，只在独立隔离副本改变一个精确维度，分别拒绝POST1，恢复及新观察另列；持久化故障使用独立故障epoch、已审核提交点与恢复步骤，保留唯一失败终态／source-event／GET404；取消使用独立socket，在首个语义输出后close并闭合观察；failed-root使用独立可归因故障点，再以1个零发送frame验证不能续接。后3类的真实设施／稳定触发／正控制缺失，**不在33预算内，真实上限待设施本地实现并审查后单独给出**。不在真实DB修改TTL，不以Provider故障探测代替本地准备。

初版41/41、210/210及其40次合成接收的原回执保留。独立审查发现的4项缺口已经先复现再修复：公开回执只输出受约束字段，完整请求在发送前保存，HTTP／socket原字节增量保存并与解码样本核对；PASS审计要求固定轮数、不同response／Request身份、捕获根和最终样本的一致性、真实parent／summary／消费关联，并拒绝任何未冻结controls。不得删根再调小计数取得PASS。

最终回归 **45/45 PASS**、285/285公共边界的中间修复证据仍保留。随后复核发现WebSocket终态后额外帧及握手原字节审计缺口，先复现再修复，见 [复核初始结果](assets/cpar-build-batch-20261011/extensions/ws-review-initial.json) 与 [复现](assets/cpar-build-batch-20261011/extensions/ws-review-red.json)。当前最终回归 **47/47 PASS**，定向公共边界 **293/293 PASS**；36次正式执行CLI包含19正例PASS、10预期BLOCKED和7预期FAIL-stop；19个PASS另经正式CLI audit读回。实际错误content-type、发送后自有进程重启、socket截断、completed后额外帧和握手body／status矛盾都会停止并保留原请求／wire。终态后使用100毫秒有界观察，不把未来无限时间承诺为通过。合成上游独立HTTP接收44＝5Pin＋36扩展CLI接收＋3故障／取消演练，全部真实Provider0。[最终本地回执](assets/cpar-build-batch-20261011/extensions/review-final2-local-receipt.json)、[回归](assets/cpar-build-batch-20261011/extensions/review-final2-unit-tests.json)、[来源](assets/cpar-build-batch-20261011/extensions/review-final2-source-freeze.json) 与 [本地退出](assets/cpar-build-batch-20261011/extensions/review-final2-local-exit.json) 分别绑定；所有旧epoch和失败保留。

Standards／Spec的限定审查、正式docs门槛、提交及GitHub写后读回由 [本批收口报告](cpar-build-batch-20261011.md) 和对应资产绑定。未完成的真实／入口／语义条件均保留，不能将本报告标为#47整票PASS。
