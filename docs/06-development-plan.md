# Rust AI Gateway 详细开发计划

## 0. 计划元数据

| 字段 | 值 |
|---|---|
| 计划版本 | `v1.45` |
| 生效日期 | `2026-07-23` |
| 状态 | `Locked for execution` |
| 当前阶段 | `P1` 至 `P6`、P9、P10 与 P11 已完成；P7 Kiro OAuth 与 P8 Official API-key E2E 仍延后。`phase-p11-remediated-3-complete` 的唯一 P11 GitHub Delivery Gate 已通过 Fast、Full supply-chain 与 Required。 |
| 当前任务 | P11-01 至 P11-08 与 G11 已完成；历史 P11 Delivery Gate 的 pre-receipt `101` 已追溯为管理 SPA `npm ci` 在首个 Cargo 调用之后，修复经无 `node_modules` worktree 与最终 GitHub Delivery Gate 验证。P12-01 是全计划唯一 `IN_PROGRESS` Task。 |
| Rust Workspace | 21-package 骨架已创建并通过 P0-03 验证 |
| 生产部署 | 尚未开始 |
| 行为参考 | CPA `v7.2.80` + 已冻结的 AxonHub/New API/Sub2API/grok2api/Kiro-RS 快照 |
| 已批准变更 | `CR-P1-G1-001`：将 G1 的 Chunk 条件精确为 P1 范围内的 Tool 语义投影一致性；原始 bytes/EventStream 不变性仍由 Provider 阶段验证。 `CR-P3-G3-001`：P3-10/G3 的真实验证公开别名改为 test-only `p3-chatgpt-compat`，不把 ChatGPT-family 上游误称为 `minimax-m3`。 `CR-P3-G3-002`：test-only SSE 单帧有限上限改为 64 KiB。 `CR-P3-G3-003`：仅 P3-10 ignored live profile 的 SSE idle 上限改为 45 秒，其他 transport 边界不变。 `CR-EXEC-001`：缓存化 Full CI、docs-only Gate、单探针诊断 harness。 `CR-EXEC-002`：缓存可见交付引用、补充供应链 Gate 与缓存度量。 `CR-EXEC-003`：Task Card、集中补丁、去重验证、证据模板和时延度量。 `CR-EXEC-004` 至 `CR-EXEC-006`：按风险路由 Luna/默认/高级模型与最低足够思考强度。 `CR-EXEC-007`：P 级开发分支与单次远端正式 Delivery Gate，保留 Task 级本地 review/test，并为 CI/cache 等不可本地证明的变更保留提前远端例外。 `CR-P4-G4-001`：新增非 HTTP、只读的管理状态查询与 403 账户受控恢复，以闭合 G4；认证 HTTP/UI 仍属 P10。 `CR-P6-03-001`：将 P6-03 已授权真实验证改为有限、可审计的模型 × 模式矩阵；每个 harness 进程仍严格只发送一次，不重试相同元组。 `CR-P6-03-002`：在前一矩阵全部得到同一脱敏失败类别后，加入一项不记录值的响应分类诊断和一个显式登记的一次性复测。 `CR-P6-03-003`：在确认 2xx JSON 错误对象后，增加最终一次仅从标准错误元数据映射安全类别的诊断调用。 `CR-P6-03-004`：新增一个与固定直连验收隔离的、服务器本地 grok2api Build 路由代理参考探针。 `CR-P6-03-005`：采用服务器参考的当前 Build 请求轮廓，并只登记新的固定端点 T11 非流式与 T12 SSE 验证。 `CR-P6-03-006`：通过 grok2api 支持的管理 API 导入指定 OAuth 文件并做账号专属额度刷新诊断；不重放固定直连元组，也不把共享路由调用误归因到该账号。 `CR-P6-03-007`：仅以本机官方 Grok CLI 做一次交互式 OAuth 重新认证并记录安全状态投影；不发送 P6 请求或改变服务器/路由。 |
| 已批准变更（续） | `CR-P6-03-008`：以 CPA、grok2api 和 Sub2API 的 clean-room 行为参考扩展 Grok Build 的已知 OAuth 凭据来源；保留标准 JSON/Device Code/Refresh，新增 CPA xAI 文件和官方 Grok CLI indexed cache 的内存导入，不纳入 Cookie/SSO Web 转换。 `CR-P6-03-009`：仅修正 T13 零发送 wrapper 的一次替代 T15 验证；T15 的 4xx 已停止矩阵。 `CR-P6-03-010`：基于官方 CLI 静态证据更正 workspace User-Agent，仅登记新的 T16 非流式直连验证，并在 T16 完整成功时条件允许一次 T17 SSE。 `CR-P6-03-011`：T16 在无网络预检的本地标签门槛前停止后，以不同的合法短标签重新登记 T18 非流式直连；仅其完整 Canonical 成功时允许条件 T19 SSE。 `CR-P6-03-013`：用户批准完成 P6 全部要求，解除 P6-03 对后续本地安全/连续性实现的流程阻塞；不声称 T18 成功、不发送 T19 或重放任何闭合 tuple。 `CR-P6-03-014`：以当前官方 CLI 的成功模型/会话和新完成的可注入执行链路登记 T20/T21，不重放任何已关闭 tuple。 `CR-P6-03-015`：T20 的新的 2xx JSON 协议失败后，只输出固定的无值结构类别诊断 T22。 `CR-P6-03-016`：T22 发现投影在合法压缩前运行，登记一次解压后无值结构诊断 T23。 `CR-P6-03-017`：T23 仍失败后，仅投影第一个固定 decoder requirement gate 的 T24。 |
| 已批准变更（续 2） | `CR-P7-G7-001`：P7 因 Kiro 外部账号重新认证而阻塞时，允许 P8 按自身顺序进行本地实现与审查；其与 `CR-P7-DEFER-002` 冲突的 P8/G8/P9-P12 顺序约束已由后者替代。 `CR-P7-DEFER-002`：Kiro OAuth 延后；P8-G12 按自身非 Kiro 依赖推进，P8 可执行自身 Phase Gate 与 Delivery Gate；真实 xAI 验证仍仅按 P8 自身明确授权进行。 `CR-P8-DEFER-001`：无 Official API Key 时，P8-07/G8 与 P7-09 一并延后至最终外部认证验收包；P9-P12 Gate 依赖不变。 `CR-P11-04-001`：用户批准把纯 loopback 合成 Soak 的最低门槛由 24 小时改为 10 小时；已完成的 10h13m 用户停止 receipt 仍如实标为 `INCOMPLETE`，P12 的真实 Canary 72h 观察不变。 |

本文是后续开发的唯一执行基线。功能矩阵定义“做什么”，行为契约定义“必须怎样表现”，本文定义“按什么顺序、交付什么、怎样证明完成”。

文档优先级：

```text
用户明确的新指令
  > 本开发计划
  > 关键行为与兼容性契约
  > 目标架构与专项设计
  > 功能矩阵的初始建议
  > 参考项目当前实现
```

## 1. 严格执行规则

### 1.1 任务状态

计划中的任务只能处于以下状态：

| 状态 | 含义 |
|---|---|
| `PENDING` | 前置条件未满足或尚未开始 |
| `IN_PROGRESS` | 当前正在执行；全计划同时最多一个 |
| `LOCAL_PASS_PENDING_PHASE_GATE` | 正常 Task 的实现、review、指定本地测试、格式与 Secret 检查已通过，证据已提交；等待本 Phase 唯一的远端正式 Delivery Gate。不是 `DONE`，也不计为 `IN_PROGRESS`。 |
| `LOCAL_PASS_PENDING_CI` | 仅用于 CI/workflow/cache/required-status 等必须提前在 GitHub 验证的例外 Task；本地条件已通过，等待该一次明确记录的提前远端 Gate。不是 `DONE`，也不计为 `IN_PROGRESS`。 |
| `DONE` | 代码、测试、文档和证据均完成 |
| `BLOCKED` | 已明确记录阻塞条件，无法继续 |
| `DEFERRED` | 经用户批准移出当前发布范围 |

### 1.2 每个任务的执行循环

1. 读取本文，确认当前 Phase、Task 和前置依赖。
2. 将且仅将一个 Task 标记为 `IN_PROGRESS`。
3. 实现该 Task 的最小完整改动，不夹带下一 Task 功能。
4. 运行该 Task 指定的定向测试、review、格式与 Secret 检查；安全、Schema、迁移、重试、依赖、
   公开边界或 CI 变更额外运行本地完整门禁。整合全局快速/完整门禁属于 Phase preflight，不对普通
   Task 机械重复。
5. 保存可复查证据：测试输出、基准、Fixture、日志或报告，并将实现、测试与该 Task 的证据尽量合并为同一提交。
6. 同步代码文档、行为契约和必要的矩阵状态，并将正常 Task 标记为 `LOCAL_PASS_PENDING_PHASE_GATE`；不得为普通 Task 单独启动远端 Code/docs Gate。
7. 在保持全计划仅一个 `IN_PROGRESS` 代码 Task 的前提下，`LOCAL_PASS_PENDING_PHASE_GATE` Task 是同一 Phase 后续依赖的有效本地证据。下一 Task 只能在前置本地证据充分时开始；发现回归时冻结受影响的后续 Task，先修复最早受影响提交。
8. 只有 CI/workflow/cache/required-status、部署控制面，或其它明确写入计划且无法由本地证明的变更，才可使用 `LOCAL_PASS_PENDING_CI` 并启动一次提前远端 Gate；该例外必须写明原因、范围和不通过时的冻结边界。
9. Phase 内全部 Task 均为 `LOCAL_PASS_PENDING_PHASE_GATE` 或已完成例外 Gate 后，运行整合本地完整门禁、Phase review 和 Phase-specific 验收。随后以 Phase closeout target 创建注释 tag，触发该 Phase 唯一的远端正式 Fast + Full Delivery Gate；不得先对同一 target 运行 Code Gate 再重复运行 tag Gate。
10. 该远端 Delivery Gate 通过后，Phase 内正常 Task 满足 Definition of Done 并一并标记 `DONE`；失败时不得进入下一 Phase，须从最早受影响提交修复并重建同一 Phase closeout target。

### 1.3 禁止事项

- 不因顺手方便实现未进入当前 Phase 的功能。
- 不用真实 Provider 的特殊字段污染 `gateway-core`。
- 不跳过 Mock、Fixture 或差分测试直接接管生产流量。
- 不在流式热路径查询 SQLite、读取配置文件或执行网络模型发现。
- 不在任何日志、Fixture、错误信息或 Git 历史中保存真实 Secret。
- 不把“能编译”“手工请求成功”单独视为 Task 完成。
- 不在 Phase Gate 失败时用 Feature Flag 掩盖必需功能后继续下一阶段。

### 1.4 计划变更流程

以下变化必须创建 Change Request，并由用户明确批准：

- 调整 Phase 顺序或跳过 Gate。
- 新增或删除公开接口、Provider、协议或持久化实体。
- 修改 Canonical Request/Event、错误分类、重试边界或 Secret 方案。
- 把 `Later/Drop/PENDING` 功能提前加入当前发布。
- 放宽安全、性能、测试或部署门槛。
- 改变 Task 状态语义、GitHub required Gate 分类或 Phase Gate 触发条件。

Change Request 格式：

```text
CR-ID:
原因:
影响的 Task / Matrix ID / ADR:
兼容性与迁移影响:
测试与回滚变化:
用户批准:
计划版本变更:
```

仅修正文案、补充测试或不改变对外行为的内部重构，可以在原 Task 内完成，但仍需记录在完成证据中。

### 已批准 Change Request：CR-EXEC-001

```text
CR-ID: CR-EXEC-001
原因: P0-P3 回顾显示，P3 的 39 次 GitHub workflow 累计约 516 分钟；近期 Full gate 的
      cargo-deny/cargo-audit 固定安装约占单次 Full 运行时长的 74%。P3-10 的真实 Endpoint
      验收还暴露出固定四探针 harness 不适合定位单个 Target/Mode 传输问题。
影响的 Task / Matrix ID / ADR: 执行规则、CI、P4-00 与未来所有需要真实 Endpoint 验证的
      Provider Task；不改变任何功能矩阵、公开 API、Canonical 类型、Provider 协议、Schema、
      数据库、部署或既有 P0-P3 验收结果。
兼容性与迁移影响: 无客户端兼容性、数据或部署迁移。只新增受控交付状态、CI 分类和 test-only
      诊断能力；正式四探针验收 harness 与其既有隐私/调用上限保持独立。
测试与回滚变化: P4-00 必须证明 cache miss/hit 的版本校验、code/docs/phase-tag Gate 分类、
      docs-only Secret/链接检查、状态转换阻断规则和单探针零授权/一探针上限。回滚为禁用缓存、
      恢复所有提交 Full Gate、停止使用 LOCAL_PASS_PENDING_CI 并移除未使用诊断 harness；
      不影响已完成 Phase。
用户批准: APPROVED，2026-07-21（要求将全部提速方案写入开发计划）
计划版本变更: v1.2
```

### 1.5 交付提速纪律（CR-EXEC-001）

1. **CI 分层而不降门槛。** Rust、`Cargo.toml`/`Cargo.lock`、工具链、workflow、脚本、
   迁移、Fixture、契约或安全策略变更必须在所属 Phase 的远端正式 Delivery Gate 运行 GitHub
   Fast + Full supply-chain Gate；普通 Task 不逐个触发远端 Gate。纯报告、索引或计划状态变更在
   不属于 Phase closeout 时运行显式 `docs-only` Gate：Markdown/格式、文档链接、Secret scan 和
   计划一致性检查；它不得伪装为 Full 成功。每个 Phase closeout tag 始终强制运行 Fast + Full。
2. **缓存只加速受版本约束的工具，不替代验证。** CI 可缓存 `cargo-deny`、`cargo-audit` 的
   二进制及其 Cargo registry/git 下载；缓存 key 必须包含 runner OS、固定 Rust 版本和
   `tools/quality-tool-versions.env` 摘要。每次恢复后仍执行版本检查；缺失或不匹配时仍以
   `cargo install --locked` 重新安装。不得缓存 Credential、环境文件、真实测试配置或把 cache hit
   当作供应链通过证明。预装镜像只有在来源固定、版本可复查且另有供应链审查后才可替代缓存。
3. **状态流水线保持一个代码 Task。** 正常 Task 使用 `LOCAL_PASS_PENDING_PHASE_GATE`；它可作为
   同一 Phase 后续 Task 的前置证据，但不允许跨 Phase、合并、发布或提前宣称 `DONE`。
   `LOCAL_PASS_PENDING_CI` 仅表示不可本地证明的提前远端例外。任何 Fast/Full 失败都会冻结
   受影响的后续工作，先恢复最早失败 Task 的绿色状态；同一时刻仍只有一个 `IN_PROGRESS`。
4. **文档证据随 Task 写入，Phase 一次收口。** 每个 Task 的实现、测试、ADR/Contract 和报告
   骨架都在其提交中完成。Phase closeout target 在 tag 前聚合不可变本地证据与报告，不为每个
   Task 或 tag 结果再创建 docs-only 状态提交；GitHub run、tag 和 job summary 是远端最终证据。
5. **真实 Endpoint 先诊断、后验收。** 未来需要真实 Endpoint 的 Phase 必须使用与正式验收
   harness 分离的 ignored 单探针诊断路径。它必须要求单独的显式授权、Target、Mode 和精确
   `max_external_requests=1`，保持 `max_attempts=1`、无自动重试/failover、脱敏输出和有界
   timeout/read；普通 CI、缺失授权或错误配置必须零网络。它只能定位一个已授权 Target/Mode，
   不能替代正式多 Target 验收，也不能重用或扩大既有 P3-10 授权。
6. **先收集 readiness，再消费真实调用。** 在每个真实 Provider 验收前，先完成不触网的
   Base URL/egress/profile/model-alias/调用预算预检；用户批准的调用预算、网络 profile 和
   停止条件必须在第一次真实请求前写入私有 operator-controlled 配置与公开脱敏计划。
7. **度量目标。** P4-00 完成后，warm Full Gate 中“安装固定质量工具”硬性目标不高于 90 秒，
   运行目标不高于 10 秒；每轮 Phase Gate 记录 Fast、Full、docs-only 的次数与 step 时长。若
   连续两轮未达到目标，先调查 cache key、安装源和 runner，而不是削弱 Full Gate。

### 已批准 Change Request：CR-EXEC-002

```text
CR-ID: CR-EXEC-002
原因: P4-00/P4-01 的实测表明，质量工具缓存对 GitHub ref 可见：P4-01 在新分支首次 Code Gate
      仍发生约 495 秒冷安装，而同一 ref 的暖态测量将安装降为约 1 秒；同时现有 Full job 在独立
      runner 重复了 Fast 已执行的 Workspace 检查。P4-00/P4-01 的两次 docs 状态提交也产生了
      无新的功能证据的额外等待。
影响的 Task / Matrix ID / ADR: 执行规则、CI、P4-02 及后续顺序代码 Task；新增 ADR-0023 与
      BC-DELIVERY-002。它不改变功能矩阵、公开 API、Canonical 类型、Provider 协议、Schema、
      数据库、部署、P0-P3 结果或“全计划最多一个 IN_PROGRESS Task”规则。
兼容性与迁移影响: 无客户端、数据或部署迁移。顺序代码 Task 使用 cache-visible delivery ref；新
      ref 必须先有同 ref cache seed 或使用经批准的共享/default ref。GitHub Fast 仍是完整 Workspace
      快速检查，GitHub Full 改为依赖同 SHA Fast 的版本校验、cargo-deny 和 cargo-audit 补充检查；
      本地 ./scripts/check.sh full 保持完整检查。代码 Gate 后只做一次 docs-only 收口。
测试与回滚变化: P4-02 必须验证 supply-chain mode、workflow 结构、cache hit/miss summary、Fast
      + Full fail-closed required gate、local full 以及单次 docs closeout。缓存 miss 不是正确性失败，
      但必须在报告中记录并触发性能调查。回滚为将 GitHub Full 恢复为 ./scripts/check.sh full、停止
      使用 cache-visible ref 规则和单次收口；不影响已有 Task 产物。
用户批准: APPROVED，2026-07-21（要求按该方案优化开发计划并以 P4-02 测试效果）
计划版本变更: v1.3
```

### 1.6 缓存可见 Phase 交付与补充供应链 Gate（CR-EXEC-002、CR-EXEC-007）

1. **共享缓存优先于短生命周期分支缓存。** P4 的 `codex/p4-01-catalog-singleflight` 曾证明同 ref
   的暖态缓存有效，但最终 tag 不能读取该分支缓存而发生冷安装。自 `P5-00` 通过后，固定质量工具
   缓存必须以 runner OS、固定 Rust 版本和 `tools/quality-tool-versions.env` 摘要为 key，并先在
   `main`/default ref 创建可供 Phase tag 读取的受版本约束 seed。一个 Phase 只在末尾交付时，单靠
   Phase 分支并不能产生可复用缓存；缓存 miss 仍 fail-safe 地重新安装，不得跳过版本校验。
2. **Fast 完整、Full 补充。** GitHub Fast 是完整 Workspace fast check。GitHub Full 依赖同一
   workflow/SHA 的 Fast，仅执行固定质量工具版本检查、`cargo deny check` 与 `cargo audit`；Required
   Gate 对两个结果均 fail-closed。本地 `./scripts/check.sh full` 继续执行完整 Fast 加供应链检查，
   因此本地需要完整门禁的变更只运行一次 `full` 即覆盖 Fast，不机械地紧接着重复 `fast`；Phase
   Tag 仍逻辑要求 Fast + Full。
3. **Phase 一次收口。** 代码提交包含实现、测试、ADR、契约和报告骨架；Phase closeout target 在
   tag 前汇总本地证据。普通 Task 与 tag Gate 通过后均不得再为 run ID 创建 docs-only 状态提交；
   tag、GitHub job summary 和下一 Phase 开始时的计划状态共同构成外部收口。独立于任何 Phase 的
   纯计划/报告变更仍走一次 docs-only Gate。
