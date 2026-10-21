# CPAR #62 原样 Codex 组合受控验收：装配交付与明确阻断

交付日期：2026-10-22（Asia/Shanghai）；执行批次始于2026-10-21，保留原批次路径。
**本轮结果 PARTIAL / BLOCKED，原样 Codex 正向验收未通过。**
已实施独立组合装配、固定原生客户端 stdio 驱动、严格工具和逐请求证据断言、Codex 专用失败／取消入口，
保留原未资格 400 与 Pi 三场景。实际运行发现原契约之外的历史表示和沙箱诊断值，未擅自放宽准入。
具体有界补充已提交用户选择，目前没有明确答复；本报告不将驱动代码存在、Full 或发送前拒绝记作客户端成功。

最终不可变代码候选：`06b7c1885af7ac02e27d2484645e0a2d9c33c01d`。
此前装配提交：`b8677f9951a67ca91eec47398649129b7b738774`。
本报告及精选证据是后续文档交付，不把代码验证改绑到报告提交。

| 范围 | 本轮结果 | 精确边界 |
| --- | --- | --- |
| 原样 Codex 正常 | FAIL / BLOCKED | 最终原生进程 exit=0，但 turn 为 failed，HTTP400、receiver/Attempt/Usage event=0，无工具结果或 42 文件 |
| 原样 Codex 失败 | BLOCKED_PRE_SEND | 独立运行仍 HTTP400、receiver/Attempt=0；未进入要求的真实上游 Attempt 后失败 |
| 原样 Codex 取消 | BLOCKED_BEFORE_DELTA | 独立运行 HTTP400，无 delta、lease=1 或 interrupt；清理退出不是取消成功 |
| 未资格 Codex 负例 | PASS_REJECTION | 独立旧装配，HTTP400、ClientRequestError，receiver/Attempt/Usage/ledger=0 |
| Pi 正常／失败／取消 | PASS_BOUNDED_CONTROLLED | 固定原版 Pi；五请求闭环、独立失败、同 PID 取消；既有失败用量展示限制保留 |
| 最终 Full | 45/45 PASS，exit=0 | 1636 passed / 0 failed / 17 ignored，130 suites；候选＋18 项继承覆盖 |
| 受影响 Responses 专项 | 77 passed / 0 failed / 2 ignored | 开发阶段专项；最终候选 Full 另覆盖新增组合门槛回归；不将 ignored 算实际 CLI |
| Standards | PASS_STATIC，0 硬性违规 | 初候选独立审查，1 项非阻断 P3 重复代码判断；最终增量 lint／静态检查通过 |
| 本轮 Spec | PARTIAL / BLOCKED | 一项新发现的组合门槛缺口已修复；正常／真实上游失败／取消完成条件仍未达到 |
| 整票 #62 | OPEN / PARTIAL / BLOCKED | #61 仍 OPEN 且为原生 blocker；本轮不关闭工单 |
| 固定 Build profile 1.0.50 | BUILD_ACTIVATION_BLOCKED | stock ledger、默认 builder/adapter、正常生产装配未改变 |
| 真实 Provider／远端 CI／发布 | NOT_RUN / NOT_RUN / NOT_RUN | 无真实或付费账号调用、push、PR、工单写入、合并、发布或部署 |

## 1. 实际基线、规则与保留

从指定 worktree `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020` 开工，
实际分支 `feature/62-client-metadata`、起始 HEAD `e8a669b67bebffc09727f749d6e5116d037767a1`。
给定 `a786cc72…` 已在历史内，仅用于定位证据，没有回退。没有从旧 repo 目录开工。

读取实际 AGENTS、CONTEXT、issue-tracker/domain、cross-boundary 尾部、指定三份报告／方案、
ADR-0115 至 0119、BC-PROTOCOL-009 至 013 及必要的客户端、生命周期、C4 读回接缝。
18 项既有 dirty/untracked 内容先备份原字节、mode、uid/gid、symlink；起始 index 为空。
两个本地代码提交只包含本轮 8 个路径。没有 reset、clean、stash、整体暂存或纳入继承内容。
[保留回执](assets/cpar62-codex-combined-20261021/preservation.json) 逐项确认 18/18 保持。

