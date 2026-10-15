# CPAR-38 serving snapshot 声明链路定位与条件式诊断

本批基线 `0e0c3e7f035cc108294a4250d402c3e78d6a0c36`。已确认两个缺口：采集器没有处理第二次 Client Key 读取错误/分页并丢失 effective 页上下文；新隔离实例复制了已经过期的 durable catalog，在后台发现关闭时仍由该继承状态限制模型可见性。本地真实管理 HTTP/runtime 已复现这条路径，隔离准备最小修复后取得 serving 声明。历史第二次 Key/effective HTTP 页未被保存，其实际读取结果仍 UNKNOWN，不能把本地复现写成历史 HTTP 原件证据。

本批已完成一次新正式诊断：**fresh Pin PASS（1次）；原 Chat JSON 1次返回 HTTP502 / UpstreamProtocolError，诊断 REPRODUCED，拒绝类别 unknown_extensions。** 新epoch已永久停止，主退出及独立最终读回PASS。实际新意图2、网关/Pin确认发送2，未重试、刷新或追加调用。声明链路缺口已在新运行中关闭；完整协议验收仍未通过。

## 基线、规则与证据复用

已读用户跨项目规则、项目 AGENTS.md、docs/agents/issue-tracker.md、CONTEXT、相关 ADR/契约、cross-boundary log 尾部及最新 cpar-build-linux-final-20261013 报告。HEAD 与用户检查点一致，不回退；用户 AGENTS.md 和无关 tracked/untracked 保留。GitHub #47/#57/#9 均 OPEN，progress 原文、票面AC及依赖已刷新；当前HEAD无远端Actions运行证据，远端不识别该本地提交，空status不能解释为CI正在运行。

产品源码仍为 `bf822b14a62c21cf582a7539e3635f69bd8b7677`。本批 Rust 只在 cfg(test) 模块新增管理 HTTP/runtime 回归；移除该新增测试后 runtime.rs 与产品源码逐字节一致，其他 apps/crates/build/Cargo/工具链没有变化。原 ARM64 binary SHA `cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e`、manifest `655c037df68c5657b250105bbb07bed665210a7f6b20ea8d8b88aab488157a56`、metadata `d5e87f208ec6ab4265a272d4be5d331bbb0865e30abfa62fa6556671d96cb78c` 已重哈希匹配。

复用 Task42/42、Phase45/45、真实 Linux 系统4/4与未变化系统设计/历史回执证据；startup_probe.py 与 launch_recipe.py 原字节一致，remote-start/exit 只换新epoch路径。新 release build0、系统槽重跑0、安装0；旧FAIL/PARTIAL及停止预算不覆盖。复用是原范围证据，不是新增候选、CI或发布证据。

## 根因与历史未知边界

受控只读核对旧停止epoch：unit not-found/MainPID0，budget stopped，Pin/Chat意图0；原inspection SHA `7d4096fbfa6537d786a074b5695031e15148cd6dda306b505e77d8aebc74fce1` 本轮实际重哈希匹配。只读取必要字段与无秘密SQL列，没有管理HTTP、账号材料、运行启动或Provider请求。

旧DB中的目标Key/group/grant/candidate均有效，配置 `cpar-build-config-20261010` revision9，候选明确 `allow_unlisted_model=true`。同账号目标catalog观察于2026-10-10，过期于 **2026-10-13 18:47:24.152 Asia/Shanghai**；旧正式启动为 **2026-10-13 23:56:53.443 Asia/Shanghai**。旧DB/inspection只读前后摘要保持。

实际链路：Client Key解析→真实管理HTTP `/admin/models/effective`→SnapshotManagementRuntimeFacade选择scheduler serving snapshot→exact upstream model唯一解析及hard eligibility→sources→同版本execution_configuration生成capability_evidence→采集器归一化→remote-pin唯一声明断言。catalog已过期会移除模型，尚未到source声明生成。Key或serving版本不可用分别是404/503，不能混同items空；生产mapper有source时缺execution_configuration会省略声明字段，不能把消费者读出的null当作wire原值。

采集器原逻辑的第二次Key错误未检查、没有翻页；effective页仅循环后丢弃；配置稳定性读取早于该链路。原 `capability_read_errors=[]` 因而不证明读取成功。单纯缺capability_evidence会追加一条未知声明，并不能单独解释归一化数组长度0。public capabilities={}也不能作为根因。

根因机制已由相同产品源码的真实管理HTTP/runtime复现；旧运行的第二次Key结果、effective是否被请求/成功、items/source数量、serving projection_id仍 UNKNOWN。过期状态是已证实的充分阻断条件；缺失的历史HTTP事实没有补造，旧502精确根因仍UNKNOWN。

## 最小修复及本地验证

