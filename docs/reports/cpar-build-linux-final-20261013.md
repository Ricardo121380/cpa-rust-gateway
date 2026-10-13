# CPAR-38 最终 Linux 系统门槛与条件式单次 Chat JSON 诊断收口

本批从2026-10-13开始，初始草稿时间 CST: 2026-10-14T00:20:53.989562+08:00；最终元数据记录时间 CST: 2026-10-14T00:56:18.742446+08:00。最终 Linux 合成系统门槛 **PASS，4/4**；独立 Standards / Spec 诊断前置审查 **PASS / PASS**。正式资格观察、准备和启动通过，fresh Pin 入口在预留与发送前因 serving snapshot 资格声明为空阻断；**实际新 Pin / Chat 为0/0，Chat NOT_RUN，正式诊断 BLOCKED**。本 epoch 已永久停止，未用2个意图封存。旧502根因保持 UNKNOWN，下一轮协议矩阵条件尚不齐备。

## 基线、规则与复用

- 初始 HEAD51281f0a7255e178ddc770034a8141b197e67d6f；用户提供的68c007e8行为审查点是定位信息，不作回退。读取项目 AGENTS.md、docs/agents/issue-tracker.md、最新 cpar-build-linux-start-20261013.md、必要契约/源/旧review/systemgate/实际回执，并刷新#47/#57/#9。
- 产品源码始终 bf822b14a62c21cf582a7539e3635f69bd8b7677。新系统执行源码修改点 daad22e1df0ad216f5051050a9b3d25fb68677cc；最终运行前审查固定HEAD9d670ba720fce2c92b7f1ba11516cee397ff1037。工具HEAD、产品source、二进制和manifest各自记录。
- Binary SHA cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e；manifest SHA655c037df68c5657b250105bbb07bed665210a7f6b20ea8d8b88aab488157a56；metadata SHA d5e87f208ec6ab4265a272d4be5d331bbb0865e30abfa62fa6556671d96cb78c。产品树/产物/构建日志与旧review实际重哈希匹配，复用Task42/42和Phase45/45，不重建、不安装或重跑Full。
- 首尾105个历史公开/私有载荷、预算、回执、旧报告和用户AGENTS摘要/权限一致。原3系统槽、旧FAIL/PARTIAL及正式0/0封存记录未覆盖，全部无关tracked/untracked保留；新资产只在本批目录。

## 最小驱动、夹具与回执改动

四槽控制器 A→B→C→D 独占执行，每槽UNKNOWN先持久化再SSH；异常立即锁停，不重开/重试旧槽。新目录/unit/身份/预算，合成disabled账号与随机钥匙，PrivateNetwork=yes、独立namespace仅lo、实际IPv4/IPv6 TEST-NET返回ENETUNREACH101；无生产/全局网络修改或新基础设施。

A 创建完整freeze/prepare/start/argv材料，真正执行完整remote-exit成功分支。适配只替换合成epoch/unit、零Provider预算、生产credentials HTTP为生产元数据摘要；完整绑定、停止意图fsync、PID/start复查、真实stop、审计/SQL/权限对账均保留。完整diff/源/实际wrapper SHA单列；合成适配不证明真实账号资格。实际source/adapter/adapted/wrapper摘要由独立原件readback核对。

