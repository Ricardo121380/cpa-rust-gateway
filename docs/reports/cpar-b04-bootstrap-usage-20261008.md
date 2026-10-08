# Release-readiness B-04 / P2：启动失败的已校验 Usage

当前状态：`IN_PROGRESS`。本报告骨架随源码提交，用于固定独立评审范围；定向验证已通过，
最终源码的独立 Standards / Spec、干净隔离 checkout Full 和本地证据收口尚待完成。
未完成这些步骤前，不将 B-04 记为 `RESOLVED_LOCAL`。

固定 base：`08812db2373583b8a725b80071efe75b69f9d7ae`；分支
`codex/prism-v4-delivery`。本任务仅对应 release-readiness 的 B-04，不是 CPAR-04，
也不关闭整个 #32、Batch B、#57 或发布。

## 已确认的修复范围

Chat 和 Responses 在一次真实启动 body read 中解码了合法开始与 Usage，再遇到损坏帧，
source 的 `begin` 返回失败，流尚未交给 HTTP 观察器。以前这份已校验用量随临时 decoder 丢失。
修复通过每请求 driver 的有界槽保留已校验 Response ID 与合并 Usage，确认准确的失败 Attempt
后，在已有 Required 事件端口确认它的 Usage，再按原策略决定是否允许备用切换。

实际合法 Chat 重试又暴露了原 Request 级事件身份只能存一条用量的限制。因此，新的明确 lineage
使用 Attempt 级幂等身份；旧身份保留原样重放，未知或冲突 lineage 不猜归属。Store/control/账本
读模型一致地保留各 Attempt 的事实。同组两个 Attempt 仍是一个 Request、两个用量观察。
持久化兼容与回滚边界见 [ADR-0103](../adr/ADR-0103-failed-bootstrap-usage-attribution.md)。

Messages 的合法 `message_start` 会先使 `begin` 交出流，再解析后面的损坏帧；它不属于相同启动
丢失窗口。公开路径测试保留输入 11、cache read 5、cache creation 2，初始输出占位零仍是未知。
这是已有保护的核验，没有为凑三协议构造非法 Usage 事件。

## 证据与剩余门禁

修复前 Responses/Chat 的真实失败输出、退出码、执行 HEAD、工作区和文件哈希已经保存。
原始 harness 编译及模型路由失败单独记录，不算行为 RED。公开路径 GREEN 使用实际 loopback，
断言 decoder 收到的首个 read 与完整 fixture 字节一致；不把一次 TCP write 当作一次 read。

持久化链路是公开 HTTP → driver/router → P12 fanout → BoundedEventQueue →
AsyncSqliteEventWriter 确认提交 → production BillingWorker → Store/账本及 protected HTTP 读回。
管理 HTTP 使用测试 facade 调用生产只读 Store/control 函数；未冒称全 deployment composition、
进程重启、远端 CI、新制品、真实 Provider 或生产验收。

当前公开协议及相邻回归通过。完成报告将记录最终源码提交、必要定向/Full 数量与退出码、
独立双轴覆盖和哈希绑定，保留失败记录，并补齐结构化回执及本地 #57 草稿。

无真实 Provider 请求、生产凭据读取、生产修改、部署、发布、push 或 GitHub 写入。
发布继续 `BLOCKED`。当前原始 P2 尚余九项；只有本切片完整验收后才能改为八项：A-02～A-08、B-05。

工程过程与偏差记录见 [Task card](assets/cpar-b04-20261008/task-card.md)。
