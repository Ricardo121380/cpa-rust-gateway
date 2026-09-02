# CPAR — Provider 隔离的 Rust AI 网关

简体中文 | [English](README.md)

CPAR 是一个以安全边界为核心、使用 Rust 编写的 AI 反向代理网关。它向下游提供兼容 OpenAI 和
Anthropic 的 API，同时把不同 Provider 的凭据、账号、路由、运行时健康、额度、连续性状态和出口
策略放在显式且相互隔离的作用域中。

本项目以 clean-room 方式实现，参考了 CPA、CLIProxyAPI、Sub2API、grok2api、Kiro-RS 等项目的
可观察行为和运维经验。这些项目只是兼容性参考，并不是 CPAR 的运行时依赖；CPAR 也不是其中任何
一个项目的源码级 fork。

> **发布状态：** 已实现的后端能力已经按开发计划正式验收到 P13-11E4，但这不代表所有 roadmap
> 任务、真实账号或外部网络边界都已经完成。请同时阅读[开发状态](#开发状态)和权威的
> [详细开发计划](docs/06-development-plan.md)。

## 为什么选择 CPAR

- **Provider 隔离：** 凭据、健康、额度、Session、Clearance 和出口状态不会跨 Provider/Channel
  静默复用。
- **协议归一化：** Chat Completions、Responses、Anthropic Messages 共用有界的 Canonical
  request/event 模型，同时保留 Provider 专属 capability 校验。
- **Fail-closed 路由：** 不可变 Route Snapshot、精确 Credential lease、显式 revision、有界重试和
  首个语义事件规则共同阻止意外 fallback。
- **受保护的管理面：** 独立的 loopback 管理监听器提供版本化配置、加密凭据、审计、计费和运行时
  投影。
- **受控部署：** Secret 不进入仓库，生产监听器只绑定 loopback，并由操作者管理的 TLS 反向代理
  对外暴露数据面。
- **证据化交付：** 阶段报告、Contract、ADR、Traceability、本地 Gate 和不可变 GitHub Delivery Tag
  都保存在仓库中。

## 公共 API

| 能力 | Endpoint | 说明 |
|---|---|---|
| 模型目录 | `GET /v1/models` | 需要 Client Key；与推理使用同一份不可变 Route Snapshot。 |
| OpenAI Chat Completions | `POST /v1/chat/completions` | 对已声明支持的 Provider/Channel 提供 JSON 和有界 SSE。 |
| OpenAI Responses | `POST /v1/responses` | JSON/SSE，并严格校验 Canonical 生命周期。 |
| Responses WebSocket | `GET /v1/responses` | 只接受严格 `response.create`；一个活跃 turn 加一个排队 turn；不是 Realtime API。 |
| Stored Response 查询 | `GET /v1/responses/{id}` | 精确 Client-Key owner；foreign、过期、删除和不存在统一返回安全 not-found。 |
| Stored Response 删除 | `DELETE /v1/responses/{id}` | 只删除当前 Client Key 精确拥有的记录。 |
| Response 压缩 | `POST /v1/responses/compact` | Gateway 自有、有界 continuity token；仅限显式 capability Route。 |
| Anthropic Messages | `POST /v1/messages` | JSON 和有界 SSE。 |
| Anthropic Token Count | `POST /v1/messages/count_tokens` | 仅在选中 capability 存在已审查计数路径时可用。 |

所有推理请求都需要 CPAR Client Key。协议兼容不等于默认开放 Provider 能力；未声明的 capability
会在发生 Provider I/O 之前被拒绝。

### Provider 形态

CPAR 按 capability 建模，而不是把所有上游视为可以互换：

- 操作者自有的 `base_url + api_key` 通用 OpenAI-compatible 或 Anthropic-compatible Endpoint，
  Krill 类中转只是这一类别中的一个实例；
- 从受支持的 CPA/Sub2API JSON envelope 导入，或由操作者完成 OAuth 的官方 Codex/ChatGPT 凭据；
- 带 Provider 专属 Credential/runtime 状态的 Grok Build 与 Grok Console 账号池；
- 处于各自已记录本地/外部证据边界内的 Grok Web 和 Kiro adapter；
- 只有在所选 Config Version 中显式声明 adapter、协议和 egress capability 后才能使用的其他兼容
  Endpoint。

凭据 JSON 格式本身不决定路由或代理行为；真正决定行为的是精确的 Config Version、Upstream、
Endpoint、adapter、Credential binding、capability 和 egress policy。

## 管理与运维平面

管理监听器与公共数据监听器完全独立，必须保持 loopback-only。它提供内嵌 Prism 管理应用和
版本化、自动生成 client 的 `/admin` API。

已实现的管理能力包括：

- Config Version 的 draft、validate、publish、rollback 和 revision/ETag；
- Upstream、Endpoint、Credential、binding、route、candidate、alias、access group 和 Client Key；
- AEAD 加密凭据导入、OAuth workflow、metadata 查询和已审查的导出格式；
- 配置态 account-pool inventory 与 Provider-owned runtime account-pool 状态；
- 精确账号 operator action 与无值 failure feedback；
- runtime availability、quota recovery 和 Provider-scoped route explain；
- usage 聚合、不可变价格目录、billing materialization 与 routing price policy；
- compatible egress pool、加密 proxy node 和 exact binding profile；
- 将 Provider-specific egress/session/clearance 作为三个独立 source-domain row 查询；
- audit、observability、backup preflight 和 fail-closed restore staging。

管理响应采用 closed schema 和有界分页，不返回 Endpoint URL、凭据明文/密文、API Key、OAuth/SSO
材料、Cookie、请求正文、Provider 原始错误或 Client-Key digest。

## 架构

```text
原生 / CLI / 服务端客户端
       │
       ├── OpenAI Chat Completions
       ├── OpenAI Responses HTTP JSON/SSE
       ├── OpenAI Responses WebSocket
       └── Anthropic Messages
                   │
                   ▼
           认证 + Access Group
                   │
                   ▼
       Canonical request/event 边界
                   │
       ┌───────────┼────────────────────┐
       ▼           ▼                    ▼
  不可变路由图   Provider-scoped      capability +
                Credential lease      egress admission
       └───────────┼────────────────────┘
                   ▼
          Provider-specific adapter
                   │
                   ▼
             有界上游 transport
                   │
       ┌───────────┴─────────────────┐
       ▼                             ▼
 Canonical 下游事件             异步无值事件
                                     │
                                     ▼
                              SQLite + 运维投影
```

Cargo workspace 按 Canonical 类型、协议、路由、凭据/运行时状态、transport、observability、加密
持久化和 Actix HTTP composition 拆分。核心 Provider/协议逻辑不依赖 Actix Request 类型。普通请求
的路由选择不读取 SQLite，而是使用预先编译的不可变状态。

## 安全模型

CPAR 将 Provider Credential、账号 Cookie、OAuth Token 和生产数据库都视为高价值 Secret。

1. 五个部署 bootstrap credential 以直接普通文件注入，不通过环境变量或命令行值传递；
2. Provider Credential 和受保护 runtime payload 使用 domain-separated AEAD，并把 owner/revision
   纳入 associated data；
3. 公共推理和受保护管理使用两个不同的 loopback listener；
4. Management Key、same-origin/CSRF、Config Version 与 revision 共同保护控制面；
5. 在 lease 或 transport 之前独立验证 Provider、Upstream、Endpoint、Credential、账号和 egress
   ownership；
6. 日志、审计、错误、cursor 和 Debug 输出坚持 value-free；
7. 默认测试不会访问真实 Provider、部署服务器或注册账号。

使用真实 Credential 前请阅读 [SECURITY.md](SECURITY.md)。怀疑 Secret 泄漏时，请使用私有 GitHub
Security Advisory，不要提交公开 issue。

## 开发者快速开始

### 前置依赖

- `rust-toolchain.toml` 固定的 Rust `1.97.1`；
- 与 `web/prism/.nvmrc` 一致的 Node.js/npm，用于内嵌管理应用；
- Linux 构建包：`build-essential`、`clang`、`cmake`、`libclang-dev`、`libssl-dev`、
  `pkg-config`、`ca-certificates`；
- macOS：当前 Xcode Command Line Tools，必要时安装 Homebrew/OpenSSL；
- `ripgrep`；完整供应链 Gate 还会使用 `cargo-deny`、`cargo-audit`、`cargo-cyclonedx`。

### 构建

```bash
git clone https://github.com/Ricardo121380/cpa-rust-gateway.git
cd cpa-rust-gateway
npm --prefix web/prism ci --ignore-scripts --no-audit --no-fund
cargo build --locked --release --package gateway
./target/release/gateway --help
```

Rust 构建期间会编译并内嵌 Prism，因此源码构建必须同时具备 Rust toolchain 和已安装的 Prism npm
依赖。

### 验证

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
npm --prefix web/prism run check
./scripts/check.sh docs
```

集成或发布候选使用：

```bash
./scripts/check.sh fast
./scripts/check.sh full
```

Full Gate 会安装或要求固定版本的供应链工具，成本高于日常开发检查。

## 部署

完整部署步骤单独维护：

- [部署指南 — 简体中文](docs/deployment-guide.zh-CN.md)
- [Deployment Guide — English](docs/deployment-guide.en.md)

| 方式 | 适用环境 | 架构状态 |
|---|---|---|
| 源码构建 OCI/Docker 镜像 | 使用 host networking 的 Linux Docker | Linux `amd64`、`arm64` |
| Docker Compose | 单机 Linux、持久化 bind mount、宿主反向代理 | Linux `amd64`、`arm64` |
| 原生 binary + systemd | Debian/Ubuntu 及兼容 systemd 的发行版 | Linux x86-64、AArch64 |
| 原生前台运行 | 开发与本机评估 | Linux、macOS 当前宿主架构 |
| WSL2 | Windows 开发/评估 | WSL2 内的 Linux x86-64 |

当前没有公开 GHCR 镜像或公开 GitHub Release binary。签名 release workflow 只为获批 revision 生成
短期私有 artifact。在单独批准 public-release workflow 之前，公开用户应使用源码 Dockerfile 或
原生编译。

### Runtime bootstrap 契约

`gateway serve` 要求两个互不相同、非零的 loopback listener，一个绝对可写 state directory 和一个
绝对只读 credential directory：

```text
gateway serve \
  --data-listen 127.0.0.1:18180 \
  --management-listen 127.0.0.1:18181 \
  --state-dir /var/lib/cpa-rust-gateway \
  --credential-dir /run/cpa-rust-gateway-credentials
```

Credential directory 必须包含以下精确命名的直接普通文件：

| 文件 | 格式 |
|---|---|
| `management-key` | `mgmt_` namespace 的 ASCII，32–512 字节 |
| `management-csrf` | 独立 `csrf_` namespace 的 ASCII，32–512 字节 |
| `master-key` | 恰好 32 个原始字节 |
| `backup-key` | 恰好 32 个原始字节 |
| `client-key-pepper` | 恰好 32 个原始字节 |

不要通过 Caddy、Nginx、云负载均衡器或 Docker 端口映射暴露管理监听器。配置 Route 和 Client Key
后，只允许通过 HTTPS 反向代理公开数据监听器。

## 第一次配置

新进程会创建/打开 `control.sqlite3`，但没有推理 Route。操作者必须创建 draft Config Version，依次
配置 egress policy、Upstream、Endpoint、Credential binding、public model、route/candidate、access
group 和 Client Key，验证整张图，发布并重启 runtime，才能组合 active immutable snapshot。

可以使用受保护管理 API/Prism UI 或本地 `gateway admin` 命令。请在仓库外维护一份不含 Secret 的
opaque ID 台账；部分底层资源有意不提供 collection/list API。详细顺序和回滚规则见
[P12 Rollout Runbook](docs/p12-rollout-runbook.md)。

## 动态上游模型目录

对于已经支持 discovery 的渠道，`/v1/models` 不是手写的 Provider/套餐模型表。CPAR 按活动
Config Version + Endpoint + Credential 发现 exact upstream model ID，把最后一次成功结果持久化到
SQLite，再从已发布的 Route 模板原子派生只允许对应 Credential 租约的运行时 Route。当前自动运行时
source 仅覆盖 Grok Build 与 official Codex；Grok Web/Console、xAI Official、Kiro 和 generic
compatible endpoint 仍需各自经过审查的 source adapter。

默认生命周期为 Fresh 6 小时、24 小时后应刷新、72 小时硬过期。瞬时失败保留最后一次成功；删除模型
必须至少连续三次成功响应都遗漏该模型，并且隔离满 24 小时，失败请求不计数。Worker 每小时检查一次
状态，但只有目标从未成功或已经到刷新截止时间时才会联系 Provider。

操作者可以检查受保护的 `GET /admin/catalog/status`。它只返回 exact opaque Endpoint/Credential ID、
freshness、目标本地 snapshot version、是否应刷新、保留模型数量和有限的失败时间/类别；不会返回模型
ID、URL、响应正文、账号身份或 Secret。启动或升级后应当：

1. 使用目标 CPAR Client Key 认证调用 `GET /v1/models`；
2. 确认列表只包含该 Key 有权访问的 exact upstream ID；
3. 用选中的 exact ID 和该渠道真实协议执行一次有界请求；
4. 模型缺失或过期时查看受保护 status，不能用前端常量补齐。

Grok Build 的 `free/supergrok/heavy` 和 ChatGPT 的 `free/go/plus/pro5x/pro20x` 属于独立权益
metadata，绝不能据此生成 model ID。首次交互 OAuth 和被撤销 refresh grant 的恢复仍由
operator/Autoreg 负责；有效 OAuth 已导入 CPAR 且渠道已支持 refresh 时，日常自动续期由 CPAR 负责。

## 客户端请求示例

操作者发布 Route 并签发 CPAR Client Key 后：

```bash
curl --fail-with-body https://your-cpar.example/v1/responses \
  -H 'Authorization: Bearer <CPAR_CLIENT_KEY>' \
  -H 'Content-Type: application/json' \
  -d '{"model":"<PUBLIC_MODEL>","input":"Reply with OK.","stream":false}'
```

WebSocket 模式升级 `GET /v1/responses`，不能带浏览器 `Origin`，随后发送一条严格的
`response.create` JSON。这个下游 WebSocket 既不是 Provider-native transport，也不是 OpenAI
Realtime API。

## 开发状态

- **已正式验收的实现：** 后端已完成到 P13-11E4，包括 P13 管理/计费/路由、Channel Pin、Stored
  Responses、Responses WebSocket、compatible egress pool 和 Provider egress 状态投影；
- **前端集成：** Prism 独立消费 generated management contract，当前 handoff 以实际分支和
  `docs/cross-boundary-log.md` 为准；
- **正在实施：** P13-15 全渠道上游模型目录透传；Build/Codex exact-Credential source 已真实观测
  `grok-4.6`、`grok-4.5`、`gpt-5.6-terra`、`gpt-5.6-luna`、`gpt-5.5` 与
  `gpt-5.4-mini`；P13-15C/D 的 durable freshness/removal、受保护 status 和 atomic
  Credential-scoped route materialization 已本地通过。生产 `grok-4.6`、其他渠道 source、隔离矩阵和
  正式 Gate 仍待完成；客户端必须消费 gateway 返回列表，不能靠手工常量补齐；
- **显式延期或外部阻塞：** Kiro/Official API-key 真实 E2E、Grok Web 外部 egress/WAF、
  P13-11E5 真实 Provider/代理/DNS canary、自动账号注册/修复、Media/Files/Batch 和更多 Provider；
- **CPAR 凭据生命周期：** 已保存、已绑定且 Provider 明确支持 refresh 的 OAuth 由 CPAR 在启动时和
  运行中自动续期、CAS 加密保存并原子替换运行时 material。API Key 与 SSO Cookie 不伪装成 OAuth
  refresh；当前 P13-16A 已在生产证明 Grok Build 自动刷新后继续服务，Codex 无效 grant 使用
  `1/2/4/.../60` 分钟退避；Claude/Kiro 激活前仍需各自 exact-channel executor；
- **不属于 CPAR：** Autoreg 的账号注册、首次登录/授权、refresh grant 撤销后的交互 reauth、权益
  修复和 replenishment。Autoreg 不参与 CPAR 已保存 OAuth 的日常 token refresh。

`DONE_WITH_BOUNDARY` 只代表文档中冻结的验收边界通过，不代表每一个 Provider 账号、外部网络路径
或生产部署都经过测试。

## Git 与发布治理

开发使用 Phase/Integration Branch 和不可变 `phase-p*-complete` 证据 Tag。普通 branch push/PR
更新只运行轻量检查，昂贵 Delivery Gate 必须显式触发并绑定精确 revision。当前每一个分支及其
合并建议见[Git 分支审计](docs/git-branch-inventory-2026-08-19.md)。

安全收口路径是一条经过 review 的 integration PR 合入 `main`，再为最终不可变 revision 运行一次
正式 Gate。已经是祖先的历史 Phase Branch 不需要重复 merge；非祖先分支必须定向 reconciliation，
不能盲目整分支合并。

## 仓库目录

| 路径 | 用途 |
|---|---|
| `apps/gateway` | CLI、进程组合、数据/管理 listener |
| `crates/gateway-*` | core、auth、catalog、control、routing、store、transport、HTTP、observability |
| `crates/protocol-*` | 下游/上游协议 codec |
| `crates/provider-*` | Provider-specific adapter 与状态 |
| `web/prism` | 内嵌 React 管理应用和 generated client |
| `docs/adr` | 已接受架构决策 |
| `docs/contracts` | 可执行行为/安全契约 |
| `docs/reports` | 阶段与验证证据 |
| `deploy` | systemd、Caddy、Docker 部署资产 |
| `scripts` | 确定性检查、release 验证和有界 operator helper |

## 文档导航

- [中文部署指南](docs/deployment-guide.zh-CN.md)
- [后端完成度审计](docs/backend-completion-audit-2026-08-19.md)
- [Git 分支清单](docs/git-branch-inventory-2026-08-19.md)
- [行为契约](docs/02-behavior-contracts.md)
- [目标架构](docs/03-target-architecture-draft.md)
- [渠道参考分析](docs/04-channel-reference-analysis.md)
- [详细开发计划](docs/06-development-plan.md)
- [管理前端计划](docs/08-management-frontend-development-plan.md)
- [Traceability](docs/traceability.md)
- [架构决策](docs/adr/README.md)
- [Contract 索引](docs/contracts/README.md)
- [报告索引](docs/reports/README.md)
- [质量门禁](docs/quality-gates.md)
- [Crate 边界](docs/crate-boundaries.md)
- [第三方声明](THIRD_PARTY_NOTICES.md)

## 贡献

请保持变更小而明确、Provider-scoped 且有证据。不要提交真实 Credential 或 Provider 原始 payload。
先更新 authoritative OpenAPI，再生成 client；遵守 `AGENTS.md` 的前后端 ownership；新安全语义需要
ADR/Contract；每个变更都应包含 focused test 和 value-free 验收记录。

## 许可证

CPAR 使用 [MIT License](LICENSE)。保留版权与许可声明后，可以使用、复制、修改、合并、发布、
分发、再许可或销售。本项目参考的第三方项目及许可证说明见
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
