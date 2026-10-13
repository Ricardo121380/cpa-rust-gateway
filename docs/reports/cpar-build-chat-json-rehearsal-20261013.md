# CPAR-38 / #47：验收驱动跨版本完整预演与受控 Chat JSON 诊断

本批已完成 **LOCAL_SIMULATED 完整预演及实际受控尝试，最终锁存停止**。第一次只读观察、上传和prepare通过；临时进程实际创建，但启动身份检查失败，未完成启动资格，fresh Pin与Chat均未执行。资源退出另以严格身份绑定证明PASS。没有取得本次Chat诊断结果，旧502根因仍UNKNOWN。

## 基线、授权与复用

当前基线 `9b6d70615d7507114aeab0007b34ea9db81d6182`；实际checkout为准，没有回退。产品 `bf822b14a62c21cf582a7539e3635f69bd8b7677`，source摘要兼容修复 `5e8768c485dda02b8b84672a7603a97d49f85a1f`。apps/crates/Cargo/工具链及构建脚本对象、工作区产品diff均匹配。binary `cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e`，metadata `d5e87f208ec6ab4265a272d4be5d331bbb0865e30abfa62fa6556671d96cb78c`，manifest `655c037df68c5657b250105bbb07bed665210a7f6b20ea8d8b88aab488157a56`，manifest两行的实际大小和摘要一致。

Task42/42、Phase45/45的gzip及完整原始日志摘要、完整历史两轴审查摘要逐项匹配后复用。产品/全局验收helper不变；依赖安装0、产品构建0、产品Full重跑0。本批资产含Python行为代码，不能称docs-only；使用实际完整驱动回归和补充docs/static检查，未变化产品门槛按用户本批明确要求复用。

独立新epoch `chat-json-rehearsal-20261013`，同一保护根下唯一bin/private/audit/unit。旧拒绝包、tar、停止state和所有失败不变。用户AGENTS和无关tracked/untracked内容保留；本批只提交报告/本批资产。不修改web/prism，不push、CI、发布、部署或生产写入。

最多7次/60分钟只读观察、相邻至少10分钟；仅正常同身份但正剩余期不足可等待。prepare严格 `expires_at_ms > now_ms + 3600000`。正式批次最多fresh Pin1、仅PASS后原合成Chat1；`POST /v1/chat/completions`、grok-4.5、stream=false、max_completion_tokens=4096与精确请求摘要 `245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062`保持。历史35意图/34确认发送独立，不恢复旧余额；费用政策独立于次数，Provider接收与实际费用无证据保持UNKNOWN。

见 [契约](assets/cpar-build-chat-json-rehearsal-20261013/batch-contract.md)、[复用](assets/cpar-build-chat-json-rehearsal-20261013/reuse.json)、[授权](assets/cpar-build-chat-json-rehearsal-20261013/user-authorization.json)。

## 数据来源、完整合成流程与驱动修复

生产source版本 `b0cf36e387b774439acc23830221389eb3ac74d0`/schema31的实际DTO/OpenAPI没有摘要credential_revision；候选版本bf822b14/schema34的该字段可选，但本批候选/实际执行校验强制完整一致。源端缺失/null只适用于摘要，非null在observe/prepare/start各次重读都核对。

候选原生DB保持native revision N；实际候选池提供runtime revision，准备时N+1仅为这个固定handler版本的预期检查。Pin请求使用native N以满足管理CAS；Pin execution.identity提供真实lease revision，冻结计划必须从该执行证据取值且与候选目标相等。Chat的Attempt sidecar再以Request-ID、Attempt原始payload SHA和canonical执行身份精确验证该lease。没有以材料revision推算或填补lease。

完整流程 `observe → upload → prepare → start → pin → chat → exit` 在隔离文件系统、实际SQLite数据库和外部操作替身中执行实际控制器与各阶段顶层程序；实际REMOTE_READ库存收集器、Pin freezer、预算save/fsync、HTTP请求序列化、捕获decoder及Request/Attempt/Usage/ledger检查都运行。systemd、Linux /proc/UID/固定校验材料、时间和HTTP传输替身明确标为LOCAL_SIMULATED；各远端阶段不返回固定PASS。测试自己的PASS资格文件仅为本地测试授权，不能用作实际独立审查。

18项测试包含多个边界子场景：缺失/null source成功；source非null漂移；候选缺失/null/漂移；fresh Pin lease缺失/漂移；prepare不足整小时或未知；启动认证首次失败不重试、启动未知和审计新增；Pin/Chat未知永久停止；操作槽与预算发送前持久化；重入/停止后拒绝；冻结字节漂移；Attempt/request/payload/lease及账本source_event错配；严格502拒绝的原生Usage保留；失败/缺证据发送计数UNKNOWN独立于资源退出；prepare/start失败无start receipt时退出。

