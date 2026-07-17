# Executable behavior contracts

本目录把 [关键行为与兼容性契约](../02-behavior-contracts.md) 拆成可由 Fixture、属性测试和端到端测试验证的契约。

## 编号与文件约定

- 契约 ID：`BC-<domain>-NNN`，例如 `BC-STREAM-001`。
- 契约说明：`BC-<domain>-NNN-short-title.md`。
- Fixture 放入 `tests/fixtures/<domain>/`，不得包含真实凭据或未脱敏生产响应。
- 每份契约必须列出入口、前置条件、事件序列、不变量、错误语义和对应测试。

## 领域

```text
HTTP       公开接口与鉴权
STREAM     SSE、Chunk、终止和取消
TOOL       Tool 定义、参数、调用和结果
ROUTE      Alias、Candidate、Credential 与 Failover
CRED       凭据状态、刷新、Quota 和错误
CATALOG    模型发现、Fresh/Stale/Expired 与移除
CONT       Cache/Response/Replay/Conversation 连续性
SEC        Secret、SSRF、租户隔离和审计
```

第一批可执行契约将在 P1 随 Canonical Mock 垂直链路创建。
