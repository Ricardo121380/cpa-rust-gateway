# P12-10I-02 Grok OAuth/SSO 与 CPAR HTTP 受控回执

| 字段 | 值 |
|---|---|
| 计划版本 | `v1.171`（执行边界登记）；本回执为 `P12-10I-02` 结果 |
| 日期 | 2026-08-05 |
| 环境 | Oracle Singapore loopback-only staging；生产 CPAR 未重启 |
| 运行方式 | Jakarta `grok-register` 源侧内存刷新/SSO 取值 → 有界 NDJSON 管道 → CPAR root-owned import → CPAR base URL + client key |
| Secret 处理 | 未写入本机、receipt、日志或 Git；未输出账号、OAuth、SSO、endpoint、model、请求/响应正文 |

## 受控边界

- 最多 2 个 Build refresh-token OAuth 与 2 个 Console SSO，串行、首败停止、不跨 Provider fallback。
- Build 只调用源项目的 refresh-first OAuth helper；不调用旧 CPA 管理 API，不改源 auth 文件。
- Console 使用源池已有 SSO envelope。Console 凭据不是 Build OAuth，未将两类凭据混用。
- 账号只进入 Oracle Singapore 临时 staging；没有写入生产 CPAR、旧 CPA、grok2api、CC Switch、Caddy、DNS 或公开监听。

## 结果

| 项目 | 结果 |
|---|---|
| Build 候选预检 | 源侧发现 174 个 enabled + refresh-token 候选；按边界选取 2 个 |
| Build OAuth refresh | 2 次尝试、0 成功、2 个 `RuntimeError` 类失败；CPAR accepted accounts = 0 |
| Console SSO 候选 | 源侧发现 128 个非空 SSO；选取 2 个，串行导入 |
| Console import | `source_records=2`, `accepted_accounts=2`, `rejected_records=0`, `created_accounts=2` |
| Console route 静态验证 | route validate 通过；隔离图可进入/发布 |
| Console CPAR `/v1/models` | 通过；目标模型可见 |
| Console CPAR 推理 | 第一条 Responses JSON 请求到达 CPAR 数据面，但在 `EgressRejected/egress`（HTTP 5xx）停止；`attempted=1`, `successful=0` |
| Console SSE | 未发送；遵守首败停止 |
| Build route/数据面 | Build 图静态验证通过；因 OAuth 0/2、Build pool 为空，激活后 staging 在启动时以 `gateway runtime is unavailable` fail closed；未发送推理 |

## 回滚与不变性

- Build 失败后未继续发送任何请求。
- staging 数据库恢复为生产快照的非生产副本，`quick_check=ok`、外键检查无输出；随后删除整个临时 staging 目录及临时 helper。
- 清理后 staging loopback listener = 0；生产服务仍为 `active`，生产回环 listener 数量保持 2，生产 active Config Version 数量保持 1。
- 本回执只记录无值计数/固定类别，不能证明 Grok Console 公共上游可用，也不能证明 Build OAuth 账号可用。

## 结论

`P12-10I-02` **未通过**，状态为 `BLOCKED_WITH_EVIDENCE`：Build 阻塞在外部 OAuth 账号，Console 阻塞在 CPAR egress admission；两者均未改变生产状态。后续若继续，应分别登记 Build 交互式 OAuth/device-code 边界和 Console egress 分类修复/复测边界，不得把本回执当作生产上线证据。
