# #69 CPA 请求控制与模型命名实施记录

日期：2026-10-25（Asia/Shanghai）。验证进行中；本文件尚不是 Full 或整票通过回执。

- 基线：`84fbfbf778b582f746c1cafe31a2ce58962e8f77`，来自 INTEGRATION-RESULT.md 并与实际 Git tree 核对。
- 分支：`feature/69-request-controls-model-names`。
- worktree：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar-69-request-controls-model-names-20261025`。
- P0 提交：`551961c0`；先完成 P0，再实施 P1。
- 固定 CPA：v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`。
- 决策：ADR-0130；契约：BC-PROTOCOL-017；无 migration，schema39 不变。
- [控制/名称行为矩阵及来源](../support/cpa-request-controls-model-names.md)。

P0 原有 effort 提取复用；后缀和计数复用已有公共名称/路由/权限链路。显式前缀材料化为现有 Alias，
不新增路由策略、payload 脚本、Provider 或前端。原集成和其他工作区均保留。

P0 router 12/12、HTTP 4/4 先通过并提交；P1 本轮 router 13/13、新公共 receiver/count 测试6/6通过。
管理 HTTP、publish/rollback、旧存储 payload、完整 count 回归和 Full/Standards/Spec 尚待本轮收齐。
早期 P0 预算替换错误、P1 -1 后缀丢失 auto 意图、编译修正日志保留于 ignored output；不改写为 PASS。

最终交付须补上源码/lock/binary/输入 hash、逐项验收、审查和 Full 结果。
实际客户端、固定 Build、真实 Provider、远端 CI、release/部署均 NOT_RUN；费用 UNKNOWN，
`cost_in_usd_ticks` 仍 NOT_ADMITTED。未 push、合并、评论/关闭工单或部署。