GitHub 起止只读核实：#62、#61 均 OPEN；#62 原生 blocked_by 为 #61，#61 的 #59/#60 已 closed。
远端 develop/main 仍为 `c39bc07b…`／`c6a981b0…`，本地功能分支没有远端 ref。
[工单快照](assets/cpar62-codex-combined-20261021/github-issue-snapshots.json) 和
[最终原生依赖](assets/cpar62-codex-combined-20261021/github-final-dependencies.json) 没有对应写入。

## 2. 已实施的具体装配与驱动

- `lifecycle/parallel.rs::fixture_for` 只扩为 sibling 测试可见，复用原受控 compiler。
  新 Codex 正向／失败／取消场景显式同时设置 namespace、search、input ID、parallel 四项资格，
  提供自有 loopback transport；旧 Codex 未资格场景仍用原 lifecycle_fixture。
  请求、URL、header、User-Agent、外部沙箱配置均不能产生产品测试资格。
- 路径继续为实际客户端→真实公开 listener／鉴权→实际 RouteCompiler／生产 factory→实际 leased
  Build adapter→独立 TCP receiver。固定 Build URL 和生产能力默认值保持。
- Python 使用固定原生 Codex 的 `app-server --listen stdio://`；这是实际安装的 CLI binary，
  不是 SDK 或手工 Responses 请求。`initialize`、`thread/start`、`turn/start`、`thread/resume`、
  `turn/interrupt` 均按固定版本协议驱动，原始 RPC 输入／输出分别保留。
- 成功驱动预设为首 turn 的 exec_command→真实结果→write_stdin→真实结果→回答（三次 HTTP），
  第二 turn 问答一次，再正常关闭、启动新原生 PID、按实际 thread id 从磁盘 resume，第三 turn 一次。
  合计仍为三个完成 turn、五次 HTTP。**该完整路径尚未实际完成，不能将预设当运行结果。**
- peer 的 exec_command 读取 seed=41，保持真实 PTY 进程等待；下一响应只从实际工具结果文本中的
  `Process running with session ID` 提取整数，再调用 write_stdin 输入 `1\n`，由该真实进程计算／
  写入／读回结果。测试进程只准备 seed，不代写 result、不伪造工具结果或 session_id。
- `tool_loop.rs` 精确检查名称、namespace 缺失、完整参数、call_id、结果全文和 PTY 换行、顺序、
  每轮原样重放、三完成 turn、两次正常进程退出、不同 PID 磁盘恢复以及文件 `42\n`。
  Codex 失败／取消另有原生终态、零工具执行、无重试与同 PID 活性判据；不借用 Pi 结果。

## 3. 逐请求证据与六项门槛

公开捕获恢复完全相同请求字节，保留 request/response 原始文件、公开 response headers 和完整性标志。
独立 receiver 增加 ordinal、实际返回 response id、非凭据 header 观察；Authorization 值被遮蔽。
不修改真实请求字段或执行语义。

`cli/evidence.rs` 逐条连接实际公开 `x-request-id`→durable Request→唯一实际 Attempt；
公开 `response.created.id`→独立 receiver receipt→Usage.response_id／attempt_id→ledger.source_event_id。
C4 从原始请求 RawExtensions 提取，使用实际 owner、Request 和 Attempt 授权解密，比较完整 raw JSON，
检查 foreign owner／错误 Attempt 拒绝。不是将已解析 Value 再序列化后冒充原字节。
字段保留断言包括 tools、parallel、choice、reasoning/include、完整 input 次序和 ID presence/value；
只对既有 instructions→developer 及 message string/type 表示作精确协议投影。
证据 join 和 ledger materialization 在严格场景计数断言前导出，后续失败也不丢已观测关系。

独立 Spec 审查发现原正向门仅比较两侧字段相等，可能接受共同缺失。`06b7c188` 修复为：
至少一个实际正向请求同时有 multi_agent_v1 namespace、精确 web_search(false)、非 assistant 输入 id、
client_metadata、parallel=true；后续 presence 可变化，逐轮记录。新增缺失／错误字段门槛回归通过。