4. **缓存可观测性与目标。** Full job 必须把 cache hit/miss 写入 GitHub job summary。miss 不会降低
   Gate 的正确性结论，但报告必须记录原因。暖态质量工具安装运行目标 `<=10s`、计划硬门槛 `<=90s`；
   暖态 Phase Delivery workflow 目标 `<=4min`（不含 GitHub queue），独立 docs-only workflow 目标
   `<=45s`。P5-00 必须以提前远端例外证明 default-ref seed、tag restore、cache hit/miss summary 和
   fail-closed fallback；之后不额外为普通 Task 手工暖态重跑。

### 已批准 Change Request：CR-EXEC-003

```text
CR-ID: CR-EXEC-003
原因: P4-02 的总 wall-clock 约 43 分钟，但已验证的 GitHub Code Gate 仅约 4 分 05 秒、docs-only
      Gate 仅约 40 秒。本地完整门禁约 43 秒；剩余时间主要来自重复范围读取、分散补丁、一次
      Clippy 返工、重叠验证、Gate 通过后才整理 closeout，以及手工证据汇总。需要压缩代理执行
      时间而不降低 review、Full Gate、Secret 扫描、文档或单 Task 纪律。
影响的 Task / Matrix ID / ADR: 后续所有顺序 Task 的执行循环、汇报与测量；不改变 Matrix、公开
      API、Canonical 类型、Provider 协议、Schema、数据库、部署、GitHub required Gate 或任何已
      完成 Task 的验收。无需新增 ADR，因为这是代理工作流而非系统架构决策。
兼容性与迁移影响: 无客户端、数据、部署或安全迁移。仍最多一个 IN_PROGRESS Task；仍使用既有
      Code Gate、LOCAL_PASS_PENDING_CI 和单次 docs-only 收口。
测试与回滚变化: 本次计划变更走 docs-only Gate。后续每个 Task 必须在报告中记录范围等级、局部
      执行、Code Gate、closeout 和总时长；若时间预算连续两次超标，先记录具体阻塞并调整执行
      方法。回滚只移除本节的代理效率纪律，不移除任何测试、review、Gate 或功能产物。
用户批准: APPROVED，2026-07-21（要求将代理执行提速方案写入开发计划）
计划版本变更: v1.4
```

### 1.7 代理执行提速纪律（CR-EXEC-003）

1. **先建 Task Card，再读代码。** 每个 Task 开始时，在本轮可见的执行说明中固定：范围等级
   `S/M/L`、依赖、最多需要修改的文件、必须成立的不变量、定向测试、最终 Gate 和禁止项。首次
   读取只限计划行、直接依赖、最近相似实现及对应行为契约；只有出现具体编译/测试/设计冲突时
   才扩展调查范围，不能为了“可能有用”重复浏览已确认文件。
2. **显式的本地执行预算。** `S`（单 Crate、无 Schema/公开边界）目标为从 Task Card 到代码提交
   `<=15min`；`M`（最多三 Crate 或一个契约边界）目标 `<=25min`；`L` 必须先给出细化子计划与
   预算，不能隐式按小 Task 推进。预算是过程度量而非降低质量的 deadline；超出时必须记录原因。
3. **集中补丁而非微补丁。** 范围已明确后，代码、定向测试、ADR/Contract/报告骨架和索引应按
   一个一致的改动集完成。格式化或由失败测试直接要求的修复可以追加；不为尚未验证的猜想拆分
   多轮文件编辑。写操作保持串行以避免冲突，互不依赖的只读检查并行执行。
4. **开发验证去重。** 开发中仅运行当前边界的定向 test/Clippy 以获得快速反馈。最终需要完整
   门禁时只运行一次 `./scripts/check.sh full`，它已覆盖 Fast 与 supply-chain；不得在其前后机械
   追加独立 `fast`、`supply-chain` 或相同范围 Clippy。staged Secret/whitespace review 仍必须在
   提交前执行，且不被 Full 替代。
5. **Phase Gate 等待与 closeout 重叠。** Phase tag 推送前完成 closeout target、报告表格和索引
   diff；远端 Gate 运行时只读取一次必要的 job 摘要。Gate 通过后以 tag/run/job summary 作为不可变
   证据，不创建额外 docs-only 收口；Gate 失败时修正最早受影响提交并重建 Phase closeout target，
   绝不借草稿进入下一 Phase。
6. **证据模板化而不删证据。** 后续 P4 Task 复用固定 ADR、Contract、报告、追踪行和 closeout
   模板，只填写本 Task 的决策、行为、测试和时延差异。没有新增架构决定或可观察行为时，不凭
   习惯额外创建文档；现有要求的证据、链接、Secret 约束仍完整保留。
7. **远端查询最小化。** workflow 运行时使用低频状态轮询；完成后先读取一次完整 job 摘要，仅在
   cache、失败或安全证据缺失时读取相关 job log。不得为复制同一状态或刷新无变化页面重复查询。
8. **统一时延报告。** 每个 Task 报告记录 `Task Card`、`代码提交`、`本地 review/测试通过`、范围
   等级、重复验证次数、返工次数和超预算原因；每个 Phase report 额外记录 `closeout target/tag`
   与 `Phase Delivery Gate 通过`。此数据用于下一 Task 纠正流程；连续两次同类超预算必须先优化
   执行方法，再考虑扩大并行度。

### 已批准 Change Request：CR-EXEC-004

```text
CR-ID: CR-EXEC-004
原因: 简单、确定且低风险的执行（例如已知命令的只读检查、固定格式的 docs-only 收口、明确状态
      查询）不需要默认占用高级模型与深度思考。一次短 GitHub TLS 验证曾与后续 P4 开发混入同一
      高强度执行回合，导致用户等待时间和实际探针耗时无法清楚区分。需要在不降低质量门禁的前提
      下，按风险和不确定性自动选择 Luna 与最低足够的思考强度，并保持短任务的独立交付边界。
影响的 Task / Matrix ID / ADR: 后续所有代理执行、Task Card、状态查询、docs-only 收口与报告。
      不改变 Matrix、公开 API、Canonical 类型、Provider 协议、Schema、数据库、部署、GitHub
      required Gate、真实 Provider 授权或既有功能验收。无需新增 ADR，因为模型路由是执行层策略。
兼容性与迁移影响: 无客户端、数据、部署或安全迁移。Luna 的可用性由执行平台决定，仓库不伪造
      已切换状态；平台不能提供 Luna 时，使用当前可用模型的等价低强度通道并明确记录 fallback。
测试与回滚变化: 本次计划变更走 docs-only Gate。后续抽样检查 Task Card/状态更新是否包含执行通道、
      思考强度和 fallback（如有）；发现简单任务被无理由提升时先纠正路由。回滚仅移除本节的模型
      路由规则，不移除 review、Secret scan、测试、Gate、单 Task 或真实调用授权约束。
用户批准: APPROVED，2026-07-21（要求简单任务自动切换 Luna 并按需选择思考强度）
计划版本变更: v1.5
```

### 1.8 模型与思考强度路由（CR-EXEC-004）

1. **先选执行通道，再开始工具调用。** 每个请求先按风险、不确定性、可逆性和影响面路由；既有
   `S/M/L` 仍只表示代码范围和时延预算，不能被误当成模型等级。用户显式指定的模型或思考强度
   始终优先于本节默认值。
2. **简单任务默认走 Luna-fast。** 具备模型切换能力时，下列任务必须优先切至 `luna`：已知命令
   的只读状态/版本/连通性检查、单目标且不改状态的重复采样、固定证据驱动的 docs-only 收口、
   已确认文件中的机械索引或格式修正、以及单一 GitHub/Git 状态查询。普通简单任务默认使用
   `low`；需要合并固定输出、执行多步检查、机械写入或有限语义判断时使用 `medium`。`minimal`
   仅限零判断的单一事实/命令查询，不能用于普通执行、诊断、写入或 Gate 判断。不得因为任务来自
   既有大 Task 就自动升级为高级模型或深度思考。
3. **用最低足够强度逐级升级。** 范围明确的代码改动、有限测试失败定位或需要语义判断的文档变更
   使用当前默认模型的 `medium`。只有架构设计、跨 Crate 公开边界、并发/内存安全、安全审查、未知
   根因排查、非可逆配置变更、真实 Provider/凭据操作，或用户明确要求深度分析时，才可使用高级
   模型和 `high`/`xhigh`。每次升级必须在 Task Card 或首个状态更新中写明触发条件；`high`/`xhigh`
   不是默认模式。
4. **Luna 不可用时 fail-transparent。** 执行平台未暴露 `luna`、切换失败或任务在执行中越过
   Luna-fast 边界时，不得悄悄以高级模型继续。应在下一条可见状态中记录 `fallback: luna unavailable`
   或升级原因，并使用当前可用模型的 `minimal`/`low`（或在风险升级后相应强度）。任何安全、真实
   调用或代码质量 Gate 都不能因 fallback 被跳过。
5. **短任务独立交付。** Luna-fast 请求应先给出其自身结果；除非用户在同一条请求明确要求继续，
   不把后续高强度开发、Gate 等待或无关排查折叠进该短任务。若外部命令超过 60 秒，先报告正在等候
   的具体对象和已知结果；不得用未归因的墙钟时间冒充简单检查成本。
6. **质量边界不因模型降级而变化。** 低强度通道可以减少推理和范围读取，不能省略已适用的 review、
   Secret scan、定向测试、Full/Code/docs Gate、授权检查或最终状态核对。任务一旦需要这些边界之外
   的判断，必须先升级通道而不是在 Luna-fast 中猜测。
7. **记录并复盘路由效果。** 每个代码 Task 的 Task Card 记录 `执行通道`、`思考强度`、选择理由和
   fallback（如有）；简单任务至少在首条状态更新中声明通道。每五个简单任务抽样比较“收到请求至
   首个结果”的墙钟时长；若两次因错误路由而超过 5 分钟，先修正规则或任务切分，再增加模型强度。

### 已批准 Change Request：CR-EXEC-005

```text
CR-ID: CR-EXEC-005
原因: 当前主会话未必能在原位切换至 Luna。为落实 CR-EXEC-004 的低成本执行通道，可以在确有
      独立、低风险和可复核工作的情况下，使用低成本子代理代替主会话持有高级模型/深度思考；但
      无上限或嵌套委派会重新引入上下文竞争、隐藏等待和多 Task 并行风险，因此必须设定硬上限。
影响的 Task / Matrix ID / ADR: 后续执行编排、Task Card 和状态汇报；不改变 Matrix、产品代码、
      Canonical 类型、Provider 协议、Schema、数据库、部署、GitHub required Gate、真实 Provider
      授权或单 Task 规则。无需新增 ADR，因为这是代理调度策略而非系统架构决策。
兼容性与迁移影响: 无客户端、数据、部署或安全迁移。若平台未暴露 Luna 或其他已批准低成本模型，
      不伪造模型选择；主会话继续使用当前模型的最低足够强度。子代理的结果始终是建议或受控执行
      结果，不能替代主会话的 review、提交、推送或验收。
测试与回滚变化: 本次计划变更走 docs-only Gate。每次委派必须可从状态更新复原模型通道、作用域、
      活跃数量、返回结果和主会话复核；发现越权、嵌套或并发超限时立即停止并由主会话收束。回滚
      仅禁用本节的子代理 fallback，不影响 CR-EXEC-004 的原位模型路由和任何质量门禁。
用户批准: APPROVED，2026-07-21（当前会话不能切换模型时允许受限低成本子代理，并限制数量）
计划版本变更: v1.6
```

### 1.9 受限低成本子代理 fallback（CR-EXEC-005）

1. **触发条件全部满足才可委派。** 主会话不能原位选择 `luna`（或用户批准的低成本模型）；工作
   与当前 Task 的主实现不存在未解决的语义依赖；作用域、文件或命令、预期返回格式和停止条件均能
   在一条 prompt 中写清。仅为“多想一点”、缩短主会话责任或绕开 review 不得委派。
2. **硬数量上限。** 每个主会话同一时刻最多一个活跃低成本子代理；禁止子代理再创建子代理。第二
   个低成本子代理、任何并行代码 Task 或超过此上限的并发，必须获得用户新的明确批准。该上限不
   改变“全计划最多一个功能 Task 为 `IN_PROGRESS`”的规则。
3. **允许的受控工作。** 子代理可做已知命令的只读检查、有限重复采样、指定测试/格式/链接检查、
   精确文件范围内的机械文档改动，或主会话预先界定且可回滚的简单执行步骤。涉及写入时，prompt
   必须声明唯一文件所有权；主会话随后必须审阅 diff、运行适用 Gate，并决定是否保留结果。
4. **禁止的工作。** 子代理不得决定架构或公开边界、进行安全/并发最终审查、读取或处理 Secret、
   发起真实 Provider 请求、修改代理/TUN/网络设置、创建/切换主分支、提交/推送 Git、标记 Task
   `DONE`、批准 Gate，或替代主会话对用户作最终交付。遇到这些需求必须立即返回主会话升级。
5. **模型与思考强度。** 可用时子代理默认使用 `luna` 的 `low`；需要汇总固定输出、执行多步
   检查或机械写入时使用 `medium`。`minimal` 仅限零判断的单一事实/命令查询，且不得用于写入、
   诊断或 Gate 判断。平台没有可调用的低成本模型时，不得把高级模型伪称为 Luna 子代理；主会话
   改用自身等价的 `low`/`medium` 通道完成，或向用户报告该 fallback 不可用。
6. **可见性与收束。** 委派前的状态更新必须写出 `delegated`、模型/强度、任务边界和“活跃子代理
   数=1”；返回后必须写出命令/改动摘要、结果、耗时和是否升级。主会话确认无其它活跃子代理后，
   才可开始下一项委派或高强度开发，避免后台工作被折叠进无关短任务。

### 已批准 Change Request：CR-EXEC-006

```text
CR-ID: CR-EXEC-006
原因: `minimal` 适合零判断的单一事实查询，但对普通简单执行过低；过度压低思考强度会把速度收益
      变成遗漏检查或返工。需要保留 Luna 的效率优势，同时把简单任务的默认判断下限设为 `low`，
      并让有限多步/机械写入工作使用 `medium`。
影响的 Task / Matrix ID / ADR: CR-EXEC-004 与 CR-EXEC-005 的模型/思考强度路由；不改变功能、
      安全边界、Task 并发上限、review、测试、Gate、Provider 授权或部署。
兼容性与迁移影响: 无客户端、数据或部署迁移。只调整执行器的默认思考强度；用户显式指定强度仍
      优先，Luna/低成本模型不可用时的透明 fallback 规则不变。
测试与回滚变化: 本次计划变更走 docs-only Gate。后续复盘检查简单执行是否至少使用 `low`，并对
      多步检查/机械写入是否使用 `medium`；若质量证据显示该下限仍不足，再显式提升而非默默返工。
      回滚仅恢复 CR-EXEC-004/005 先前的强度映射，不影响质量门禁。
用户批准: APPROVED，2026-07-21（不将普通简单任务默认降至过低思考强度）
计划版本变更: v1.7
```

### 1.10 简单执行的思考强度下限（CR-EXEC-006）

1. **默认下限。** Luna-fast 和受限低成本子代理的普通简单执行默认 `low`，不是 `minimal`。
2. **有限步骤下限。** 需要汇总固定输出、运行多个已知检查、编辑指定文档或进行有限判断时使用
   `medium`；这仍不是高级模型/深度思考的默认入口。
3. **minimal 的窄例外。** 仅限不写入、不诊断、不作 Gate 结论的单一事实或单命令查询；不能因
   “任务很小”自动选择它。

### 已批准 Change Request：CR-EXEC-007

```text
CR-ID: CR-EXEC-007
原因: 用户要求将远端上传与正式测试从“每个 Task 一次”改为“每个 Phase 一次”，并将开发分支
      从“每个 Task 一个”改为“每个 Phase 一个”。P4 的实测也表明，同一开发分支的质量工具缓存
      暖态有效，但最终 tag 因 GitHub ref 缓存作用域未能读取该分支缓存，重新安装工具约 8 分 10 秒。
      需要减少重复远端 Gate，同时保留每个 Task 的本地 review 和定向测试。
影响的 Task / Matrix ID / ADR: 执行状态、Task 循环、Git 规则、Definition of Done、CI/cache
      交付流程、P5-00 及所有尚未开始的 Phase；仅取代 CR-EXEC-001 至 CR-EXEC-003 中“普通 Task
      必须逐个远端 Gate/逐个分支/docs-only 收口”的部分。不改变 P0-P4 已完成的验收、功能矩阵、
      公开 API、Canonical 类型、Provider 协议、Schema、数据库、部署、Secret 规则或真实调用授权。
兼容性与迁移影响: 无客户端、数据或部署迁移。每个 Task 保持独立提交、review、定向自动化测试、
      格式与 Secret 检查；远端 Fast + Full 仍 fail-closed，只改为 Phase closeout 的唯一正式运行。
      CI/workflow/cache/required-status 等无法本地证明的改动保留一次提前远端例外。
测试与回滚变化: P5-00 必须证明 Phase branch/tag 只产生一次普通正式 Delivery Gate、default-ref
      质量工具 cache seed 可被 tag 恢复、缓存 miss 安全回退、Fast + Full required 状态与例外 Gate
      冻结规则。每个普通 Task 必须保留本地定向测试/review；Phase 必须保留整合本地 full、跨 Crate
      /契约/Phase-specific 验收和一次远端 Fast + Full。回滚为恢复每 Task 分支和远端 Gate；不回溯
      或重跑已完成 Phase。
用户批准: APPROVED，2026-07-21（要求按 P 级分支与 P 级远端验收方案修改开发计划）
计划版本变更: v1.9
```

### 1.11 P 级分支与单次远端正式验收（CR-EXEC-007）

1. **一个 Phase，一个开发分支。** 未开始的 Phase 使用 `codex/p<phase>-<short-name>` 分支，
   例如 `codex/p5-anthropic`。该分支只承载该 Phase 的顺序 Task 和其修复，不跨 Phase、不并行
   功能开发；每个 Task 仍使用自己的 `P5-01:` 等提交前缀，保持可 review、可 bisect 和可回滚。
2. **Task 本地验收不延后。** 每个普通 Task 必须完成范围内的定向测试/Clippy、`cargo fmt --check`、
   staged Secret/whitespace review、行为契约/文档更新和独立代码 review 后才可进入
   `LOCAL_PASS_PENDING_PHASE_GATE`。安全、Schema、迁移、重试、依赖或公开边界改动额外运行一次
   本地 `./scripts/check.sh full`；不得把所有测试积压到 Phase 末尾。
3. **一次普通远端交付。** 全部 Task 本地验收后，执行整合本地 full、跨 Crate/契约/Phase-specific
   测试和 Phase review，将结果写入 closeout target，创建注释 `phase-p<phase>-complete` tag，并以
   Phase 分支和 tag 的单次交付事件运行 Fast + Full。tag 通过前，不得开始下一 Phase、合并或发布。
4. **不重复状态提交。** 正常 Task 不逐个 push、不逐个跑 Code/docs Gate，也不在 tag 通过后为 run ID
   另建 docs-only 提交。需要记录的远端证据放在 tag、GitHub run/job summary 和下一 Phase 开始时的
   计划状态中；独立计划文档变更仍可走 docs-only Gate。
5. **提前远端例外是窄边界。** CI workflow、cache、required-status、分支/发布控制或其它只能在
   GitHub/目标环境证明的改动，必须在该 Task 后跑一次提前远端 Gate，并在 Task Card 和报告中写明。
   该 Gate 不是普通 Task 的模板；真实 Provider 请求仍只按单独授权和既有调用边界执行。
