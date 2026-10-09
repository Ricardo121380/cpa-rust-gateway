# GitHub #36 同步草稿：E-05 仍待裁定（未发布）

本地批次固定 base `01dae81bb9778c92d7e76e49bbed14c3fdc565d7`。本文件仅供审查，没有 GitHub 写入，也不请求关闭 #36。

当前核对：#36 的验收要求 credential revision 变化后拒绝旧 Responses 续接；
[BC-RESP-003](../contracts/BC-RESP-003-exact-continuity-and-compaction.md) 则允许严格证明仍属同一次授权的 Grok Build 正常刷新跨 revision 续接。
2026-10-09 的初始和最终只读查询均没有评论裁定；当前会话已发一次二选一卡，尚未取得明确答复。
本批不以推荐项、现有实现、Full 通过或沉默代替裁定，规则状态为 **AWAITING_USER_DECISION**。

## 选择 1 后可同步的文本

保留受限的 Grok Build 正常刷新例外。只有完整、连续、无冲突的同授权证明允许跨 revision；
exact revision 继续正常处理。手动换凭据、重新授权、证明缺失或冲突以及其他 lineage／所有权变化拒绝，
拒绝不会切换账号、fallback 到成功、生成可继续使用的错误会话或写入错误归属。
工单应明确这一受限例外，与 BC-RESP-003 同步。

此文本尚未生效。决定后需补齐公开 handler、持久化和续接边界的正反验证回执，再填入源码及结果；
既有内部函数或旧测试不能独立满足本批验收。

## 选择 2 后可同步的文本

所有 credential revision 变化均拒绝旧 Responses 续接，包括已经证明同授权的 Grok Build 正常刷新；
客户端需重新开始会话。exact revision 的合法续接保留，手动重新授权和其他 lineage 变化继续拒绝。
同步移除 BC-RESP-003 的刷新例外，并以公开处理、持久化和续接验证不会完成成功或产生错误可继续状态。

此文本尚未生效。决定后需做必要的最小实现／测试／契约修订，保留 RED/GREEN 和最终源码绑定；
不由本草稿推定用户选择了此项。

## 本次交付边界

E-05 最终产品裁定和所选规则的本批验收均 **NOT_RUN / 等待用户裁定**。
本批可独立交付的设备恢复、API 接入和桌面验证见
[本地报告](cpar-continuity-operations-local-20261009.md)；它们不关闭此语义冲突。