最终默认严格 runner 的六项结果：

| 判据 | 结果 | 实际观察 |
| --- | --- | --- |
| 客户端正常结束 | true | 原生进程 exit=0；这独立于 turn 是否成功 |
| 三个完成回合 | false | 只有一个 failed turn |
| 五次增量公开 200 | false | 一个 HTTP400 |
| 五次独立接收和精确 accounting | false | receiver=0，durable Attempt/Usage event=0；完整场景 accounting oracle 未到达 |
| 两个准确关联工具结果 | false | 无结果 |
| 真实 result.txt 为 42 换行 | false | 无该文件 |

`observations.json` 因严格断言失败而缺失，会明确列为 missing receipt，不能被默认值补成成功。
未资格负例有独立完整 accounting，不能覆盖正向缺失。费用与 #61 成功 companion 始终 UNKNOWN。

## 4. 实际抵达的新阻点与授权边界

13 份完整一手源码／schema 已按固定 URL、revision、抓取日、字节数和 SHA-256 冻结，见
[source-manifest](assets/cpar62-codex-combined-20261021/source-manifest.json)。
Codex 固定 commit 为 `979011409de0a60b52f179721948e65531d26144`。
公开文档没给出的 publication date 保持 null，没有把抓取日期当发布日期。

| 原始运行 | 观察 | 限定结论 |
| --- | --- | --- |
| 未修改驱动的 fresh baseline | preparation/Pi 通过，Codex 六项均 false | 原严格 RED 保留；不是能力通过 |
| 组合装配 driver-2 | 首请求 200、receiver=1，真实 exec 开始，但 openpty 被外层写入隔离拒绝 | 字段组合可以到达受控 Build；工具闭环未成 |
| driver-3 | 只允许内核 PTY 设备后，真实工具返回内部 sandbox_apply EPERM；续轮 HTTP400 | 外层继承 Seatbelt 与客户端内部 sandbox_apply 有实际冲突；没有通过放松普通文件隔离绕过 |
| 同次真实续轮 | reasoning 无 id，content/encrypted_content 为 null；function_call_output 有自身 id | core 要求 reasoning id，Router 的结果扩展仅允许 name/namespace；后者是同份原请求里的独立静态阻点，未声称已经动态越过前者 |
| external-red／最终三个 Codex 场景 | 原生 externalSandbox/restricted 被 CLI 接受；实际 C4 发送 external／external-sandbox，公开 400、receiver=0 | 这些值必然违反当前 C4 selector 合同；不是已验证的工具执行或完整唯一根因证明 |

[当前 C4 契约](../contracts/BC-PROTOCOL-010-local-codex-client-metadata.md) 只允许 sandbox=seatbelt、
sandbox_mode=workspace-write；当前 `ReasoningHistory` 要求 id。公开 create schema 的 reasoning id
也仍 required。固定 Codex 源码能证明实际序列化形状，不能单独授予新的本地协议或固定服务语义。

用户明确要求遇到未知字段新语义／产品决定时说明具体阻断，不静默扩大契约。因此尚未修改
core、Router、Build builder 或 C4 准入。待选择的具体补充见
[有界草案](assets/cpar62-codex-combined-20261021/compatibility-proposal.md)：

1. 单独局部资格的无 id、仅 summary 的 reasoning 历史；原样保留 absent/null，不生成身份或假密文。
2. 工具结果自身 id 的有界原生保留、跨 item 唯一性与独立资格；仍由 call_id 精确关联。
3. C4 外部沙箱值配对仅作完整加密本地诊断，不提供权限、隔离证明或 Build 资格。

即使批准这三项，仍须继续真实续轮和失败／取消验证；不能预先保证没有后继阻点。

## 5. Codex 分场景与 Pi 回归

