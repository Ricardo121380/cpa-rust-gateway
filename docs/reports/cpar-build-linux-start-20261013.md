# CPAR-38 / #47：真实 Linux 启动观测修复与受控诊断门槛

本批为 **PARTIAL；正式 Chat JSON 诊断 BLOCKED**。启动观测、exec 握手与安全退出的最小修复已实现；本地完整流程34/34、真实回执的离线 oracle3/3通过。实际使用3/3个零Provider系统验证槽位，正常场景取得真实systemd、exec、/proc、socket/PID及65.46秒静默证据；两个预设负例原始FAIL保留，尚缺有效的内核exec失败负例及最终修改的完整Linux复测。三个临时资源均已退出。fresh Pin0、Chat0，没有本次Chat结果，旧502根因仍UNKNOWN，不能标NOT_REPRODUCED。

## 基线、范围与复用

实际base `9aa08db64fa74767064db65f906fa7bae0f2aa50`，没有回退。读取项目AGENTS、issue-tracker、跨边界日志尾、原20261013 rehearsal报告及必要契约/回执，刷新GitHub#47/#57/#9正文、既有progress与依赖。三票OPEN；#47的8项依赖closed，但自身全部基础组合和声明扩展尚未完成；#57的23项依赖仍含11个open渠道票，父规格仍未完成。工单正文/AC不作修改。

保留用户AGENTS和其他未提交内容，仅拥有本报告及同名assets。原报告、原执行400bb672、停止后0602faaf、原payload/freeze/tar和所有停止预算均不恢复。旧首次argv/state断言的原始观测缺失，确切根因仍UNKNOWN；稍后参数一致只证明可做资源退出。原退出PASS不升级原启动FAIL。

产品仍 `bf822b14a62c21cf582a7539e3635f69bd8b7677`。apps/crates/Cargo/工具链/构建脚本Git对象、工作区产品diff、binary/metadata/manifest摘要及实际大小、Task42/42与Phase45/45完整raw/gzip摘要、适用完整两轴原件摘要均核对一致。manifest与metadata原件位于20261011资产，不能误用同名20261013回执。[复用回执](assets/cpar-build-linux-start-20261013/reuse.json)记录精确路径。产品build0、install0、Full重跑0，未修改Rust/UI/公开契约。

## 修复及针对性回归

先保留4项RED，再把派生launch的Type=simple改为Type=exec，增加TimeoutStartSec=20s和25秒launch超时。真实systemd为255.4；官方v255说明simple在fork后、execve前即可报告成功，exec等待execve成功。它修复已确认的同步缺口，不证明旧实际失败就是竞态。见[语义与限制](assets/cpar-build-linux-start-20261013/system-semantics.md)。监听、应用资格和65秒审计继续独立，不用固定就绪sleep或重复HTTP。

新startup_probe在首次身份执行断言前fsync持久化脱敏systemd状态、PID/start、exe路径/摘要、argv摘要及精确比对、state、guard映射、唯一环境、能力、时间顺序和闭合失败原因。launch原生stdout/stderr只留在0700/0600私有目录，公开回执仅有摘要/长度，凭据不进入报告、GitHub或普通日志。追加RED揭示非零launch缺原生stderr、缺少state值会丢诊断字段、清理重新绑定可能把失败identity门槛改PASS；最小修复分别保留原生证据、先保存身份摘要、锁存identity FAIL。晚到有效身份只可用于退出。

失败清理先完整核对argv/exe路径与摘要/state/guard/唯一环境/PID-start，持久化stopped预算与停止意图，再重新读取PID/start后停本批unit。未知归属不停止；完整启动成功回执仍需exec、identity、listeners、application、quiet_audit全部PASS。保留原0602faaf未成功start时的严格退出分支。独立Standards首审另发现成功start后的退出分支遗漏完整归属、durable意图与复查；先保存两项测试的6个RED断言，随后复用StartupProbe完整检查/停止顺序，独立exit-observations保持原startup观测不变，未知身份仅停止预算而不停止进程。此补修33/33合成通过、尚未Linux实测；首审FAIL和不完整Spec记录保留在[原审查](assets/cpar-build-linux-start-20261013/review-initial.md)。第二轮Spec终态另以纯内存复现审计最后一次等待期间出现事件却仍PASS；先留1项RED，再在最终identity/listeners检查后复查audit，任何非空即阻断，不改65秒阈值或等待策略；最终34/34通过，未Linux重跑。第二轮Standards PASS/Spec FAIL原件保存在[第二轮审查](assets/cpar-build-linux-start-20261013/review-second.md)。新控制器把真实system-gate的摘要和PASS绑定到正式transport；PARTIAL即使有匹配freeze和审查回执也不能观察材料或发送。新的材料确认仅一次，不继承旧7次等待预算；不足整小时即BLOCKED。

最终命令 `python3 -B docs/reports/assets/cpar-build-linux-start-20261013/tools/full-flow-tests.py` 为34/34、exit0，外部systemd/proc/clock/HTTP明确为LOCAL_SIMULATED。`python3 -B docs/reports/assets/cpar-build-linux-start-20261013/tools/linux-oracle-tests.py` 为3/3、exit0，仅重放已有脱敏真实回执，不能称Linux复测。所有RED、中间GREEN及命令/raw/gzip摘要保留在[验证命令](assets/cpar-build-linux-start-20261013/verification-commands.json)与资产中。

## 三次真实 Linux 运行

既有new-vps保护根内，三个独立synthetic目录、unit、state与随机合成钥匙，单个disabled合成Build账号；没有读取或复制真实账号凭据。使用固定候选binary和guard，每unit增加PrivateNetwork=yes；只有lo且不同于host namespace，IPv4/IPv6 TEST-NET返回ENETUNREACH，未改变全局网络或其他服务。准备CLI只有create/validate/publish，无Provider流程。