6. **缓存与单次交付配套。** 因为最后才 push 的 Phase 分支没有自己的暖态 cache，P5-00 必须先建立
   default-ref 的受版本约束 cache seed，并验证 tag 能恢复它。缓存仅缩短安装；每次仍做版本验证、
   `cargo deny` 和 `cargo audit`，cache miss 必须安全重新安装并记录为性能调查项。
7. **完成语义。** `LOCAL_PASS_PENDING_PHASE_GATE` 是本地通过而非 `DONE`；Phase 唯一正式 Gate
   成功才把其中 Task 一并转为 `DONE`。任何本地或远端失败都回到最早受影响提交修复，不能用后续
   Task、Feature Flag 或报告文字掩盖。

### 已批准 Change Request：CR-P6-03-001

```text
CR-ID: CR-P6-03-001
原因: 用户于 2026-07-22 明确授权 P6-03 对指定 CPA OAuth 测试账号进行不受既有请求数/预算限制的
      综合验证。为避免把该授权误解为无限重试或扩大操作面，需要把它收敛为有限、预先记录且可复核
      的模型 × 模式验证矩阵。
影响的 Task / Matrix ID / ADR: 仅 P6-03 的真实验证操作政策、报告和 BC-PROVIDER-003；覆盖 Matrix
      `C28` 的固定 Build Responses 非流式与 SSE 兼容性。不改变公开 API、固定端点、Canonical 类型、
      Provider 代码、OAuth 账户状态、P6-04+ 行为、服务器配置或代理/TUN 配置。
兼容性与迁移影响: 无客户端、数据、数据库、部署或账号迁移。只使用一个由用户指定的 CPA OAuth
      账号；凭据在内存中投影为严格 OAuth JSON，不写入磁盘、日志或 Git。
测试与回滚变化: 在发送前记录有限的不同 `(模型候选, 模式, 网络配置)` 元组。每次 ignored harness
      调用仍必须固定 `P6_03_MAX_EXTERNAL_REQUESTS=1`，只会调用一次 send；同一元组不得自动重试、
      failover、刷新或循环。每个候选/网络配置组合只测试 `non_streaming` 和 `sse`；遇到非 2xx、超时、协议/语义
      错误或凭据不可用时，只记录脱敏类别并继续下一个不同元组。回滚为停止未执行元组并恢复此前的
      单探针授权文案；不需要撤销 Provider、服务器或网络状态。
用户批准: APPROVED，2026-07-22（“你自己决定，不限制次数方案，可以都测试”）
计划版本变更: v1.10
```

### 1.12 P6-03 有限真实验证矩阵（CR-P6-03-001）

1. **有限性优先。** “不限制次数”只取消历史的预算计数，不允许无界探测。开始前必须在 P6-03 报告中
   列出候选模型、`non_streaming`/`sse` 两种模式、网络配置及停止条件；没有记录的元组不得发送。
2. **一进程一请求。** 现有 ignored harness 的 `P6_03_MAX_EXTERNAL_REQUESTS=1` 不得放宽。不同元组
   必须通过独立进程调用；同一 `(模型, 模式, 网络配置)` 元组最多一次，且不含自动重试、刷新、
   candidate 选择、failover 或续接。只有前一有限矩阵全部得到无法区分的安全类别、另有 Change
   Request 明确登记诊断目标、并且该诊断只输出状态/内容类型/无值结构分类时，才可增加同元组的
   非自动诊断调用；每项诊断仍一进程一 send，必须比前一项增加更窄的新安全事实。原矩阵之外最多
   两项此类诊断，且第二项后绝不继续同元组调用；它们不能作为成功重试或替代功能验收。
3. **单账号、固定边界。** 仅使用用户指定的一个 CPA OAuth 账号，且仅访问 P6-03 固定
   `https://cli-chat-proxy.grok.com/v1/responses` 端点。不得枚举其它认证文件、改变账号、服务器、
   代理规则或 TUN 排除地址。
4. **最小语义与安全记录。** 每请求固定 `Reply with exactly: ready`、32 token 上限和已验证的精确
   Egress policy；报告只可记录候选标签、模式、网络配置、墙钟时间和脱敏的成功/失败类别，绝不记录
   Token、OAuth JSON、私有模型映射、请求体或响应内容。
5. **通过与停止。** 至少一个候选模型的固定端点非流式与 SSE 都得到有效 Canonical
   `ResponseStart`、文本和 `ResponseEnd`（无 `StreamError`）才可让 P6-03 进入
   `LOCAL_PASS_PENDING_PHASE_GATE`。所有候选均失败时保持 `IN_PROGRESS`/`BLOCKED` 并提交证据；
   P6-04 在此之前不得开始。

### 已批准 Change Request：CR-P6-03-002

```text
CR-ID: CR-P6-03-002
原因: 已登记的 T1-T8 都独立发送一次，并都安全地停止为 `response_protocol_failed`。原 harness
      故意不输出 HTTP 状态、内容类型或 JSON 结构，因而无法判断是上游以 2xx 返回错误对象、协议
      漂移，还是解码器不兼容。需要一项不记录原始响应值的分类诊断，外加一个明确的一次性诊断调用。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored harness、报告和 `C28` 的 live evidence；不改变
      生产 Provider、固定端点、Canonical 行为、请求体、凭据、账号状态、P6-04+、服务器或代理/TUN。
兼容性与迁移影响: 无客户端、数据、部署或账号迁移。诊断只输出 HTTP 状态类、内容类型是否符合预期
      和无值 JSON body shape；不输出 Header 值、URL、模型值、Token、请求/响应文本、ID 或错误文本。
测试与回滚变化: 增加合成 redaction 测试和一项 T9-DIAG：仅重观察已失败的 `build-static-01`、
      `non_streaming`、`socks5` 组合一次。它每进程仍为一 send，不自动重试，不能使该组合变为通过。
      回滚移除安全诊断输出并停止 T9；T1-T8 的事实证据保留。
用户批准: APPROVED，2026-07-22（“你自己决定，不限制次数方案，可以都测试”）
计划版本变更: v1.11
```

### 已批准 Change Request：CR-P6-03-003

```text
CR-ID: CR-P6-03-003
原因: T9-DIAG 已证明指定 OAuth token 取得 `2xx`、期望 JSON 内容类型和 error-like object，排除了
      网络/内容类型层，但还不能区分模型、凭据、请求或额度类的应用层错误。需要最终一次只检查错误
      对象标准 `code`/`type`/`param` 元数据的白名单映射；不读取或输出 message 等自由文本。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored harness、报告和 `C28` live diagnosis；不改变生产
      Provider、请求、固定端点、Canonical 行为、账号状态、P6-04+、服务器或代理/TUN。
兼容性与迁移影响: 无客户端、数据、部署或账号迁移。输出只可能是 `model`、`credential`、`request`、
      `quota` 或 `unrecognized` 类别；任何未知 code/type/param 值均不输出原值。
测试与回滚变化: 增加合成白名单/未知值脱敏测试，并登记最终 T10-DIAG：只重观察 `build-static-01`、
      `non_streaming`、`socks5` 一次。它是第二且最后一次同元组诊断，仍为一 send、无重试、不能通过
      P6-03；若结果仍不足以支持修复，Task 保持阻塞而非继续探测。
用户批准: APPROVED，2026-07-22（“你自己决定，不限制次数方案，可以都测试”）
计划版本变更: v1.12
```

### 已批准 Change Request：CR-P6-03-004

```text
CR-ID: CR-P6-03-004
原因: 用户明确授权使用当前服务器已部署 grok2api 的现有 endpoint 与 API key，取得一条实际
      Build 路由代理请求的脱敏参考证据，以区别“固定直连 Build profile 不被接受”和“Build
      账号/路由本身不可用”两种情况。
影响的 Task / Matrix ID / ADR: 仅补充 P6-03 报告的诊断证据；不改变 C28 的固定端点、P6-03
      生产代码、行为契约、Canonical 语义、P6-04+、账号/路由配置、服务器、代理或 TUN。
兼容性与迁移影响: 无。grok2api 是独立的 OpenAI-compatible 代理边界；其成功绝不替代 P6-03
      固定 `cli-chat-proxy.grok.com` 直连端点的非流式与 SSE 验收。
测试与回滚变化: 服务器内存中仅选取一把已启用且仅绑定 Build 路由的既有 client key；恰好一次
      `GET /v1/models` 用于选择该 key 已许可的模型，随后恰好一次、非流式 `POST /v1/responses`
      （固定短提示、`max_output_tokens=32`）。无重试、无 key/model fallback、无 SSE，且不由
      操作员进行账号、路由或数据库写入；服务正常的 last-used/用量会计写入是该一次调用的固有
      副作用。只记录状态类、内容类型类、结构形状、耗时和紧随其后的安全日志投影；不记录或
      输出 endpoint 私有部分、API key、加密材料、模型映射、头、请求/响应正文或文本。
用户批准: APPROVED，2026-07-22（“grok2api有endpoint和apikey可以使用，你可以自己调用然后获取日志”）
计划版本变更: v1.14
```

### 已批准 Change Request：CR-P6-03-005

```text
CR-ID: CR-P6-03-005
原因: CR-P6-03-004 的受控服务器参考表明，本地 P6-03 所冻结的 Build 客户端身份和请求元数据已
      过时且不完整。用户已明确批准以该参考的当前、非秘密协议轮廓更新本地 Builder，并对固定
      `cli-chat-proxy.grok.com` 边界做新的双模式验收。
影响的 Task / Matrix ID / ADR: 仅 P6-03 的 Build 请求轮廓、合成测试、BC-PROVIDER-003、报告与
      C28 固定端点证据。更新静态客户端身份/版本、客户端标识与模式、模型覆盖和安全关联元数据，
      并在语义等价受测后允许恰好一个纯文本 user 输入采用标量编码。不改变固定 URL、OAuth
      账户、Canonical 请求/响应语义、P6-04+、服务器、代理或 TUN 配置。
兼容性与迁移影响: 无客户端、数据库、部署或账号迁移。关联值每次请求生成、仅存在于请求内存，
      `Debug`、报告、日志与 Git 不得输出其值；已知可用的 Build 模型值仅在忽略的操作员调用内存
      中获得和使用，不记录或提交。
测试与回滚变化: 先完成静态轮廓、标量输入语义等价、零泄露 Debug、精确 egress 和原有解码
      测试，再运行本地 full gate 与独立 review 并提交。之后仅由两个独立 harness 进程发送：
      `T11=(build-profile-02, non_streaming, direct)` 与
      `T12=(build-profile-02, sse, direct)`；每进程仍固定
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、固定短提示和 32 token 上限。T1-T10（含诊断）绝不
      重放，也不进行 retry、refresh、failover、候选选择或网络配置变更。任一非 2xx、超时、
      非预期内容类型、协议/语义错误或泄露防护失败均结束该元组；只有 T11 和 T12 均产生
      Canonical `ResponseStart`、文本及无 `StreamError` 的 `ResponseEnd` 才可使 P6-03 进入
      `LOCAL_PASS_PENDING_PHASE_GATE`，否则恢复 `BLOCKED`，P6-04 仍不得开始。
用户批准: APPROVED，2026-07-22（“批准”）
计划版本变更: v1.15
```

### 已批准 Change Request：CR-P6-03-006

```text
CR-ID: CR-P6-03-006
原因: 用户指出 T11/T12 的 4xx 可能与指定 OAuth 账号的额度或账号状态有关，并明确要求将该当前服务器
      CPA OAuth JSON 导入已部署的 grok2api 后调用测试。需要把该外部、持久化账号导入收敛为可归因的
      额度诊断，避免以共享账号池的生成结果错误推断该指定账号。
影响的 Task / Matrix ID / ADR: 仅补充 P6-03 报告的账号/额度诊断证据；不改变 C28 固定直连端点、
      T1-T12、P6-03 生产代码、Canonical 语义、BC-PROVIDER-003、P6-04+ 或任何 Phase 状态。
兼容性与迁移影响: 目标 OAuth 文件将通过 grok2api 受支持的管理员导入 API 持久化为外部服务账号；这是
      用户要求的唯一服务器写入。不得直接写 SQLite、导出凭据、修改路由、client key、优先级、账号
      enabled 状态、代理/TUN 或服务配置；不得在 Git、日志或报告中记录 OAuth 内容、JWT、内部 ID、
      API key、模型映射、请求/响应正文或原始 Header。
测试与回滚变化: 先仅在服务器内存中以 bootstrapAdmin 登录，上传恰好一个指定 OAuth 文件，随后以
      只读账号投影确定其内部关联，并恰好一次调用该账号的 `POST /accounts/{id}/refresh-quota`。该调用
      是唯一可证明绑定到导入账号的 Provider 测试。仅当现有服务无需新增 route/key、禁用其它账号或
      调整调度就能同样证明绑定时，才允许额外一次非流式生成；共享 `/v1/responses` 调用一律不作为
      此诊断证据。导入后的外部账号按用户意图保留；如需删除，必须有单独的显式服务器变更授权。无论
      结果如何都不重放 T1-T12，也不解除 P6-03 `BLOCKED` 或启动 P6-04。
用户批准: APPROVED，2026-07-22（“4xx可能是账号的额度的问题，你可以把这个账号…导入grok2api然后调用测试”）
计划版本变更: v1.16
```

### 已批准 Change Request：CR-P6-03-007

```text
CR-ID: CR-P6-03-007
原因: 本机已发现的 Grok Build OAuth 缓存均为过期副本，且与刚导入服务器诊断的账号不同；用户在得知
      需要重新认证后明确回复“可以”。需要以官方本机 CLI 重新获得可验证的本地 OAuth 会话，才可判断
      该会话是否能安全、正确地形成 P6 所需的标准 OAuth 输入。
影响的 Task / Matrix ID / ADR: 仅补充 P6-03 的本机认证诊断证据；不改变 C28、T1-T12、P6-03
      生产代码、固定端点、Canonical 语义、BC-PROVIDER-003、P6-04+、服务器、路由、代理或 TUN。
兼容性与迁移影响: 允许恰好一次本机官方 `grok login --oauth` 交互式认证。用户只在该正式认证页面中自行
      输入账号、密码、MFA/验证码；不得由代理读取、代填、输出、复制、提交或记录任何凭据、完整邮箱、
      JWT、Token、Refresh Token、Cookie 或会话标识。CLI 自身按其既有安全存储写入的本地会话是该
      用户操作的预期副作用；不得修改、重命名或手工转换缓存文件。
测试与回滚变化: 认证前后仅允许对已知本机 CLI 会话做安全投影：登录是否成功、缓存的字段命名类别、
      有效期类别，以及能否在不伪造字段的条件下取得 P6 严格导入所需的标准 OAuth 输入。不得调用模型、
      不得发送 `/v1/responses` 或任何 P6 harness、不得重放 T1-T12、不得刷新/测试服务器账号，也不得
      启动 P6-04。若认证未完成或安全投影仍不能形成正确输入，P6-03 保持 `BLOCKED`；任何新的直连验证
      均须另行登记 CR 和新 tuple。
用户批准: APPROVED，2026-07-22（“可以”）
计划版本变更: v1.17
```

### 已批准 Change Request：CR-P6-03-008

```text
CR-ID: CR-P6-03-008
原因: 用户要求 Grok Build 参考 CPA、grok2api、Sub2API 的源码做法，并把多个 OAuth 方案纳入本
      Provider。只读差分确认三者使用同一公开 xAI client ID 与 refresh grant；差异集中在合法 OAuth
      凭据的落盘/缓存形状：标准 token response、CPA xAI auth file、grok2api account credential 与官方
      Grok CLI indexed auth cache。
影响的 Task / Matrix ID / ADR: 重新打开 P6-03 为 `IN_PROGRESS`，仅扩展其凭据来源适配、合成测试、
      BC-PROVIDER-003 与 C28 的新版受控 live matrix。P6-01/02 的 Device Code、Refresh Singleflight、
      Revision/CAS 与 Secret 边界保留；不改变 Canonical Request/Event、固定 Responses URL、P6-04+、
      服务器、路由、代理或 TUN。
兼容性与迁移影响: 保留现有严格 `access_token` + `refresh_token` + `expires_in` JSON 导入；新增只接收
      明确版本化/已知形状的内存适配：(a) CPA/grok2api xAI OAuth JSON 的绝对 `expired` 或 `expires_at`；
      (b) 官方 Grok CLI 的精确 issuer+public-client indexed cache，其中 `key` 仅映射为当前 Bearer
      access token。所有绝对时间必须严格解析、未过期且安全转换为 P6 绝对毫秒；client ID 必须匹配
      P6 公开 client。Provider API 只接收 bytes、不得自行读取任意文件路径；忽略 harness 才可读取一份
      用户已登录的明确本机 cache 路径。不得写回/重命名/导出缓存，不得将 Token 放入命令行、环境变量、
      日志、报告、Fixture 或 Git。Sub2API 的 SSO/Cookie→Build 转换不是 OAuth 凭据来源，留在独立
      Grok Web 安全边界，未获本 CR 许可。
测试与回滚变化: 以纯合成 Fixture 覆盖四类来源（标准 JSON、CPA、grok2api、官方 CLI indexed cache）、
      RFC3339 绝对时间、client/issuer 不匹配、重复字段、多个匹配条目、过期/过长/未知形状和 Debug/
      persisted secret-redaction；不把真实 CPA 文件传至本机。完成定向测试、Clippy、格式、Secret scan、
      full local gate 和 review 后，才允许新版固定端点矩阵：`T13=(official-cli-cache-01, non_streaming,
      direct)` 与 `T14=(official-cli-cache-01, sse, direct)`，均使用已登录本机 cache、同一既有不记录值
      的 Build 候选、短提示、32 token、独立进程且每进程恰好一 send。不得 refresh、retry、failover、
      proxy/TUN 变更或重放 T1-T12；任一失败即停止，P6-03 恢复 `BLOCKED`，任何更多 tuple 须新 CR。
用户批准: APPROVED，2026-07-22（“针对grok build这个渠道，我建议你参考grok2api/cpa/sub2api这几个项目的源码是怎么操作的，当然可以有多个oauth方案，你可以都做进来；测试的账号我也给你了”）
计划版本变更: v1.18
```

### 已批准 Change Request：CR-P6-03-009

```text
CR-ID: CR-P6-03-009
原因: CR-P6-03-008 的 T13 唯一 harness 进程已在本地 wrapper 的环境变量展开错误处停止，且在
      `result=started`、credential import、DNS、HTTP 和 `send` 之前退出。用户明确批准登记一次
      修正 wrapper 后的替代验证；不能把零发送自动当作原 tuple 可重试。
影响的 Task / Matrix ID / ADR: 仅重新打开 P6-03 的 C28 live evidence、BC-PROVIDER-003、报告与
      traceability；不改变生产 Provider、固定 Responses URL、Canonical 类型、OAuth source adapter、
      P6-04+、服务器、路由、账号、代理或 TUN。T1-T13 及其历史结果保持关闭。
兼容性与迁移影响: 无。wrapper 只在单一子进程内以已赋值的本地 shell 变量向受控 `env -i` 传递
      非秘密的授权开关、cap、标签、模式、cache path 和既有不记录值的 model candidate；不把 Token、
      cache 内容、path、model 或响应值打印、写入 Git 或持久化。generic credential JSON、P6 SOCKS
      变量及环境 proxy 均不传入。
测试与回滚变化: 先恰好一次 ignored no-network preflight，以 T15 的同一配置读取最多 64 KiB cache 并
      只构造请求；它不得 DNS/HTTP/refresh/send，失败则停止且不发送 T15。预检通过后只登记
      `T15=(official-cli-cache-corrected-01, non_streaming, direct)`，独立进程、
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、固定短提示与 32 token，恰好一次 send。T15 的任一非
      Canonical success、超时、内容类型、协议、语义或安全失败均停止，T14 不得发送并使 P6-03
      `BLOCKED`。仅 T15 出现 Canonical `ResponseStart`、文本、无 `StreamError` 的 `ResponseEnd`
      时，才重新授权原本未发送的 `T14=(official-cli-cache-01, sse, direct)` 一次；T14 失败同样
      `BLOCKED`。不得 refresh、retry、failover、candidate selection、proxy/TUN change 或新增 tuple。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.19
```

