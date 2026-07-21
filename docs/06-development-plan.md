# Rust AI Gateway 详细开发计划

## 0. 计划元数据

| 字段 | 值 |
|---|---|
| 计划版本 | `v1.8` |
| 生效日期 | `2026-07-21` |
| 状态 | `Locked for execution` |
| 当前阶段 | `P1 - Canonical Core + Mock 垂直链路`、`P2 - 聚合控制面、安全与 RouteSnapshot` 与 `P3 - OpenAI Responses 聚合 MVP` 已完成；`P4 - Catalog、Health、Quota、Explain、观测` 进行中 |
| 当前任务 | `P4-00` 至 `P4-10` 已完成 Code Gate；本次 P4-10 docs-only 收口等待 Docs Gate，随后执行 G4；不进入 P5。 |
| Rust Workspace | 21-package 骨架已创建并通过 P0-03 验证 |
| 生产部署 | 尚未开始 |
| 行为参考 | CPA `v7.2.80` + 已冻结的 AxonHub/New API/Sub2API/grok2api/Kiro-RS 快照 |
| 已批准变更 | `CR-P1-G1-001`：将 G1 的 Chunk 条件精确为 P1 范围内的 Tool 语义投影一致性；原始 bytes/EventStream 不变性仍由 Provider 阶段验证。 `CR-P3-G3-001`：P3-10/G3 的真实验证公开别名改为 test-only `p3-chatgpt-compat`，不把 ChatGPT-family 上游误称为 `minimax-m3`。 `CR-P3-G3-002`：test-only SSE 单帧有限上限改为 64 KiB。 `CR-P3-G3-003`：仅 P3-10 ignored live profile 的 SSE idle 上限改为 45 秒，其他 transport 边界不变。 `CR-EXEC-001`：保留质量与单 Task 开发纪律的前提下，采用缓存化 Full CI、显式 docs-only Gate、`LOCAL_PASS_PENDING_CI` 状态与单探针诊断 harness。 `CR-EXEC-002`：按缓存可见交付引用、Fast 后补充供应链 Full、单次 docs-only 收口、缓存可观测性和明确的暖态时延目标进一步缩短交付等待。 `CR-EXEC-003`：以任务卡、集中补丁、非重复验证、Gate 等待重叠、模板化证据和全程时延度量减少代理执行时间。 `CR-EXEC-004`：按任务风险和不确定性路由 Luna/默认/高级模型及最低足够思考强度，避免简单任务默认占用高级模型和深度思考。 `CR-EXEC-005`：当前会话不能切换 Luna 时，允许一个受限、低成本子代理处理明确的低风险工作；主会话保留 review、提交与验收责任。 `CR-EXEC-006`：将普通简单执行的 Luna 思考下限提升为 `low`，多步或写入任务提升为 `medium`，避免为了提速而不当压低判断质量。 `CR-P4-G4-001`：新增非 HTTP、只读的管理状态查询与 403 账户受控恢复，以闭合 G4；认证 HTTP/UI 仍属 P10。 |

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
| `LOCAL_PASS_PENDING_CI` | 实现、review、所需本地测试和本地快速门禁已通过，等待规定的 GitHub Gate；不是 `DONE`，也不计为 `IN_PROGRESS` |
| `DONE` | 代码、测试、文档和证据均完成 |
| `BLOCKED` | 已明确记录阻塞条件，无法继续 |
| `DEFERRED` | 经用户批准移出当前发布范围 |

### 1.2 每个任务的执行循环

