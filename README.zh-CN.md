# CPAR — Provider 隔离的 Rust AI 网关

简体中文 | [English](README.md)

CPAR 是 CPA/CLIProxyAPI 网关能力的 Rust 实现，面向自托管使用。协议、账号、模型和路由
优先对照固定版本的 CPA Go 源码，再在现有 Rust 模块中实现。CPA 未覆盖的渠道采用专项参考。

> **当前入口（2026-10-23）：** [当前状态与开发顺序](docs/engineering-roadmap.md)统一记录已实现功能、
> 生产接线、验证边界、固定 CPA 版本和下一项。[后端架构](docs/03-target-architecture-draft.md)负责代码位置，
> [候选支持清单](docs/support/first-release-candidates.md)保存客户端与交付证据。
> 旧 Prism 已退役，新前端以后根据管理 API 重新设计。

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
| Anthropic Token Count | `POST /v1/messages/count_tokens` | 路由已存在，当前生产装配仍使用不支持计数的 executor；计数实现与接线待补。 |

所有推理请求都需要 CPAR Client Key。协议兼容不等于默认开放 Provider 能力；未声明的 capability
会在发生 Provider I/O 之前被拒绝。

### Provider 形态

CPAR 按 capability 建模，而不是把所有上游视为可以互换：

- 操作者自有的 `base_url + api_key` 通用 OpenAI-compatible 或 Anthropic-compatible Endpoint，
  Krill 类中转只是这一类别中的一个实例；
- 从受支持的 CPA/Sub2API JSON envelope 导入，或由操作者完成 OAuth 的官方 Codex/ChatGPT 凭据；
- 通过兼容协议 adapter 执行推理的 Claude、Kimi 授权、导入及刷新路径；
- 带 Provider 专属 Credential/runtime 状态的 Grok Build 与 Grok Console 账号池；
- 处于各自已记录本地/外部证据边界内的 Grok Web 和 Kiro adapter；
- 只有在所选 Config Version 中显式声明 adapter、协议和 egress capability 后才能使用的其他兼容
  Endpoint。

凭据 JSON 格式本身不决定路由或代理行为；真正决定行为的是精确的 Config Version、Upstream、
Endpoint、adapter、Credential binding、capability 和 egress policy。

## 管理与运维平面

管理监听器与公共数据监听器完全独立，必须保持 loopback-only。它提供版本化的 `/admin` API；
旧管理页面已移除，当前可用管理 API 和本地 CLI 操作。

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
- Linux 构建包：`build-essential`、`clang`、`cmake`、`libclang-dev`、`libssl-dev`、
  `pkg-config`、`ca-certificates`；
- macOS：当前 Xcode Command Line Tools，必要时安装 Homebrew/OpenSSL；
- `ripgrep`；完整供应链 Gate 还会使用 `cargo-deny`、`cargo-audit`、`cargo-cyclonedx`。

### 构建

```bash
git clone https://github.com/Ricardo121380/cpa-rust-gateway.git
cd cpa-rust-gateway
cargo build --locked --release --package gateway
./target/release/gateway --help
```

后端独立构建，不需要 Node/npm 或前端目录。

### 验证

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
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
group 和 Client Key，再验证和发布。通过运行中的管理 API 发布会准备并原子切换运行代；
使用离线 admin CLI 修改后，需要重新启动 runtime 采用已发布配置。

可以使用受保护管理 API 或本地 `gateway admin` 命令。请在仓库外维护一份不含 Secret 的
opaque ID 台账；部分底层资源有意不提供 collection/list API。详细顺序和回滚规则见
[P12 Rollout Runbook](docs/p12-rollout-runbook.md)。

## 动态上游模型目录

对于已经支持 discovery 的渠道，`/v1/models` 不是手写的 Provider/套餐模型表。CPAR 按活动
Config Version + Endpoint + Credential 发现 exact upstream model ID，把最后一次成功结果持久化到
SQLite，再从已发布的 Route 模板原子派生只允许对应 Credential 租约的运行时 Route。当前自动运行时
source 包括 Grok Build、official Codex、Kiro，以及配置了 `models_path` 的 compatible endpoint。
Grok Web/Console 和 xAI Official 不自动继承这些 discovery 实现；source 已接线不代表每个账号和
endpoint 的真实调用都已验收。

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

## 首版客户端选择（2026-10-23）

**Claude Code D2 已选入首版**；Codex、Claude D2、Pi 仍须全部通过约定的 Grok Build 门槛才可 READY。
采用完整、固定版本的[D2局部配置](docs/support/grok-build-messages.md)，并核对
[候选支持清单](docs/support/first-release-candidates.md)。实际客户端受控 PASS 来自既有证据；
固定 Build 资格仍 BLOCKED、#66 仍 OPEN、真实 Provider 验收 NOT_RUN。原始 Claude 的
adaptive/display、overall effort、block cache 义务作为后续待办保留。
正式决定见[ADR-0123](docs/adr/ADR-0123-first-release-claude-d2-restricted-support.md)。

ADR-0125 请求兼容调整已在本地实现，历史原始 Claude 的拒绝及请求计数不再作为当前预期。
D2 产品选择保持；当前源码的原版客户端与固定服务验收待重跑，见[当前状态入口](docs/engineering-roadmap.md)。

## 开发状态

已实现功能、生产接线、证据缺口和下一项建议统一见[当前状态入口](docs/engineering-roadmap.md)。
已实施的后端整理和前端退役见[架构文档](docs/03-target-architecture-draft.md)。

