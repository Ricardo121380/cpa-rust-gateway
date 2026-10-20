# Spec review — #64

Independent read-only review of `0d13e485fca189fefd2801b0d2c72c7a991a45df...06f779f24434f7de95426b05f33484a1c4e33a15`.
The reviewer inspected all ten changed files, the frozen #58/#64 specifications and observation-02 receipts.
It did not run tests or clients. Existing overlays are excluded from the product diff.

**Spec：PARTIAL_BLOCKED**。新增 bug / scope creep：未确认。

- AC1 PASS_BOUNDED。规格要求“冻结当前实际 CLI 版本、入口、独立配置及请求行为”；驱动冻结相同文件并在
  OS 负例后运行，历史 observation-02 身份和 isolation 回执确认 2.1.302、固定 SHA 与网络/文件/子进程拒绝。
- AC2 BLOCKED。规格要求“完成回答、两次相关工具循环、结果返回与后续问答”；`messages/claude.rs:95`
  只接受 HTTP400，`:116` 明示 BLOCKED_BEFORE_FIRST_RESPONSE；第二工具依赖第一实际结果等未完成。
- AC3 PARTIAL。规格要求“观察并兑现实际必需的……thinking……历史语义”；准确预拒绝成立、兑现未成立。
  报告分别列 adaptive/display、effort、context、metadata、cache 阻塞。独立协议正例不构成 Claude PASS。
- AC4 BLOCKED。规格要求“客户端不展示摘要，同时完整加密摘要可按 exact Attempt 授权读回”；
  `messages/claude.rs:117` 为 BLOCKED_NO_ATTEMPT。无成功回答不能证明隐藏后续轮、Usage 或返回模型。
- AC5 PARTIAL。规格要求“验证流式增量、失败、取消与必要启动请求”；400 失败有证据，成功增量及
  活客户端 delta→lease1→abort→peer close→lease0 缺失。无 lease 记录与实际取消 BLOCKED；启动/count 未观察。
- AC6 PARTIAL。规格要求“以独立接收数证明无额外发送……缺失的必需能力继续 BLOCKED”；
  `messages/claude.rs:106-107` 分别检查完整请求与 accepts=0。报告区分客户端 fallback / 网关重试并保留 UNKNOWN cost。

收尾证据缺口：初审时报告明确固定源码、Full、评审和归档待录；observation-02 有 binary SHA，但没有
source-inputs.json，尚不能绑定该历史实际运行到最终候选。#58 要求“每项结果绑定……源码/构建”，须补齐。
此项属于待补证据，不是新增兼容缺陷；主代理将在同一固定候选的独立验证树重采实际运行并核对身份。

修改限测试、观察器、驱动和说明，没有生产字段准入放宽；非诊断模式保持失败，没有冒充兼容。
Standards/Full 的绿色不消除上述 AC 缺口。

## Supplemental identity delta

另一位干净探子只读审查 `06f779f24434f7de95426b05f33484a1c4e33a15...6edb8085f9e850edc55e68d382ed97acbc54a0c5`：
**增量 PASS_BOUNDED（静态），整票仍 PARTIAL_BLOCKED**，新 Spec finding / 授权越界 0。
确认 port=None 不生成 allow，版本/帮助使用该 fence；正式任务仍验证 numeric loopback 和端口。
runtime 只作为 header 声明值，报告继续保留实际 tools/summary/cancel、最终 Full 和身份补证缺口，
没有把 diagnostic 变成兼容成功。此补审未新增动态执行或最终 Full 证明。

## Primary closeout evidence

初审提出的最终源码身份补证已由主代理在评审后完成，而非由上述静态审查代验：
产品源码 `6edb8085f9e850edc55e68d382ed97acbc54a0c5` + 原 16 文件 overlay 的验证提交
`ad40a2e5f7f8ed05fd21917aabb329e617fac32b`，Full 45/45、1575 passed / 0 failed / 15 ignored。
同一验证目录的最终 opt-in 实际运行保存 `source-inputs.json`，393 个源码、manifest、lock、配置与
driver 的 hash 逐项匹配 Full 及产品工作树；实际二进制/配置身份见 `source-manifest.json` 和
`actual-cli.json`。默认外层退出 1、内层观测通过、两次实际 400 与独立上游 accepts/receives=0 均保留。
仅此“最终候选来源可绑定”缺口已补齐；AC2/3/4/5 的实际兼容、工具、摘要与取消缺口仍为
PARTIAL_BLOCKED，没有以 Full 或观测成功关闭工单。