1. 读取本文，确认当前 Phase、Task 和前置依赖。
2. 将且仅将一个 Task 标记为 `IN_PROGRESS`。
3. 实现该 Task 的最小完整改动，不夹带下一 Task 功能。
4. 运行该 Task 指定的测试、review 与全局本地快速门禁；安全、Schema、迁移、重试或 CI 变更额外运行本地完整门禁。
5. 保存可复查证据：测试输出、基准、Fixture、日志或报告，并将实现、测试与该 Task 的证据尽量合并为同一提交。
6. 同步代码文档、行为契约和必要的矩阵状态，启动本计划规定的 GitHub Gate。
7. 本地条件已满足但远端 Gate 未完成时标记 `LOCAL_PASS_PENDING_CI`；远端通过前不得标记 `DONE`。
8. 在保持全计划仅一个 `IN_PROGRESS` 代码 Task 的前提下，可开始与 `LOCAL_PASS_PENDING_CI` Task 无共享公开接口、Schema、迁移、行为契约或发布依赖的下一 Task；若远端 Gate 失败，立即停止受影响的后续推进并修复失败 Task。
9. 远端 Gate 成功后满足 Definition of Done，标记 `DONE`；Phase Gate 开始前不得遗留任何 `LOCAL_PASS_PENDING_CI` Task。
10. Phase 内全部任务完成后执行 Phase Gate；Gate 未通过不得进入下一 Phase。

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
   迁移、Fixture、契约或安全策略变更必须运行 GitHub Fast + Full supply-chain Gate。纯报告、
   索引或计划状态变更运行显式 `docs-only` Gate：Markdown/格式、文档链接、Secret scan 和
   计划一致性检查；它不得伪装为 Full 成功。每个 Phase Tag 始终强制运行 Fast + Full。
2. **缓存只加速受版本约束的工具，不替代验证。** CI 可缓存 `cargo-deny`、`cargo-audit` 的
   二进制及其 Cargo registry/git 下载；缓存 key 必须包含 runner OS、固定 Rust 版本和
   `tools/quality-tool-versions.env` 摘要。每次恢复后仍执行版本检查；缺失或不匹配时仍以
   `cargo install --locked` 重新安装。不得缓存 Credential、环境文件、真实测试配置或把 cache hit
   当作供应链通过证明。预装镜像只有在来源固定、版本可复查且另有供应链审查后才可替代缓存。
3. **状态流水线保持一个代码 Task。** `LOCAL_PASS_PENDING_CI` 只表达远端证据等待；它不允许
   合并、发布、跨 Phase 依赖、Phase Gate 或把风险隐去。下一 Task 只能在没有共享关键边界时
   使用该流水线，且同一时刻仍只有一个 `IN_PROGRESS`。有任何 Fast/Full 失败时，相关后续工作
   立即冻结，先恢复失败 Task 的绿色状态。
4. **文档证据只做一次收口。** Task 实现、测试、ADR/Contract 和报告骨架应在代码提交中；GitHub
   Code Gate 通过后，以一个 docs-only 提交记录该不可变 Gate 证据并标记 `DONE`。不得再为复制该
   docs-only run ID 产生第二个状态提交；Phase 报告可批量引用既有外部证据。
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

### 1.6 缓存可见交付与补充供应链 Gate（CR-EXEC-002）

1. **缓存可见交付引用。** 顺序 P4 代码 Task 留在可见质量工具缓存的 delivery ref。P4-02 使用
   当前 `codex/p4-01-catalog-singleflight`；新 Task 分支只有先 seed 同 ref cache 或使用经批准的
   shared/default ref 后才可作为交付引用。该规则不允许并行代码 Task，始终最多一个 `IN_PROGRESS`。
2. **Fast 完整、Full 补充。** GitHub Fast 是完整 Workspace fast check。GitHub Full 依赖同一
   workflow/SHA 的 Fast，仅执行固定质量工具版本检查、`cargo deny check` 与 `cargo audit`；Required
   Gate 对两个结果均 fail-closed。本地 `./scripts/check.sh full` 继续执行完整 Fast 加供应链检查，
   因此本地需要完整门禁的变更只运行一次 `full` 即覆盖 Fast，不机械地紧接着重复 `fast`；Phase
   Tag 仍逻辑要求 Fast + Full。
3. **单次 docs-only 收口。** 代码提交包含实现、测试、ADR、契约、报告骨架和 `IN_PROGRESS` 状态。
   Code Gate 通过后只创建一个 docs-only 收口提交，写入 Code Gate 证据并改为 `DONE`；不为该收口
   的 run ID 再创建提交。
