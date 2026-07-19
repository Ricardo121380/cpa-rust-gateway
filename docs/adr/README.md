# Architecture Decision Records

本目录保存影响多个 Crate、公开行为、安全边界或部署方式的架构决策。

## 编号与状态

- 文件名：`ADR-NNNN-short-title.md`。
- 状态只能是 `Proposed`、`Accepted`、`Superseded` 或 `Rejected`。
- 已接受 ADR 不直接覆盖；改变决定时新建 ADR，并在旧记录中链接替代项。
- 任何 `unsafe` 例外、核心依赖方向变化、公开协议变化或持久化模型变化都必须有 ADR。

## 必填结构

```text
Title
Status
Date
Task / Matrix / Contract references
Context
Decision
Consequences
Alternatives considered
Validation and rollback
```

## 当前索引

P0 期间不创建重复描述开发计划技术基线的 ADR。P1 开始只对具体实现选择建立记录，并在 [需求追踪索引](../traceability.md) 中登记。

- [ADR-0001 Version-scoped control-plane SQLite schema](ADR-0001-version-scoped-control-plane-schema.md) — `P2-01`。
- [ADR-0002 Version-scoped route and access schema](ADR-0002-version-scoped-route-access-schema.md) — `P2-02`。
- [ADR-0003 XChaCha20-Poly1305 Secret Store](ADR-0003-xchacha20poly1305-secret-store.md) — `P2-03`。
- [ADR-0004 Client Key HMAC credential](ADR-0004-client-key-hmac-credential.md) — `P2-04`。
