# 渠道接入缺口修复与验收

2026-09-27。基线 b6876e5。结论：本轮发现的公共接入缺口已修复并完成本地验证；未部署，不宣称真实渠道推理验收完成。

## 修复内容

- API 四渠道可直接创建服务与接口，或为已选服务增加接口；新建时只提供当前渠道的协议预设，不借用 Codex/Krill。无服务时提示创建并禁止空目标导入。
- 复用 ProviderDialog 和真实 CRUD；接入向导中的连接表单不重复收取凭据。活动配置遵守校验/发布流程，草稿保留草稿，不自动开放模型。
- 新连接返回时重读服务/接口，并带回用户刚创建的目标。实际浏览器发现配置切换会重建 Outlet；通过一次性路由状态接续渠道和连接引用，立即消费，不携带凭据。
- 原生 Grok 导入结果与 Build 授权结果提供配置接口/查看已有连接入口，明确授权保存、运行应用、模型开放是不同状态。
- Kimi Coding 多服务时显式选择，后端拒绝未知或非 Kimi 目标，保留出口和端点校验。
- 导入完成先选择结果配置，再触发父页面刷新。冲突/部分失败不自动重放。
- 更新旧 managed-accounts E2E 的渠道卡片入口，并补四 API 创建后继续导入的回归规格。

## 逐渠道浏览器记录

EgoLite 空间 22，本地 Vite `127.0.0.1:55717`，开发 fixture；全部使用合成凭据。共 11 个入口分别操作，列表最终读到 11 份授权。

| 渠道 | 本轮实际动作 | 结果 |
|---|---|---|
| Kimi API | 无服务→创建→返回正确 Chat Completions/api.moonshot.cn→导入→重读连接 | PASS（模拟后端） |
| OpenAI 兼容 | 创建→返回 Responses/api.openai.com→导入→重读 | PASS（模拟后端） |
| Anthropic 兼容 | 创建→返回 Messages/api.anthropic.com→导入→重读 | PASS（模拟后端） |
| Grok Official | 创建→返回 Responses/api.x.ai→导入→重读 | PASS（模拟后端） |
| Grok Console | 导入→明确下一步→创建渠道接口→进入提供商页 | PASS（模拟后端） |
| Grok Web | 导入→明确下一步→创建渠道接口→进入提供商页 | PASS（模拟后端） |
| Grok Build | 导入→明确下一步→创建渠道接口→进入提供商页 | PASS（模拟后端） |
| Codex / ChatGPT | 合成授权文件导入→自动准备目标→完成 | PASS（模拟后端） |
| Claude | 合成授权文件导入→自动准备目标→完成 | PASS（模拟后端） |
| Kimi Coding | 切换导入→准备目标→导入→完成 | PASS（模拟后端） |
| Kiro | 切换导入→按区域准备目标→导入→完成 | PASS（模拟后端） |

## 自动检查

- Vitest：账号接入、Kimi 设备流程、提供商相关 7 文件 61 项通过。
- Rust：`cargo test -p gateway-http-actix --test managed_resource_inventory`，41 项通过；包含实际 Actix 管理 HTTP、存储、鉴权和修订校验，Provider 外部行为由测试控制。
- 新增 Kimi 多服务/未知目标拒绝及显式目标成功用例通过。
- `npm --prefix web/prism run type-check` 通过。
- `npm --prefix web/prism run check:full` 通过。
- `node scripts/check-management-spa.mjs` 通过：153 个生成操作，双构建检查。
- `cargo fmt --all`、`git diff --check` 通过。
- Playwright runner 未运行：用户要求浏览器测试使用 EgoLite；对应路径已通过 EgoLite 实操，E2E 文件保留供仓库流水线执行。

## 边界与后续

本轮没有真实推理请求、生产凭据读写、账号删除或生产发布。官方登录/授权交换、真实目录/额度、模型开放后客户端请求不因这份模拟接入验收自动成为 PASS。生产发布仍需签名产物、隔离副本演练及发布后登录复查。

Kimi 多服务选择仍要求所选服务具备符合渠道约束的出口与唯一标准端点；不将不兼容的自定义服务偷偷改造成官方服务。API 显式选择“稍后连接”继续保留仅保存凭据的能力，此状态不代表可推理。

补充交互检查：Kimi API 弹窗在 1440、1280、390 宽度均无页面横向溢出；390 下弹窗 x=12、width=366。编辑连接名称后取消触发未保存确认，选择放弃后返回 Kimi 导入表单。EgoLite 空间 22 已按流程结束。