| 运行 | 实际启动与观测 | 原始测试状态 | 资源退出 |
|---|---|---|---|
| 1 normal | launch0、Type=exec、PID2225925/start680898110，完整身份和监听归属；GET200；真实65.4558秒审计平坦 | PASS，首个system-freeze版本 | PASS；PID消失，unit未加载，停止后5秒平坦 |
| 2 preset exec failure | launch1；首次MainPID0/LoadState=not-found，未取得预期ExecMainStatus203；原生stderr仅有摘要，确切拒绝原因UNKNOWN | FAIL；不猜测或补203 | PASS；没有当前unit/PID |
| 3 preset wrong management key | 新capture版本launch0、PID2237084/start680957878、身份/监听/隔离通过；首个GET404后立即停止，无重发 | 原FAIL（oracle预期401）；依产品404契约离线重放PASS，原FAIL不改 | PASS；完整绑定、预算/意图fsync、PID/start复查后停止 |

准确实际命令为 `python3 -B docs/reports/assets/cpar-build-linux-start-20261013/tools/system-control.py 1`、`2`、`3`：第1个local exit0，后两次exit1；各自远端脚本SHA、stdout/stderr摘要与结果在[run1](assets/cpar-build-linux-start-20261013/system-1.json)、[run2](assets/cpar-build-linux-start-20261013/system-2.json)、[run3](assets/cpar-build-linux-start-20261013/system-3.json)。systemd-run调用3次，观测到创建unit和执行gateway两次。另一次本地第3槽调用被原controller的stop锁在SSH前拒绝，未占槽/未启动；回执单列。

run2原controller stopped及两个stopped零Provider预算保持原样。在核实预设负例已无unit/PID、resource_exit PASS且无越界后，使用用户授权内尚未分配的独立第3槽，另存third-stage/freeze，不重开前两次。run3之后没有第4次系统运行，没有重复GET。系统预算3/3已耗尽；后续只做离线修复与只读退出检查。

错误管理Key返回404有固定候选源码依据：management_security.rs的key mismatch和management_denied_response。没有放宽身份、吞掉异常或改变产品认证。缺失binary负例改为存在且有执行位的无效格式文件，避免把客户端路径检查当内核exec失败；这个修正未在Linux运行。最终观测字段/失败锁存修正也未重新上传。完整系统门槛[system-gate](assets/cpar-build-linux-start-20261013/system-gate.json)保持PARTIAL。

最终额外只读退出检查exit0：[fresh-exit](assets/cpar-build-linux-start-20261013/fresh-exit.json)确认三unit均PID0/not-found、旧PID消失、19180/19181无host监听、5秒审计平坦、私有权限保持；各合成DB事件/账本新增0。新旧预算均stopped；生产PID499070/NRestarts0、启动时刻、exe、unit/current/Caddy摘要与原退出观察一致。本次没有读取生产账号/钥匙或用管理HTTP重新确认active配置。此为资源/进程/文件摘要证据，不是正式账号资格。

## 正式诊断状态及下一道门槛

全新计划epoch `chat-json-linux-20261013` 已在本地停止，远端private/bin及正式unit均NOT_CREATED。新的17个payload文件、实际driver字节、manifest/binary/metadata、37项派生launch与原请求SHA分别冻结在[prepared-freeze](assets/cpar-build-linux-start-20261013/prepared-freeze.json)；它明确PREPARED_ONLY/BLOCKED，不沿用旧freeze冒充执行身份。没有正式上传tar、材料读取、账号复制、正式launch、Pin或Chat。

同授权连续性/剩余一整小时、正式进程/监听/候选池/65秒门槛均NOT_RUN，因更早system gate PARTIAL而停止；实际账号材料、runtime/lease/execution identity保持null，不猜测。旧source DTO摘要缺失/null兼容与候选runtime及fresh Pin真实lease严格校验继续分离，完整合成流程已覆盖。实际lease只有fresh Pin证据可建立，本批未建立。

本批新增授权Pin1+Chat1共2意图，实际0/0/0；未用2随本地停止封存，不恢复历史预算或转移。历史35意图/34网关及Pin确认发送独立。新正式Request/Attempt/Finished/Usage/ledger/native捕获均NOT_CREATED；网关/Pin新确认发送0，Provider独立接收数与费用UNKNOWN。没有本次Chat结果，旧502及旧启动具体根因均UNKNOWN；不标NOT_REPRODUCED，不启动六组合矩阵。

下一步需要新的、明确有界的零Provider Linux验证授权，实测修正后的exec失败负例及最终观测/清理代码，先取得完整系统PASS。此后才可在独立Standards/Spec终态通过、新冻结且同源材料大于整小时条件下，另行建立正式诊断资格；不能复用本次已停止的意图或系统槽位。成功启动后的正式退出补修及审计末端检查也属于下一批实测范围。无需重建未变化产品，不主动刷新/重新授权/换账号/模型。

## 审查、GitHub及交付

本报告冻结版尚待独立Standards/Spec终态。按固定base9aa08db6与本批提交head/完整文件集进行两轴审查，保留所有FAIL/PARTIAL与后续收口。最终回执及提交号在交付附记中记录；两轴结论不升级system PARTIAL。

本批计划将经核对的脱敏结果追加到#47/#57/#9的既有progress，保留完整前缀及正文/AC/元数据/依赖/OPEN，并独立GET读回。本冻结版GitHub同步尚NOT_RUN，后续实际结果单独归档。remote CI NOT_RUN，源码/报告仅本地提交；无push、发布、部署、生产变更或矩阵。
