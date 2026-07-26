# CR-FE-001 · 管理前端 Prism 立项与契约缺口包

| 项目 | 值 |
|---|---|
| 状态 | **Approved(用户于 2026-07-26 批准)** |
| 范围 | 采纳 H18(新 Web 管理面板)/H19(凭据与 403 可视化)/H21(管理审计 UI)为 New;H20(实时推送)维持 Later;新增 G9(运营者成本估算)为 New |
| 设计基线 | [docs/07 · 设计文档 v0.2](../07-management-frontend-design.md)、[docs/08 · 开发文档](../08-management-frontend-development-plan.md) |
| 备注 | 本文件为 CR 记录;`docs/01-feature-selection-matrix.md` 与 `docs/06-development-plan.md` 的正式条目合并由项目维护流程执行,本 CR 不直接改写锁定文档 |

## 1. 用户批准记录(2026-07-26)

1. **成本分析(G9)批准**:引入模型价格簿(LiteLLM/OpenRouter 手动同步 + 本地覆盖 + cache read/write/creation、service tier、长上下文语义)与运营者成本估算。边界:BL-22 不变 —— 仅运营者侧估算展示,不构成 Client Key 计费;
2. **观测平面 = 产品一半**:G2(事件管道接线)、G3(组合分析端点)升为 P0;
3. **巡检自动化 v1 只读**:凭据健康仅做只读投影;自动禁用/恢复(含所有权规则)为后续独立 CR。

## 2. 契约缺口包(按 BL-19「API 先于 UI」,实现前逐项补入 OpenAPI 契约)

| # | 契约变更 | 形状要点 | 前置于 |
|---|---|---|---|
| G1 | `GET /admin/config-versions/{id}/graph` | redacted 全图投影(upstreams+endpoints+credentials(元数据)+bindings+egress+models+aliases+routes+candidates+access_groups+grants+client_keys(redacted));复用 `load_configuration` | FE-0 任务 0.7 |
| G7 | `GET /admin/capabilities` | `{features: {name: {available: bool, reason?: enum}}}`;管理鉴权后访问 | FE-0 任务 0.8 |
| G2 | serve 进程装配事件管道 | BoundedEventQueue + AsyncSqliteEventWriter;AttemptEvent 时间戳入库;无新 HTTP 面 | FE-2 |
| G3 | `POST /admin/analytics` + `GET /admin/dashboard/summary`(+ JSONL 导出/分块导入) | 形状草案见 docs/08 §4.4;rollup 纪律(checkpoint 增量、format-version 重建、严格无过滤才走 rollup) | FE-2/FE-4 |
| G9 | `GET/PUT /admin/model-prices`、`POST /admin/model-prices/sync`、成本字段并入 analytics 响应 | 同步仅手动触发;逐字段 configured 标志;来源与原始记录留痕 | FE-5 |

P1 项(G4 子资源 update/delete、G5 ClientKey 元数据、G6 家族运行时投影)随各自实现阶段单独提契约 diff。

## 3. 工程决策记录(FE-0,2026-07-26 修订)

- **新前端为独立仓库 `../prism/`**(用户 2026-07-26 指定,替代此前的 `web/prism/` 内嵌方案):本仓库(后端会话)与 prism 仓库(前端会话)零工作树交集;现有 `web/admin-ui`(P10 操作台)原样保留;
- **唯一耦合点**:`docs/openapi/management-v1.json` 是契约唯一真源(本仓库维护)→ prism 侧 `npm run sync-contract` 复制快照并重新生成 API 客户端,其 CI 在客户端与契约失步时机械失败;后端契约变更(G1/G7 等)落地后无需通知细节,前端同步一条命令完成;
- **产物回流**:FE-1 出口做嵌入切换时,prism 的 `dist/`(固定文件名清单:`index.html`, `assets/{main.js, vendor.js, index.css}`)交付给 `gateway-http-actix` 的 `include_bytes!` 清单与 build.rs;在此之前 Rust 构建与现有二进制完全不受影响;
- H20 未批准 → 观测页一律轮询(TanStack Query refetchInterval),SSE 留待后续 CR。
