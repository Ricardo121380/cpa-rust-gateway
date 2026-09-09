# Prism V4 完成条件核对（进行中）

本文件以正式执行计划为准。已有通过证据不代表整个 Goal 完成；交付报告尚未生成。

| 范围 | 当前证据 | 结论 / 剩余动作 |
|---|---|---|
| M0 契约、会话与乱序 | authority/generated 门禁；客户端所有权/版本回归；真实 gateway access_denied 锁定、清空秘密和停止读取 | 已有证据；最终报告需逐项引用测试名，不能用总测试数代替 |
| 14 栏目 + 解锁页 | 3 尺寸 × 浅深色 84 页面视图、6 解锁页；真实样式计算；251 单测、123 E2E 基线及后续定向回归 | 页面入口与基础布局通过；下列差异/确认缺口仍未完成 |
| 配置差异 | 16类记录SQL比较、独立只读并发受限API、V4基线选择/分页/409重读；真实桌面/手机操作与字段无溢出 | 已实现并验收，见progress与真实browser-flow证据 |
| 发布 / 回滚确认 | 共享V4确认页；实际目标/revision；活动身份与生命周期事件前置条件；真实取消/发布/回滚/ABA拒绝证据 | 已补齐，见progress最新记录 |
| BE-FE-01 | 同一 serving snapshot 授权投影；组/Key ID 上下文；目录证据；真实浏览器 model→draft handoff | 已实现并有真实链路证据 |
| BE-FE-02 | 完整 route/candidate/alias 有界枚举；真实候选增改删/重读/审计；旧策略详情 | 已有实现与针对性证据；最终报告需列清楚旧策略写入契约边界 |
| B1 计费运行接线 | 真实请求→持久事件→物化→有价/无价账本；重启 checkpoint 幂等；坏记录修复存储/服务回归 | 已有证据；不把 partial 金额标 exact |
| B2 有界读取 | 99999/100000/100001/100005 历史真实 HTTP 窄窗账本/用量/失败和完整摘要；存储 snapshot/cursor 回归 | 已有证据 |
| B3 硬过期 | 真实进程合法历史目录+刷新失败证据，过期新选择/租约被拒，旧租约请求完成 | 已有证据；自动 discovery 渠道未扩展 |
| B4 既有 TTL | 独立连接/有界双表删除与有效内容存储回归；真实 serve 到期清除、未到期保留 | 已有证据；未扩展账本/事件删除 |
| 真实浏览器写操作 | model handoff、candidate CRUD、route grant、validate/publish、audit；移动辅助偏好、拒绝后锁定 | 已有证据；新增确认/差异后需更新相关验收 |
| 运行装配兼容性 | 新 Endpoint 能力表、模型能力、路由参数、已存目录接线修复 | 仍核对已有启停操作与 runtime shape 约束；不能把管理写成功等同重启可用 |
| 最终仓库门禁 | check.sh fast 已通过前置脚本/SPA/格式，在 all-targets Clippy 首次失败；修复后完整 Clippy 通过；全 Rust 回归已通过（1183 passed，9 ignored） | 续跑 serve envelope、源码/边界/文档/secret/whitespace 均通过；新增功能仍需重新验证受影响范围 |
| 可审查交付 | 功能批次提交、cross-boundary-log、progress 与真实临时 evidence | **缺失最终 prism-v4-delivery.md 与可持续查看方式**；验收脚本默认会停止服务，不能把已停止 URL 当可查看实例 |

## 本轮最终门禁发现与修正

- all-targets Clippy 检查到新增测试的数字分隔、unit pattern、长测试和冗余闭包；修正格式并提取语义明确的测试 helper，没有新增 Clippy 豁免。
- 完整 Rust 首次运行发现旧测试仍要求 6 次尝试失败；契约现允许 16 次，已更新为验证 6 次可装配。已有新边界回归仍拒绝 17 次和超时越界。
- 原始门禁日志 `/tmp/prism-v4-final-gates.log` 与 `output/prism-v4-final-gates.md` 保留首次失败事实；续跑日志 `/tmp/prism-v4-final-clippy.log`、`/tmp/prism-v4-final-rust-tests.log` 分别记录实际结果。

- serve envelope 的旧夹具缺少当前必需的 grok-build-cache-key；补齐临时合成32字节文件后，真实封装就绪/listener隔离/管理保护通过。
- 续跑源码策略（249 Rust文件）、crate边界（21包）、610文档链接、107契约测试引用、secret scanner自检及全量tracked scan均通过。随后格式、全目标Clippy、全HEAD whitespace复核通过。