### 已批准 Change Request：CR-P6-03-010

```text
CR-ID: CR-P6-03-010
原因: T15 的安全 `4xx` 不能归因，但其后的只读官方 CLI 静态审计已证明本地 Builder 当时硬编码的
      Linux shell User-Agent 与本机 `0.2.106` 官方 CLI 的 workspace 请求轮廓不一致。该常量已在
      已审查的本地提交中更正为版本化 workspace 值；需要一个不同的、有限的直连元组验证该修正，
      不能重放 T15 或把未发送的 T14 自动视为可发送。
影响的 Task / Matrix ID / ADR: 仅重新打开 P6-03 的 C28 live evidence、BC-PROVIDER-003、报告与
      traceability；不改变固定 URL、Canonical 类型、OAuth source adapter、Provider 生产接口、
      P6-04+、服务器、路由、账号、代理或 TUN。T1-T15 的历史结果保持关闭。
兼容性与迁移影响: 无。只使用已登录本机官方 CLI cache 和既有不记录值的 Build candidate；wrapper
      仅向受控 `env -i` 子进程传递非秘密授权、cap、标签、模式、cache path 和既有 candidate，
      不打印、写入 Git 或持久化 cache 内容、Token、模型、响应、请求体或原始头。generic credential
      JSON、P6 SOCKS5 和环境 proxy 变量均不得传入。
测试与回滚变化: 先恰好一次 ignored no-network preflight，以 T16 的相同配置读取最多 64 KiB cache
      并只构造请求；它不得 DNS/HTTP/refresh/send，失败则停止且不发送 T16。预检通过后仅登记
      `T16=(official-cli-cache-workspace-ua-01, non_streaming, direct)`：独立进程、
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、固定短提示、32 token、恰好一次 send。任一非 Canonical
      success、超时、内容类型、协议、语义或安全失败均停止并使 P6-03 `BLOCKED`，T17 不得发送。
      仅 T16 出现 Canonical `ResponseStart`、文本、无 `StreamError` 的 `ResponseEnd` 时，才条件
      允许 `T17=(official-cli-cache-workspace-ua-01, sse, direct)` 一次；T17 亦为独立进程且恰好
      一 send，任一失败同样 `BLOCKED`。不得 refresh、retry、failover、candidate selection、
      proxy/TUN change、T1-T15 replay 或新增 tuple。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.20
```

执行结果（2026-07-23）: 唯一 T16 无网络预检以 `InvalidTargetLabel` 停止。登记的标签超过既有
      harness 32 字符不透明标签上限，因此 `from_values` 在 credential/cache 导入、DNS、HTTP、refresh
      或 `send` 之前失败。不得修正标签后重跑该预检；T16 和条件 T17 均未发送，P6-03 恢复 `BLOCKED`。

### 已批准 Change Request：CR-P6-03-011

```text
CR-ID: CR-P6-03-011
原因: CR-P6-03-010 的唯一预检只在本地不透明标签长度门槛停止；该停止发生在 credential/cache
      导入、DNS、HTTP、refresh 和 send 之前，未消耗其登记的直连 tuple。用户明确批准以一个不同、
      合法的短标签完成受控验证，并在成功后继续 P6 的后续任务。
影响的 Task / Matrix ID / ADR: 仅重新打开 P6-03 的 C28 live evidence、BC-PROVIDER-003、报告与
      traceability；不改变 Provider 生产接口、固定 URL、Canonical 类型、OAuth source adapter、
      P6-04+、服务器、路由、账号、代理或 TUN。T1-T16 的历史结果保持关闭。
兼容性与迁移影响: 无。仅使用已登录本机官方 CLI cache 和既有不记录值的 Build candidate；wrapper
      只向受控 env -i 子进程传递非秘密授权、cap、短标签、模式、cache path 和 candidate，且不输出、
      写入 Git 或持久化 cache 内容、Token、模型、响应、请求体或原始头。generic credential JSON、
      P6 SOCKS5 和环境 proxy 变量均不得传入。
测试与回滚变化: 先恰好一次 ignored no-network preflight，以 T18 的相同配置、合法短标签
      `cli-cache-ua-01` 读取最多 64 KiB cache 并只构造请求；它不得 DNS/HTTP/refresh/send，失败则
      停止且不发送 T18。预检通过后仅登记
      `T18=(cli-cache-ua-01, non_streaming, direct)`：独立进程、
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、固定短提示、32 token、恰好一次 send。任一非 Canonical
      success、超时、内容类型、协议、语义或安全失败均停止并使 P6-03 `BLOCKED`，T19 不得发送。
      仅 T18 出现 Canonical `ResponseStart`、文本、无 `StreamError` 的 `ResponseEnd` 时，才条件允许
      `T19=(cli-cache-ua-01, sse, direct)` 一次；T19 同为独立进程且恰好一 send，任一失败同样
      `BLOCKED`。只有 T18/T19 均成功，才可按依赖顺序开始 P6-04 至 P6-08。不得 refresh、retry、
      failover、candidate selection、proxy/TUN change、T1-T16 replay 或新增 tuple。
用户批准: APPROVED，2026-07-23（“测试完成后如果没有问题就继续完成 P6 剩下的内容”）
计划版本变更: v1.21
```

执行结果（2026-07-23）: 唯一 T18 无网络预检通过；它读取受限官方 CLI cache 并构造固定请求，未做
      DNS、HTTP、refresh 或 send。随后唯一 T18 独立直连进程在 2.66 秒到达固定端点并安全停止为
      `4xx / error_like_object / unrecognized`，没有 Canonical `ResponseStart`、文本和无
      `StreamError` 的 `ResponseEnd` 生命周期。T19 未发送；不得重试 T18、发送 T19 或开始 P6-04，
      P6-03 恢复 `BLOCKED`。

### 已批准 Change Request：CR-P6-03-012

```text
CR-ID: CR-P6-03-012
原因: T18 已到达固定端点，但脱敏 `4xx / error_like_object / unrecognized` 不能单独归因到 OAuth
      权限、账号权益/额度、模型可用性或请求轮廓。用户批准先做有限的只读诊断，再决定是否需要新的
      修复或验证 CR。
影响的 Task / Matrix ID / ADR: 仅重新打开 P6-03 的故障归因证据、BC-PROVIDER-003、报告与
      traceability；不改变生产 Provider、固定 URL、Canonical 类型、OAuth source adapter、
      P6-04+、服务器、路由、账号、代理或 TUN。T1-T18 保持关闭，T19 仍不具备发送条件。
兼容性与迁移影响: 无。只读检查已有的本机官方 CLI 状态/日志和已有服务器日志或其安全状态投影；
      不打开新的 debug 日志、不读取或输出 Token、cache path、模型、请求/响应体或原始 headers，
      不写入账户、数据库、日志、Git、服务器或网络配置。
测试与回滚变化: 最多检查一个本机官方 CLI 的既有状态来源与一个既有服务器日志/状态来源；只输出
      可审计的无值类别。禁止 DNS、HTTP、Provider send、OAuth refresh、官方 CLI 交互、server action、
      retry、failover、candidate selection、proxy/TUN change、T18 replay 或 T19。若没有足够的既有
      证据，结论必须为 `unattributed` 并使 P6-03 恢复 `BLOCKED`；不得以诊断为由新增请求。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.23
```

执行结果（2026-07-23）: 本机已有的官方 CLI 状态证据仅证明 indexed cache 通过严格导入并且未过期；
      它不能证明上游已接受该次请求。经配置的字面 IP SSH 做只读容器状态检查，grok2api 名称匹配
      不是唯一实例，因此没有读取日志、也没有可与本机直连 T18 关联的服务器证据。未执行 DNS、HTTP、
      Provider send、OAuth refresh、CLI 交互、服务/账号/代理变更。结论为 `unattributed`；P6-03
      恢复 `BLOCKED`，T19 及 P6-04 仍不得开始。

### 已批准 Change Request：CR-P6-03-013

```text
CR-ID: CR-P6-03-013
原因: 用户明确批准“完成 P6 所有要求”，并确认此前 P6-03 的单 tuple 停止规则不再阻塞后续
      本地实现、验证和 clean-room 差分工作。T18 已有的安全 4xx 仍不能被伪称为直接成功。
影响的 Task / Matrix ID / ADR: 解锁 P6-04 至 P6-08；新增 P6 runtime-state migration、
      ADR-0045、BC-PROVIDER-004 和差分证据。P6-03 的 fixed URL、Canonical 行为、OAuth 凭据
      边界、T1-T18 历史和 T19 发送条件均不改变。
兼容性与迁移影响: 新增 schema version 7 的 Provider-private Billing/catalog/quota、affinity、
      ownership 与 AEAD replay 表；Build cache key 改为版本化 tenant HMAC identity，原始客户端
      key 不再可直接发送上游。无服务器、账号、代理/TUN、路由或管理 HTTP 变更。
测试与回滚变化: P6-04 至 P6-07 的合成隔离、单调性、加密和错误矩阵必须通过；P6-08 只允许
      结构化无值服务器证据。G6 仍需本地 Full gate、独立 review 和一次 Phase Delivery Gate。
      回滚移除 version 7 与 P6 Provider-private modules，不重放任一真实 tuple。
用户批准: APPROVED，2026-07-23（“批准”；继续完成 P6 所有要求）
计划版本变更: v1.25
```

### 已批准 Change Request：CR-P6-03-014

```text
CR-ID: CR-P6-03-014
原因: 重新审计发现 P6-03 原有的 Builder/Decoder 与运行时状态没有实现可由网关执行的
      `InferenceAdapter`，且 T18 未满足 C28 的真实双模式通过条件。随后，本机官方 Grok CLI
      `0.2.106` 使用当前 OAuth 会话实际完成了一个最小请求；其安全会话投影表明调用模型为
      `grok-4.5`，完成模型为 `grok-4.5-build`。需要补齐可注入执行链路，并以此当前、已验证
      的模型选择登记新的验收 tuple；这不是对 T18 的重放。
影响的 Task / Matrix ID / ADR: 重新打开 P6-03 的 `C28`；新增 Provider-private
      `InferenceAdapter`、DNS-pinned transport 注入和 Router mode bridge，以及 loopback/fixture
      E2E。固定 URL、OAuth 来源、Canonical 类型、P6-04 至 P6-08 的私有状态模型不变。
兼容性与迁移影响: 无公开 API、数据库、服务器、账号、代理或 TUN 变更。应用入口不读取 OAuth
      文件；credential、transport、egress policy 和 resolver 都由运行时显式注入。不会记录 token、
      cache path、模型映射、请求/响应正文或 header 值。
测试与回滚变化: 在真实发送前必须通过 adapter 非流式/SSE 分块、Router 选择和失败分类 E2E，
      然后仅登记 `T20=(official-cli-current-01, non_streaming, direct)`；它由独立进程、
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、本机官方 cache、`grok-4.5`、固定短提示和 32 token
      构成，恰好一 send、无 refresh/retry/failover。只有 T20 得到 Canonical `ResponseStart`、
      文本和无 `StreamError` 的 `ResponseEnd` 时，才允许 `T21=(official-cli-current-01, sse,
      direct)` 一次；T21 使用相同限制。任一失败则 P6-03 保持 `BLOCKED`，并且不得启动 P7。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.26
```

### 已批准 Change Request：CR-P6-03-015

```text
CR-ID: CR-P6-03-015
原因: T20 已确认当前官方 CLI OAuth cache 与 `grok-4.5` 模型选择到达固定端点并取得 `2xx` 和
      预期 JSON Content-Type，但严格 Responses 解码在任何 Canonical 事件前停止。保留的安全输出
      没有 2xx JSON 的对象类别，无法区分成功对象格式漂移、包装对象或 2xx 错误对象。
影响的 Task / Matrix ID / ADR: 仅 P6-03 C28 诊断 harness 和报告；不改变生产 Builder、Adapter、
      URL、OAuth、Canonical 类型、状态表、服务器、账号、代理/TUN 或 T1-T20 历史。
兼容性与迁移影响: 无。新增投影只可能输出 `responses_object`、`wrapped_response_object`、
      `error_like_object`、`chat_choices_object`、`other_object`、`non_object` 或 `invalid_json`；它不
      输出 key 全集、值、请求/响应文本、ID、token、模型映射或 headers。
测试与回滚变化: 先以合成私有值证明投影无值；随后仅登记
      `T22=(official-cli-current-shape-01, non_streaming, direct)`，仍为独立进程、32 token、
      一 send、无 refresh/retry/failover。T22 只缩小归因，不能通过 C28，且 T21 仍不得发送。
      若无法形成明确修复，P6-03 为 `BLOCKED`；回滚仅移除该 ignored-harness 投影。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.27
```

### 已批准 Change Request：CR-P6-03-016

```text
CR-ID: CR-P6-03-016
原因: T22 的 `invalid_json` 是在应用当前已允许的 non-streaming Content-Encoding 之前对原始
      bytes 做的安全投影；它不能区分 gzip 压缩的成功 JSON 与实际无效响应。需要在与生产 decoder
      相同的 1 MiB 有界 gzip/identity 步骤之后，才投影固定对象类别。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored diagnostic harness 和报告；不改变 Builder、
      Adapter、固定 URL、OAuth、Canonical、状态、服务器、账号、代理/TUN 或历史 tuple。
兼容性与迁移影响: 无。投影仅增加 `content_encoding` 的 `identity`/`gzip`/`other_or_missing` 类别，
      以及 `decode_failed`；不输出 header 值、压缩/解压 bytes、JSON key/value、文本、token 或模型。
测试与回滚变化: 合成测试覆盖 private JSON 与安全无值类别；仅登记
      `T23=(official-cli-current-decoded-shape-01, non_streaming, direct)`，独立进程、一 send、
      无 refresh/retry/failover。它不能通过 C28，T21 仍不得发送；若诊断不足，P6-03 `BLOCKED`。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.28
```

### 已批准 Change Request：CR-P6-03-017

```text
CR-ID: CR-P6-03-017
原因: T23 已确认响应在 gzip 解压后是 `responses_object`，但严格 decoder 仍在产生任何
      Canonical Event 前停止。需要一次镜像 decoder 核心固定前置条件的无值 projection，避免将
      未知 output 类型、未完成状态或无效 item 误判为 OAuth/transport 故障。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored diagnostic harness；不变更生产 Builder、
      Adapter、URL、OAuth、Canonical、状态、服务器、账号、代理/TUN 或任何关闭的 tuple。
兼容性与迁移影响: 无。结果仅为预定义 requirement label，不输出 response/item ID、文本、字段值、
      token、model、arguments、header 或 JSON key 集合。
测试与回滚变化: 合成 fixture/私有值测试后，只登记
      `T24=(official-cli-current-core-shape-01, non_streaming, direct)`，独立进程、一 send、
      无 refresh/retry/failover。它不能通过 C28，T21 仍不得发送；无法修复即 P6-03 `BLOCKED`。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.29
```

### 已批准 Change Request：CR-P6-03-018

```text
CR-ID: CR-P6-03-018
原因: T24 将 current Build 的 2xx Responses JSON 失败缩小为 `reasoning_content_invalid`。
      需要只观察 reasoning item 的固定 content 类别，确定是否应安全忽略空 reasoning，或映射一个
      已知的非敏感 text 变体；不读取 reasoning 内容本身。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored diagnostic harness/报告；生产 API、URL、OAuth、
      Canonical、状态、服务器、账号、代理/TUN 和关闭 tuple 均不变。
兼容性与迁移影响: 无。唯一输出是 `missing_content`、`empty_content`、`reasoning_text`、
      `summary_text`、`text`、`missing_text` 或 `other_or_missing` 等固定类别。
测试与回滚变化: 合成投影检查后，仅 T25 一次直连 non-streaming send；它不能通过 C28，T21 未授权。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.30
```

### 已批准 Change Request：CR-P6-03-019

```text
CR-ID: CR-P6-03-019
原因: T25 将差异精确定位为 completed reasoning item 可合法缺少 `content`。该 item 没有可投影的
      reasoning 文本；将其映射为零 `ReasoningDelta` 保留 response lifecycle，且不伪造内容。
影响的 Task / Matrix ID / ADR: P6-03 decoder 兼容性、合成 fixture、C28 live evidence；不变更 URL、
      OAuth、请求 profile、Canonical 公共类型、运行时状态、服务器、账号、代理/TUN 或历史 tuple。
兼容性与迁移影响: 无。缺失 `content` 仅对 `reasoning` completed item 接受；message 和 Tool 仍 fail-closed。
测试与回滚变化: 必须通过零 Delta fixture、既有 arbitrary-chunk/non-streaming equivalence、adapter
      E2E 和 Clippy；随后仅 T26 non-streaming 一 send。只有它得到完整 Canonical 成功时才允许 T27 SSE。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.31
```

### 已批准 Change Request：CR-P6-03-020

```text
CR-ID: CR-P6-03-020
原因: T27 的固定端点 SSE 取得 2xx/预期 Content-Type，但严格 decoder 在任何可验收生命周期完成前
      停止。需要只输出最后一个完整 SSE record 的预定义事件类别，判断是未知事件还是已知事件 payload
      差异；不读取 data 值。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored SSE diagnostic harness/报告；生产接口、URL、OAuth、
      Canonical、状态、服务器、账号、代理/TUN 和历史 tuple 均不变。
兼容性与迁移影响: 无。投影仅输出预定义 record 类别，未知值合并为 `unknown_or_malformed`。
测试与回滚变化: 仅 T28 SSE 一 send；不通过 C28，未有具体修复前不再发送新的成功验收 SSE tuple。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.32
```

### 已批准 Change Request：CR-P6-03-021

```text
CR-ID: CR-P6-03-021
原因: T28 的 `unknown_or_malformed` 类别不足以区分已知标准 Responses 的 content-part、output-text
      completion 和 reasoning-summary event families。仅扩展固定白名单类别后，才可决定精确兼容修复。
影响的 Task / Matrix ID / ADR: 仅 ignored SSE diagnostic harness/报告；其它行为和历史 tuple 不变。
兼容性与迁移影响: 无。未知名称继续合并，无原始 event/data 输出。
测试与回滚变化: 仅 T29 SSE 一 send；不能通过 C28，具体修复前不再发送验收 tuple。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.33
```

### 已批准 Change Request：CR-P6-03-022

```text
CR-ID: CR-P6-03-022
原因: T29 识别了 standard `response.reasoning_summary_text.delta`，其为现有 `ReasoningDelta` 的
      明确语义同类。配套 summary-part 只携带结构边界，受限验证后不产生合成文本。
影响的 Task / Matrix ID / ADR: P6-03 SSE decoder/fixture 和 C28；不改变 OAuth、请求、URL、状态、
      服务器、账号、代理/TUN 或关闭的 tuple。
兼容性与迁移影响: summary text delta 映射为 Canonical ReasoningDelta；summary part 仅验证
      reasoning item identity 和 object 结构。其它未知 SSE event 仍 fail-closed。
测试与回滚变化: 定向 decoder/adapter tests 后，T30 为唯一修复后 SSE send；通过才闭合 C28。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.34
```

### 已批准 Change Request：CR-P6-03-023