B 使用实际存在且0700的无效格式文件，Type=exec及CollectMode=inactive；保存原生systemd show/journal及失败证据后才reset-failed。目标系统d255的 [Type=exec语义](https://raw.githubusercontent.com/systemd/systemd/v255/man/systemd.service.xml) 和 [failed unit保留语义](https://raw.githubusercontent.com/systemd/systemd/v255/man/systemd.unit.xml) 已按官方源码核对；本批通过依据仍是原生实测203/EXEC与journal Exec format error。

C 沿固定产品404契约，仅一次错误管理Key请求，首次失败即停止。启动FAIL、身份、资源退出与负例测试结果分别保存。

D 追加明确synthetic marker并文件/目录fsync。初次独立Spec发现64.999→65.001跨截止漏注入路径；原RED/FAIL冻结保留，最小clock adapter在首个实际elapsed≥65读数返回前持久化事件，真实monotonic/sleep和原最终检查不变。已有audit不会删除/清空。修复后设计7/7，实际窗口末端事件阻断quiet PASS。

HTTP200诊断驱动增加原生与公开文本、reasoning、id/model、usage及可无损表达结构核验；unsupported/lossy结构不获NOT_REPRODUCED资格，产品不丢字段。原fidelity RED及修复后本地证据保留。本批未执行真实Chat，不声称真实保真通过。

初始新增独立只读readback helper误比完整private receipt与public projection，exit1记录/源码原样保留；仅helper改为原remote-exit完全相同的投影和inventory digest严格比较，后独立readback PASS。核心系统/正式执行驱动与freeze未改，未迁移旧PASS给新执行字节。中断且无终态的探子均PARTIAL归档，不用作PASS。

最终交付初审 Standards FAIL、Spec材料FAIL/执行契约PASS/完整目标PARTIAL：报告生成器全局替换时间占位符时损坏了7处UNKNOWN文字，机器回执未改变。原报告副本与两路初审终态保留在report-initial-delivery-fail.md、review-delivery-initial.json及对应原文；该修正增量仅修复7处文字并补充失败保留说明，执行源、冻结、实际运行证据均不变。独立窄范围核验确认修正文案PASS，不将完整诊断目标升级为PASS。Spec探子额外比对误猜不存在的公开文件名后工具BLOCKED原样保留；新的独立探子只补三份17项公开摘要映射，终态complete/PASS。upload没有单独freeze字段，prepared与presend的实际freeze声明相同；不把探子任意重序列化对象得到的不同派生hash当作真实freeze证据。

## 场景—实际入口—预期及实测证据—退出

| 场景 | 实际入口 | 实测 | 场景 / 启动 / 退出 |
| --- | --- | --- | --- |
| A | system-control A→StartupProbe.launch/wait_listeners/quiet_audit→完整adapted remote-exit | 五门PASS；真实65.450398秒；冻结/start/argv/guard/零预算绑定；原startup观测未改、独立exit观测；SQL/事件/ledger新增0 | PASS / PASS / PASS |
| B | 同启动入口，存在0700 invalid executable，经Type=exec到内核 | launch1；清理前loaded/failed、ExecMainPID2654171、Code1、Status203、Resultexit-code；native journal Exec format error保留 | PASS / FAIL(exec) / PASS |
| C | 完整exec/身份/监听后一次wrong synthetic management GET | request_intents1，HTTP404，first_management_request_rejected；无第二GET，无quiet PASS | PASS / FAIL(application) / PASS |
| D | 实际65秒窗口末端首次截止读数前追加marker，再执行原最终检查 | elapsed65.419759秒；quiet_audit_not_flat；无start receipt；原159-byte audit SHA afef9b39…保留；synthetic1/network0 | PASS / FAIL(quiet未过) / PASS |

四份回执绑定同一system freeze f67a5091ba93741ecfd1e0e1608a8b25df708aeb605714f91bab384b8b960d4f。A/B/C/D各launch一次，4槽全部使用后stopped。各资源退出后真实5秒审计稳定，unit not-found/PID0/cgroup与监听无遗留，生产PID499070/NRestarts0及unit/build/current-target/Caddy摘要保持。B无成功进程身份是预设内核拒绝后的事实，未用晚到身份升级启动。

system-readback-final.json独立读取原冻结文件、实际执行wrapper/hash、stop intent、startup/exit观察、B原生失败与before-reset摘要、C一次404、D原审计/injection/no-start、PID/unit/cgroup/权限/生产元数据，全部PASS；此只读核对新增launch/Provider均0。

## 正式epoch：资格通过，发送前阻断，永久停止

新epoch chat-json-linux-final-20261013，独立预算Pin≤1+Chat≤1/总≤2；历史35意图/34 gateway+Pin确认发送独立，不恢复旧预算。实际payload17文件SHA、tar、driver/binary/manifest、systemgate/执行前双轴review新冻结，payload freeze d00c82797519a31c5973717c0eb5c628d70a9078baa089248c75ac63fe3d5f61。新ClientKey实际wire摘要只由授权只读捕获，秘密未输出；运行PID/start/argv/guard/实际账号来源及配置/原目标分别绑定，不用预期revision代替fresh lease。

- 一次read-only同授权连续性观察PASS，余效16395.674秒；实际prepare记录余效16231.104秒，严格>3600。无本任务主动refresh/重新授权/账号替换/等待再观察。正常已存在refresh字段变化与identity连续性有单独证据。
- 旧source DTO credential_revision=null被作为可选摘要处理。candidate严格观测revision147；账号native146；这些是source/candidate字段，实际fresh Pin lease从未建立，保持null。
- 上传/准备PASS。正式启动PASS，PID2683088，监听19180/19181归属验证，guard/cap4/唯一env/argv准确、候选池可用且无lease，实际65.472122秒静默、Provider/catalog请求0。
- 一次Pin入口检查后BLOCKED，原native summary category AssertionError。账号/候选池/余效/实际revision检查均过；保留inspection哈希7d4096fb…中的capability_observations=[]，首个不成立断言是 frozen remote-pin.py:91 的len(declarations)==1。ClientKey/group/route/model/candidate均active、route grant enabled，capability_read_errors=[]；public model capabilities={}、candidate override仅allow_unlisted_model=true。这些事实不证明为何effective声明为空；原因保持UNKNOWN。
- snapshot离线比较不发管理HTTP/Provider，第一次比较脚本source_line标签低3行，原执行回执保留，正确91行出处另存pin-presend-assertion-analysis.json。未执行/重放Pin程序或真实请求。
- 断言发生在reserve_pin之前；pin-request/private-pin不存在，实际Pin意图0、Chat意图0，Chat根本未执行。未产生新Request/Attempt/Finished/Usage/native capture/ledger，独立最终DB/文件/readback验证新增均0。Provider独立接收数与费用仍UNKNOWN；不以gateway0推算Provider独立证据。
- 首次BLOCKED即永久锁停，仅调用冻结remote-exit完成安全退出。正式exit与count reconciliation PASS；原startup观测保留，停止意图PID/start绑定，unit/PID/cgroup/监听无遗留；budget stopped，未用2意图永久封存。没有retry/fallback/换模型账号/refresh或修复后真实追加。
- 本地prepare命令一次因上传仍持锁被本地flock拒绝(rc2)，在main/预留/派发之前，没有prepare或Provider意图；原local-lock-denial单列。实际upload结束PASS后才进行唯一prepare派发。本地调度拒绝未当作正式执行结果或未知副作用。

原Chat POST /v1/chat/completions、grok-4.5、stream=false、max_completion_tokens=4096，原请求SHA245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062不变。Chat NOT_RUN，不能写NOT_REPRODUCED，旧502精确根因仍UNKNOWN。

新增本地缺serving声明回归覆盖no_models/no_sources两种模拟输入，均在预留前阻断、预算stopped、不可重入Pin/Chat、无POST且安全退出PASS。全流现在37/37；只改变测试文件，原执行驱动/systemfreeze/实际17payload字节保持。历史36/36日志仍保留；不重跑真实槽或调用。

## 分层状态、计数及范围

| 层 | 状态 | 证据及限制 |
| --- | --- | --- |
| 本地合成完整流程 | PASS37/37 | local-full-flow-declaration-negative；旧34/36及RED保留 |
| 本地系统设计 | PASS7/7 | local-system-design-tail-fix，含实际quiet_audit模拟调度反例 |
| 历史回执离线重放 | PASS3/3 | 原旧raw FAIL不改，historical-replay；OFFLINE_RECEIPT_REPLAY |
| 产品构建/Task/Phase | PASS_REUSED | 同产品/产物摘要，Task42/Phase45；本批build0/Full新执行0 |
| 真实Linux系统验证 | PASS4/4 | 全新隔离A/B/C/D各一次，Provider意图0/真实网络事件0；合成账号资格 |
| 正式材料/准备 | PASS | 一次同授权观察，prepare严格>3600秒 |
| 正式启动 | PASS1次 | 原正式入口、实际65.472秒、Provider0 |
| fresh Pin | BLOCKED_BEFORE_RESERVATION；actualNOT_RUN | 入口1次，声明0要求1；意图/发送0，实际lease未知/null |
| 原Chat JSON | NOT_RUN / 诊断BLOCKED | 无HTTP结果；不能NOT_REPRODUCED |
| 独立Standards/Spec前置 | PASS/PASS | 固定9d670ba7源码复用+实际四槽+新freeze；不等于正式输出通过 |
| 本批最终材料审查 | Standards PASS；Spec PARTIAL | 执行契约及修正文案PASS；正式资格缺口1；原材料FAIL/工具BLOCKED保留，公开映射补核PASS |
| 资源退出 | PASS5/5 | 系统4次＋正式1次，独立原件回读；保留失败启动 |
| Provider独立receive/费用 | UNKNOWN/UNKNOWN | 无独立Provider接收/费用证据 |
| GitHub#47/#57/#9 | PASS3/3；均OPEN | 既有progress各body-only追加1次，独立comment/issue/分页deps读回；票面AC、标签/assignees/milestone与依赖边状态不变 |
| 远端CI | NOT_RUN | 固定执行审查点9d670ba7 Actions匹配run0；未push/dispatch |
| 发布/生产/六组合矩阵 | NOT_RUN | 只跑本批系统和正式启动，没有生产服务变更 |

实际使用 **4/4新的系统验证运行＋1次正式启动**。正式Pin入口派发1次但Provider意图未预留；新fresh Pin调用0、Chat调用0、总新Provider意图/确认发送0。旧35/34保持；未用新2意图封存，旧额度不恢复。各命令、原生输出hash、冻结、readback、账本和执行身份链都在本批资产与受保护私有目录。

## 剩余缺口与后续矩阵条件

Linux五缺口已收口。正式资格声明缺口仍存在：必须以真实serving snapshot及其采集契约证明唯一且完整的声明，保持ClientKey/config/route/candidate/model/capabilities严格绑定；不得将空声明改成默认能力或跳过断言。需要实际fresh Pin执行lease及原Chat JSON的真实保真诊断证据；本批永久停止，不能使用这2个封存意图追加。

目前 **尚不具备下一轮协议矩阵验收条件**。即使之后一条诊断通过，六组合/扩展资格/Remote CI/Artifact/生产及父票门槛仍独立，只有全票验收条件全部满足才关票。本批#47/#57/#9保持OPEN，只提交拥有文件，不push/发布/部署。历史FAIL/PARTIAL与旧未知根因保留。

## GitHub独立读回

既有progress [#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490) 各PATCH一次，原body逐字节前缀保留。独立GET验证完整body与准备载荷相等，三票body/AC/state/标签/assignees/milestone等保持。分页blocked_by读取：#47共8/OPEN0，#57共23/OPEN11（40–50），#9为0；没有修改依赖或关闭票。

初次本地readback validator要求整个dependency对象逐字相等而FAIL，原stderr和快照保留在github-sync/initial-verification-fail.json。逐字段核对唯一变化为#57依赖#47的updated_at，从10:34:55Z到16:50:40Z，后者精确等于本次#47授权progress PATCH及独立comment/issue GET时间；其他所有字段、边顺序/数量及状态完全相等。限定接受这一绑定活动时间变化后核验PASS；没有额外远端写入，没有放宽依赖状态或票面AC核对。

主要证据：本批assets下 baseline/reuse、batch-contract、system-freeze/system-A/B/C/D/system-gate、system-readback-final、review-system-initial/source/pre-execution/final、prepared-freeze、observation-1/prepare/start/presend-binding/pin/exit、pin-presend-assertion-analysis/pin-capability-snapshot、formal-final-readback、formal-stopped-summary、local37/design7/historical-replay3、remote-ci-readback、github-sync/summary、delivery-status及static-delivery-final。私有原始材料保留在output/cpar-build-linux-final-20261013与new-vps acceptance的新epoch。最后交付元数据由主代理静态核对，独立审查各自固定head/复用范围不变；本地提交不等于remote CI或发布证据。
