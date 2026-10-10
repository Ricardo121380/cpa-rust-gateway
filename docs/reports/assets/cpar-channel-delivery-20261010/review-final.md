# 本批固定diff独立两轴终态

固定base `d3d2e3c5bf0d9521f3fb0d830344b34053f62155`，review head `85f43c2f28ee0a9bbaea3916abaec302b2cedcac`；使用 `git diff d3d2e3c5bf0d9521f3fb0d830344b34053f62155...85f43c2f28ee0a9bbaea3916abaec302b2cedcac`。

三笔提交：611949b6 T12驱动；c1b304d0 legacy Usage Store修复／契约／收紧驱动；85f43c2f证据与报告。实际行为和最终Full/T12执行源码仍为c1b。报告及本终态归档不改变执行来源。两位探子各自独立只读、单轮完成，未修改文件、未运行测试、未访问真实凭据或Provider。以下保留返回原文；本文件是在被审tree外追加的终态证据，不声称它自身已经处于该固定diff内。

## Standards

Sender：`/root/final_standards_review`；终态：complete。

**Standards：PASS；hard_count=0；heuristic_count=1（非阻断）。**

覆盖固定 `d3d2e3c5…85f43c2f` 三笔提交：四个核心路径全文、三份报告、证据索引及36组归档日志/回执的哈希和来源一致性。读取实际含用户未提交修改的 `AGENTS.md`、domain/issue-tracker、CONTEXT、ADR索引、0100/0103/0027及OBS001/002、MGMT010/012。未修改文件、未运行测试、未访问真实凭据或Provider。

**文档标准硬违反：未发现。**

- `docs/adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md:60` 原文：“Legacy canonical Usage without evidence fields defaults to unknown.” `crates/gateway-store/src/event_store.rs:947` 的 `payload_matches_current_or_legacy_usage` 落实现有义务；没有schema、事件模型或公开协议变更，未发现违反 `docs/adr/README.md:10` 的缺ADR情形。
- `event_store.rs:981` 同时要求两项证据为Unknown，`:1002` 严格比较冻结旧编码；读与重放共用比较函数（`:381`、`:926`）。`:2229` 回归覆盖原字节不变、重放零新增、计数改变及补Attempt拒绝，符合BC-OBS-002:39–53和ADR-0103:35–44。
- 按 `AGENTS.md:19–32` 列出的共享路径逐项查询固定diff，结果为空；无需跨边界日志/trailer。当前AGENTS修改仍在，未纳入三笔提交。
- `docs/reports/assets/cpar-channel-delivery-20261010/execution-index.json:1101`、`:1133` 绑定clean c1b的T12与完整Full；36组raw/gzip/receipt哈希及来源字段均一致，失败仍保留。Full原日志统计1500/0/12；frontend原来源611且611→c1b的web/prism diff为空。85仅增加报告资产，未冒充执行来源。

**以下全部是 labelled heuristic，不能作为硬违反：**

| Smell | 本批判断与关键hunk |
| --- | --- |
| Duplicated Code | **1项低优先级**：`scripts/acceptance/cpar_t12_upgrade_http.py:304`、`:314` 重复state/credentials的删除后复制步骤。 |
| Mysterious Name；Feature Envy；Message Chains；Middle Man | 无新增实质触发：`event_store.rs:947` 名称明确；T12 `start`/`stopped`（`:46`、`:54`）分别承载flush及隔离进程行为。 |
| Data Clumps；Primitive Obsession | 无新增实质触发：`event_store.rs:954–972` 将冻结旧字段分组，沿用RequestId/ResponseId/AttemptId。 |
| Repeated Switches；Shotgun Surgery；Divergent Change | 无新增实质触发：`:381`、`:926` 汇入同一兼容比较函数，修改理由一致。 |
| Speculative Generality；Refused Bequest | 无新增实质触发：T12 `verify`（`:27`）参数用于当前历史读者/恢复场景；无继承结构。 |

主代理处理：保留该非阻断建议，不为两处明确pre/post恢复步骤新增通用恢复抽象；行为与验证均不受影响。若以后增加第三种恢复路径，再考虑窄函数提取。

## Spec

Sender：`/root/final_spec_review`；终态：complete。

固定 `d3d2e3c5…85f43c2f` 三点diff；已查四个实现/契约路径、三份报告、后继台账、五票快照，并实算抽查5份gzip/raw/receipt哈希，全部匹配。**当前实施切片 Spec PASS；已查 hard_count=0；原承诺完整覆盖仍 PARTIAL。** 未评Standards、未运行测试、未全面重复184份历史核验。

- **实现事实：** 原规格附件 `已粘贴的文本.txt:119` 要求“未知历史继续如实隔离，不猜测归属或补造账本”。`crates/gateway-store/src/event_store.rs:947–1003` 的 `payload_matches_current_or_legacy_usage` 仅接受exact current或两证据均Unknown的冻结legacy编码；两处调用分别覆盖读取和重放（381、926）。符合 `ADR-0100-exact-protocol-semantics-and-usage-evidence.md:60`、`ADR-0103-failed-bootstrap-usage-attribution.md:35–44`，未发现放宽归属、重写历史或未修复产品错误。
- **范围事实：** T12实际归档为clean c1b、71项PASS、18集合非空且升级前后相等、真实loopback接收5次；旧billing读者拒绝新Attempt身份，binary-only rollback明确UNSAFE。出处：`docs/reports/assets/cpar-channel-delivery-20261010/clean-t12/receipt.json:2–4,338–363`、`clean-t12/t12.json:847–867`；恢复实现见 `scripts/acceptance/cpar_t12_upgrade_http.py:273–328`。未发现diff内无授权scope creep。
- **仍未完成：** 原规格70/92明确允许具体BLOCKED；Kiro cap、Web续接/Messages cap仍按此保留（`docs/reports/cpar-channel-native-semantics-20261010.md:22,34–40`）。C的#26/#55已有独立COMPLETE终态，旧Web/Console专属页面仍PARTIAL（`assets/cpar-channel-delivery-20261010/review-historical-supplement.md:40–54`）。186真实行、49项实际声明未完成；GitHub同步和最终双轴归档仍待执行（`cpar-channel-delivery-local-20261010.md:68,76`）。

**AC02证据判断（供主代理决定）：在用户限定T12样本和local边界下，可以勾选。** 原规格115明确“不擅自扩展为所有历史版本”，116–123要求的身份、停用、权限、连接、路由、Usage/账本、历史隔离、密钥和一致恢复均有上述证据。现后继台账 `issue57-successor.json:14774–14783` 以“无生产验证／非所有历史版本”维持PARTIAL，理由超出该AC本轮完成门槛；同步时应使机器状态与勾选理由一致，保留 `PASS_LOCAL_BOUNDED` 限定及生产NOT_RUN，#57整体继续OPEN。

主代理处理：采纳AC02判断，后继台账改为PASS_LOCAL_BOUNDED，GitHub仅勾该兼容AC；#57整体OPEN，整体历史Spec仍PARTIAL。后续远程同步及文档终态只追加归档／有限状态说明，不扩大这次评审为所有渠道、真实Provider或生产通过。

两轴计数独立：Standards hard0／heuristic1；当前切片Spec hard0；历史Spec完整覆盖PARTIAL（已查hard0），#26/#55 COMPLETE。冻结候选仍NOT_MET。