```text
CR-ID: CR-P6-03-023
原因: T30 在已验证的 summary 兼容修复后仍以 SSE protocol failure 停止；原 diagnostic observer 在把
      一整个上游 transport chunk 交给 decoder 后才记录 record，因此其“最后事件”可能晚于真正失败的
      record，不能据此修改 strict decoder。需要把观察和 decoder 都按单字节推进，并只投影该精确
      完成 record 的固定 event 类别及 output-item 安全形状。
影响的 Task / Matrix ID / ADR: 仅 P6-03 C28 的 ignored SSE diagnostic harness、合成观察测试和
      报告；不改变生产 Builder/Adapter/decoder、OAuth、请求 profile、URL、Canonical、状态、服务器、
      账号、代理/TUN 或关闭的 T1-T30。
兼容性与迁移影响: 无。输出仅能为既有有限 SSE event 类别和 `message_valid_id`、
      `reasoning_valid_id`、`function_call_valid_id`、`other_or_invalid`、`not_output_item`；不得输出
      record/body/header、ID、字段值、文本、token、模型或 cache path。observer 的逐字节合成测试
      证明 transport 分块不影响此诊断归因。
测试与回滚变化: 定向 harness test/Clippy/docs check 通过后，仅登记
      `T31=(official-cli-current-sse-byte-shape-01, sse, direct)`；独立进程、官方 CLI cache、
      `grok-4.5`、32 token、一 send、无 refresh/retry/failover。T31 仅定位，不能通过 C28；若它
      显示明确兼容差异，必须先写 strict synthetic fixture/test，再以新的 CR 登记替代 SSE 验收。
      无明确差异则 P6-03 保持 IN_PROGRESS/BLOCKED，不启动 P7。回滚只移除该 ignored observer。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.35
```

### 已批准 Change Request：CR-P6-03-024

```text
CR-ID: CR-P6-03-024
原因: T31 以逐字节归因确认 strict decoder 停在 standard `response.reasoning_summary_text.done`，而非
      output-item 形状。clean-room 本地 Responses 行为参考及其合成 fixture 表明这是 summary text 的
      终态确认事件：它携带与已累积 delta 相同的完整 text。此前 decoder 只接受 delta，因而错误地将
      该已知终态当作未知事件。
影响的 Task / Matrix ID / ADR: P6-03 C28 的 SSE decoder/fixture 和替代验收 tuple；不改变 OAuth、
      请求、URL、Adapter/Router、Canonical 公共类型、状态、服务器、账号、代理/TUN 或关闭 T1-T31。
兼容性与迁移影响: 只对已登记 reasoning item 接受此 event，要求 string `text`。若已有 delta，
      `text` 必须与按顺序累积值完全一致且不重复 Canonical delta；若上游只给该终态 text，则仅映射
      一次 `ReasoningDelta`。错 item、缺失 text、冲突 text 仍 fail-closed。message/tool 终态不放宽。
测试与回滚变化: 合成成功 fixture 覆盖 matching delta/done 且断言不重复事件；mismatch fixture 必须
      是 UpstreamProtocolError；既有 arbitrary-chunk、adapter/Router 与 Clippy 必须通过。随后仅登记
      `T32=(official-cli-current-sse-summary-done-01, sse, direct)`，独立进程、官方 CLI cache、
      `grok-4.5`、32 token、一 send、无 refresh/retry/failover。仅 T32 的 ResponseStart/text/
      无 StreamError ResponseEnd 可闭合 C28；失败时 P6-03 保持 IN_PROGRESS/BLOCKED，不启动 P7。
      回滚只移除该 event 分支和 fixtures。
用户批准: APPROVED，2026-07-23（此前无次数上限的真实测试授权及本轮“批准”）
计划版本变更: v1.36
```

### 已批准 Change Request：CR-P6-03-025

```text
CR-ID: CR-P6-03-025
原因: T32 在 summary terminal 兼容后继续到 `response.content_part.added` 才停止，证明上次修复已生效。
      当前 decoder 只消费 text delta/最终 output item，遗漏标准 Responses 的 text content-part 边界和
      adjacent `output_text.done`/`reasoning.done` 完整文本确认。已从 clean-room 本地行为参考和合成
      byte-chunk fixture 确认该序列的严格结构。
影响的 Task / Matrix ID / ADR: 仅 P6-03 C28 SSE decoder/fixtures 和替代验收 tuple；不改变 OAuth、
      请求、URL、Adapter/Router、Canonical 公共类型、状态、服务器、账号、代理/TUN 或关闭 T1-T32。
兼容性与迁移影响: 只接受已登记 message item 的 `output_text` part；added 必须为空 text，done 必须
      匹配已累积 text（无 delta 时才映射一次 TextDelta），且 part 未关闭时 output item 不得完成。
      `output_text.done` 使用相同完整文本确认；`reasoning.done` 复用已验证的 reasoning terminal rule。
      refusal/未知 part、错误身份、非空 added、重复/未关闭 part、缺失/冲突 text 全部 fail-closed。
测试与回滚变化: byte-by-byte 完整 lifecycle fixture 必须证明一次 TextDelta/ResponseEnd；非空 part
      start fixture 必须是 UpstreamProtocolError；summary matching/mismatch、adapter/Router、Clippy 和
      docs check 必须通过。随后仅登记 `T33=(official-cli-current-sse-content-part-01, sse, direct)`，
      独立进程、官方 CLI cache、`grok-4.5`、32 token、一 send、无 refresh/retry/failover。仅 T33 的
      ResponseStart/text/无 StreamError ResponseEnd 可闭合 C28；失败则 P6-03 保持 IN_PROGRESS/BLOCKED，
      不启动 P7。回滚只移除该标准 text-content event 处理和 fixture。
用户批准: APPROVED，2026-07-23（此前无次数上限的真实测试授权及本轮“批准”）
计划版本变更: v1.37
```

### 已批准 Change Request：CR-P4-G4-001

```text
CR-ID: CR-P4-G4-001
原因: G4 复核发现 P4 已有 Health、Quota、Circuit、Route Explain 与事件能力，但没有可供管理
      进程只读查询的统一状态投影；`CredentialForbidden` 也不会转换为可恢复的精确账户状态。
      因而不能证明 G4 的“403 账号状态、429、Quota、Circuit 和恢复可见”条件。
影响的 Task / Matrix ID / ADR: 新增 P4-10；Matrix G20、G21、G26、H19、H20；新增 ADR-0032
      与 BC-MGMT-001。P4-04/P4-05/P4-06 的既有运行时状态与 Explain 被只读组合，不改其
      已验收的目标范围。
兼容性与迁移影响: 新增 `gateway-router` 的 in-process 只读管理查询和绑定级 403 账户状态/受控
      恢复 API；无 HTTP 路由、认证机制、Provider 请求、SQLite schema、Canonical Event、数据或
      部署迁移。P10 仍独占认证 HTTP 管理 API、Web UI 与持久化管理读模型。
测试与回滚变化: P4-10 必须证明 403 精确绑定隔离、恢复票据、429/Quota source-confidence、
      Circuit/Recovery、Route Explain 可见性、读取无副作用、调度 fail-closed 与 Secret-safe Debug。
      回滚移除新状态查询与账户恢复状态；既有 Health/Quota/Explain/SQLite/Event 行为不变。
用户批准: APPROVED，2026-07-21（用户选择“新增一个只读、非 HTTP 的管理状态查询边界”）
计划版本变更: v1.8
```

### 1.13 G4 只读管理状态边界（CR-P4-G4-001）

1. `P4-10` 的管理 API 是供受控管理进程调用的 in-process Rust 查询边界，不注册 HTTP route，
   不处理认证、不会读/写 SQLite、不会发送 Provider 请求，也不进入 HTTP/Router 响应热路径。
2. 查询固定在调用方提供的观察时间，返回精确 Endpoint/Credential（可选 model）上的结构化
   Health、403 账户、Quota/429 source-confidence、Circuit 与受控恢复状态；错误必须是无目标、
   无 URL、无 Header、无 Body、无 Secret 的 fail-closed 类别。
3. 已验证的 Provider/driver 只有在给出既有 `CredentialForbidden` 安全错误时才记录 403 账户状态；
   它不透明重试。恢复必须经非克隆票据完成，普通流量在恢复完成前保持不可调度。
4. P10 才能将该边界暴露为认证 HTTP 管理接口、UI、持久化查询或远程控制面；P4-10 不提前实现
   其中任何一项。

### 已批准 Change Request：CR-P7-G7-001

```text
CR-ID: CR-P7-G7-001
原因: P7-01 至 P7-09 的本地实现、定向测试、完整本地门禁和原生 Adapter 垂直链路已经通过；
      唯一未闭合的 P7 工作是外部 Kiro 账号重新认证后才能进行的真实 CLI/IDE 验证，当前安全投影
      为 `auth_failed`。用户批准在该外部阻塞期间先开启 P8，以避免本地开发停滞。
影响的 Task / Matrix ID / ADR: 仅调整 P7→P8 的执行顺序：P8-01 至 P8-06 可在同一 `codex/p8-official`
      分支上按自身依赖进行本地实现、review 与测试。P7/G7、P8/G8、既有 Matrix、ADR、公开 API、
      Canonical 类型、Provider 协议、Schema、数据库、部署、Secret 规则及真实请求授权均不改变。
兼容性与迁移影响: 无客户端、数据或部署迁移；不切换任何生产流量。P7 继续是
      `BLOCKED_AUTH_REAUTH_REQUIRED`，不因 P8 的本地进度而成为完成状态。
测试与回滚变化: 每个 P8 Task 仍须按既定本地定向测试、review、格式和 Secret 检查执行；真实
      Official E2E 仍需测试账号与明确授权。P8 不得执行 Phase closeout、创建正式 Delivery tag、
      推送 Phase Delivery Gate、合并、发布或宣称 `DONE`，直到 G7 真实通过且 P7 的正式 Delivery
      Gate 成功。若 P7 认证复核出现本地实现回归，立即冻结 P8，从最早受影响的 P7 提交修复；
      回滚为将 P8-01 至 P8-06 恢复 `PENDING` 并停止 P8 分支工作。
用户批准: APPROVED，2026-07-23（“P7就只剩kiro账号的oauth吗？如果是的话可以先开启P8”）
计划版本变更: v1.38
```

### 1.14 P7 阻塞期间的 P8 本地开发边界（CR-P7-G7-001）

> 历史记录：本节中将 P8/G8 与 G7 绑定的顺序约束，已由 §1.15 的
> `CR-P7-DEFER-002` 完整替代；当前执行仅以 §1.15 为准。

1. 全计划仍然最多一个 `IN_PROGRESS` 代码 Task；P7-09 是 `BLOCKED`，P8-01 至 P8-06 的本地
   序列已完成，因此当前没有 `IN_PROGRESS` 代码 Task。不得由此开启 P9 或跳过 P7/G7。
2. P7 的 G7 和其 Phase Delivery Gate 保持 fail-closed。P8 的每项本地证据仅为后续 P8 本地 Task
   的前置条件，不能跨越 G7 作为下一 Phase、合并、发布或完成的证据。
3. P8 的真实 Official E2E 与 P8 closeout 均不得早于 G7；本 CR 不新增 Endpoint、Credential、
   Probe 或额度授权。外部账号、真实流量与现有服务器均不得因先行开发而修改。
4. Kiro 账号完成重新认证后，优先完成 P7-09、G7 和 P7 的唯一正式 Delivery Gate；通过后再以
   已保存的 P8 本地证据继续常规 P8 验收。

### 已批准 Change Request：CR-P7-DEFER-002

```text
CR-ID: CR-P7-DEFER-002
原因: 用户明确确认 P8 是独立的 Grok Official API-key Provider，而非 Kiro OAuth 工作；要求先跳过
      Kiro 账号认证，完成其余开发后再回补 P7。原先 CR-P7-G7-001 将 P8 收口与 G7 绑定只是流程
      顺序，不是技术或认证依赖，现予以解除。
影响的 Task / Matrix ID / ADR: P7-09/G7 保持 BLOCKED；P8-01 至 P8-06、G8、P8 Delivery Gate
      以及后续 P9-P12 的非 Kiro 依赖路径可继续。P8 的前序 Gate 改为 G6；P9 仍依赖 G8，P10-P12
      仍各自依赖前一非 Kiro Phase Gate。既有 P8 ADR/Contract/Task 的功能矩阵、公开 API、Canonical
      类型、Provider 协议、Schema、数据库和 Secret 规则均不改变。
兼容性与迁移影响: 不启用、删除或修改 Kiro Provider、账号、路由、Credential、服务器或生产流量。
      Kiro 保持不可调度，直到独立完成 P7-09/G7。P8 与后续 Phase 不得把本地 Fixture 当作真实 xAI
      行为证明；任何 Official live E2E 仍必须使用 P8 自身测试 Credential 和单独明确授权。
测试与回滚变化: P8 必须完成其 G8 本地验收、Phase review 和一次正式远端 Delivery Gate；P9-P12
      各自保持原有 Gate。若非 Kiro Phase 发现其与 P7 有实际代码依赖，冻结该 Phase 并先修复最早
      受影响项。回滚为恢复 CR-P7-G7-001 的顺序约束，不影响 P7 已有 BLOCKED 证据。
用户批准: APPROVED，2026-07-23（“P8不是grok的认证吗，应该跟kiro无关吧，还是先跳过kiro的认证，等后续其他都开发完了之后再回来完成认证”）
计划版本变更: v1.39
```

### 1.15 P7 延后与独立 Phase 收口边界（CR-P7-DEFER-002）

1. P7-09、G7 和 P7 Delivery Gate 保持 `BLOCKED_AUTH_REAUTH_REQUIRED`；它们不再是 P8-G12 的
   流程前序条件。Kiro Provider 继续不得进入测试/生产路由或宣称完成。
2. P8 在完成 G8 本地验收与自己的唯一 Phase Delivery Gate 后，可按常规 Definition of Done 进入
   `DONE`；P9-P12 各自仍必须等前一**非 Kiro** Phase Gate 成功。
3. 本 CR 只解除 Kiro 对顺序的阻塞，不授予 xAI 请求、API-key、配额、服务器、路由或发布以外的
   权限。Official live E2E 的证据若被 P8 Gate 要求，仍必须先以独立授权登记。

### 已批准 Change Request：CR-P8-DEFER-001

```text
CR-ID: CR-P8-DEFER-001
原因: 用户当前没有 xAI Official API Key，明确要求先跳过 P8 Official 的真实 E2E，并在其余开发
      完成后与 P7 Kiro OAuth 认证一并回补。P8-07 的本地安全 harness 已完成，但不得以 Fixture
      或未持有的 Credential 代替 Provider Gate 的真实证据。
影响的 Task / Matrix ID / ADR: 仅将 P8-07 与 G8 从 BLOCKED 改为 DEFERRED 外部认证工作；
      关联 ADR-0060、BC-E2E-004、§20.1 的真实 E2E 要求和已提交的零网络 harness 均不改变。
      P7-09 同样保持 DEFERRED。P9-P12 的既有 Gate 依赖、公开 API、Provider 协议、Schema、
      数据库、服务器、路由与生产流量均不改变。
兼容性与迁移影响: 无。不会搜索、导入、生成或替代 Official API Key，不发 xAI 请求，也不改变
      Kiro/OAuth、代理/TUN、服务器、账号或部署。
测试与回滚变化: 已有本地默认零网络测试和 Full gate 仍有效。恢复时先将 P8-07 复位为 BLOCKED，
      再登记 Official key/model/单请求授权；不得因延期而开始 P9 或声称 P8/G8 完成。回滚只恢复
      P8-07/G8 的 BLOCKED 等待状态，不执行外部请求。
用户批准: APPROVED，2026-07-23（“我也没有这个official的api key，可以先跳过，等后面跟kiro一起再测试”）
计划版本变更: v1.41
```

### 1.16 P8 Official E2E 延后边界（CR-P8-DEFER-001）

1. P8-07 与 G8 是 `DEFERRED`，不是 `DONE`、本地通过替代品或对 xAI 行为的断言。P8-01 至
   P8-06 的本地证据保持有效，但不能越过 §20.1。
2. P8-07 与 P7-09 在其他本地 Phase 全部完成后进入同一外部认证验收包；两者仍使用各自的
   Credential、授权与验收标准，不互相替代。
3. 本项原 P9 顺序约束已由 §1.17 的 `CR-P9-LOCAL-001` 替代；P10-P12 仍依赖各自的前序非 Kiro Gate。
4. P12/G12 之后、任何包含 Kiro 的发布前，必须回到 P7-09，完成 OAuth、G7 及 P7 Delivery Gate。
   不得以延后状态永久遗漏 Kiro Provider 验收。

### 已批准 Change Request：CR-P9-LOCAL-001

```text
CR-ID: CR-P9-LOCAL-001
原因: 用户要求继续按照最新开发计划完成 P9，而 P8-07/G8 的唯一缺口是没有 Official API Key 的
      外部 E2E，已明确延后。P9 的独立 Web Provider 本地实现不读取、不改变、也不依赖该 Credential。
影响的 Task / Matrix ID / ADR: 允许 P9-01 至 P9-08 按自身依赖顺序进行实现、review、定向测试和
      本地 Full gate；P9-09、G9、P9 Delivery Gate、合并、发布及 `DONE` 仍需 P9 自身的真实 Web
      账号/Canary 授权。P10-P12 的 Gate 依赖不变。既有 P8/P7 状态、公开 API、Canonical 类型、
      Provider 协议、Schema、数据库、服务器、路由、Secret 规则和生产流量均不改变。
兼容性与迁移影响: 无。P9 开发不搜索 Cookie/SSO/浏览器 Profile/代理/TUN/服务器文件，不发送 Web
      请求，不改变账号、出口、Feature Flag 或生产配置。
测试与回滚变化: 每个 P9 本地 Task 仍执行适用的格式、Clippy、定向测试、Secret scan 和 review；
      P9-09 保持 DEFERRED，只有显式测试账号/Canary 授权才可恢复。若发现对 P8 或 P7 的实际代码
      依赖，冻结 P9 并修复最早受影响项。回滚为恢复 P9-01 至 P9-08 为 PENDING，不发送外部请求。
用户批准: APPROVED，2026-07-23（“继续按照最新的开发计划完成P9”）
计划版本变更: v1.42
```

### 1.17 P9 本地开发边界（CR-P9-LOCAL-001）

1. P9-01 至 P9-08 可以以 P8 的已验证本地隔离证据为前置，不能将 P8/G8 的延期状态误写成通过。
2. P9-09 是 `DEFERRED`：无真实 Web 测试账号、明确 Canary、单独流量授权时，不进行 Cookie/SSO
   导入、浏览器自动化、Web API/Statsig/gRPC-Web 请求或出口变更。
3. P9 的任何本地 fixture、差分或安全 harness 不证明远程 Web 协议、额度、WAF、Cookie 或账户状态。
4. P9-01 至 P9-08 通过后只可进入 `LOCAL_PASS_PENDING_PHASE_GATE`；不得创建 P9 Delivery tag、
   推送远端 Gate、合并、发布或宣称 P9/G9 `DONE`，直到 P9-09 与 G9 的真实证据补齐。

### 已批准 Change Request：CR-P9-CANARY-001

```text
CR-ID: CR-P9-CANARY-001
原因: 用户指定“账号去grok2api里面拿web账号就行”，以当前服务器 grok2api 的 active `grok_web/sso`
      账号补齐 P9-09 所需的独立 Web 测试来源。只读元数据和受控导出已确认其为 Web SSO，而不是
      Build OAuth、Official API Key 或 Kiro 凭据。
影响的 Task / Matrix ID / ADR: 仅恢复 P9-09 与 G9 的 Canary 实施；覆盖 P9 Matrix
      `C29-C34`、`D28-D30`、`E27-E29`、`F17`、`G24-G28` 及 ADR-0061 至 ADR-0068。
      不解除 P7-09/G7、P8-07/G8 的外部认证延期，也不提前开始 P10。
Canary 边界: 至多三次真实 `POST`，固定为一个临时 grok2api `grok_web/sso` 导出的独立 SSO
      生命周期、固定 `https://grok.com` Web Chat 目标、短文本、无附件、默认关闭 Tool emulation。
      第一次只发现受限协议形状；后续仅在前次结果支持时用于本仓库的非流式/SSE Canonical
      验证。熔断/403/协议漂移演练优先复用本地 P9-06/P9-07 确定性状态，不为了制造失败额外发送
      请求。任何凭据、Cookie、Authorization、email、账号 ID、完整 URL Query、上游 Body 和原始
      响应均不得输出、持久化或提交。
