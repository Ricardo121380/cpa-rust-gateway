# Prism V4 实施进度

启动：2026-09-09；分支 `codex/prism-v4-delivery`；原始 HEAD
`1ba4340b56e9f4214c98912f78dc477fdc817ea2`。总 Goal 正在执行，尚未完成。

| 里程碑 | 状态 | 剩余 |
|---|---|---|
| M0 | 已完成 | 无；提交见 git 历史 |
| M1 | 待实施 | V4 正式 React 视觉、14 栏目与解锁页 |
| M2 | 待实施 | BE-FE-01/02、B3、资源管理接线 |
| M3 | 待实施 | B1/B2/B4、真实账本与运营状态 |
| M4 | 待实施 | 全栏目视觉/E2E、相关后端门禁、本地真实 gateway、最终报告 |

## M0 已完成

- 权威契约已同步，三个 schema 相关 DTO、账号权益及目录状态消费更新。
- 请求绑定会话 generation 与版本选择 generation，拒绝晚到成功/错误/正文；同版本 revision
  单调前进，支持超出 JS Number 精度的序号；非版本响应不污染所选草稿。
- 会话失效集中锁定、移除秘密、取消请求、清空查询/ mutation 缓存、卸载受保护页面；
  换版本关闭旧表单；配置冲突与运行时冲突保持分离，不重放写入。
- 日常 check 现在直接核对权威 OpenAPI；[新增能力 CR](../change-requests/CR-PRISM-V4-001.md) 已记录。
- 当前类型检查及 245 项单测通过，包含 13 项新增 API 所有权回归；这不替代 M4 真实验收。
- Chromium 定向 E2E：session ownership + provider pools 7/7，最终 session ownership + smoke 8/8。
  权威 SPA 门禁通过（99 operations、CSP、四文件、双构建一致）。

## 受保护的旧功能基线

既有 12 页和主路由以 [旧计划 §3.0](../08-management-frontend-development-plan.md) 及
[正式计划的栏目矩阵](../handoffs/prism-v4-execution-plan.md) 为基线。保留所有现有
`web/prism/e2e/` 用例及操作：版本创建/校验/发布/回滚、上游/端点/凭据/绑定/OAuth、
公开模型/Route/候选创建、访问组授权/Key 签发/启停、价格导入/回退/策略、用量六类 token、
失败/账本分页/请求 attempts、账号恢复/冷却、Explain/Channel Pin、三类出口、审计/备份预检、
外观/语言/锁定。runtime 的 query 深链继续兼容；不能因为新增 accounts/catalog 删除已有子视图。

历史 12 个明确未接操作继续保留原边界：9 个无须重复预读的单资源 GET，加秘密导出、
恢复预览与在线恢复。本轮不以调用率取代功能验收。

## 环境边界

仅本地代码与合成数据。未访问 SSH/生产，未执行真实 Provider 调用。已有未跟踪设计、
计划、报告和辅助脚本均保留，未清理或并入代码批次。
