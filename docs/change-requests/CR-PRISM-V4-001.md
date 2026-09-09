# CR-PRISM-V4-001 · 管理目录、配置枚举及计费处理状态

日期：2026-09-09。状态：范围已由用户批准，后端契约与实现待 M2/M3 定稿。
本轮 Codex 统一负责两侧；本文件不是接口已交付的声明。

依据：[正式执行计划](../handoffs/prism-v4-execution-plan.md)。

## BE-FE-01：有效模型投影

在同源管理鉴权下提供 serving snapshot 的只读授权模型投影。上下文仅接受既有
Access Group ID 或 Client Key ID；不接收 Client Key secret。复用数据面授权及模型
可见性代码，返回 exact ID、安全来源标识、serving 配置/目录快照及观测证据。
不同授权上下文的结果分别与数据面验证；草稿不得冒充 serving。

## BE-FE-02：完整配置图

现有 Route GET/PATCH/DELETE 继续使用。新增 Route/Candidate/Alias 的完整枚举，包含
孤立草稿；分页绑定配置 ID 和 revision，上限、游标冲突遵循既有管理投影模式。
新增 Candidate PATCH/DELETE，保留 If-Match、配置可编辑性、引用约束、审计和持久化。
不能以运营 inventory 或前端已创建对象缓存替代完整枚举。

## B1：最小处理状态

为持久事件到计费账本的增量 worker 提供安全状态：消费 checkpoint、源水位、观察时间、
待处理程度和封闭失败分类。区分未启用、处理落后、失败及已追平的空账本。
不返回事件正文、秘密或内部异常消息；请求趋势、延迟分布不在本 CR 中。

## 定稿与验证

实现时先核对现有 facade、snapshot、查询/错误模型并更新本 CR 的最终路径与 operationId；
提交权威 OpenAPI 和后端测试后运行 sync-contract，再接前端。验证授权一致性、snapshot
跨页冲突、旧 revision 拒绝、孤立候选持久化、物化重启幂等和状态安全投影。
这些是本轮内部待实施项，不等待另一位实现者。
