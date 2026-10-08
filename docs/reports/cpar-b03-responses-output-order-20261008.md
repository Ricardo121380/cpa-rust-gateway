# CPAR B-03：Responses 原生 SSE 整体输出顺序

本地执行中。任务为 release-readiness B-03/P2，固定起点
`9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2`。不是 CPAR-03 或整个 Batch B。

任务范围、依赖、测试边界与授权见 [Task card](assets/cpar-b03-20261008/task-card.md)。
当前公开 RED 已保留：[退出 101 和原始输出](assets/cpar-b03-20261008/red.json)、
[基线源码绑定](assets/cpar-b03-20261008/red-binding.json)。
初次实现源码 `4dd28268e277fa5b223c090ebca73a4707b32505` 已完成一次干净 Full，
但复核发现 bootstrap 分类可能允许重放，不能据此关闭 B-03。
补充的公开双候选 RED 显示，顺序冲突被备用端点的合法完成挽救：HTTP 200，
实际上游接收 1+1 次、Attempt 2 次，真实测试退出 101；见
[补充 RED](assets/cpar-b03-20261008/bootstrap-fallback-red.json) 和
[源码与 harness 绑定](assets/cpar-b03-20261008/bootstrap-fallback-red-binding.json)。
Responses bootstrap 修复仅保留明确协议错误为不可重试失败，普通传输 EOF
保持原有重试分类；不涉及 B-04 的 Usage ownership。
最终验证和两轴评审尚待收口，B-03 目前不标 RESOLVED_LOCAL。发布仍 BLOCKED。