兼容性与迁移影响: 导出只存在于受控临时内存/进程生命周期；不导入数据库、不创建路由、Client Key、
      Server 配置、浏览器 Profile、代理/TUN 变更或生产开关。真实观察只能更新 P9-09 的脱敏报告，
      不能改变 Build/Official/Kiro 的状态。
测试与回滚变化: 新增默认 ignored、显式授权、严格最多三次的 P9-09 harness；每次请求前必须通过
      精确目标、HTTPS/DNS-pinned Egress、固定超时、零值日志和一条请求计数审查。失败时停止，不重试
      同一元组，并将分类限定到 P9 egress/account/protocol。回滚为恢复 P9-09/G9 的 `DEFERRED`，
      清除临时进程环境并不发送后续请求。
用户批准: APPROVED，2026-07-23（“账号去grok2api里面拿web账号就行”）
计划版本变更: v1.43
```

### 已批准 Change Request：CR-P9-CANARY-002

```text
CR-ID: CR-P9-CANARY-002
原因: 第一针以本机直连且未携带当前 Statsig 签名得到 WAF 类 403，不能归因到 grok2api Web
      账号。用户明确授权“你想怎么测试都可以”，允许以同一服务器现有 Web 运行轮廓进行受控复测。
影响的 Task / Matrix ID / ADR: 仅扩展 P9-09/G9 的受控 Canary；不改变 P7/P8 延期、P10 前置、
      公开 API、Provider 路由、Credential 存储或生产 Feature Flag。
Canary 边界: 保留 CR-P9-CANARY-001 的最多三次固定 Conversation `POST`，已消耗一次；每个后续
      固定目标 POST 最多配套一次服务器直连的 `GET /index` 与一次已知 Statsig 签名端点 POST，以取得
      同一 `POST /rest/app-chat/conversations/new` 的当期签名。允许本机仅绑定 loopback 的临时 SSH
      SOCKS5 转发到同一服务器出口；不修改服务器路由、代理/TUN、账号、配置、数据库或生产开关。
      所有辅助请求拒绝重定向、固定超时与响应上限；签名只在单进程内存中存在。
兼容性与迁移影响: 受控复测必须仍由本仓库的 DNS-pinned Rust transport 发送固定 Conversation
      POST；服务器只提供已存在的 Web 账号、同源出口和当期签名材料，不能把 grok2api 的代理响应当作
      本仓库 E2E 成功。
测试与回滚变化: 对新增的 Statsig Header 与 loopback SOCKS5 输入补充零网络合成测试；每次真实
      Conversation POST 前执行任务级 review。若结果仍非接受的 Conversation frame，停止该 tuple，
      仅记录脱敏类别并保持 G9 fail-closed。回滚为关闭临时 SSH 转发、清除进程环境与恢复
      `DEFERRED_EXTERNAL_CANARY`。
用户批准: APPROVED，2026-07-23（“允许，你想怎么测试都可以”）
计划版本变更: v1.44
```

## 2. Release 1 范围

### 2.1 必须交付

- Rust + Actix Web 单节点高性能网关。
- `GET /healthz`。
- `GET /v1/models`。
- `POST /v1/responses`，非流式与 SSE。
- `POST /v1/messages`，非流式与 SSE。
- `POST /v1/messages/count_tokens`，仅在存在可证明准确的 Provider/本地能力时返回结果。
- 自有 Client API Key 和 Access Group。
- Upstream、单协议 Endpoint、Endpoint-Credential Binding。
- Public Model、Alias、Model Route、Route Candidate。
- Priority + Smooth Weighted Round-robin。
- Candidate 与 Credential 两阶段调度。
- 首语义事件前 Failover；首语义事件后禁止透明重放。
- 每 Endpoint+Credential 模型发现和最后成功快照。
- 结构化 Request、Attempt、Usage、Health、Quota 和 Route Explain。
- OpenAI-compatible Responses 与 Anthropic-compatible Messages。
- Grok Build、Kiro、Grok Official、Grok Web 四个专项切片。
- 管理 API、最小可用管理 Web UI、备份与恢复。
- systemd 和固定版本 Docker 产物。
- 与现有服务器链路的差分、灰度和回滚。

### 2.2 Release 1 明确不包含

- 公共 OpenAI Chat Completions 入口及通用 Chat Endpoint。
- 图片、音频、视频、Gemini 与 Interactions 入站协议。
- 动态二进制插件、插件商店和在线安装。
- PostgreSQL、S3、Git Store、Redis 协议复用和多节点控制面。
- 商业计费、支付、余额充值和按美元扣费。
- New API/AxonHub 持续同步；只允许后续的一次性 Import Adapter。
- Grok Web Tool Emulation 默认启用。
- 未经显式 Route Policy 的跨 Provider 或跨协议 Failover。

## 3. 已冻结的技术基线

这些决定在计划 `v1.0` 中视为已确认，开发期间不得重新隐式选择。

| ID | 决定 |
|---|---|
| BL-01 | Rust stable + Edition 2024，HTTP 服务使用 Actix Web；核心不依赖 Actix 类型。 |
| BL-02 | Release 1 公开推理入口为 Responses 和 Anthropic Messages；Chat Completions 延后。 |
| BL-03 | 所有请求先进入 `CanonicalRequest`，所有输出转换为 `CanonicalEvent`。 |
| BL-04 | Tool、Reasoning、Text、Usage、Error 使用显式流状态机；网络 Chunk 边界不影响语义。 |
| BL-05 | `FirstSemanticEvent` 是透明重试边界；SSE Keepalive 不算语义事件。 |
| BL-06 | 一个 Upstream Endpoint 只绑定一种 API Format；共享 URL 也拆成不同 Endpoint。 |
| BL-07 | 先选择 Route Candidate，再在 Endpoint 内租用 Credential；两层权重独立。 |
| BL-08 | 路由使用不可变 `RouteSnapshot` + `ArcSwap`；一个请求固定使用开始时版本。 |
| BL-09 | SQLite 保存控制面、版本、长期状态和异步事件；请求热路径不查询 SQLite。 |
| BL-10 | Request/Attempt/Usage 明细进入有界异步队列；Body 默认关闭，队列满时只允许丢低优先级诊断。 |
| BL-11 | 上游 Secret 使用 AEAD；Client Key 保存 Prefix + HMAC 摘要；主密钥与数据库分离。 |
| BL-12 | CacheAffinity、ResponseOwnership、ReasoningReplay、WebConversationState 使用不同存储命名空间。 |
| BL-13 | Session/Cache Identity 至少隔离 `client_key + provider + upstream_model`。 |
| BL-14 | Grok Official、Build、Web 使用独立 Provider ID、凭据池、Quota、错误和连续性状态。 |
| BL-15 | Kiro IDE/CLI 是同一 Kiro Provider 下的 Endpoint Policy，不与通用中转站 Endpoint 混淆。 |
| BL-16 | 未知 403 默认归类为短期 `EgressRejected`；只有账号级证据才设为 Credential Forbidden。 |
| BL-17 | Unauthorized 退出调度直到重新授权；Quota 到 Reset 后受控探测；长期状态重启后恢复。 |
| BL-18 | Catalog 默认建议：Fresh 6h、Stale 24h、Expired 72h；移除需连续 3 次成功缺失且不少于 24h。 |
| BL-19 | 管理 API 先于 UI；UI 使用独立 TypeScript SPA，不能进入推理热路径。 |
| BL-20 | Grok Web 使用 Feature Flag，Tool Emulation 默认关闭；通过独立 Gate 后才允许生产启用。 |
| BL-21 | 现有 CPA/grok2api/Kiro-RS/New API 仅做一次性迁移或临时兼容上游，不成为新网关运行时数据库。 |
| BL-22 | Release 1 Client Key 只实现权限、到期、RPM、并发和可选 Token 上限，不做美元计费。 |

## 4. 代码库与交付物约定

### 4.1 目标目录

```text
Cargo.toml
rust-toolchain.toml
deny.toml

apps/
  gateway/

crates/
  gateway-core/
  gateway-protocol/
  protocol-openai-responses/
  protocol-anthropic/
  gateway-provider/
  gateway-upstream/
  gateway-catalog/
  gateway-access/
  gateway-router/
  gateway-continuity/
  gateway-auth/
  gateway-stream/
  gateway-observability/
  gateway-store/
  gateway-control/
  gateway-http-actix/
  provider-openai-compatible/
  provider-anthropic-compatible/
  provider-grok/
  provider-kiro/

docs/
  adr/
  contracts/
  reports/

migrations/
tests/
  fixtures/
  integration/
  differential/
  e2e/
benchmarks/
deploy/
  systemd/
  docker/
```

### 4.2 Git 规则

- `cpa-rust-gateway` 使用独立 Git 仓库，不依赖父目录的未跟踪状态。
- 主分支：`main`。
- 开发分支：`codex/p<phase>-<short-name>`。
- 一个分支只承载一个未完成 Phase 的顺序 Task 与其测试修复；每个 Task 保持独立提交，不因共用分支
  而合并范围、跳过 review 或跨 Phase 混入功能。
- Commit 标题以 Task ID 开头，例如 `P1-03: add canonical event state machine`。
- 普通 Phase 只在 closeout 时上传 Phase 分支并创建带说明的阶段 Tag，例如 `phase-p3-complete`；该
  单次交付触发 Fast + Full。提前远端例外只能按 `CR-EXEC-007` 记录并运行。
- Release 使用 SemVer，首个服务器候选版本从 `v0.1.0-alpha.1` 开始。
- 不提交 `.env`、真实数据库、Token、Cookie、OAuth JSON 或生产日志。

### 4.3 Definition of Done

一个 Task 只有同时满足以下条件才能标记为 `DONE`：

- 实现满足本 Task 和对应行为契约。
- 正常、边界、错误和取消路径有自动化测试。
- `cargo fmt --check` 通过。
- 受影响 Crate 的 Clippy 与测试通过；整合 Phase preflight 再运行
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` 与 `cargo test --workspace`。
- 没有新增未解释的 `TODO/FIXME`、明文 Secret 或宽泛 `unwrap/expect`。
- 对外行为、配置或 Schema 变化已更新文档和迁移说明。
- 保存完成证据，并更新本文状态。
- 普通代码 Task 已达到本地验收条件，并由所属 Phase 唯一的远端 Fast + Full Delivery Gate 覆盖；
  CI/workflow/cache 等例外 Task 另有明确的提前远端 Gate。纯文档/状态变更为显式 docs-only；
  Phase Tag 为 Fast + Full。`LOCAL_PASS_PENDING_PHASE_GATE` 与 `LOCAL_PASS_PENDING_CI` 都不满足
  本条件。

## 5. Phase 总览

| Phase | 目标 | 进入条件 | 退出 Gate | 状态 |
|---|---|---|---|---|
| P0 | 仓库、工具链、ADR、CI 基线 | 本计划锁定 | G0 | DONE |
| P1 | Canonical Core + Mock 垂直链路 | G0 | G1 | DONE |
| P2 | 聚合控制面、Secret、RouteSnapshot | G1 | G2 | DONE |
| P3 | OpenAI Responses 聚合 MVP | G2 | G3 | DONE |
| P4 | Catalog、Health、Quota、Explain、观测 | G3 | G4 | DONE |
| P5 | Anthropic/Claude Code 兼容 | G4 | G5 | DONE |
| P6 | Grok Build | G5 | G6 | DONE |
| P7 | Kiro IDE/CLI | G6 | G7 | DEFERRED_EXTERNAL_AUTH |
| P8 | Grok Official | G6（`CR-P7-DEFER-002`） | G8 | DEFERRED_EXTERNAL_E2E |
| P9 | Grok Web | P8 local isolation evidence（`CR-P9-LOCAL-001`） | G9 | DONE |
| P10 | 完整管理 API、Web UI、备份恢复 | G9 | G10 | DONE |
| P11 | 差分、性能、安全与发布加固 | G10 | G11 | DONE |
| P12 | 服务器部署、灰度、切换与回滚 | G11 | G12 | IN_PROGRESS |
| P13 | Release 1.1 候选功能 | G12 + 新 CR | 独立计划 | DEFERRED |

## 6. P0 - 仓库与工程基线

目标：得到可重复构建、可审计、无业务功能的 Rust Workspace。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P0-01 | 为目录建立独立 Git 仓库、忽略规则和 Secret 扫描规则 | 计划 v1.0 | `git status` 干净；测试 Secret 被阻止提交 | DONE |
| P0-02 | 创建 `docs/adr`、`contracts`、`reports` 和需求追踪索引 | P0-01 | 文档链接检查通过 | DONE |
| P0-03 | 固定 Rust stable/Edition 2024、Workspace、基础 Crate 骨架 | P0-01 | `cargo metadata`、`cargo check --workspace` | DONE |
| P0-04 | 配置 fmt、Clippy、测试、license/advisory 检查 | P0-03 | fmt/clippy/test/deny/audit 全通过 | DONE |
| P0-05 | 配置本地统一命令和 CI 快速/完整两条流水线 | P0-04 | 干净环境 CI 成功日志 | DONE |
| P0-06 | 记录本地 Mac 与 Jakarta VPS 的硬件、Rust、内核和基准环境 | P0-05 | `docs/reports/environment-baseline.md` | DONE |

### G0 门禁

- Workspace 在全新目录可重复构建。
- 所有 Crate 依赖方向符合目标架构。
- `#![deny(unsafe_code)]` 默认启用；例外必须新建 ADR。
- License Allowlist 不允许无意引入 AGPL 代码。
- CI 和本地命令得到相同结果。

## 7. P1 - Canonical Core 与 Mock 垂直链路

目标：不依赖真实上游，跑通从 Actix 入站到 Canonical、Mock Provider、再到目标协议输出的完整路径。

主要矩阵：`A01 A03 A07 B01-B17 B23-B30 K01 K02 K05 K09 K10`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P1-01 | 定义稳定 ID、RequestContext、GatewayError 和错误作用域 | G0 | 单元测试 + 错误编码快照 | DONE |
| P1-02 | 定义 `CanonicalRequest`、消息、内容、Tool、Thinking 和 Raw Extension | P1-01 | JSON/结构 Round-trip 测试 | DONE |
| P1-03 | 定义 `CanonicalEvent` 与 Response/Text/Reasoning/Tool/Usage 状态机 | P1-02 | 状态转换和非法序列测试 | DONE |
| P1-04 | 实现有界流、背压、取消传播和 FirstSemanticEvent Tracker | P1-03 | 慢消费者、取消和容量测试 | DONE |
| P1-05 | 实现 OpenAI Responses 入站/非流式/SSE Adapter | P1-02,P1-03 | 官方形态 Fixture + 事件快照 | DONE |
| P1-06 | 定义小能力 Provider Trait，并实现 Deterministic Mock Provider | P1-02,P1-03 | Mock 文本、Tool、错误、延迟 Fixture | DONE |
| P1-07 | 实现 Actix `/healthz` 与 `/v1/responses` 最小 Handler | P1-04,P1-05,P1-06 | HTTP E2E 测试 | DONE |
| P1-08 | 实现内存 Client Key Auth Port，为 P2 持久实现保留接口 | P1-07 | 有效、无效、禁用 Key 测试 | DONE |
| P1-09 | 建立 Chunk 随机切片、并行 Tool、空参数 Tool 的属性测试 | P1-03,P1-05 | 固定 Seed 与随机 Seed 报告 | DONE |

### G1 门禁

- `/v1/responses` 非流式和 SSE 均通过 Mock E2E。
- 对保留每个 Tool 本地片段顺序的任意合法、已解码 UTF-8 参数片段切分与交错，得到相同 Tool 语义投影：`call_id`、`name`、最终 `RawJson`、Responses SSE 和非流式 Function Call 输出一致；`ToolCallArgumentsDelta` 的片段边界可不同。原始网络 bytes、UTF-8 标量内切分及 EventStream 帧不变性仍由后续 Provider 阶段验证。
- `EnterPlanMode`、`ExitPlanMode` 和普通无参数 Tool 输出 `{}`。
- 客户端取消后没有遗留上游任务或无限缓冲。
- FirstSemanticEvent 前后重试状态可被测试明确区分。

## 8. P2 - 聚合控制面、安全与 RouteSnapshot

目标：建立可版本化配置、加密 Secret、自有 Key 和无数据库热路径的路由快照。

主要矩阵：`D01-D31 E01-E29 H01-H13 J01-J03 J08 J09 J15 J18-J20 L01-L05 L17-L36`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P2-01 | 设计并迁移 Config Version、Upstream、Endpoint、Credential 与 Binding 表 | G1 | Migration up/down + FK 测试 | DONE |
| P2-02 | 设计并迁移 PublicModel、Alias、Route、Candidate、AccessGroup、ClientKey 表 | P2-01 | Schema 约束和唯一性测试 | DONE |
| P2-03 | 实现 AEAD Secret Store、Key Version、Nonce 和主密钥加载 | P2-01 | 加解密、错误密钥、轮换测试 | DONE |
| P2-04 | 实现 Client Key 生成、Prefix、HMAC 摘要和常量时间验证 | P2-02,P2-03 | 创建一次可见、验证和撤销测试 | DONE |
| P2-05 | 实现 Repository/Service 事务，禁止控制面实体泄露到 Provider | P2-01,P2-02 | Repository 集成测试 | DONE |
| P2-06 | 实现 Route Compiler：Alias、引用、能力、Catalog 和冲突校验 | P2-05 | 冲突矩阵与错误快照 | DONE |
| P2-07 | 实现 `RouteSnapshot`、ArcSwap、版本固定与回滚 | P2-06 | 并发读/发布/回滚测试 | DONE |
| P2-08 | 将 P1 内存 Auth 替换为 Snapshot ClientKeyView | P2-04,P2-07 | 热更新、禁用、过期测试 | DONE |
| P2-09 | 实现 EgressPolicy：Scheme、Host、Port、CIDR、DNS、Redirect 校验 | P2-01 | SSRF、DNS Rebinding、私网 Allowlist 测试 | DONE |
| P2-10 | 提供最小管理 API/CLI：创建配置、验证、发布、回滚 | P2-05,P2-07 | 原子发布 E2E + 审计事件 | DONE |

### G2 门禁

- 无效 Alias、悬空 Candidate、重复 Endpoint Format 整版拒绝发布。
- 100 个并发请求跨 Snapshot 发布仍固定使用各自起始版本。
- 数据库和备份中不存在明文上游 Secret 或完整 Client Key。
- 本机上游只有显式 Egress Allowlist 才能访问。
- 推理热路径通过测试证明不调用 Repository。

## 9. P3 - OpenAI Responses 聚合 MVP

目标：用两个独立 Upstream 实现自有 Key、统一模型名、平滑轮询和首事件前 Failover。

