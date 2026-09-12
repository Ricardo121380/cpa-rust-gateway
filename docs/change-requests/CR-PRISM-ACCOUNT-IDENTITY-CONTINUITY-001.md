# CR: 账号身份在目录、运行状态及 SSO 导入中的一致性

用户人工验收发现：旧 SSO 未读取会话身份，Codex 同一授权重复显示，运行状态仍用机器名称。
本轮 Codex 统一实施前后端；不删除或停用生产凭据，不更改已有路由绑定。

- 运行账号快照增加 presentation：真实身份、渠道分类/名称及连接协议/主机，来自当前 serving
  配置及对应凭据，不用浏览器当前草稿拼接运行对象。运行分页保留同一观察快照；原生身份
  元数据可较新，不据此改变运行状态。
- 新增管理鉴权/CSRF 保护的 `POST /admin/native-accounts/{account_id}/identity`，以 revision
  拒绝过期请求。Web/Console 向固定 `https://grok.com/api/auth/session` 读取身份，使用
  配置的 Web 出口、Chrome 传输及凭据中的 Cookie；单次有界请求，无重定向/自动重试。
- 导入 SSO 时自动尝试读取；失败保留成功导入的凭据，界面可显式重试已有账号。
  GET 列表不访问 Provider。未经认证/blocked 的 session 不接受残留邮箱。
- 新增加密身份观察表，绑定原生账号当前凭据密文的指纹；CAS 持久化与列表 generation
  保持一致；额外比较先前观察指纹，拒绝同毫秒并发晚响应覆盖。凭据变化后不沿用过期观察，
  不改变原 SSO 凭据格式或授权状态。
- 普通目录按已观测身份分组；同邮箱等身份下明确列出多份授权，操作仍指向具体凭据。
  不能按邮箱自动删除或合并底层账号；缺失身份的记录不互相归并。

参考：grok2api 的 [Session 身份实现](https://github.com/chenyme/grok2api/blob/main/backend/internal/infra/provider/sessionidentity/session.go)。
服务端 API 与权威 OpenAPI 定稿后运行 sync-contract。适用验证包含身份解析、加密/CAS/
失效、运行快照及分页、重复身份操作目标、移动端、辅助偏好和真实本地 gateway。

状态：后端与前端已实现、契约已同步（123 操作），本地验收通过。线上两条 Console 资料
GET 返回 403，本批未取回邮箱、未部署候选；证据与限制见
[交付记录](../reports/prism-account-identity-continuity-20260912.md)。
