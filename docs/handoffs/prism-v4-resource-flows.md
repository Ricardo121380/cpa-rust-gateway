# V4 资源管理补充设计

日期：2026-09-10。这是 M2 的交互设计补充，不是后端实现或真实网关验收。

- [补充原型](../design/prism-v4-resource-flows.html)；完整 14 栏目仍以 [V4 基准](prism-opendesign-v4.md) 为准。
- OpenDesign MCP 项目 `prism-gateway-console-redesign-a735`，文件 `prism-v4-resource-flows.html`。
- 本会话模型编写；MCP `create_artifact`、`write_file`、`get_artifact` 保存、迭代与读回。
  第 3 版 source 为 `manual`，没有调用内置生成器、外部模型或模型 CLI。
- 读回与仓库一致：76,737 字节，SHA-256
  `1ef75caad201476db1e3912fc51c4c4932e3bfd92321de2eaab245151757b425`。

## 实施约定

1. 有效模型先选既有访问组或 Key ID，服务端用同一 serving snapshot 授权逻辑投影 exact ID。
   UI 不接受 Client Key secret，不根据套餐合成模型，不把当前草稿冒充 serving。
2. 来源详情沿用 V4 右侧实底检查面板，展示安全的 Provider/Channel/Endpoint/Credential、
   serving 配置和目录证据。不同上下文有各自集合；不泄漏未授权模型。
3. “用于草稿候选”显式携带选定模型；普通编辑必须预填原候选，不因更换授权上下文而悄悄改值。
4. Route/Candidate/Alias 从完整配置枚举获取，包含尚未绑定授权、无候选的草稿资源。
   分页必须固定配置 ID 和 revision；发生冲突时丢弃旧分页，重新读取，不自动重放写入。
5. 候选新增、编辑居中；保留 stable ID、父 Route、exact model、Endpoint、Credential scope、
   transform、启用、优先级/权重。删除须明确目标及对路由校验的影响。既有能力覆写字段沿用
   当前正式表单；补充原型不限制后端已支持字段。
6. 原型只有合成数据和内存状态，按钮不发送请求、不生成真实审计、不触发发布。
   原型的 CSS 复用 V4；正式应用继续使用 React 组件、PrismLens 与四文件构建。

## 已做检查

`node --check` 通过；1440×900 查看补充布局与模型上下文切换，390×844 查看来源详情。
访问组样本与 Key ID 样本返回不同的 exact model；Key ID 样本来源明确显示匹配的
provider-b/channel-b/endpoint-b/credential-b。MCP 读回哈希一致。
从 Key ID 样本进入“用于草稿候选”时正确预填模型与 Endpoint；新增 candidate-b、权重 3
后候选数变为 2，原 candidate-a 保留，设计草稿 revision 从 7 前进至 8。
这些检查不替代正式 API 的授权一致性、CRUD、冲突和持久化测试。

## 已核对的后端复用点

- `crates/gateway-http-actix/src/lib.rs::models` 用已认证 snapshot client 的 `exact_upstream_models()`。
- `crates/gateway-router/src/route_snapshot.rs` 已有 `public_models_for_access_group`、
  `exact_upstream_models_for_access_group` 及精确模型消歧，不新建第二套授权策略。
- `SqliteControlPlaneRepository::load_configuration` 在 SQLite transaction 内读取，已有一致性基础。
  新枚举仍须有界读取，不读取或返回凭据密文/Key digest 来拼前端列表。
- `ManagementMutationService` 已有 Route GET/create/update/delete；Candidate 仅有 create。
  后续补候选 update/delete 与三类分页读取，复用现有草稿 revision、引用校验和审计事务。

最终路径、operationId、schema 和实现证据继续更新 [CR](../change-requests/CR-PRISM-V4-001.md)。
