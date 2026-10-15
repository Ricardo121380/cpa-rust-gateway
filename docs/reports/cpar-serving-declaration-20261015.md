# CPAR-38 serving snapshot 声明链路定位与条件式诊断

本批基线 `0e0c3e7f035cc108294a4250d402c3e78d6a0c36`。已确认两个缺口：采集器没有处理第二次 Client Key 读取错误/分页并丢失 effective 页上下文；新隔离实例复制了已经过期的 durable catalog，在后台发现关闭时仍由该继承状态限制模型可见性。本地真实管理 HTTP/runtime 已复现这条路径，隔离准备最小修复后取得 serving 声明。历史第二次 Key/effective HTTP 页未被保存，其实际读取结果仍 UNKNOWN，不能把本地复现写成历史 HTTP 原件证据。

截至前置审查准备，新增正式 Pin/Chat 意图及发送均0，真实诊断 NOT_RUN。下文运行部分将在实际门槛和退出回读后填写，不以准备载荷代替执行证据。

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

前置Standards/Spec、实际同授权连续性、prepare严格>3600秒、正式身份/监听/候选池/65秒静默、唯一完整声明及Pin/Chat结果：待本批固定范围审查与实际执行后记录。新epoch为 `chat-json-serving-20261015`，最多Pin1+Chat1/新意图总2；历史35意图/34确认发送及所有旧未用预算独立且停止。

原Chat字节SHA `245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062` 不变：POST `/v1/chat/completions`，grok-4.5、stream=false、max_completion_tokens=4096。fresh lease只能来自新Pin实际执行证据。首个正式失败、UNKNOWN或矛盾永久停止；不重试/fallback/换账号模型/刷新/修复后追加。没有Chat结果不能写NOT_REPRODUCED。

## 分层状态与交付

| 层 | 当前状态 | 边界 |
| --- | --- | --- |
| 根因机制 | CONFIRMED_LOCAL_RUNTIME | 过期继承catalog限制可见性，采集器错误/分页可丢链路 |
| 旧HTTP具体读取结果 | UNKNOWN | 原始第二次Key/effective页未留存 |
| 本地受影响回归 | PASS | 详见各原始日志和local-validation回执 |
| 产品/系统证据 | PASS_REUSED | 同产品source/artifact；新release build0/系统run0 |
| 新远端零Provider观察 | NOT_NEEDED | 0/1 |
| 正式启动、fresh Pin、原Chat | NOT_RUN | 尚未启用新正式epoch |
| Standards / Spec前置 | NOT_RUN | 固定范围独立审查待执行 |
| 资源退出 | 旧epoch STOPPED_VERIFIED | 新正式epoch尚未创建 |
| Provider独立接收/实际费用 | UNKNOWN / UNKNOWN | 不以网关计数代替独立证据 |
| GitHub更新 | 待授权范围内追加并独立读回 | 三票OPEN，局部诊断不关闭全票 |
| 远端CI、发布、生产、六组合 | NOT_RUN | 无push/dispatch/部署/生产修改/矩阵 |

下一轮协议矩阵资格仍BLOCKED；声明链路修复及单次诊断都不能代替六组合、扩展、父票、远端CI和正式交付条件。
