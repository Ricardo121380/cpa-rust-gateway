# A-01 修复范围与固定点

日期：2026-10-07，Asia/Shanghai。

- 发现编号：2026-10-05 发布复核 A-01/P2。
- 任务身份：CPAR-05 / GitHub #14；父规格 #9；ADR-0099。不是 CPAR-01/#10，也不是 AC-01。
- 固定起点：`602874b9ed9943c146c8d12f77ab1cc1adbfa167`；历史候选源码：`80f5838902eb56be6dccc3ce047eb227d6c18e6c`。
- 起点工作区只有用户的 AGENTS.md 修改和无关未跟踪内容；无后续 A-01 修复源码或报告。原始输入状态与 hash 留在本地 output/cpar-a01-20261007/initial-baseline.json。
- 相关规格在 [#14 快照](issue-14.md)、[父规格快照](issue-9.md)、[#57 快照](issue-57.md)；均按项目 issue-tracker 的 gh 只读流程读取，OPEN、无评论。本轮无 GitHub 写入。

## 必须保持的行为

1. 实际注册的 POST /admin/credentials/{id}/oauth/callback，只能替换同渠道且可靠确认为同一账号的原授权。
2. 不同或无法比较的身份明确返回既有 409 冲突，不改变原材料、revision、停用意图、绑定和权限。
3. 同身份授权正常保存；draft 更新 credential/config revision，active 只旋转 credential revision。原连接、权限及账号关联保持。
4. 旧记录的 version/id/revision 与最终 CAS 的保护对象一致；晚到回调不得覆盖合法并发获胜状态。
5. 首次授权、新账号接入、正常后台续期各自保持原契约。不得把交互式缺失身份规则用于正常续期。
6. 受控人工材料，真实 Actix 资源表与管理鉴权，磁盘 SQLite 与真实 AEAD/CAS；不以新 helper 的自测代替 callback 行为。
7. 秘密一致性仅在进程内以 boolean 断言检查；输出不包含 token/code/key/解密材料。审计不在“所有字段都必须不变”的断言里。
8. 先 RED，再最小修复，GREEN；定向回归/静态检查、本地 Full、独立 Standards/Spec、精确提交与本地报告交付。

## 限定修改与评审

- gateway-control 的交互式 OAuth 持久化入口；gateway-store 的原 identity guard 迁移复用。
- gateway-http-actix 的 legacy callback 单个调用点、原 enrollment guard 调用、管理 HTTP 回归。
- 必要相邻路径：enrollment、正常 renewal、draft/active Credential CAS。只读取必要路径。
- 其他 A/B/C/D 发现、web/prism、Responses same-grant/revision 合同与 E-05 均不修改。
- 本轮是本地修复检查点；不推送、发布、关单、签名、构建发布候选、部署或调用真实 Provider。

## 测试矩阵

| 行为 | 参数组合 | 数据边界 |
| --- | ---: | --- |
| foreign stable ID，即使邮箱相同 | 4 | draft/active × active/disabled，原 ciphertext/revisions/关系保持 |
| 无法比较/缺少旧观察/身份冲突 | 24 | 6 组 old/new × 上述 4，未知不能匹配 |
| 同身份新授权 | 4 | 保存 exact exchange 结果，停用、连接、权限、其他账号保持 |
| exchange 期间凭据与合法连接更新获胜 | 4 | winner 为基线，晚到 callback 拒绝 |
| exchange 期间仅 draft 连接变化获胜 | 2 | 最终配置 CAS 失败，原凭据和 winner 关系保持 |

总计 5 个 HTTP 测试、38 个受控组合。测试通过实际 configure_management_resources 注册及 Actix service（不绑定监听端口），外部 token exchange 为已有注入接口的受控替身；SQLite/AEAD/事务未 mock。UI 未改变，本切片无需新增浏览器验收。
