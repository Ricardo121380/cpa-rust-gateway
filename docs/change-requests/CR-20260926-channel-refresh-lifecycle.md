# M2 渠道刷新生命周期

状态：实现 `1a2fdb8` 与完整本地 fast 门禁通过；见 [M2 报告](../reports/cpar-reliability-m2-20260926.md)。对应可靠性计划 M2-01 至 M2-05，基线 `1c94c5b`。

普通 OAuth 在启动及周期 worker 中使用同一持久 claim、凭据 revision CAS 和有界退避。
schema30 新增每配置/凭据一行的刷新状态：记录 claim 截止、失败类别、次数和下次时间；
只保存安全分类，不保存响应正文、token 或邮箱。Claim 不跨凭据/配置复用；过期后可恢复，
成功 token 轮转、状态和审计同事务提交。永久授权失效标记 Unauthorized，须更新或重新授权。
网络/解析错误仅退避；旧 claim/CAS 冲突不能覆盖较新凭据，不能把普通 403 当作已证明撤销。

新增 Claude/Kiro worker 装配，API Key/SSO 不制造 refresh grant。保留请求路径拒绝过期材料。
目录仍按精确账号租约、来源时间及 freshness 保存；发现不创建公开模型或权限。
续接仅在证明原授权连续的条件下放行轮转，未知/撤销/重新授权仍拒绝。

无新增管理写端点；管理投影若改变，先同步权威 OpenAPI。本阶段不部署、不做真实推理。
schema30 回退需先停止新 worker；新状态表可丢弃但不得回退已轮转 token，生产最终回滚演练属 M4。

## 实现边界

- Codex/Kimi Coding 复用既有 token 协议；Claude/Kiro 使用各自真实刷新请求。Kiro OIDC 使用
  camelCase 参数，Social 使用独立固定地址。刷新保留原凭据 kind，不改账号绑定或模型权限。
- 普通刷新每轮最多8次交换，30秒后不再启动下一次；单次 HTTP 总超时15秒，不跟随重定向。
  每分钟调度，启动使用同一持久退避。停止后仅完成已发出的交换及 CAS；generation 切换互斥。
- 明确的 invalid/revoked grant 才转 Unauthorized；网络、HTML拒绝、429/5xx与坏响应只持久退避。
  成功响应还须通过渠道解析。Claude 明确不同 account UUID 被拒，原身份元数据保留。
- 普通池发布 OAuth 到期时间，启动刷新失败不会令过期凭据参与新租赁。损坏的 AEAD 密文按条
  退避；结构已经损坏到无法加载配置的存储仍失败关闭，不掩盖无效配置。
- Grok Build 仅对同 subject/client/完整 scope 集的 refresh 保存连续 revision 区间。
  重新授权、人工替换、撤销、未知主体使旧续接失效；其它渠道仍要求精确 revision。
- Kiro IDE OAuth 目录通过固定区域 ListAvailableModels 分页获取，最多100页、每页1MiB、
  最多10,000个 exact ID；重复 cursor、部分失败不发布成功。租约、来源、时效及权限仍沿用
  通用持久目录。CLI/API Key 元数据接口尚无统一可核验契约，显式 Unsupported；额度未知不补零。
- 未扩展 Kiro 身份字段：OIDC token 响应不保证邮箱/主体；现有显式“替换授权”可以更新未知
  身份凭据，但不继承历史续接证明。授权成功不等于已取得人类身份或已验证相同主体。

## 来源与回退

Kiro OIDC 请求采用 [AWS CreateToken](https://docs.aws.amazon.com/singlesignon/latest/OIDCAPIReference/API_CreateToken.html)
字段；IDE 元数据来源核对 [kiro.rs 固定实现](https://github.com/ZyphrZero/kiro.rs/blob/be0c04219d9d1b93b7fe5c3d7b9e7c9cf0d05863/src/kiro/token_manager.rs)
和 [分页实现](https://github.com/chaogei/Kiro-account-manager/blob/447adcdb468157312621b1f09448278bd9bca748/Kiro-account-manager/src/main/proxy/kiroApi.ts)。
这些是请求结构依据，不代表本轮真实 Provider 验收；不复制其跨区域回退或猜测 ARN。

schema30 为新增刷新状态表和 Grok 两个证明列。回退须停 worker，先删除 current 列再删 floor 列，
不回退已经轮转的密文、账号 revision、历史或审计。失去证明后旧历史续接保守拒绝，退避状态需
重新建立。29→30→29→30 本地合成回归随存储测试执行；schema28 的 M1 事件兼容限制仍有效，
未来生产回滚必须使用 M4 验证过的候选与快照，不能直接换旧二进制。

管理 API 结构与错误枚举未改变，本次无需生成客户端。无前端或共享嵌入路径修改。