最终三个 Codex 入口均用同一候选和固定 native binary，分别保留独立原始回执。
正常：exit=0／failed turn、HTTP400；失败：exit=0／failed turn、HTTP400，未进入真实上游 Attempt；
取消：HTTP400 后未收到 delta，驱动等待 delta 超时（原始 driver_error 为 Empty），没有 cancel-ready
或 abort-complete，未发送 turn/interrupt。随后 stop_process 仅清理，不记作取消。
原生取消协议入口有固定源码与实际 CLI schema 依据，但完整运行证据仍 BLOCKED。

Pi 0.84.3 的最终回归与 Codex 分开：

- 正常：三 turn stop，五 HTTP200 增量，五独立接收、实际 Attempt、Usage、ledger；两个相关
  read/bash 工具和后续问答、文件 `42\n`，五条逐请求 join 全部通过。另有一个 background400 的
  Request，所以 durable Request／Finished 各6；401 无上游增量。
- 失败：一次受控接收／实际 Attempt／Usage／ledger，唯一 failed、无 completed；20/7 tokens 精确。
  Pi 失败界面的默认零用量显示仍是既有限制，不能推断零消耗或费用。
- 取消：实际 text delta→真实 lease=1→RPC abort→同 PID 92377 一直存活→独立 peer close=1、lease=0，
  均在客户端／server 清理之前。观测上界7ms，一秒 fast probe 与20秒 bounded eventual 分开记录，
  均通过；没有修改超时或新增产品 SLA。Usage/ledger 行数0，不等于已知 Provider 零用量或零费用。

实际 C1–C6／parallel 覆盖必须区分阶段：开发阶段的首个成功请求已观察原生声明／input ID／
parallel=true 到达独立 receiver；最终 externalSandbox 场景止于发送前。未完成原样 Codex 的五轮组合。
hosted search、namespace 子工具均 NOT_CALLED；parallel=true 的声明／控制不等于同响应多工具执行。
既有公共合成 JSON/SSE 组合、ParallelTools 多调用与 C1–C6 回归在 Full 中保持，不能替代实际 CLI。
[字段观察](assets/cpar62-codex-combined-20261021/actual-field-observations.json) 逐轮保留实际 presence。

## 6. 隔离、客户端身份与最终绑定

临时 HOME/CODEX_HOME/工作目录，合成凭据，继承外层 Seatbelt。每个场景实测 IPv4／IPv6 非 loopback、
真实 `/Users/huangrui` 读取、临时区外普通文件写入均 EPERM。内核 PTY 例外只含 `/dev/ptmx` 和
数字 `/dev/ttys…`，没有开放真实用户数据目录或普通持久文件。externalSandbox 是客户端执行入口，
不代替这些 OS 探针，也不由 C4 声明取得资格。

Codex launcher 和实际 native 均为0.161.0；launcher 既有 vendor fallback 所用的同一 Mach-O binary
现在直接启动，以便 PID 活性对应真正客户端。Pi shell／声明 bin 均0.84.3，Node v26.8.2。
12,564 个安装文件／link 的前后 manifest 完全相同；only-my-pi 仅辨识、未执行。

| 绑定 | SHA-256 |
| --- | --- |
| Codex 实际 native | `12ac11d2c7eee27cfae34393986d7b7c9ed0dea537cb749831cdd7033893e6de` |
| 最终 runner | `a081d3c5e97eba680caa0094db633118dc7807b2a5ea02d051e853a18f3dbfbb` |
| Cargo.toml | `1b1a37ce8d8fcafa51d379ffae2ff33fff091c5a48c0f3458e47b99c12b22355` |
| Cargo.lock | `36564c0219707741d79ecb37d9fc4ee620ce2fe6801fdbe09b32fb4c8687c858` |
| Full 日志 | `d7da3aaa40edfee34cbf6d5e7692aaf0dbe22bd5a568a5718c832f9cfc9ab024` |
| Full 实际 gateway test binary | `5e447c0a6fed5462c817460f1ff27fe39a07fcf8675238aa2090fc9db2ba70e3` |