1. 采集器保留逐页HTTP状态、响应字节摘要/长度、cursor摘要、Key/group/config/projection上下文、模型/source计数以及声明missing/null/object状态；处理第二次Key分页、错误、循环、页上下文变化，并把配置末尾核对放在整条读取之后。新增明确完整性与目标Key枚举门槛；仍严格要求唯一完整声明及所有身份/能力绑定，不填默认能力。
2. 仅新隔离副本的准备层：对同配置、同候选/Key/grant、同已批准账号的已过期继承catalog，先在新私有epoch fsync归档，再从新副本分离三张派生catalog表的该目标记录。原DB与旧证据保留；新鲜cache、错误owner、缺显式例外或错误Key拒绝。没有写生产、改变模型或扩大Key权限，没有修改Rust产品catalog owner/expiry约束。
3. 从准备后的新副本启动全新runtime；继续沿真实编译后的serving source读取能力。它表示既有显式配置例外，不冒充新目录发现或Provider能力验收。已编译旧snapshot不因DB准备自动重编译；初次同进程reload尝试的FAIL日志单列保留，最终回归使用真实fresh bootstrap。

原collector RED、编译/测试夹具问题、HTTP/runtime RED及full-flow初次错误owner负例FAIL全部保留。最终受影响验证在本批assets记录：collector定向9项（含多组子例）、完整实际控制器/阶段39项（声明链路15组负例、分页成功、cache范围拒绝）、formal oracle54项、真实管理HTTP/runtime1项、原native catalog owner隔离1项。所有本地测试是LOCAL_SIMULATED/真实本地HTTP-runtime范围，不是真实Provider验收；负例在reserve_pin前停止、无推理POST、预算锁停、重入拒绝、资源退出由实际阶段程序核验。

可选远端零Provider新观察运行：本地与旧证据足以定位，NOT_NEEDED，已用0/最多1，不启动旧epoch或借用旧预算。

## 独立前置审查与正式epoch

独立Standards/Spec前置 **PASS/PASS，硬阻塞均0**，固定base `0e0c3e7f…` / head `3163542007fb4fab56a64f34d1bf17f573a5dd5a`。独立重哈希核对18项payload、实际19个tar成员、控制器和复用证据。实际payload freeze `13c1404942aa62215ebc3f89be090b2c27ea473f6f5a4d1001ef56e80cfcba72`；冻结后执行源未改变，读回helper修正没有改变执行载荷。

新epoch `chat-json-serving-20261015`：一次同授权只读连续性观察PASS，余效7330.966秒；prepare完成回执计算余效7283.987秒，严格>3600。没有等待续期、主动refresh、重新授权或换账号。18/18上传通过；新副本中5条过期catalog记录先私有归档再分离，archive SHA `1c46ebfa4e37e65d2e80606183213eab6395845e43722755984a82cf39ac5901`。旧DB/缓存/预算保持原样。

正式启动1次PASS：PID972264 / process_start696962272，五门PASS，19180/19181监听归属准确，实际静默65.465481秒；后台Provider任务disabled，静默期间网络事件0、catalog请求0。发送前独立绑定实际binary/manifest/source、argv、guard、ClientKey wire摘要及原请求SHA；没有把该预期绑定当作fresh lease证据。

Pin预留前、Pin之后、Chat发送前三份真实管理快照均独立读回：Key枚举完整1页、目标Key唯一；effective HTTP200/1页、models1/source1、声明object，capability_read_complete=true，configuration_stable=true。唯一声明来自serving_snapshot，配置revision9，projection `d7775c19a4e6e01e2bcc67df31e61e534784838a1daba34422b5e495f2f5dafa`，完整六布尔值与Key/route/candidate/endpoint/grok-4.5严格绑定。public capabilities={}没有被用来填充能力。

fresh Pin一次成功，attempt_count1、upstream_sent=true。**实际lease revision153** 从Pin execution.identity读取，并与Chat Attempt sidecar独立一致；native account152和候选pool153分别保留，不以它们推算lease。

唯一原Chat请求结果：网关HTTP502 / UpstreamProtocolError，安全拒绝类别unknown_extensions；原生上游transport HTTP200/Json/Gzip，673字节完整捕获 SHA `7b5a3c2766766397af638665aaa7160f351780232a2c06f11d8f784ebbfdaa96`。本次Request `p1-request-71cc49273ebb7382fa61cd5f984e8c51-0` 已失败，唯一Attempt succeeded/retry_decision completed，说明失败位于响应投影/输出阶段。新Request/Attempt/Finished/Usage各1，账本1且source-event精确一致；native input228/output159/reasoning158/cached128，其余缺失计数保持null，账本cost仍null。只公开闭合结构、计数及摘要，没有输出原生文本或原始私有响应。

本次502已复现并定位到响应元数据投影的拒绝类别；具体哪个扩展字段尚未做原生→Canonical→Chat的逐字段离线确证，不能将该类别扩张成精确字段根因，也不能回填旧502的缺失原件。本批不修改该产品投影实现，不追加真实请求。

