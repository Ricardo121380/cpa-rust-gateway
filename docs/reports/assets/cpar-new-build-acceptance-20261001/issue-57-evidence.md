# #57 本地归集：新构建真实渠道验收工具与环境准备

目标 [CPAR-48 / #57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57)，父规格
[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)。本包是新切片，旧 C/D 成果和证据
仍按原版本/范围有效；原包文件/manifest 未改。**准备独立 PASS，#57/C BLOCKED，父 #9 未完成。**

- 实现：64e5cf9a；复现/评审修复/正式验证源码 ff4b59ed。
- [主报告](../../cpar-new-build-acceptance-20261001.md)、
  [正式入口与命令](../../../../scripts/acceptance/cpar-new-build-acceptance.md)。
- [Full 45/45](full-check.md)、[固定源码/文件hash/环境/时间运行回执](full-run-receipt.json)、
  [完整输出](full.log)。新工具 24/24，6 basic receipts 在 local-receipts；LOCAL_SIMULATED。
- [Standards / Spec](review.md) 分别记录初始 FAIL、先复现后修复和限定关闭 PASS；
  [固定旧实现 red 复现](review-red-receipt.json) 不被记为最终实现失败。
- [前置条件](real-channel-preconditions.md)、[当前环境](current-environment.json)、
  [旧历史 Usage/Attempt/账本的当前只读采集](historical-attribution.json)、
  [出口只读 DNS/TCP/TLS](egress-probes.json)、[逐组合 readiness](combination-readiness.json)。
- [具体首轮计划](next-krill-responses-json-plan.json)、[旧构建拒绝预检](current-runtime-preflight.json)。
  null 有意阻止执行，不用其他成功/配置快照倒填 executing facts。

## #57 要求与当前边界

| 验收要求 | 本轮可审查成果 | 保留未完成 |
| --- | --- | --- |
| exact build/target/actual attempts，不靠 HTTP200 | /proc artifact+manifest+每轮 runtime；11 target fields/原生 Usage/ledger exact source | 新 Linux artifact/部署及实际 execution revision 证据缺失，真实格 0 |
| 三协议/JSON-SSE/文本与两工具周期 | stdlib 原 live runner extension，6×7 loopback 正例；停止负例与完整语义校验 | 真实组合尚 NOT_RUN，使用固定账号/route/模型后再逐格运行 |
| 实际声明扩展逐项，未知不能false | null 声明 BLOCKED；declared checks 缺样本 NOT_RUN | 当前 capabilities={} 非 effective declaration，受控边界/扩展真实样本未取得 |
| 审查与固定源码门禁 | 两轴确定问题关闭；ff4b59ed Full 45/45，原 C/D 有效证据保留 | review scope 明确，未宣布 C 全规格/全批验收通过 |
| 真实环境和授权边界 | 只读当前 runtime/accounts/routes/grants/出口；操作目标/缺口/下一顺序具体 | 未部署/启停/改配置/恢复账号/改权限；实际推理0，远程 push/评论/关单0 |
| 既有协议及兼容边界 | Kiro/Web/capped/条件 metadata 格保留；两条既定 restriction 保留 | 未修大型 Provider native gaps，不删参/模拟/下游截断/关闭声明取得通过 |
| 区分本地/真实/生产 | receipts layer；basic PASS vs overall NOT_RUN；历史采集独立 observation layer | 真实新构建/生产/远程CI/release均NOT_RUN，不能作为批准发布或关单的证据 |

11 渠道、198 基础规划格、24 Kiro capped、12 条件 metadata、49 扩展准备项归集完整；
读取更多实际声明后继续扩展。准备完成不等于 #57 完成；本轮没有改变工单状态或规格。