4. **缓存可观测性与目标。** Full job 必须把 cache hit/miss 写入 GitHub job summary。miss 不会降低
   Gate 的正确性结论，但报告必须记录原因。暖态质量工具安装运行目标 `<=10s`、计划硬门槛 `<=90s`；
   暖态 code workflow 目标 `<=4min`（不含 GitHub queue），docs-only workflow 目标 `<=45s`。P4-02
   不额外触发手工暖态重跑，正常 Code Gate 的 summary 即为测量证据。

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
5. **Gate 等待与 closeout 重叠。** Code Gate 运行期间可以预写不含 run ID、状态结论或 `DONE`
   标记的 closeout 草稿、报告表格和索引 diff。Gate 通过后仅填入不可变证据并创建唯一 docs-only
   收口；Gate 失败时丢弃或修正草稿，绝不借草稿提前推进状态或下一依赖。
6. **证据模板化而不删证据。** 后续 P4 Task 复用固定 ADR、Contract、报告、追踪行和 closeout
   模板，只填写本 Task 的决策、行为、测试和时延差异。没有新增架构决定或可观察行为时，不凭
   习惯额外创建文档；现有要求的证据、链接、Secret 约束仍完整保留。
7. **远端查询最小化。** workflow 运行时使用低频状态轮询；完成后先读取一次完整 job 摘要，仅在
   cache、失败或安全证据缺失时读取相关 job log。不得为复制同一状态或刷新无变化页面重复查询。
8. **统一时延报告。** 每个 Task 报告记录 `Task Card`、`代码提交`、`Code Gate 通过`、`docs 提交`
   和 `docs Gate 通过`五个时间点，以及范围等级、重复验证次数、返工次数和超预算原因。此数据
   用于下一 Task 纠正流程；连续两次同类超预算必须先优化执行方法，再考虑扩大并行度。

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

### 1.11 G4 只读管理状态边界（CR-P4-G4-001）

1. `P4-10` 的管理 API 是供受控管理进程调用的 in-process Rust 查询边界，不注册 HTTP route，
   不处理认证、不会读/写 SQLite、不会发送 Provider 请求，也不进入 HTTP/Router 响应热路径。
2. 查询固定在调用方提供的观察时间，返回精确 Endpoint/Credential（可选 model）上的结构化
   Health、403 账户、Quota/429 source-confidence、Circuit 与受控恢复状态；错误必须是无目标、
   无 URL、无 Header、无 Body、无 Secret 的 fail-closed 类别。
3. 已验证的 Provider/driver 只有在给出既有 `CredentialForbidden` 安全错误时才记录 403 账户状态；
   它不透明重试。恢复必须经非克隆票据完成，普通流量在恢复完成前保持不可调度。
4. P10 才能将该边界暴露为认证 HTTP 管理接口、UI、持久化查询或远程控制面；P4-10 不提前实现
   其中任何一项。

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
- 开发分支：`codex/<task-id>-<short-name>`。
- 一个分支只承载一个 Task 或同一 Task 的测试修复。
- Commit 标题以 Task ID 开头，例如 `P1-03: add canonical event state machine`。
- Phase Gate 通过后创建带说明的阶段 Tag，例如 `phase-p3-complete`。
- Release 使用 SemVer，首个服务器候选版本从 `v0.1.0-alpha.1` 开始。
- 不提交 `.env`、真实数据库、Token、Cookie、OAuth JSON 或生产日志。

### 4.3 Definition of Done

一个 Task 只有同时满足以下条件才能标记为 `DONE`：

- 实现满足本 Task 和对应行为契约。
- 正常、边界、错误和取消路径有自动化测试。
- `cargo fmt --check` 通过。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过。
- 受影响测试和 `cargo test --workspace` 通过。
- 没有新增未解释的 `TODO/FIXME`、明文 Secret 或宽泛 `unwrap/expect`。
- 对外行为、配置或 Schema 变化已更新文档和迁移说明。
- 保存完成证据，并更新本文状态。
- 已满足对应 GitHub Gate：代码相关变更为 Fast + Full；纯文档/状态变更为显式 docs-only；
  Phase Tag 为 Fast + Full。`LOCAL_PASS_PENDING_CI` 不满足本条件。

## 5. Phase 总览