<details>
<summary>10 月 18 日及更早的开发状态快照</summary>

截至 2026-10-18，主仓库基线为 `3369c730`。网关主体及管理流程已存在，当前仍在完成
[现有渠道收敛 #9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)。GitHub 有 14 个
开放工单；[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57)仍有 11 个开放直接前置。

- **最新本地门槛：** 10 月 17 日 Full 收据绑定 `2f43057c`，45/45 PASS，Rust 1515 PASS、
  0 FAIL、13 IGNORED；不是今天重跑，也不代表真实渠道或新候选通过。
- **下一迭代方向：** [#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47) 的
  Grok Build → Responses → 多轮工具调用作为内部起点；首个可用版本必须同时通过 Codex、
  Claude Code、Pi，三者均调用 Grok Build，Messages 路径也是必做项。
  [ADR-0107](docs/adr/ADR-0107-standard-response-metadata-separation.md)
  已接受经审查元数据分离并完整加密留存的方向，字段准入与实现待完成；当前代码仍因额外 Usage
  拒绝完整样本。[ADR-0108](docs/adr/ADR-0108-build-summary-standard-messages.md)另行允许
  Claude Code 不展示 Grok 思考摘要，须完整加密留存且证明工具续轮不受影响；该路径尚未实现。
- **版本与扩展：** 按明确支持范围分版本交付，#9 未完成项继续保留。`native-v1` 已接受的
  扩展方向独立推进；profile、专用产品消费者及完整证据持久化仍未实现。
- **保留的阻塞：** Kiro 原生 cap、Grok Web 多轮/部分 cap、逐渠道真实证据、同候选交付条件。
  具体范围与证据见[10 月 18 日路线图](docs/reports/engineering-roadmap-20261018.md)，本轮未复核生产运行状态。

<details>
<summary>历史 P13 阶段状态原文（按原报告时间理解，不作为当前执行队列）</summary>

- **已正式验收的实现：** 后端已完成到 P13-11E4，包括 P13 管理/计费/路由、Channel Pin、Stored
  Responses、Responses WebSocket、compatible egress pool 和 Provider egress 状态投影；
- **前端集成：** Prism 独立消费 generated management contract，当前 handoff 以实际分支和
  `docs/cross-boundary-log.md` 为准；
- **正在实施：** P13-15 全渠道上游模型目录透传；Build/Codex exact-Credential source 已真实观测
  `grok-4.6`、`grok-4.5`、`gpt-5.6-terra`、`gpt-5.6-luna`、`gpt-5.5` 与
  `gpt-5.4-mini`；P13-15C/D 的 durable freshness/removal、受保护 status 和 atomic
  Credential-scoped route materialization 已在 Oracle Singapore 通过 Grok Build 生产验收：授权
  `/v1/models` 已动态包含 `grok-4.6`，唯一一次真实 non-streaming Responses canary 已完成。其他渠道
  source、目标 ChatGPT Go/Codex 证据、隔离矩阵、Claude Code 接线和正式 Gate 仍待完成；客户端必须
  消费 gateway 返回列表，不能靠手工常量补齐；
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

</details>

</details>

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
| `docs/adr` | 已接受架构决策 |
| `docs/contracts` | 可执行行为/安全契约 |
| `docs/reports` | 阶段与验证证据 |
| `deploy` | systemd、Caddy、Docker 部署资产 |
| `scripts` | 确定性检查、release 验证和有界 operator helper |

## 文档导航

- [当前状态、CPA 参考与开发顺序](docs/engineering-roadmap.md)
- [中文部署指南](docs/deployment-guide.zh-CN.md)
- [后端完成度审计](docs/backend-completion-audit-2026-08-19.md)
- [Git 分支清单](docs/git-branch-inventory-2026-08-19.md)
- [行为契约](docs/02-behavior-contracts.md)
- [目标架构](docs/03-target-architecture-draft.md)
- [渠道参考分析](docs/04-channel-reference-analysis.md)
- [历史阶段开发计划与技术基线](docs/06-development-plan.md)
- [历史管理前端计划](docs/08-management-frontend-development-plan.md)
- [Traceability](docs/traceability.md)
- [架构决策](docs/adr/README.md)
- [Contract 索引](docs/contracts/README.md)
- [报告索引](docs/reports/README.md)
- [质量门禁](docs/quality-gates.md)
- [Crate 边界](docs/crate-boundaries.md)
- [第三方声明](THIRD_PARTY_NOTICES.md)

## 贡献

请保持变更小而明确、Provider-scoped 且有证据。不要提交真实 Credential 或 Provider 原始 payload。
先更新 authoritative OpenAPI，再生成 client；遵守 `AGENTS.md` 的模块边界；新安全语义需要
ADR/Contract；每个变更都应包含 focused test 和 value-free 验收记录。

## 许可证

CPAR 使用 [MIT License](LICENSE)。保留版权与许可声明后，可以使用、复制、修改、合并、发布、
分发、再许可或销售。本项目参考的第三方项目及许可证说明见
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

## Grok Build 标准 Messages 受控切片（#63）

此路径不展示 Grok 思考摘要；已确认 summary 在 JSON/SSE 成功收口前完整加密留存。
受控范围使用标准文本与工具历史，实际 Claude Code、真实 Provider 及 #61 字段/profile
资格仍单独验收。参见[支持范围](docs/support/grok-build-messages.md)。