保留RED后做三处本批驱动最小修复：source摘要非null校验补到prepare/start；Chat `error:null`不会中断发送后证据收集，HTTP200仍要求无具体错误、成功Attempt与Finished；无成功start receipt时走只读资源证明，使用本批观察持久化的新鲜生产基线，保留启动失败的实际停机状态，不停止身份不明进程。不改Rust产品、生产schema或保真规则。

第一次完整审查覆盖36/36对象与8/8完整gzip：Standards PASS，Spec FAIL，保留原结论后实际回归得到16项/2 FAIL。最小修复将观察返回的生产基线在资格判断前持久化，并绑定、接入未上传时的只读退出程序；没有此前观察基线时明确标记NOT_OBSERVED，同时检查当次新鲜生产状态稳定，不虚构恢复目标。对应版本DTO夹具扩展到全部引用闭包（source8、candidate12），完整校验池字段、Pin请求和响应：补request_id/attempt_id，成功不输出非法stage=completed。源端摘要compat字段与旧DTO分开说明；候选和实际lease仍严格。修复后完整流程17/17 PASS，总计30/30。未发生真实观察或发送。

见 [第一次完整审查](assets/cpar-build-chat-json-rehearsal-20261013/review-preparation-first.json)、[审查缺口RED](assets/cpar-build-chat-json-rehearsal-20261013/review-gap-red.log.gz)和[修复GREEN](assets/cpar-build-chat-json-rehearsal-20261013/review-gap-green.log.gz)。

见 [对应版本的契约与脱敏观察夹具](assets/cpar-build-chat-json-rehearsal-20261013/contract-fixtures.json)、[完整流程测试](assets/cpar-build-chat-json-rehearsal-20261013/tools/full-flow-tests.py)、[初始RED](assets/cpar-build-chat-json-rehearsal-20261013/full-flow-red.log.gz)、[无start退出RED](assets/cpar-build-chat-json-rehearsal-20261013/prestart-exit-red.log.gz)、[完整GREEN](assets/cpar-build-chat-json-rehearsal-20261013/full-flow-green-bound.log.gz)。

第二轮完整Spec审查覆盖45/45对象、14/14 gzip，精确DTO与载荷绑定通过，但仍发现上传失败后的退出不可达（Spec FAIL）；Standards未取得终态，记录PARTIAL。保留第二轮结论和18项/1 FAIL的上传退出RED后，退出分支按是否已进入prepare选择：尚未prepare时只执行本地冻结的只读资源证明，不依赖远端上传完整性。真实上传程序注入创建前拒绝、部分tar和完整上传后结果未知三种故障：禁止重入/prepare，保留上传字节并报告NOT_UPLOADED/PARTIAL/PASS，资源证明独立于载荷完整性。完整流程18/18、受影响控制器2/2新通过；资格9/9和启动2/2代码未变复用，总计31/31 LOCAL_SIMULATED。

见 [第二轮审查](assets/cpar-build-chat-json-rehearsal-20261013/review-preparation-second.json)、[上传退出RED](assets/cpar-build-chat-json-rehearsal-20261013/upload-exit-red.log.gz)、[上传退出GREEN](assets/cpar-build-chat-json-rehearsal-20261013/upload-exit-green.log.gz)。

## 已执行命令与退出状态

工作目录 `/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway`，本地环境 Darwin27 arm64。以下为已有实际执行的准确命令主体；stdout/stderr的受控重定向、退出码0和原始日志摘要完整记录于 [命令回执](assets/cpar-build-chat-json-rehearsal-20261013/verification-commands.json)。本段补录不重跑检查。

