# CPAR — Rust AI Gateway

> A provider-isolated, protocol-compatible AI gateway written in Rust.
> 一个用 Rust 实现、按 Provider 隔离、兼容常见 AI 协议的反向代理网关。

CPAR is a clean-room Rust implementation inspired by the operational lessons of CPA,
CLIProxyAPI, Sub2API and provider-specific reference projects.  It is not a copy of any one
upstream codebase.  The gateway separates public client protocols from provider adapters,
credentials, routing state and egress policies so that one provider's credentials or failures
cannot silently cross into another provider.

CPAR 是基于 CPA、CLIProxyAPI、Sub2API 及各 Provider 参考项目的行为经验，以 clean-room 方式
重新实现的 Rust 网关，不是对某个上游项目的逐文件复制。项目将客户端协议、Provider adapter、
凭据、路由状态和出口策略分层建模，禁止不同 Provider 之间静默复用凭据或故障状态。

## What this repository provides / 当前能力

### Public data plane / 公共数据面

- OpenAI Chat Completions: `/v1/chat/completions`
- OpenAI Responses: `/v1/responses`
- Anthropic Messages: `/v1/messages`
- JSON and bounded SSE projection for the supported protocol/provider matrix
- Authenticated `/v1/models` generated from the same immutable route snapshot as inference
- Request-scoped retry and fail-closed protocol transforms before the first downstream byte

公开数据面当前提供：

- OpenAI Chat Completions：`/v1/chat/completions`
- OpenAI Responses：`/v1/responses`
- Anthropic Messages：`/v1/messages`
- 对已声明支持的协议/Provider 组合提供 JSON 和有界 SSE 投影
- `/v1/models` 与推理使用同一份不可变 Route Snapshot
- 首字节之前的请求级重试，以及未知语义的 fail-closed 协议转换

### Provider isolation / Provider 隔离

The runtime is designed for independent provider families, including OpenAI-compatible
upstreams, official Codex/ChatGPT OAuth, Grok Build, Grok Console and Krill-style upstreams.
Provider-specific capabilities remain explicit.  Grok Web, Kiro OAuth and other external
boundaries are not silently advertised as production-ready when their external evidence is
missing.

运行时支持独立的 OpenAI-compatible 上游、官方 Codex/ChatGPT OAuth、Grok Build、Grok Console
以及 Krill 类上游。Provider 专属能力必须显式声明；Grok Web、Kiro OAuth 等外部边界在缺少
有效证据时不会被隐式标记为生产可用。

### Management plane / 管理面

The protected management listener provides versioned configuration workflows, encrypted
credentials, optimistic revision/ETag checks, audit records, backup/restore boundaries and a
generated TypeScript client.  The P13-04 operations surface adds:

- `GET /admin/operations/account-pools` — secret-free Provider/Channel/Account/Binding inventory
- `GET /admin/operations/usage` — bounded durable Request/Attempt/Usage aggregation

管理监听器提供版本化配置、AEAD 加密凭据、revision/ETag 乐观并发控制、审计、备份/恢复边界和
自动生成的 TypeScript client。P13-04 新增：

- `GET /admin/operations/account-pools`：不含 Secret 的 Provider/Channel/Account/Binding 库存
- `GET /admin/operations/usage`：有界 durable Request/Attempt/Usage 聚合

The operations projection never returns endpoint URLs, credential ciphertext/plaintext,
request bodies, cookies or client-key digests.  Usage counters carry `exact`, `partial` or
`unknown` confidence.  Until a versioned price catalog exists, cost is explicitly
`null`/`unpriced`; CPAR never guesses a bill from token counts.

运营投影不会返回 endpoint URL、凭据密文/明文、请求正文、Cookie 或 client-key digest。Usage
计数带有 `exact`、`partial`、`unknown` confidence。版本化价格目录完成前，费用固定为
`null`/`unpriced`，不会根据 token 数量猜测账单。

## Architecture / 架构

```text
Client protocol
  ├─ Chat Completions
  ├─ Responses
  └─ Anthropic Messages
          │
          ▼
Canonical request/event boundary
          │
          ├─ Auth + Access Group
          ├─ immutable Route Snapshot
          ├─ provider-scoped credential lease
          ├─ provider adapter + protocol projection
          └─ bounded Request / Attempt / Usage events
                          │
                          ├─ SQLite event log
                          └─ protected management read models
```

The workspace is split into focused crates for canonical types, protocol codecs, routing,
credential state, upstream transport, observability, encrypted control-plane storage and Actix
HTTP composition.  Actix is an edge adapter; core protocol and provider logic does not depend on
Actix types.

Workspace 按职责拆分为 canonical 类型、协议 codec、路由、凭据状态、上游传输、可观测性、AEAD
控制面存储和 Actix HTTP composition 等 crate。Actix 只负责边界适配，核心协议与 Provider 逻辑
不依赖 Actix 类型。

## Development status / 开发状态

The current development plan is tracked in [`docs/06-development-plan.md`](docs/06-development-plan.md).
P0–P6, P9–P12 have completed their approved local/phase boundaries.  P13-04 (management
operations foundations) is locally implemented and reviewed; P13-05 is the next task for a
versioned price catalog and durable billing ledger.

当前唯一执行基线是 [`docs/06-development-plan.md`](docs/06-development-plan.md)。P0–P6、P9–P12
已完成各自批准的本地/阶段边界。P13-04（管理运营后端基础）已完成本地实现与 review；下一项
是 P13-05：版本化价格目录和 durable billing ledger。

The following boundaries remain explicit and are not hidden by the public README:

