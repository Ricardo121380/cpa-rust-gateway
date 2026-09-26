# M4 请求查询与容量观测

状态：实施中；基线 fa5f280。无新增业务权限或历史裁剪。

请求历史保持双水位、literal 筛选、精确 P50/P95、历史未知和缺价含义。
页查询批量关联账本；完整聚合只计算一次匹配集合，不近似、不截断。
宽页按 Request ordinal 索引行走，窄窗按终态时间索引选取，均在存储完成筛选。
schema31 增加按 request_id/event_ordinal 排序的 attempt 部分索引，以及终态时间与
结果/时延字段的覆盖索引，避免聚合时读取完整 payload；不修改原事件。31→30
只删除这两个索引，账号和历史不受影响。
此项不解决 schema28 对 M1 新事件的兼容问题；最终回退需验证理解新事件的 schema30 候选。

现有受保护 Prometheus 接口新增无标签容量 gauges：required queue capacity/used/remaining、
storage observed/observed_at_ms/collection_failed/database_bytes/wal_bytes/available_bytes/total_bytes、
disk_low/wal_high。观测不存在时省略对应 gauges，不能伪造零空间或健康。
后台每分钟采样一次；磁盘低于1GiB或5%，WAL超过256MiB时提示。
这些是告警阈值，不改变准入、也不自动清理；准入继续依赖真实必需记录确认。
采样失败保留上次观测并明确失败/时间；HTTP只读取内存，不执行磁盘、子进程或SQLite。
现有物化水位可计算积压，展示为未处理序号跨度（不是请求数或费用）。

前端沿用V2现有状态面板；权威OpenAPI说明先变更再sync-contract。无新依赖、外部遥测或秘密输出。
本地验证覆盖精确聚合/双快照、多账本行、队列压力、容量正常/临界/失败、旧响应未知与时效。