[candidate-binding](assets/cpar62-codex-combined-20261021/candidate-binding.json) 明确记录18项继承覆盖。
初候选 b8677f99 的 Full 在独立 validation worktree 通过45/45、1635/0/17；Spec 修复后最终 Full
在冻结的 source worktree 上运行，7,226 项输入与 detached validation 副本逐字节相同。
最终 Full 和三次实际 CLI 运行前后，全部7,226项源码／manifest／覆盖输入 hash、mode、link 相同。
这是“候选＋明确覆盖层”的 Full；没有另称 bare-tree Full。

Full 和 package-only 实际 CLI 的 feature unification／binary 可能不同，分别保存其路径与 hash，
见 [实际 CLI 汇总](assets/cpar62-codex-combined-20261021/actual-cli-summary.json)。
Full binary 已独立留存，没有把 package runner binary 冒充同一文件。
生产 runtime/default builder/C4 准入代码及锁文件逐字未变，见
[生产边界绑定](assets/cpar62-codex-combined-20261021/production-and-lock-binding.json)。

## 7. 复现命令、历史失败与交付限制

在指定 source worktree，使用其自身 manifest/lock；不设置 test-thread、RUSTFLAGS 或超时放大项。
输出参数必须是 OS tempfile.gettempdir() 下尚不存在的新目录；端口、临时配置与合成 key 由驱动生成。

```sh
CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-codex-combined-20261021/target-full-final ./scripts/check.sh full
CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-codex-combined-20261021/target-full-final python3 scripts/acceptance/cpar62-cli.py --require-prompt-release --require-codex-success --output <new-OS-temp-directory>
CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-codex-combined-20261021/target-full-final python3 scripts/acceptance/cpar62-cli.py --case codex-failure --require-prompt-release --require-codex-success --output <another-new-OS-temp-directory>
CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-codex-combined-20261021/target-full-final python3 scripts/acceptance/cpar62-cli.py --case codex-cancel --require-prompt-release --require-codex-success --output <another-new-OS-temp-directory>
```

当前三个严格实际命令均应 FAIL；单独 --case 不取消六项门槛，不能以缺少正常路径回执通过严格门。
source worktree 的8份代码对应最终候选；报告后续仅静态检查，不重跑无变化的行为验证。

所有中间失败保留：OS 临时路径误选导致 runner 预拒绝；旧共享 binary 内嵌另一 worktree 路径，
被隔离规则阻止；fresh baseline 严格失败；Python nonlocal 语法错误；PTY／nested sandbox 两轮失败；
实际历史400；external C4 400；各轮 Clippy 反馈；新增门槛测试的 test 宏歧义编译失败及后续修正。
没有将这些失败改称通过，也没有 retry until green。重复运行均对应源码／驱动或判据变化。
开发中间运行未完整冻结每个 test binary 的，明确不是最终候选证据；原始请求／RPC／日志仍保留。

本地私有证据根：`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-codex-combined-20261021`。
保留416份按显式允许列表复制的原始回执，目录0700、文件0600，不纳入 Git 的原始 C4 请求、
临时凭据配置、数据库或真实用户文件。起始18项备份、完整源码输入清单、原始 Full／失败日志、
初始 RED binary、最终 Full binary 与13份完整来源均在该本地根保留。
[精选证据清单](assets/cpar62-codex-combined-20261021/evidence-manifest.json) 只绑定可审查摘要与 hash。

[独立双轴审查](assets/cpar62-codex-combined-20261021/reviews.json) 的范围是初候选8份增量；
Standards 0硬违规、1项非阻断 P3；Spec 的组合门槛 P2 已在最终提交修复并验证。
最后小增量由主代理静态核对、回归和 Full 验证，没有声称另有一次完整独立审查。

**继续完成原任务的最小输入是对三项有界补充的明确选择。** 当前保持原契约与生产拒绝。
若允许补充，先冻结明确后继契约，再实现必要的最小修复并继续真实工具／历史／失败／取消验收；
若保持原契约，这三项就是具体阻断，不能将本轮标成功。#61 companion、cost_in_usd_ticks、
固定 Build 服务、真实 Provider、CI 和发布条件均不因该选择自动解除。
