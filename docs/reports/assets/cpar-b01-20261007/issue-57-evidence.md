# CPAR-48 / #57 本地补充：B-01 修复检查点

日期：2026-10-07，Asia/Shanghai。**本地补充，未写入 GitHub。#57／#27／#31／#9 仍 OPEN；release-readiness = BLOCKED。**

## 本轮唯一状态变化

2026-10-05 发布复核的 **B-01 / P2**，对应 #27／CPAR-18、#31／CPAR-22、父规格 #9，现为 **RESOLVED_LOCAL**。这是 Responses 多文本片段同协议历史回传切片，未重新实施整个 B 批次。

- 固定起点：`ee521c82dad311f8e82e15f0f63f1e4f5189b35f`。
- 最终修复源码、新 Full 与两轴审查对象：`939028d63519126f16252c3e72d3e1d5ace7ca75`。
- [原始 B-01 发现](../cpar-release-readiness-20261005/review-results.md)、[原发布复核](../../cpar-release-readiness-and-controlled-deployment-preparation-20261005.md)和[此前 A-01 本地补充](../cpar-a01-20261007/issue-57-evidence.md)保持原文，未覆盖历史失败结论。
- [JSON RED](red-json.md)／[SSE RED](red-sse.md)：第一轮合法双片输出成功；从实际返回 output 构造的第二轮 400，上游总接收／handoff 为 1，exit 101。[原始回执](red-receipts.json)与[当时测试 patch](red-tests.json)保留。
- [GREEN](execution.md)：生产公开 handler、路由／准入、provider 编码与真实本地 HTTP 上游闭环，两轮成功；第二轮实际接收的完整 input 保持片段内容、数量、顺序、id/status、每片独立 citation 与 null/empty logprobs。SSE 四次分段 delta、两个完整 part、唯一 completed 均核对，实际完整结果进入下一轮。
- 10 种非法语义 × JSON/SSE 两模式共 20 组合仍 400，实际上游接收、handoff、Attempt 均为 0。单片段、reasoning-only、工具关联、pending-tool、Chat/Messages 既有回归通过；Grok 原生两片 phase／reasoning metadata 回归通过。
- [新源码 Full](full-check.md)：clean checkout 前后干净，45/45 PASS，1461 Rust tests passed、0 failed、12 原有 ignored；[精确回执](full-receipt.json)和[完整输出](full-output.json)绑定新修复源码。
- [独立两轴](review.md)：Standards PASS／Spec PASS；各 COMPLETE 19/19，零可行动发现。静态审查与实际测试执行独立记录；文档收口不扩增探子的覆盖。

完整实施、因果关系和信息保真断言见 [B-01 修复报告](../../cpar-b01-responses-multipart-history-20261007.md)与 [verification](verification.json)。只有本轮指定的本地完成标准满足后才更新上述状态。

## 其余发现实际状态保留

| 编号 | 既有状态 | 本轮处理 |
| --- | --- | --- |
| A-01 | RESOLVED_LOCAL；源码 `42bc473f2eba8c34484b54aaf09fff455aa13c9c` | 保留既有精确范围及证据 |
| A-02–A-08 | FAIL，7 项 P2 规格问题 | 未修改，仍待独立修复 |
| B-02–B-05 | FAIL，4 项 P2 规格问题 | 未修改，仍待独立修复 |
| S-01 | FAIL，P3 ADR 结构问题 | 未修改 |
| E-01 | PARTIAL，四种 API preset 执行证据 | 保留 |
| E-02 | PARTIAL，保存后响应丢失与恢复证据 | 保留 |
| E-03 | FAIL_FIXTURE，Kiro fixture 材料分类 | 保留；不推断生产材料错误 |
| E-04 | PARTIAL，public isolation matrix | 保留 |
| E-05 | PARTIAL_SPEC_CONFLICT，revision 与 inherited same-grant 合同 | 保留；未修改规格或增加豁免 |
| O-01 | OPEN_OUT_OF_SCOPE，本轮发现的 phase-only 普通 JSON 输出准入 | [独立记录](additional-observation.md)，未修，未混入原 P2 计数 |

原 13 项 P2 中 A-01、B-01 现有独立本地修复检查点，**尚余 11 项 P2**。原 A 96/96、B 73/73 静态审查覆盖仍是历史事实；本轮没有重新审查整批，A 历史 Spec FAIL、B 历史 Standards／Spec FAIL 不升为整批 PASS。C、D 和此前 execution 切片的固定范围结论保持；C／#57／#9 未完成。

## 发布与正式 Gate

本轮为本地修复检查点。Task 定向、review、fmt／secret 及公开边界 Full 已满足；正式远端 Phase Gate 留待后续批次冻结，不新增 Phase tag 或宣称 Phase DONE。#27／#31 全范围验收及 #57／#9 收口不由 B-01 单项修复完成。

旧 source80 `80f5838902eb56be6dccc3ce047eb227d6c18e6c` 的 Full、签名、制品和迁移演练保留原绑定。新源码有自己的本地 Full；新候选冻结／构建／签名、远端 Required Gate、真实 Provider、生产迁移与部署均 NOT_RUN。没有推送、发布、GitHub 写入或关闭工单。

[工作区保留](workspace-preservation.json)确认原 4277 个用户修改／未跟踪文件哈希不变，AGENTS.md 未纳入提交。下一项建议独立处理 **B-02：Responses SSE done 确认事件的闭集与一致性校验**。