主要矩阵：`C16 D10-D25 E15-E24 G05 G12 G13 G15 G21 K03-K06 L06 L20-L26 L28-L32 L40`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P3-01 | 实现 OpenAI-compatible Responses Endpoint URL/Header/Body 组装 | G2 | URL 组合和 Header 脱敏测试 | DONE |
| P3-02 | 实现共享上游 Client Pool、connect/TTFB/idle/total timeout 和代理隔离 | P3-01 | 连接复用、超时、代理测试 | DONE |
| P3-03 | 实现 Priority Tier + 预编译 Smooth Weighted Schedule + 原子 Cursor | P2-07 | 权重分布和并发公平性测试 | DONE |
| P3-04 | 实现 Endpoint Credential Pool、并发租约、站内权重和释放保证 | P3-03 | 泄漏、取消、饱和和双层公平测试 | DONE |
| P3-05 | 实现 Runtime Health/Cooldown/Circuit 基础状态与分片存储 | P3-04 | 状态隔离和恢复测试 | DONE |
| P3-06 | 实现 Attempt Orchestrator、排除集合、Retry Budget 和 FirstSemanticEvent Gate | P3-02,P3-05 | 连接/429/5xx/已开流故障矩阵 | DONE |
| P3-07 | 实现从 RouteSnapshot 生成的 `/v1/models` 与响应模型名回写 | P2-07,P3-03 | AccessGroup、hard-eligible、回写测试 | DONE |
| P3-08 | 发出 Request/Attempt/Usage 结构化事件，不阻塞响应 | P3-06 | 事件关联和队列背压测试 | DONE |
| P3-09 | 建立两个可控 Mock HTTP Upstream 的聚合 E2E 套件 | P3-01-P3-08 | 轮询、Failover、取消完整报告 | DONE |
| P3-10 | 使用两个真实测试中转 Endpoint 做最小非流式与 SSE 验证 | P3-09 | 脱敏请求/响应和 Trace 证据 | DONE |

### 已批准 Change Request：CR-P3-G3-001

`2026-07-20` 用户批准。P3-10 的真实测试此前把 `minimax-m3` 同时用作公开路由名和
验收文字；而两个已核对的私有 Candidate 映射属于 ChatGPT-family 上游。这会混淆“稳定的
客户端别名”与“实际提供方模型身份”。

- P3-10/G3 的真实验证公开模型改为 test-only `p3-chatgpt-compat`，其请求别名为
  `p3-chatgpt-compat-alias`。它仅表示对两条 ChatGPT-family 中转的 Responses 兼容性验证，
  不是任何具体上游模型版本或产品承诺。
- A、B 继续分别使用 operator-controlled 私有配置中明确给出的上游模型；P3-10 不发现、
  不展示、也不将它们写入 Git。
- 此变更不追溯修改 P3-09 的纯 Mock fixture，也不决定 P10 以后的产品 PublicModel。此前
  两条停止记录保留为安全/传输事实，但不构成修订后公开别名的兼容性证据。
- 任何修订别名下的真实请求都需要新的明确外部调用与预算授权；先前未使用的调用额度不能
  自动迁移到这个不同的测试边界。

### G3 门禁

- test-only `p3-chatgpt-compat` 通过本项目 Base URL/Key 可调用两个独立的 ChatGPT-family Candidate；它不声明具体上游模型身份。
- 等权 1000 次选择分布偏差不超过 10%；加权分布符合预期区间。
- 一个站配置多个 Key 不改变站间目标流量比例。
- 首事件前故障可切站，首事件后故障绝不透明重放。
- 429、5xx、连接错误和凭据饱和都有独立排除原因。
- 热路径无 SQLite、全局 Mutex 和无界 Channel。

## 10. P4 - Catalog、Health、Quota 与可观测性

目标：让模型目录、可用性和每次路由决策可查询、可解释、可恢复。

主要矩阵：`C26 C27 D20 D24 E18-E22 F13-F15 G01-G28 H12 H13 H19 H20 L09-L16 L30-L33`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P4-00 | 落实 `CR-EXEC-001`：受版本校验的质量工具缓存、code/docs/tag Gate 分类、`LOCAL_PASS_PENDING_CI` 状态守卫和独立单探针诊断 harness | G3 | cache miss/hit、Gate 分类、状态阻断、零授权/单请求/脱敏诊断测试；不发送未经单独授权的真实流量 | DONE |
| P4-01 | 实现每 Endpoint+Credential 的 ModelCatalogSource 调度与 Singleflight | P4-00 | 并发同步和凭据差异测试 | DONE |
| P4-02 | 实现 CatalogSnapshot、Fresh/Stale/Expired 与最后成功回退 | P4-01 | 时间推进和失败保留测试 | DONE |
| P4-03 | 实现 added/suspected_removed/removed Diff、Preview/Apply 和移除隔离 | P4-02 | 3 次缺失 + 24h Fixture | DONE |
| P4-04 | 实现 Endpoint/Model/Credential Probe、EWMA 和 Circuit 恢复 | P3-05 | 健康时间线和半开测试 | DONE |
| P4-05 | 实现 QuotaSnapshot、来源/置信度、Reset 与受控恢复探测 | P4-04 | 429/Quota/Reset Fixture | DONE |
| P4-06 | 实现 Route Explain 和 Candidate 排除原因查询 | P3-06,P4-04,P4-05 | 固定输入决策快照 | DONE |
| P4-07 | 实现 SQLite 异步 Request/Attempt/Usage/Health Event Writer | P3-08 | 队列、批写、崩溃恢复、quick_check | DONE |
| P4-08 | 实现 tracing JSON、Prometheus 和 OpenTelemetry 导出 | P4-07 | 指标/Trace 关联测试 | DONE |
| P4-09 | 实现日志脱敏、Body 采样开关和 Secret 泄漏测试 | P4-07,P4-08 | 自动 Secret 扫描报告 | DONE |
| P4-10 | 落实 `CR-P4-G4-001`：只读管理状态查询、403 账户状态与受控恢复 | P4-04,P4-05,P4-06 | 精确状态/恢复、调度隔离、无副作用与 Secret-safe 查询测试 | DONE |

`P4-00` 是 P4 的工程效率与验证前置项，不交付 Catalog、Health、Quota 或公开 API；它完成前，
不得将任一 P4-01 至 P4-09 标记为 `IN_PROGRESS`。该 Task 的单探针路径只在未来获得新的明确
operator 授权后才可发送一条真实请求，不能回溯或重跑 P3-10。

### G4 门禁

- 某 Credential 模型同步失败不影响其它 Credential 和最后成功快照。
- Catalog 临时故障不会导致 `/v1/models` 抖动。
- Route Explain 能说明每个 Candidate 的保留、排除和最终选择原因。
- 403 账号状态、429、Quota、Circuit 和恢复在管理 API 中可见。
- Event Queue 满时不阻塞推理；关键失败事件不得静默丢失。
- SQLite `quick_check`、重启恢复和事件关联通过。

`CR-P4-G4-001` 中“管理 API”仅指 P4-10 的只读 in-process 查询边界；认证 HTTP 管理面仍在 P10。

## 11. P5 - Anthropic 与 Claude Code 兼容

目标：完成 `/v1/messages`、Claude Code Tool 流和同一 Upstream 多协议 Endpoint 的安全能力门禁。

主要矩阵：`A07 A08 B04 B09-B16 B22 B24-B28 F01-F04 F09-F11 L08 L21 L22 L40`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P5-00 | 落实 `CR-EXEC-007`：P 级 delivery trigger/default-ref cache seed/tag restore 与提前远端例外验证 | G4 | workflow/计划守卫、本地 full、一次提前 GitHub Gate、tag cache hit/miss 与 fail-closed 证据 | DONE |
| P5-01 | 实现 Anthropic Messages 入站、非流式和 SSE 出站 Adapter | P5-00 | Anthropic Fixture 与事件快照 | DONE |
| P5-02 | 实现 `count_tokens` Canonical 路由和 Provider Capability；无准确能力时明确拒绝 | P5-01 | 准确路径和 Unsupported 测试 | DONE |
| P5-03 | 完成 Tool 增量 JSON、并行 Tool、空参数、必填参数和 ID 映射状态机 | P5-01 | 1-byte Chunk 属性测试 | DONE |
| P5-04 | 实现 Pass-through/Canonical/Lossless Bridge 能力分析器 | P5-01,P3-01 | 字段/Tool/Reasoning 不可损转换矩阵 | DONE |
| P5-05 | 支持同一 Upstream 的 Responses 与 Anthropic 独立 Endpoint/健康/熔断 | P5-04 | 单协议故障隔离 E2E | DONE |
| P5-06 | 实现 Thinking、Stop Reason、Usage、Cache 字段和响应模型回写 | P5-01 | 协议对照 Fixture | DONE |
| P5-07 | 建立 Claude Code `--bare` 最小 E2E 和 Plan Mode 回归 | P5-03-P5-06 | 真实客户端脱敏日志 | DONE |
| P5-08 | 加入未知字段、畸形流、截断 Tool 和取消 Fuzz/Property Test | P5-03 | 固定 Corpus 和无 Panic 报告 | DONE |

`P5-00` 是 P5 的交付流程前置项，不交付 Anthropic 功能，也不因本计划更新自动开始 P5。它是
`CR-EXEC-007` 的窄提前远端例外：必须先在 `codex/p5-anthropic` 上证明 GitHub 的实际 trigger、
required-status 与 tag cache 行为，之后 P5-01 至 P5-08 恢复“Task 本地验收、P5 一次正式远端验收”。

### G5 门禁

状态：`DONE`；P5-01 至 P5-08 的本地 review/验收和 `phase-p5-complete` 的唯一远端
Fast、Full supply-chain、Required Delivery Gate 均已通过。

- Claude Code 普通对话、普通 Tool、并行 Tool、Plan Mode 全部通过。
- 无参数 Tool 补 `{}`；非空未闭合 JSON 必须明确失败。
- Responses/Anthropic 跨协议只在能力分析通过时参与路由。
- 同一站一种协议故障不污染其它 Endpoint。
- SSE 终止事件、Usage 和错误语义符合目标协议。

## 12. P6 - Grok Build

目标：完成第一个专项 Provider，验证 OAuth、Quota、Cache 和 Response Continuity。

主要矩阵：`C02 C28 C32 C33 E06-E12 E25-E29 F07 F10-F18 G24-G28`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P6-01 | 实现 Grok Build Credential、OAuth JSON 导入和 Device Code | G5 | OAuth Mock + 脱敏导入测试 | DONE |
| P6-02 | 实现每 Credential Refresh Singleflight、Revision/CAS 和持久化 | P6-01 | 刷新风暴与旧 Token 覆盖测试 | DONE |
| P6-03 | 实现 Build Responses HTTP 请求、流和错误解析；兼容已知 OAuth 凭据来源 | P6-02 | `InferenceAdapter`/Router vertical link plus real T26 non-streaming and T33 SSE Canonical ResponseStart/text/clean ResponseEnd; T1-T33 are closed; remediated Delivery Gate passed | DONE |
| P6-04 | 实现模型、Billing、Quota Window 和 Reset 同步 | P6-03 | 来源/置信度和窗口测试；Credential-scope Catalog/Quota、version-7 migration up/down revalidation passed; remediated Delivery Gate passed | DONE |
| P6-05 | 实现租户隔离 Cache Identity 与 Cache Affinity | P6-03 | 稳定性、隔离和断裂事件测试；tenant isolation/rebind durable-break revalidation passed; remediated Delivery Gate passed | DONE |
| P6-06 | 实现 ResponseOwnership 与 ReasoningReplay | P6-03,P6-05 | previous_response 与多轮 Tool 测试；ownership/replay encrypted-clear revalidation passed; remediated Delivery Gate passed | DONE |
| P6-07 | 实现 Build 专用 401/403/429/Quota/Transient 分类 | P6-04 | 错误 Fixture 矩阵；failure action matrix revalidation passed; remediated Delivery Gate passed | DONE |
| P6-08 | 与 CPA/grok2api Build 行为做 clean-room 差分 | P6-03-P6-07 | 差分报告和 intentional diff 清单；clean-room source-boundary/report revalidation passed; remediated Delivery Gate passed | DONE |

### G6 门禁

状态：`DONE`。旧 `phase-p6-complete` Delivery Gate 只证明其旧 closeout target，不能替代 C28 的
真实双模式生命周期或可执行 Provider 垂直链路。修复后的 `phase-p6-remediated-complete` tag 已在
[GitHub Actions 29974725810](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29974725810)
完成：Classify、Fast、Full supply-chain 和 Required 均成功（Docs-only 正确跳过），所以 P6-03 至
P6-08 已按本计划的 Definition of Done 一并恢复为 `DONE`。

- 两个 Build Credential 的并发、轮询、刷新、Quota 和 Failover 通过。
- 固定直连 Build 的非流式与 SSE 都产生 Canonical `ResponseStart`、文本和无 `StreamError` 的
  `ResponseEnd`。
- Cache Identity 和 Affinity 均稳定，跨 Client Key 不串缓存。
- Response Ownership 不允许静默换账号续接。
- 旧请求不能覆盖新 Token 或错误封禁已刷新 Credential。

## 13. P7 - Kiro IDE/CLI

目标：原生实现 Kiro，不依赖 Kiro-RS 作为长期运行层，并保持 Claude Code 兼容。

主要矩阵：`C35-C47 E25 E26 G24 G28`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P7-01 | 实现 Social、IdC/Enterprise、`ksk_` 三类 Credential | G6 | 各类解析、加密和刷新 Fixture；singleflight/CAS/AEAD revalidation passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-02 | 实现 IDE/CLI Endpoint Policy、Region、Header、Origin 和 URL | P7-01 | 请求快照对照测试 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-03 | 实现 `profileArn` 查询、回退、注入、来源和审计 | P7-01,P7-02 | Builder/Enterprise 场景测试 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-04 | 实现 CanonicalRequest 到 Kiro Conversation Request | P7-02 | 多轮消息/Tool Fixture passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-05 | 实现 AWS EventStream 增量解析、CRC、边界和错误恢复 | P7-04 | 任意 Chunk + 损坏帧测试 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-06 | 实现每 Credential 动态模型与订阅能力、最后成功快照 | P7-01,P4-02 | 部分失败和 stale 测试 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-07 | 实现 Kiro Tool、AskUserQuestion、Plan Mode 和 Thinking 映射 | P7-04,P7-05 | Claude Code 回归套件 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-08 | 实现 Kiro 网络、账号、模型、额度和普通 429 分类 | P7-06 | 错误与恢复矩阵 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-09 | 与服务器定制 Kiro-RS 做差分、原生 `InferenceAdapter` 垂直链路和真实 `--bare` E2E | P7-03-P7-08 | 原生 IDE/CLI Adapter Fixture、差分报告、日志、模型列表；外部 OAuth 验证延后至最终认证验收包 | DEFERRED |

### G7 门禁

- CLI 和 IDE Endpoint 各自通过非流式、SSE、Tool 和 Thinking。
- 模型列表不出现重复 `-thinking` 模型。
- 单 Credential 模型查询失败不拖垮模型并集。
- EventStream 任意 Chunk 切分结果一致，CRC 错误不可静默忽略。
- 现有 Kiro-RS 生产路径可作为明确回滚方案。

## 14. P8 - Grok Official

目标：实现 xAI 官方 API Key 路径，并与 Build/Web 完全隔离状态。

主要矩阵：`C01 C03 C04 C31 C33 F07 G24-G27`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P8-01 | 实现 Official API Key、Endpoint、Header 和模型发现 | G6（`CR-P7-DEFER-002`） | 请求/目录 Fixture passed；见 P8-01 report | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-02 | 实现 Official Responses HTTP 非流式与 SSE | P8-01 | 本地 HTTP/SSE Fixture、严格分块/错误/Redaction 验收 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-03 | 实现 Quota/Rate Header、Reset 和 Billing 元数据 | P8-02 | 严格 Header/Reset/Usage Fixture、Redaction 和本地 Full Gate passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-04 | 实现 Official Tool、Reasoning、Search 能力声明与转换 | P8-02 | Capability/Tool/Reasoning Fixture、Search 显式非能力与本地 Full Gate passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-05 | 验证 Official/Build 状态、Affinity、Quota 和故障完全隔离 | P8-02-P8-04 | Exact Header-to-quota、Build state/affinity、401/403/429/transient 隔离 Fixture 与本地 Full Gate passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-06 | 完成官方路径差分、负载和错误矩阵 | P8-05 | JSON/SSE Tool/Reasoning 差分、12 OS 线程 × 8 解码器的 96 实例隔离、精确错误/Quota 矩阵、本地 Full Gate 与 review passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-07 | Official API Key 一次真实 E2E | P8-06、§20.1 Provider Gate E2E | 已忽略的零网络 harness、精确一 send、DNS-pinned direct egress、脱敏 lifecycle 结果；无 Official API Key，延后至与 P7-09 的最终外部认证验收包 | DEFERRED |

### G8 门禁

- Official 与 Build 同名 Public Model 只有显式 Route 才能共同候选。
- 一个来源的 401/403/429 不改变另一个来源状态。
- 官方 Tool/Reasoning 能力与公开元数据一致。
- 使用 P8 测试 API Key 的一次已授权真实 E2E，必须产生 `ResponseStart`、文本和无 `StreamError` 的 `ResponseEnd`；该授权与 P7 Kiro OAuth 无关。

当前状态：`DEFERRED_EXTERNAL_E2E`；仅缺少 Official API Key/授权，和 P7-09 一起在最终外部认证验收包回补。

## 15. P9 - Grok Web

目标：实现独立 Web/Console Provider，处理浏览器会话、出口指纹和网页协议漂移。

主要矩阵：`C29-C34 D28-D30 E27-E29 F17 G24-G28`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P9-01 | 实现 SSO/Cookie Credential、血缘和独立生命周期 | P8 local isolation evidence（`CR-P9-LOCAL-001`） | 导入、加密、失效测试 | DONE |
| P9-02 | 实现 BrowserEgressSession：Cookie、UA、TLS Profile、Proxy 绑定 | P9-01 | 指纹一致性和隔离测试 | DONE |
| P9-03 | 实现 Grok Web Chat 请求和流响应解析 | P9-02 | 脱敏网页 Fixture | DONE |
| P9-04 | 实现 WebConversationState 与账号/出口强绑定 | P9-03 | 多轮、过期和账号不可用测试 | DONE |
| P9-05 | 实现 Statsig 签名缓存、受限失效和 SSRF 防护 | P9-02 | 403、Redirect、域名测试 | DONE |
| P9-06 | 实现 REST/gRPC-Web Quota、Tier、Window、Source/Confidence | P9-03 | Quota Fixture | DONE |
| P9-07 | 实现 WAF/EgressRejected 与账号 Forbidden 分离 | P9-02,P9-03 | 403 分类矩阵 | DONE |
| P9-08 | 实现 Tool Emulation Feature Flag，默认关闭并标记 `emulated` | P9-03 | 开关与能力元数据测试 | DONE |
| P9-09 | 完成 Feature Flag 下真实账号 E2E、协议漂移和熔断演练 | P9-04-P9-08 | [Canary 报告](reports/p9-09-authorized-web-canary.md)：`CR-P9-CANARY-001/002` 下临时 grok2api Web SSO 账号的三次固定请求与 [G9 Delivery Gate](reports/g9-gate-report.md) | DONE |

### G9 门禁

- Web 账号、出口、Conversation 和 Cookie 不与 Build/Official 共享状态。
- WAF 403 默认只影响 Egress Session，除非存在账号级证据。
- Web 协议漂移只熔断 `grok.web`。
- Tool Emulation 关闭时不会对 Prompt 做隐式注入。
- 未经 Canary 报告批准，生产 Feature Flag 保持关闭。

## 16. P10 - 管理 API、Web UI、备份恢复

目标：形成可安全运营的完整控制面，但不影响数据面延迟。

