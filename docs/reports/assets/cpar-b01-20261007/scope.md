# B-01 fixed scope

日期：2026-10-07，Asia/Shanghai。固定起点：`ee521c82dad311f8e82e15f0f63f1e4f5189b35f`，分支 `codex/prism-v4-delivery`。上次产品修复为 `42bc473f2eba8c34484b54aaf09fff455aa13c9c`（A-01）；起点没有后续 B-01 修复。用户已有 AGENTS.md 修改及无关工作区内容保留。

任务是 [2026-10-05 B-01/P2](../cpar-release-readiness-20261005/review-results.md)：[#27 / CPAR-18](issue-27.md)、[#31 / CPAR-22](issue-31.md) 和[父规格 #9](issue-9.md)中的 Responses 同协议多片段历史回传；不是重新实施 B 批。闭环为受控上游 → 公开 JSON/SSE → 使用实际返回的 output items 构造下一轮 input → 准入／目标编码 → 受控上游实际接收。

本次请求与父规格已经确认测试边界：公开三协议 HTTP/SSE、临时 SQLite、合成账号和 loopback 上游。公开 handler 使用实际 Actix resource 注册、请求解码、Client Key auth、route compiler、leased executor、provider request builder 与响应投影；下游请求由 Actix test service 分派，上游使用真实 loopback HTTP listener。这里不声称部署进程或下游 TCP listener 验收。

约束：保持片段内容、数量、顺序和所属元数据；保持角色／闭集字段、工具关联及能力准入；不可表示的信息明确拒绝；不拼接或合并片段。生产修复只恢复 Responses 纯文本 assistant 内容数组的既有表达，不改变共享工具历史规则、Canonical 类型、Schema、依赖或能力声明。依据 [ADR-0100](../../../adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md)、[ADR-0101](../../../adr/ADR-0101-grok-native-item-metadata-fidelity.md) 和 [BC-ROUTER-004](../../../contracts/BC-ROUTER-004-protocol-transform-admission.md)，属于既有决策的缺陷修复，不新增协议决定。

日常实施、定向／静态／Full、只读独立两轴评审和本地原子提交已授权。子代理只读、单轮、fork_turns=none；派发后主代理等待，代码修改、方案判断及最终验证由主代理负责。

不扩展到 A-02–08、B-02–05、E-05；不改前端、生产、真实账号、Provider 或外部模型；不推送、部署、发布或写入／关闭 GitHub。source80 签名、制品与演练维持原绑定。B-01 仅在全部本地要求满足后记 RESOLVED_LOCAL；远端 Phase Gate、C、#57、#9 和项目发布保持独立状态。