| 检查 | 已执行命令（在上述目录） | 退出码 | 当前证据 |
|---|---|---|---|
| 完整流程 | `python3 -B docs/reports/assets/cpar-build-chat-json-rehearsal-20261013/tools/full-flow-tests.py` | 0 | 上传退出修复后新执行18/18；[日志](assets/cpar-build-chat-json-rehearsal-20261013/upload-exit-green.log.gz) |
| 控制器/锁 | `python3 -B docs/reports/assets/cpar-build-chat-json-rehearsal-20261013/tools/control-regression-tests.py` | 0 | 修复后新执行2/2；[日志](assets/cpar-build-chat-json-rehearsal-20261013/upload-exit-control.log.gz) |
| 资格/准备/权限/哈希 | `python3 -B docs/reports/assets/cpar-build-chat-json-rehearsal-20261013/tools/local-tests.py` | 0 | 代码未变，复用本批9/9；[日志](assets/cpar-build-chat-json-rehearsal-20261013/review-gap-local.log.gz) |
| 启动认证/监听归属 | `python3 -B docs/reports/assets/cpar-build-chat-json-rehearsal-20261013/tools/start-regression-tests.py` | 0 | 代码未变，复用本批2/2；[日志](assets/cpar-build-chat-json-rehearsal-20261013/review-gap-start.log.gz) |
| docs/static补充门槛 | `CHECK_REPORT_PATH=docs/reports/assets/cpar-build-chat-json-rehearsal-20261013/docs-gate-upload-exit.md scripts/check.sh docs` | 0 | 已执行7/7；[门槛](assets/cpar-build-chat-json-rehearsal-20261013/docs-gate-upload-exit.md)、[日志](assets/cpar-build-chat-json-rehearsal-20261013/docs-gate-upload-exit.log.gz) |

第三轮完整阅读52/52对象及18/18 gzip：Standards发现本报告缺命令/退出状态，Spec因历史manifest/metadata误定位返回PARTIAL，范围内未确认实现缺口。本段补齐命令回执；[复用回执](assets/cpar-build-chat-json-rehearsal-20261013/reuse.json)现明确两个历史原件的实际位置，原件与私有payload逐字节一致且manifest两行大小/摘要匹配，产品构建0。保留 [第三轮原结论](assets/cpar-build-chat-json-rehearsal-20261013/review-preparation-third.json)，由新的独立审查者关闭未完成门槛；不把之前PARTIAL改写成PASS。

## 本批执行与剩余条件

执行前累计完整覆盖54对象，Standards与Spec均PASS。Standards使用原完整52对象审查、四份文档全文收口审查和独立16/17/3及历史原件哈希核验组合，未改写历史FAIL/PARTIAL；Spec独立累计54对象完整PASS。见 [完整审查依据](assets/cpar-build-chat-json-rehearsal-20261013/review-pre-execution.md)及[机器门槛](assets/cpar-build-chat-json-rehearsal-20261013/review-pre-execution.json)。额外Standards尝试中断且无终态，不作为PASS依据。

实际结果见 [汇总](assets/cpar-build-chat-json-rehearsal-20261013/actual-summary.json)及[执行命令/退出状态](assets/cpar-build-chat-json-rehearsal-20261013/actual-commands.json)。真实观察只进行1次，返回同一授权连续性PASS、剩余9355秒，正常续期等待0次；生产版本仍b0cf/schema31，源端摘要credential_revision=null。观察没有创建远端epoch或Provider调用。

上传16/16实际字节与冻结摘要匹配，覆盖0；prepare重新核对有效期大于整小时、身份连续和同一授权source，复制1个账号，native revision=144、candidate schema34，runtime预期=145。没有主动刷新、重新授权、账号切换、产品构建或生产写入。见 [观察](assets/cpar-build-chat-json-rehearsal-20261013/observation-1.json)、[上传](assets/cpar-build-chat-json-rehearsal-20261013/upload.json)、[准备](assets/cpar-build-chat-json-rehearsal-20261013/prepare.json)。

实际systemd临时进程创建后，`remote-start.py:145` 首次argv/state身份断言失败；启动失败处理也未取得绑定，自动清理状态为IDENTITY_OR_EXIT_UNKNOWN。远端预算随后锁存，Pin/Chat禁止；原退出程序因缺成功start receipt而只允许MainPID=0，同样失败。最初断言读取的argv值未保存，不能确认exec过渡、实际错误参数或其他确切启动原因；合成exec未就绪场景只用于离线回归，不是实际根因证明。65秒审计、候选池runtime revision及socket/PID完整门槛没有完成，不能以稍后进程证据替代。见 [启动失败](assets/cpar-build-chat-json-rehearsal-20261013/start.json)、[退出失败](assets/cpar-build-chat-json-rehearsal-20261013/exit.json)、[最终锁存](assets/cpar-build-chat-json-rehearsal-20261013/stop-latch.json)。