主要矩阵：`H01-H22 J02 J08 J09 J11-J15 J18-J20`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P10-01 | 完整管理 OpenAPI：Upstream、Endpoint、Credential、Catalog、Route、Group、Key | G9 | OpenAPI Contract Test、独立 review、本地 Full gate 与 P10 Delivery Gate passed | DONE |
| P10-02 | 实现管理鉴权、仅本机/私网策略、审计和 CSRF/CORS 边界 | P10-01 | 未授权和跨站测试、独立 review、本地 Full gate 与 P10 Delivery Gate passed | DONE |
| P10-03 | 建立 TypeScript SPA、生成 API Client 和静态资源构建 | P10-01,P10-02 | 65-operation generated client、可重复前端构建、独立 review、本地 Full gate 与 P10 Delivery Gate passed | DONE |
| P10-04 | 实现 Upstream/Endpoint/Credential 管理与测试工作流 | P10-03 | 浏览器 E2E、独立 review、本地 Full gate 与 P10 Delivery Gate passed | DONE |
| P10-05 | 实现 PublicModel/Route/Candidate/AccessGroup/ClientKey 工作流 | P10-03 | 创建两站 `minimax-m3` 聚合 E2E、一次性 Key redaction/reload、独立 review 与 P10 Delivery Gate passed | DONE |
| P10-06 | 实现 Catalog Diff、Health、Quota、403、Route Explain 和请求追踪页面 | P10-03,P4-06 | [Runtime 管理工作流报告](reports/p10-06-runtime-management.md)：loopback browser E2E、定向 HTTP/SPA checks、Secret/docs checks、独立 review 与 P10 Delivery Gate passed | DONE |
| P10-07 | 实现 Config Version、发布、回滚和操作审计页面 | P10-03,P2-10 | [Configuration 生命周期报告](reports/p10-07-configuration-lifecycle.md)：受保护 HTTP、loopback browser E2E、定向 checks、Secret/docs checks、独立 review 与 P10 Delivery Gate passed | DONE |
| P10-08 | 实现加密备份、恢复预检、Schema Version 和 Secret Key 说明 | P10-01 | [加密备份与空目标恢复报告](reports/p10-08-encrypted-backup-restore.md)：SQLite 空机恢复演练、受保护 HTTP、browser E2E、Secret/docs checks、独立 review 与 P10 Delivery Gate passed | DONE |
| P10-09 | 嵌入静态资源并验证 UI 不进入推理热路径 | P10-03-P10-08 | [嵌入管理 UI 与推理隔离报告](reports/p10-09-embedded-management-ui.md)：clean embedded build、精确静态 HTTP、资源隔离、独立 review 与 P10 Delivery Gate passed | DONE |

### G10 门禁

- 可以仅通过 UI/API 完成两站 `minimax-m3` 聚合配置。
- 所有 Secret 只显示一次或掩码，不可通过 API 回读明文。
- 配置发布失败时数据面继续使用上一 Snapshot。
- 空数据库可从备份恢复，并通过 SQLite `quick_check` 和真实请求。
- UI 开启/关闭对数据面基准无显著影响。

## 17. P11 - 发布加固

目标：证明兼容性、可靠性、性能和安全达到服务器灰度条件。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P11-01 | 建立 CPA v7.2.80、grok2api、Kiro-RS 的脱敏差分 Fixture Harness | G10 | [差异分类报告](reports/p11-01-differential-fixture-harness.md)：离线、脱敏、默认拒绝的六条 source-labelled Fixture 与 review/定向验证 passed | DONE |
| P11-02 | 完成网络、DNS、TLS、429、5xx、截断流、慢客户端和取消故障注入 | P11-01 | [Fault Matrix](reports/p11-02-fault-matrix.md)：loopback-only injection、Router ownership regressions、定向验证与 review passed | DONE |
| P11-03 | 建立 Mock Provider Criterion/HTTP 基准和回归阈值 | P11-01 | [基准报告](reports/p11-03-benchmark-baseline.md)：受控 offline Criterion、`baseline.json`、P99/吞吐/RSS fail-closed comparator、完整本地门禁与 review passed | DONE |
| P11-04 | 执行并发、长流、连接池、内存、背压和 ≥10h 本地 Soak | P11-02,P11-03 | [性能与 Soak 报告](reports/p11-04-load-soak.md)：`CR-P11-04-001` 下的 10h13m loopback receipt、回归、Full gate 与 review | DONE |
| P11-05 | 执行 SSRF、Secret、Auth、权限、依赖和供应链安全审计 | P11-01 | [Security Report + SBOM](reports/p11-05-security-audit.md)：SSRF/Secret/Auth/权限/依赖审计、路径脱敏 CycloneDX 与 Full gate/review passed | DONE |
| P11-06 | 验证优雅停机、流 Drain、崩溃重启、磁盘满和事件队列降级 | P11-02 | [Recovery Report](reports/p11-06-recovery-report.md)：loopback stream drain、crash/replay、deterministic `SQLITE_FULL` recovery、queue degradation、Full gate 与 review passed | DONE |
| P11-07 | 完成升级/降级 Migration、备份恢复和旧版本回滚演练 | P10-08 | [Upgrade/rollback report](reports/p11-07-upgrade-rollback.md)：in-place schema drill、loss-aware downgrade、empty-target backup recovery、Full gate 与 review passed | DONE |
| P11-08 | 生成 Release Candidate 清单、已知差异和生产默认配置 | P11-01-P11-07 | [`v0.1.0-alpha.1` candidate ledger](reports/p11-08-release-candidate.md)：inventory、safe defaults、known differences、P12 handoff 与 docs review passed；P11 最终 Delivery Gate 的 Fast、Full supply-chain 与 Required 已通过 | DONE |

### G11 门禁

- 所有差异均标记为 Intentional、Compatible 或已修复 Regression。
- 无未分类 Panic、数据竞争、流截断或 Secret 泄漏。
- 性能基准相对已批准 Baseline：吞吐下降不超过 10%，P99/RSS 恶化不超过 15%。
- Mock 上游网关附加延迟目标：本地 warm-path P99 不超过 5ms；服务器不超过 10ms。
- ≥10h loopback Soak 无内存持续增长、连接泄漏或 SQLite 损坏；真实部署后的 72h 观察仍由 P12-10 完成。
- 回滚包和恢复步骤已经实际演练，不是只写文档。

### 已批准 Change Request：CR-P11-04-001

```text
CR-ID: CR-P11-04-001
原因: 用户确认纯 loopback 合成 Soak 已运行足够久，可以视为完成；其重复有限流负载能证明
      本地任务/内存/连接稳定，但不能替代真实上游或服务器长期行为。
影响的 Task / Matrix ID / ADR: P11-04 与 G11 的本地 Soak 门槛；P12-10 的 72h Canary 保持不变。
兼容性与迁移影响: 无公开 API、Canonical、Provider、Schema、客户端或部署迁移。现有 receipt 保持
      `INCOMPLETE`，不将用户停止的 10h13m 运行伪造成 24h `COMPLETED`。
测试与回滚变化: 保留所有定向/Full/receipt 回归和 15% RSS fail-closed 条件；最低本地观察改为
      10h，真实长期运行由 P12-10 的 72h Canary 承担。回滚为恢复 P11 的 24h 本地门槛并新开 receipt，
      不重写本次证据。
用户批准: APPROVED，2026-07-24（“已经测试很久了，可以视作完成”）
计划版本变更: v1.45
```

## 18. P12 - 服务器部署与灰度

目标：在不破坏现有 CPA/AxonHub/New API/Kiro-RS 的前提下部署、验证、灰度和切换。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P12-01 | 构建固定版本二进制、Docker 镜像、SBOM、Checksum 和签名 | G11 | 可验证发布产物 | IN_PROGRESS |
| P12-02 | 编写 systemd Unit、只读 Secret、数据目录、日志和资源限制 | P12-01 | `systemd-analyze verify` | PENDING |
| P12-03 | 备份当前服务器网关配置、数据库、版本和回滚命令 | P12-01 | 带时间戳备份清单 | PENDING |
| P12-04 | 在独立端口和独立数据目录部署 Staging 实例 | P12-02,P12-03 | Health、日志、资源状态 | PENDING |
| P12-05 | 录入测试 Upstream/Key，验证 Responses、Messages、Tool、模型和 Explain | P12-04 | 端到端报告 | PENDING |
| P12-06 | 执行现有网关与新网关 Shadow/Differential 流量 | P12-05 | 差异与性能报告 | PENDING |
| P12-07 | 配置独立 Cloudflare/Caddy 测试域名和最小暴露策略 | P12-04 | DNS/TLS/Auth 验证 | PENDING |
| P12-08 | 使用单独 Client Key 开始 10%→25%→50%→100% Canary | P12-06,P12-07 | 每阶段成功率、P95/P99、缓存和错误证据 | PENDING |
| P12-09 | 在 Canary 中实际执行一次回滚并再次恢复 | P12-08 | 回滚时长和一致性报告 | PENDING |
| P12-10 | 完成生产切换、72h 观察、发布 Tag 和运维手册 | P12-09 | G12 报告 | PENDING |

### Canary 推进与回滚规则

- 每个流量阶段至少持续 2 小时并包含至少 100 个成功请求；低流量时使用固定合成请求补足。
- 进入下一阶段前检查：状态码、TTFT、P95/P99、缓存、Tool、Usage、Credential 状态和 Route 分布。
- 任一条件触发立即回滚：
  - 相对旧网关新增错误率超过 1%。
  - P95 延迟持续增加超过 20%。
  - 出现 Tool/Reasoning/Usage 语义回归。
  - 出现 Secret 泄漏、数据库损坏、流重复或错误跨账号连续性。
  - 无法用 Route Explain 解释实际选路。

### G12 门禁

- 100% 流量运行 72h，无 P0/P1 故障。
- 现有生产路径仍保留固定版本回滚包。
- 备份、恢复、升级、降级、Secret 轮换和故障排查手册齐全。
- Release 1 完成后才允许为 P13 创建新计划版本。

## 19. P13 - Release 1.1 候选范围

本阶段当前为 `DEFERRED`，不能在 Release 1 开发中顺手实现。完成 G12 后，通过 Change Request 选择：

| ID | 候选功能 | 当前状态 |
|---|---|---|
| P13-01 | OpenAI Chat Completions 入站与 compatible Chat Endpoint | DEFERRED |
| P13-02 | Chat/Responses 受限无损桥接 | DEFERRED |
| P13-03 | New API/AxonHub 一次性 Import Adapter | DEFERRED |
| P13-04 | Cost-aware、Fill-first、Least-loaded 路由 | DEFERRED |
| P13-05 | 管理调试用单请求 Channel Pin | DEFERRED |
| P13-06 | 其它矩阵 `Later` 项重新筛选 | DEFERRED |

## 20. 测试体系

### 20.1 测试层级

| 层级 | 目标 | 运行时机 |
|---|---|---|
| Unit | 纯函数、状态机、解析器、错误分类 | 每个 Task |
| Property/Fuzz | Chunk、JSON、EventStream、Alias、Route 冲突 | 相关 Task + Nightly CI |
| Integration | SQLite、HTTP Client、RouteSnapshot、Credential Lease | 每个 Phase |
| Contract | OpenAI/Anthropic API 形态和错误语义 | P1/P3/P5 后持续运行 |
| Differential | CPA/grok2api/Kiro-RS 行为差异 | Provider Gate 与 P11 |
| E2E | 真实客户端和测试账号 | 每个 Provider Gate |
| Load/Soak | 延迟、吞吐、RSS、连接、背压 | P3 基线、P11 完整 |
| Security | Secret、SSRF、Auth、权限、依赖 | P2、P10、P11 |

### 20.2 提交分类与快速门禁

每个代码、工具链、workflow、脚本、迁移、Fixture、契约或安全策略 Task 都必须在本地运行适用的
定向格式/Clippy/测试、Secret scan 与 changed-doc link check；整合本地 Phase preflight 和该 Phase 的
唯一远端 Delivery Gate 至少各运行一次以下完整集合：

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
secret scan
changed-doc link check
```

普通代码 Task 不逐个触发 GitHub Gate；CI/workflow/cache/required-status 的窄例外按 `CR-EXEC-007`
提前验证。纯报告、索引或计划状态变更使用 `docs-only` Gate：Markdown/格式、文档链接、Secret
scan 和计划一致性检查。它不能以未执行的 Rust/供应链检查冒充代码 Full Gate；具体 workflow 分类、
required-status 与 P 级 trigger/cache 行为由 P4-00 的既有基础和 P5-00 共同验证。

### 20.3 Phase 完整门禁

```text
all-feature tests
integration tests
contract snapshots
relevant property/fuzz corpus
cargo deny / cargo audit
coverage report
phase-specific E2E
phase report
```

关键状态机、路由编译器、Secret 和重试逻辑要求分支覆盖完整；全 Workspace 行覆盖目标不低于 80%，但不得用无意义测试追求数字。

## 21. 性能与资源纪律

- 所有流使用 `Bytes` 和有界 Channel。
- 上游 Client 按 Endpoint/EgressPolicy 复用连接池，不按请求创建。
- RouteSnapshot 读取无全局锁、无磁盘访问。
- Credential Runtime State 按 Provider/Endpoint/Credential 分片。
- Token Refresh 按 Credential Singleflight。
- Body 日志默认关闭；结构化事件批量异步写入。
- 每次 Phase Gate 记录 CPU、RSS、分配、连接复用、P50/P95/P99 和吞吐。
- 基准变化超过 G11 阈值必须创建性能说明或修复，不得直接更新 Baseline 掩盖回归。

## 22. 安全纪律

- 日志默认拒绝 Authorization、Cookie、OAuth、SSO、API Key 和原始 Cache Key。
- Fixture 只允许脱敏或人工生成数据。
- 自定义 URL 在连接前和 Redirect 后都重新执行 EgressPolicy。
- Client Key、Management Key 和上游 Credential 使用不同命名空间和权限。
- 管理端默认只监听本机/私网；公网暴露必须经过 Caddy/Cloudflare 和额外鉴权。
- Backup 不包含主密钥；恢复流程必须明确数据库与主密钥的组合要求。
- 依赖升级单独执行测试和差分，不使用无审核的自动大版本更新。

## 23. 需求追踪

| 功能矩阵模块 | 主要执行阶段 |
|---|---|
| A 接口与服务器 | P1、P3、P5、P12 |
| B 协议与流 | P1、P5、P6-P9 |
| C Provider | P3、P5-P9 |
| D 模型与路由 | P2-P4 |
| E 凭据与错误 | P2-P9 |
| F Thinking/缓存 | P5-P9 |
| G 可观测性 | P3、P4、P10、P11 |
| H Management | P2、P4、P10 |
| I 插件 | Release 1 Drop/Deferred |
| J 配置与部署 | P0、P2、P10-P12 |
| K 性能 | P1、P3、P11 |
| L 上游聚合 | P2-P5，延后项进入 P13 |

每个 Task 实现前需在代码或测试说明中引用对应 Matrix ID/行为契约；若无法映射，必须先创建 Change Request。

## 24. 后续每轮汇报格式

后续开发回合统一使用：

```text
Plan version:
Current phase / gate:
Current task:
Completed in this turn:
Verification evidence:
Files changed:
Execution timing (Task Card / code commit / local review-test pass / Phase closeout tag / Phase Delivery Gate):
Scope level and execution budget:
Repeated validations / rework count:
Risks or deviations:
Next task:
```

最终回复必须明确区分：已完成、仅设计、未验证、受阻和 Deferred，不能用“基本完成”替代状态。

## 25. 计划变更记录

| 版本 | 日期 | 变化 | 批准状态 |
|---|---|---|---|
| v1.0 | 2026-07-18 | 建立 Release 1 全阶段、任务、Gate、测试、安全、Git、灰度和变更控制基线 | 当前执行基线 |
| v1.1 | 2026-07-19 至 2026-07-21 | 记录 P1/G1 的 Tool 语义投影澄清，以及 P3/G3 的公开别名、有限 SSE 帧与 idle 边界 Change Request | 已批准的历史执行基线 |
| v1.2 | 2026-07-21 | `CR-EXEC-001`：缓存化受版本约束的质量工具、code/docs/tag Gate 分类、`LOCAL_PASS_PENDING_CI` 流水线、文档证据批处理、单探针诊断与真实 Provider readiness 纪律；新增 P4-00 前置项 | APPROVED；当前执行基线 |
| v1.3 | 2026-07-21 | `CR-EXEC-002`：缓存可见 delivery ref、Fast 后补充供应链 Full、单次 docs-only 收口、cache summary 与暖态时延目标；P4-02 开始 | APPROVED；当前执行基线 |
| v1.4 | 2026-07-21 | `CR-EXEC-003`：Task Card、S/M/L 执行预算、集中补丁、去重验证、Gate 等待重叠、证据模板与全程时延报告 | APPROVED；当前执行基线 |
| v1.5 | 2026-07-21 | `CR-EXEC-004`：按任务风险与不确定性路由 Luna/默认/高级模型及最低足够思考强度；简单任务独立交付并透明记录 fallback | APPROVED；当前执行基线 |
| v1.6 | 2026-07-21 | `CR-EXEC-005`：原位无法切换 Luna 时的受限低成本子代理 fallback；最多一个活跃代理、禁止嵌套，主会话保留 review 与验收 | APPROVED；当前执行基线 |
| v1.7 | 2026-07-21 | `CR-EXEC-006`：普通简单执行默认 Luna `low`，多步检查或机械写入使用 `medium`；`minimal` 收窄至零判断查询 | APPROVED；当前执行基线 |
| v1.8 | 2026-07-21 | `CR-P4-G4-001`：新增 P4-10 的只读管理状态查询、403 账户状态与受控恢复，闭合 G4 而不提前实现 P10 HTTP/UI | APPROVED；当前执行基线 |
| v1.9 | 2026-07-21 | `CR-EXEC-007`：未开始 Phase 改用一条 P 级分支、每 Task 本地 review/test、每 Phase 一次正式远端 Fast + Full；新增 P5-00 以验证 GitHub trigger/default-ref cache/tag restore，并保留 CI/cache 等提前远端例外 | APPROVED；当前执行基线 |
| v1.38 | 2026-07-23 | `CR-P7-G7-001`：P7 的外部 Kiro 账号认证阻塞期间，允许 P8 仅进行顺序本地开发；保留 G7、两阶段 Delivery Gate、完成语义、真实 E2E 与发布的 fail-closed 边界 | 已由 v1.39 的冲突顺序部分替代 |
| v1.39 | 2026-07-23 | `CR-P7-DEFER-002`：P7 Kiro OAuth 延后；P8-G12 按自身非 Kiro 依赖推进，P8 可进行自身 Gate/Delivery，Kiro 仍必须在 P12/G12 后、含 Kiro 发布前回补 | APPROVED；当前执行基线 |
| v1.40 | 2026-07-23 | 新增 P8-07：为既有 §20.1 的每 Provider Gate 真实 E2E 要求登记一个默认零网络、单目标/单模式/单发送的 Official API-key harness；不改变公开 API 或 Kiro 的延后状态 | APPROVED；当前执行基线 |
| v1.41 | 2026-07-23 | `CR-P8-DEFER-001`：因无 Official API Key，将 P8-07/G8 延后至与 P7-09 的最终外部认证验收包；不改变 P9-P12 的 Gate 依赖或提前开发权限 | APPROVED；当前执行基线 |
| v1.42 | 2026-07-23 | `CR-P9-LOCAL-001`：允许 P9-01 至 P9-08 在 P8/G8 外部 E2E 延后时完成本地实现；P9-09/G9/Delivery Gate 仍需 P9 自身真实 Web Canary 证据 | APPROVED；当前执行基线 |
| v1.43 | 2026-07-23 | `CR-P9-CANARY-001`：用户指定当前 grok2api `grok_web/sso` 账号作为 P9-09 的独立测试来源；恢复单目标、最多三次、无值日志的 Web Canary，P7/P8 延期与 P10 前置不变 | APPROVED；当前执行基线 |
| v1.44 | 2026-07-23 | `CR-P9-CANARY-002`：在用户扩大测试授权后，允许 P9-09 按现有服务器 Web 运行轮廓使用受限 Statsig 辅助请求与临时 loopback SSH SOCKS5 出口复测；固定目标 POST 预算与 G9 fail-closed 不变 | APPROVED；当前执行基线 |