- external Grok Web egress/WAF validation is deferred;
- Kiro OAuth and Official API-key external E2E require their own credentials and approval;
- automatic refresh/reauth/replenishment is a separate controlled task;
- WebSocket, media and additional providers are conditional roadmap items;
- no real credentials, production database, server key, account pool or deployment endpoint is
  stored in this repository.

以下边界仍保持显式延期，不会被 README 淡化：

- Grok Web 的外部 egress/WAF 验收延后；
- Kiro OAuth 和 Official API-key 的真实 E2E 需要独立账号与授权；
- 自动 refresh/reauth/replenishment 是单独受控任务；
- WebSocket、Media 和更多 Provider 是条件性路线图项目；
- 仓库不保存真实凭据、生产数据库、服务器私钥、账号池或部署端点。

## Quick start / 快速开始

### Prerequisites / 前置依赖

- Rust toolchain pinned by `rust-toolchain.toml`
- Cargo and `rustfmt`
- Node.js/npm for the generated management client and admin SPA
- `ripgrep` for repository checks
- Optional: `cargo-deny` and `cargo-audit` for the full supply-chain gate

### Build and test / 构建与测试

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
npm --prefix web/admin-ui run check
./scripts/check.sh docs
```

The repository also contains the approved fast/full quality gates:

```bash
./scripts/check.sh fast
./scripts/check.sh full
```

首次执行完整供应链门禁时，如本机尚未安装工具，可按 `scripts/install-quality-tools.sh` 的说明
准备固定版本。普通开发先执行受影响 crate 的定向测试和 `./scripts/check.sh docs`。

### Local serving / 本机启动

The binary expects credentials and state through operator-selected files or deployment-managed
secret sources.  Never put real values in shell history, fixtures, README files or issue reports.
For an isolated local run, use synthetic credentials and loopback-only listeners; do not point the
sample configuration at a real provider until a separate provider-specific approval exists.

二进制通过操作者指定的文件或部署系统 Secret 源读取凭据和状态。不要把真实值写入 shell history、
fixture、README 或 issue。进行本机演练时使用 synthetic 凭据和 loopback listener；在没有独立
Provider 授权前，不要把示例配置指向真实上游。

## Security and public-release policy / 安全与公开发布策略

The public repository is intentionally value-free:

1. Real API keys, OAuth access/refresh tokens, SSO cookies, passwords, private keys and production
   databases are excluded and ignored by Git.
2. Test fixtures use clearly synthetic values and must assert that those values never appear in
   management responses or logs.
3. Management responses use closed schemas, bounded pagination, `no-store` for sensitive reads,
   AEAD at rest and value-free errors.
4. Production deployment, account registration and provider probes are never performed by the
   default build or test commands.
5. Please report suspected credential exposure privately according to [`SECURITY.md`](SECURITY.md).

公开仓库坚持 value-free 原则：

1. 真实 API key、OAuth access/refresh token、SSO Cookie、密码、私钥和生产数据库均不进入 Git；
2. Fixture 只使用明确的 synthetic 值，并验证这些值不会进入管理响应或日志；
3. 管理响应使用 closed schema、有界分页、敏感读操作 `no-store`、AEAD 存储和 value-free 错误；
4. 默认构建/测试不会执行生产部署、账号注册或真实 Provider 探针；
5. 怀疑凭据泄露时，请按 [`SECURITY.md`](SECURITY.md) 的方式私下报告。

## Reference and licensing notes / 参考与许可说明

CPA, CLIProxyAPI, Sub2API, grok2api and Kiro-RS are behavior and compatibility references,
not bundled runtime dependencies.  Their licenses and notices are recorded in
[`docs/00-reference-baseline.md`](docs/00-reference-baseline.md) and
[`docs/04-channel-reference-analysis.md`](docs/04-channel-reference-analysis.md).  Any future
direct code import must preserve the upstream license and attribution; clean-room behavior
reimplementation must not be described as copied source.

CPA、CLIProxyAPI、Sub2API、grok2api 和 Kiro-RS 是行为/兼容性参考，不是本项目的运行时依赖。
相关许可和声明记录在 [`docs/00-reference-baseline.md`](docs/00-reference-baseline.md) 与
[`docs/04-channel-reference-analysis.md`](docs/04-channel-reference-analysis.md)。未来如直接
引入代码，必须保留上游许可和署名；clean-room 行为重实现不得描述为复制源码。

## Documentation map / 文档导航

- [Behavior contracts / 行为契约](docs/02-behavior-contracts.md)
- [Target architecture / 目标架构](docs/03-target-architecture-draft.md)
- [Channel references / 渠道参考](docs/04-channel-reference-analysis.md)
- [Upstream aggregation / 上游聚合](docs/05-upstream-aggregation-design.md)
- [Development plan / 开发计划](docs/06-development-plan.md)
- [Traceability / 需求追踪](docs/traceability.md)
- [ADRs / 架构决策](docs/adr/README.md)
- [Contracts / 可执行契约](docs/contracts/README.md)
- [Reports / 阶段报告](docs/reports/README.md)
- [Quality gates / 质量门禁](docs/quality-gates.md)
- [Crate boundaries / Crate 边界](docs/crate-boundaries.md)

## License

No project license has been selected yet.  Public visibility does not by itself grant permission
to copy, modify or redistribute this repository.  Until a maintainer adds a license, treat the
project as “all rights reserved” and review the third-party notices and compatibility references
before using any part of it.

当前尚未选择项目许可证。仓库公开并不自动授予复制、修改或再分发权限。在维护者添加明确许可
之前，请按“保留所有权利”理解，并在使用任何部分前检查第三方声明和兼容性参考。