| Phase | 目标 | 进入条件 | 退出 Gate | 状态 |
|---|---|---|---|---|
| P0 | 仓库、工具链、ADR、CI 基线 | 本计划锁定 | G0 | DONE |
| P1 | Canonical Core + Mock 垂直链路 | G0 | G1 | DONE |
| P2 | 聚合控制面、Secret、RouteSnapshot | G1 | G2 | DONE |
| P3 | OpenAI Responses 聚合 MVP | G2 | G3 | DONE |
| P4 | Catalog、Health、Quota、Explain、观测 | G3 | G4 | PENDING |
| P5 | Anthropic/Claude Code 兼容 | G4 | G5 | PENDING |
| P6 | Grok Build | G5 | G6 | PENDING |
| P7 | Kiro IDE/CLI | G6 | G7 | PENDING |
| P8 | Grok Official | G7 | G8 | PENDING |
| P9 | Grok Web | G8 | G9 | PENDING |
| P10 | 完整管理 API、Web UI、备份恢复 | G9 | G10 | PENDING |
| P11 | 差分、性能、安全与发布加固 | G10 | G11 | PENDING |
| P12 | 服务器部署、灰度、切换与回滚 | G11 | G12 | PENDING |
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
| P5-01 | 实现 Anthropic Messages 入站、非流式和 SSE 出站 Adapter | G4 | Anthropic Fixture 与事件快照 | PENDING |
| P5-02 | 实现 `count_tokens` Canonical 路由和 Provider Capability；无准确能力时明确拒绝 | P5-01 | 准确路径和 Unsupported 测试 | PENDING |
| P5-03 | 完成 Tool 增量 JSON、并行 Tool、空参数、必填参数和 ID 映射状态机 | P5-01 | 1-byte Chunk 属性测试 | PENDING |
| P5-04 | 实现 Pass-through/Canonical/Lossless Bridge 能力分析器 | P5-01,P3-01 | 字段/Tool/Reasoning 不可损转换矩阵 | PENDING |
| P5-05 | 支持同一 Upstream 的 Responses 与 Anthropic 独立 Endpoint/健康/熔断 | P5-04 | 单协议故障隔离 E2E | PENDING |
| P5-06 | 实现 Thinking、Stop Reason、Usage、Cache 字段和响应模型回写 | P5-01 | 协议对照 Fixture | PENDING |
| P5-07 | 建立 Claude Code `--bare` 最小 E2E 和 Plan Mode 回归 | P5-03-P5-06 | 真实客户端脱敏日志 | PENDING |
| P5-08 | 加入未知字段、畸形流、截断 Tool 和取消 Fuzz/Property Test | P5-03 | 固定 Corpus 和无 Panic 报告 | PENDING |

### G5 门禁

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
| P6-01 | 实现 Grok Build Credential、OAuth JSON 导入和 Device Code | G5 | OAuth Mock + 脱敏导入测试 | PENDING |
| P6-02 | 实现每 Credential Refresh Singleflight、Revision/CAS 和持久化 | P6-01 | 刷新风暴与旧 Token 覆盖测试 | PENDING |
| P6-03 | 实现 Build Responses HTTP 请求、流和错误解析 | P6-02 | 固定 Fixture + 测试账号验证 | PENDING |
| P6-04 | 实现模型、Billing、Quota Window 和 Reset 同步 | P6-03 | 来源/置信度和窗口测试 | PENDING |
| P6-05 | 实现租户隔离 Cache Identity 与 Cache Affinity | P6-03 | 稳定性、隔离和断裂事件测试 | PENDING |
| P6-06 | 实现 ResponseOwnership 与 ReasoningReplay | P6-03,P6-05 | previous_response 与多轮 Tool 测试 | PENDING |
| P6-07 | 实现 Build 专用 401/403/429/Quota/Transient 分类 | P6-04 | 错误 Fixture 矩阵 | PENDING |
| P6-08 | 与 CPA/grok2api Build 行为做 clean-room 差分 | P6-03-P6-07 | 差分报告和 intentional diff 清单 | PENDING |

### G6 门禁

- 两个 Build Credential 的并发、轮询、刷新、Quota 和 Failover 通过。
- Cache Identity 和 Affinity 均稳定，跨 Client Key 不串缓存。
- Response Ownership 不允许静默换账号续接。
- 旧请求不能覆盖新 Token 或错误封禁已刷新 Credential。

## 13. P7 - Kiro IDE/CLI

目标：原生实现 Kiro，不依赖 Kiro-RS 作为长期运行层，并保持 Claude Code 兼容。

