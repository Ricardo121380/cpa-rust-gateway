# M3 安全尝试诊断投影

状态：本轮后端定稿并实施。范围：既有 `GET /admin/requests/{request_id}/attempts` 的增量字段，不新增写接口或存储迁移。

- `Attempt.observation` 可选：仅持久 Attempt 具备证据时提供，兼容仅有进程内终态的嵌入实现。
- 对象包含 attempt_number、upstream_id、started_at_ms、ended_at_ms、duration_ms、error_code、error_scope、retry_decision。错误及决策均使用现有闭集；成功时错误两字段为 null。耗时仅由该次持久起止时间计算，非法时间不构造观测。
- 不含 URL、headers、正文、模型私有内容、HTTP 原文或秘密。stage 仍只在同一单次 attempt 有证据时提供，不能把最后阶段赋给全部重试。
- `succeeded` 是该次驱动结果，不能代替外部请求终态或流完整性。无 observation 显示未观测，不补零；外部请求统计沿用 RequestFinished，费用沿用账本置信度。
- 管理 UI 展示时间、分类与下一步建议，并链接当前账号、提供商、目录和绑定诊断；历史绑定可能不在当前配置，界面如实说明。
- 权威 OpenAPI 先变更，运行 sync-contract；Rust 投影/契约、前端恢复/分类及真实本地 gateway 验证，生产及真实渠道留在 M4。