原Chat字节SHA `245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062` 不变：POST `/v1/chat/completions`，grok-4.5、stream=false、max_completion_tokens=4096。fresh lease只能来自新Pin实际执行证据。首个正式失败、UNKNOWN或矛盾永久停止；不重试/fallback/换账号模型/刷新/修复后追加。没有Chat结果不能写NOT_REPRODUCED。

## 退出与独立读回

Chat结果产生后预算永久stopped，2/2新意图已用，无未用额度。身份绑定停止意图、PID/start复查、真实stop与5秒审计稳定均PASS；unit not-found/MainPID0，PID/cgroup/监听无遗留。生产PID499070/NRestarts0、unit/current/binary/Caddy摘要保持。网关/Pin实际确认新发送2；历史35意图/34确认发送独立，累计37/36。Provider独立接收数与实际费用仍UNKNOWN。

补充只读helper先有两份FAIL：第一次把base相对清单路径误拼到epoch，第二次把结构化sha256/bytes/mode事实与字符串比较；原脚本、实际wrapper、stderr和回执均保留。错误发生前unit/PID/cgroup/stop/startup证据已通过，不是新增执行失败或调用；仅修正helper清单投影，执行驱动与freeze未变。最终formal-final-readback-final PASS：59项不可变捕获重哈希、原exit公开投影精确相等、SQLite持久事件/账本/catalog/account SQL一致、三份实际声明快照完整、权限和历史文件摘要保持。SQLite checkpoint/WAL文件生命周期与退出时清单摘要单列，不把捕获时文件摘要当作之后的数据库字节承诺。上述读回新增启动/Provider均0。

## 分层状态与交付

| 层 | 当前状态 | 边界 |
| --- | --- | --- |
| 根因机制及修复 | CONFIRMED；新运行声明链路PASS | 过期继承catalog限制可见性，采集器错误/分页盲区修复 |
| 旧HTTP具体读取结果 | UNKNOWN | 原始第二次Key/effective页未留存 |
| 本地受影响回归 | PASS | 详见各原始日志和local-validation回执 |
| 产品/系统证据 | PASS_REUSED | 同产品source/artifact；新release build0/系统run0 |
| 新远端零Provider观察 | NOT_NEEDED | 0/1 |
| 正式启动 | PASS1次 | 五门、实际65.465秒静默、同源身份 |
| fresh Pin | PASS1次 | 唯一attempt及实际lease153 |
| 原Chat JSON | REPRODUCED / HTTP502 | unknown_extensions；没有保真PASS，不标NOT_REPRODUCED |
| Standards / Spec前置 | PASS / PASS | 固定31635420及实际载荷；不等于Chat验收通过 |
| 资源退出 | PASS；独立最终读回PASS | unit/PID/cgroup/监听释放，预算永久停止；两份helper FAIL保留 |
| Provider独立接收/实际费用 | UNKNOWN / UNKNOWN | 不以网关计数代替独立证据 |
| GitHub更新 | PASS3/3 | 既有progress各body-only PATCH1次，独立comment/issue/分页依赖读回，三票OPEN |
| 远端CI、发布、生产、六组合 | NOT_RUN | 无push/dispatch/部署/生产修改/矩阵 |

下一轮协议矩阵资格仍BLOCKED；声明链路修复及单次诊断都不能代替六组合、扩展、父票、远端CI和正式交付条件。

GitHub既有progress [#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490) 已完成授权追加，旧body逐字节前缀保持。独立GET确认完整新body等于准备载荷，issue正文/AC/state/labels/assignees/milestone等保持；依赖分页均1页，数量8/23/0，OPEN数0/11/0，边与状态保持。仅#47的updated_at随本次授权comment修改更新，按独立issue读回精确绑定；没有忽略依赖状态变化或关闭票。

执行审查点31635420的Actions匹配run0，Checks因远端不识别本地提交返回422；记录为NOT_RUN，没有把空pending当作正在运行。没有push/dispatch、发布/部署/生产变更。最终静态核验用户AGENTS摘要保持，209份本地历史文件内容/权限不变；远端历史保护由prepare/start/exit及最终读回核对。实际原生捕获、凭据和私有快照仅保留在0700/0600保护范围。

最终独立交付核验固定31635420→f72dddb1：**Standards PASS（硬缺陷0/建议0）；Spec本批执行及材料契约PASS（新增阻塞0/scope creep0），完整协议验收FAIL（本次Chat502），整票BLOCKED/未完成。** 两路终态均complete，实际payload/原始freeze/日志/读回wrapper/GitHub历史与依赖得到独立字节核对；未重跑测试、SSH或Provider。此次最终审查和本地提交不升级真实Chat、矩阵或发布资格。