新的只读观察确认本批PID2026813/start679814074，exe摘要与冻结binary一致，完整argv与冻结launch精确相等，state目录归属本批，guard已映射，五个环境字段唯一。随后独立资源清理在停止前重新核对这些身份、上传freeze原字节及prepare私有freeze的JSON内容相等，持久化清理意图，并再次核对PID/start；只停止该临时unit。首个补充清理脚本误将重新序列化的私有freeze当原字节核验，在停止前被拒绝，失败原件保留；纠正为上传原件后完成停止。这些操作只为资源退出，没有重新执行formal start/exit、恢复epoch资格或发送推理。见 [只读身份](assets/cpar-build-chat-json-rehearsal-20261013/identity-observation.json)、[补充清理首个拒绝](assets/cpar-build-chat-json-rehearsal-20261013/identity-bound-cleanup.json)、[最终资源证明](assets/cpar-build-chat-json-rehearsal-20261013/identity-bound-cleanup-final.json)及[实际清理代码](assets/cpar-build-chat-json-rehearsal-20261013/tools/identity-bound-resource-exit.py)。

退出结果PASS：MainPID=0、旧PID消失、19180/19181释放、停止后5秒审计零变化且总审计字节0；381个历史文件摘要、历史停止预算、36个私有文件及0700/0600权限均核验通过。生产进程/文件与本批prepare新鲜基线一致，活动配置仍kimi rev-1；没有把旧报告配置当恢复目标。控制器原失败phase EXECUTING_EXIT和stopped=true保持，不改写为成功formal退出；独立资源退出为STOPPED_VERIFIED。本批epoch为CREATED，不能填NOT_CREATED/NOT_STARTED。

| 计数与来源 | 本批结果 |
|---|---|
| 历史意图 / 网关及Pin确认发送 | 35 / 34；不恢复历史剩余预算 |
| fresh Pin / Chat / 新意图合计 | 0 / 0 / 0；两者均NOT_RUN，未建立实际lease或执行计划 |
| 新网关及Pin确认发送 | 0，预算0、审计0、新事件0及无Pin/Chat动作相互支持 |
| Request / Attempt / Finished / Usage / 账本新行 | 全为0；prepare继承的各1行事件及1行账本单列，不能归本批 |
| 本次Request-ID / 实际lease / native捕获 | 未建立；原请求精确SHA与4096限制保持，未发送 |
| Provider独立接收数 / 实际费用 | UNKNOWN / UNKNOWN，不能从网关或资源证明投射为0 |
| 旧502根因 / 本次NOT_REPRODUCED | UNKNOWN / 不适用；未取得Chat结果，也未进入严格保真拒绝判定 |

## 停止后的离线最小修复

实际证据确认的驱动缺陷是：没有成功start receipt时，即使进程稍后可完整绑定为本批，原退出分支仍只允许PID=0，不能完成受控资源退出。先运行真实控制器/阶段逻辑的合成场景得到1项FAIL，再修改本批remote-exit：完整核对冻结launch、exe路径及摘要、argv/state、guard映射与摘要、五个唯一环境字段；预算和新鲜PID/start绑定先持久化，再复查PID/start后停止。拒绝身份缺失或漂移，不等待恢复执行资格；退出明确startup_validation_status=NOT_PASSED。最初启动断言及其未知实际根因未被放宽，也没有改变systemd启动或产品规则。

新增完整流程测试覆盖晚到稳定身份的安全退出及停止前持久化，并以exe路径、argv/state、guard、重复环境和PID启动时刻五种错误确认不停止未知进程。完整流程20/20新通过；未改控制器2/2、资格9/9、启动2/2复用，总计33/33 LOCAL_SIMULATED；docs/static7/7通过。准确命令、exit1 RED与exit0 GREEN、源代码及raw/gzip摘要见 [停止后检查](assets/cpar-build-chat-json-rehearsal-20261013/post-stop-checks.json)、[RED](assets/cpar-build-chat-json-rehearsal-20261013/post-stop-exit-red.log.gz)、[GREEN](assets/cpar-build-chat-json-rehearsal-20261013/post-stop-exit-green.log.gz)、[门槛](assets/cpar-build-chat-json-rehearsal-20261013/docs-gate-post-stop.md)。

实际执行driver仍为400bb672，原freeze `53c1e388...`、私有payload、tar和远端已上传代码保持不变；停止后源代码修复没有重新打包、上传、启动或真实调用。原执行代码可从该Git提交及保留的私有payload取得，不能把修复后的源码摘要当本次实际执行身份。停止后修复和交付材料的独立两轴完整审查单列归档，尚待收口。

诊断与保真规则缺口消除后才提出下一轮基础矩阵。六组合最多60新意图及扩展仍需独立授权，本批不执行。#47/#57/#9保持正文/AC/依赖/状态，只在本批真实停止交付后追加既有progress并独立读回，不按局部结果关闭整票。