主要矩阵：`C35-C47 E25 E26 G24 G28`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P7-01 | 实现 Social、IdC/Enterprise、`ksk_` 三类 Credential | G6 | 各类解析、加密和刷新 Fixture | PENDING |
| P7-02 | 实现 IDE/CLI Endpoint Policy、Region、Header、Origin 和 URL | P7-01 | 请求快照对照测试 | PENDING |
| P7-03 | 实现 `profileArn` 查询、回退、注入、来源和审计 | P7-01,P7-02 | Builder/Enterprise 场景测试 | PENDING |
| P7-04 | 实现 CanonicalRequest 到 Kiro Conversation Request | P7-02 | 多轮消息/Tool Fixture | PENDING |
| P7-05 | 实现 AWS EventStream 增量解析、CRC、边界和错误恢复 | P7-04 | 任意 Chunk + 损坏帧测试 | PENDING |
| P7-06 | 实现每 Credential 动态模型与订阅能力、最后成功快照 | P7-01,P4-02 | 部分失败和 stale 测试 | PENDING |
| P7-07 | 实现 Kiro Tool、AskUserQuestion、Plan Mode 和 Thinking 映射 | P7-04,P7-05 | Claude Code 回归套件 | PENDING |
| P7-08 | 实现 Kiro 网络、账号、模型、额度和普通 429 分类 | P7-06 | 错误与恢复矩阵 | PENDING |
| P7-09 | 与服务器定制 Kiro-RS 做差分和真实 `--bare` E2E | P7-03-P7-08 | 差分报告、日志、模型列表 | PENDING |

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
| P8-01 | 实现 Official API Key、Endpoint、Header 和模型发现 | G7 | 请求/目录 Fixture | PENDING |
| P8-02 | 实现 Official Responses HTTP 非流式与 SSE | P8-01 | 官方测试账号 E2E | PENDING |
| P8-03 | 实现 Quota/Rate Header、Reset 和 Billing 元数据 | P8-02 | Header Fixture | PENDING |
| P8-04 | 实现 Official Tool、Reasoning、Search 能力声明与转换 | P8-02 | Capability 测试 | PENDING |
| P8-05 | 验证 Official/Build 状态、Affinity、Quota 和故障完全隔离 | P8-02-P8-04 | 隔离 E2E | PENDING |
| P8-06 | 完成官方路径差分、负载和错误矩阵 | P8-05 | Phase 报告 | PENDING |

### G8 门禁

- Official 与 Build 同名 Public Model 只有显式 Route 才能共同候选。
- 一个来源的 401/403/429 不改变另一个来源状态。
- 官方 Tool/Reasoning 能力与公开元数据一致。

## 15. P9 - Grok Web

目标：实现独立 Web/Console Provider，处理浏览器会话、出口指纹和网页协议漂移。

主要矩阵：`C29-C34 D28-D30 E27-E29 F17 G24-G28`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P9-01 | 实现 SSO/Cookie Credential、血缘和独立生命周期 | G8 | 导入、加密、失效测试 | PENDING |
| P9-02 | 实现 BrowserEgressSession：Cookie、UA、TLS Profile、Proxy 绑定 | P9-01 | 指纹一致性和隔离测试 | PENDING |
| P9-03 | 实现 Grok Web Chat 请求和流响应解析 | P9-02 | 脱敏网页 Fixture | PENDING |
| P9-04 | 实现 WebConversationState 与账号/出口强绑定 | P9-03 | 多轮、过期和账号不可用测试 | PENDING |
| P9-05 | 实现 Statsig 签名缓存、受限失效和 SSRF 防护 | P9-02 | 403、Redirect、域名测试 | PENDING |
| P9-06 | 实现 REST/gRPC-Web Quota、Tier、Window、Source/Confidence | P9-03 | Quota Fixture | PENDING |
| P9-07 | 实现 WAF/EgressRejected 与账号 Forbidden 分离 | P9-02,P9-03 | 403 分类矩阵 | PENDING |
| P9-08 | 实现 Tool Emulation Feature Flag，默认关闭并标记 `emulated` | P9-03 | 开关与能力元数据测试 | PENDING |
| P9-09 | 完成 Feature Flag 下真实账号 E2E、协议漂移和熔断演练 | P9-04-P9-08 | Canary 报告 | PENDING |

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
| P10-01 | 完整管理 OpenAPI：Upstream、Endpoint、Credential、Catalog、Route、Group、Key | G9 | OpenAPI Contract Test | PENDING |
| P10-02 | 实现管理鉴权、仅本机/私网策略、审计和 CSRF/CORS 边界 | P10-01 | 未授权和跨站测试 | PENDING |
| P10-03 | 建立 TypeScript SPA、生成 API Client 和静态资源构建 | P10-01 | 可重复前端构建 | PENDING |
| P10-04 | 实现 Upstream/Endpoint/Credential 管理与测试工作流 | P10-03 | 浏览器 E2E | PENDING |
| P10-05 | 实现 PublicModel/Route/Candidate/AccessGroup/ClientKey 工作流 | P10-03 | 创建 `minimax-m3` E2E | PENDING |
| P10-06 | 实现 Catalog Diff、Health、Quota、403、Route Explain 和请求追踪页面 | P10-03,P4-06 | 浏览器 E2E | PENDING |
| P10-07 | 实现 Config Version、发布、回滚和操作审计页面 | P10-03,P2-10 | 发布失败/回滚 E2E | PENDING |
| P10-08 | 实现加密备份、恢复预检、Schema Version 和 Secret Key 说明 | P10-01 | 空机恢复演练 | PENDING |
| P10-09 | 嵌入静态资源并验证 UI 不进入推理热路径 | P10-03-P10-08 | 性能对比与资源隔离报告 | PENDING |

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
| P11-01 | 建立 CPA v7.2.80、grok2api、Kiro-RS 的脱敏差分 Fixture Harness | G10 | 差异分类报告 | PENDING |
| P11-02 | 完成网络、DNS、TLS、429、5xx、截断流、慢客户端和取消故障注入 | P11-01 | Fault Matrix | PENDING |
| P11-03 | 建立 Mock Provider Criterion/HTTP 基准和回归阈值 | P11-01 | `benchmarks/baseline.json` | PENDING |
| P11-04 | 执行并发、长流、连接池、内存、背压和 24h Soak | P11-02,P11-03 | 性能与 Soak 报告 | PENDING |
| P11-05 | 执行 SSRF、Secret、Auth、权限、依赖和供应链安全审计 | P11-01 | Security Report + SBOM | PENDING |
| P11-06 | 验证优雅停机、流 Drain、崩溃重启、磁盘满和事件队列降级 | P11-02 | Recovery Report | PENDING |
| P11-07 | 完成升级/降级 Migration、备份恢复和旧版本回滚演练 | P10-08 | Upgrade/rollback report | PENDING |
| P11-08 | 生成 Release Candidate 清单、已知差异和生产默认配置 | P11-01-P11-07 | `v0.1.0-alpha.1` 候选说明 | PENDING |

### G11 门禁

- 所有差异均标记为 Intentional、Compatible 或已修复 Regression。
- 无未分类 Panic、数据竞争、流截断或 Secret 泄漏。
- 性能基准相对已批准 Baseline：吞吐下降不超过 10%，P99/RSS 恶化不超过 15%。
- Mock 上游网关附加延迟目标：本地 warm-path P99 不超过 5ms；服务器不超过 10ms。
- 24h Soak 无内存持续增长、连接泄漏或 SQLite 损坏。
- 回滚包和恢复步骤已经实际演练，不是只写文档。

## 18. P12 - 服务器部署与灰度

目标：在不破坏现有 CPA/AxonHub/New API/Kiro-RS 的前提下部署、验证、灰度和切换。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P12-01 | 构建固定版本二进制、Docker 镜像、SBOM、Checksum 和签名 | G11 | 可验证发布产物 | PENDING |
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

代码、工具链、workflow、脚本、迁移、Fixture、契约或安全策略变更必须运行：

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
secret scan
changed-doc link check
```

纯报告、索引或计划状态变更使用 `docs-only` Gate：Markdown/格式、文档链接、Secret scan 和
计划一致性检查。它不能以未执行的 Rust/供应链检查冒充代码 Full Gate；具体 workflow 分类和
required-status 由 P4-00 实现并验证。

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
Execution timing (Task Card / code commit / Code Gate / docs commit / docs Gate):
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
