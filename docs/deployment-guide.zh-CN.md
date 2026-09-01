# CPAR 部署指南

简体中文 | [English](deployment-guide.en.md)

本文说明如何在不削弱 Listener、Credential 和 Provider 隔离契约的前提下构建、部署、升级和回滚
CPAR。内容以 2026-08-20 的仓库状态为基线。

## 1. 系统与架构支持矩阵

| 平台 | 原生源码构建 | 容器 | 建议用途 | 状态 |
|---|---:|---:|---|---|
| Linux x86-64，glibc | 支持 | `linux/amd64` | 生产/社区自托管 | 已有 release workflow 与 systemd envelope |
| Linux AArch64，glibc | 支持 | `linux/arm64` | 生产/社区自托管 | 已有原生 ARM64 release runner |
| macOS Apple Silicon | 支持 | Docker Desktop host mode 不是已审查生产路径 | 开发、本机评估 | 本地构建/Gate 已使用 |
| macOS Intel | 固定 toolchain 预期可构建 | 不是已审查生产路径 | 仅开发 | 当前无公开 release artifact |
| Windows | 使用 WSL2 | 仅 Docker/WSL2 开发 | 开发/评估 | 无原生 Windows Service 契约 |
| Alpine/musl | 当前不支持 | 无 Alpine runtime | 无 | 需要独立依赖与 toolchain 审计 |
| ARMv7/32-bit | 当前不支持 | 无镜像 | 无 | 需要独立架构和 atomic-width 审计 |
| Kubernetes | 可以容器化，但没有已审查 manifest/operator | 非开箱即用 | 后续 | 需要独立 Listener/Secret/Persistence 契约 |

签名私有 release workflow 目前只识别 `x86_64-unknown-linux-gnu` 和
`aarch64-unknown-linux-gnu`。musl、ARMv7、Windows、Kubernetes 在构建、运行时、Secret、持久化和
回滚路径完成独立验收前，不应宣称为正式支持。

## 2. 选择部署方式

- 单机 Linux 且可使用 host networking/bind mount 时，使用 **Docker/Compose**；
- 需要当前最完整的主机 hardening 与 `LoadCredential=` 时，使用 **原生 systemd**；
- Linux/macOS 开发或隔离评估，使用 **原生前台构建**；
- 获得授权的 Maintainer 可以使用 **签名私有 artifact workflow**，但它不是公开下载渠道。

根目录 `Dockerfile` 是签名 release 的 runtime envelope，要求先提供已验证、名为 `gateway` 的
binary。社区从源码构建镜像应使用 `deploy/docker/Dockerfile`。

在本文档快照时，Compose 文件已通过静态配置验证，但编写文档的 Mac 没有运行 Docker daemon。
因此源码镜像的 Build/Run 路径只是有边界的部署说明，不应被表述为本机已经完成的 Smoke Test；将
社区镜像视为生产可用前，应在原生 Linux amd64 和 arm64 Builder 上分别实际构建并运行验证。

## 3. 所有部署方式共用的安全要求

### 3.1 Listener

CPAR 只接受 loopback listener，且数据面与管理面必须是两个不同的非零地址：

```text
gateway serve \
  --data-listen 127.0.0.1:18180 \
  --management-listen 127.0.0.1:18181 \
  --state-dir /var/lib/cpa-rust-gateway \
  --credential-dir /run/cpa-rust-gateway-credentials
```

- TLS 反向代理只能连接 `127.0.0.1:18180`；
- 绝不能把 `127.0.0.1:18181` 暴露给公网 Load Balancer、Caddy Site、Docker `ports:` 或防火墙规则；
- 远程管理请使用带认证的 SSH Tunnel。

### 3.2 Bootstrap Credential

必须创建六个直接普通文件。符号链接、目录、Special File、空值、超限值或错误长度都会被拒绝。

| 文件名 | 要求 |
|---|---|
| `management-key` | 以 `mgmt_` 开头的 ASCII，总长度 32–512 字节 |
| `management-csrf` | 独立、以 `csrf_` 开头的 ASCII，总长度 32–512 字节 |
| `master-key` | 恰好 32 个随机原始字节 |
| `backup-key` | 恰好 32 个随机原始字节 |
| `client-key-pepper` | 恰好 32 个随机原始字节 |
| `grok-build-cache-key` | 恰好 32 个随机原始字节；仅用于租户隔离的 Grok Build Prompt Cache Identity |

Linux root 管理示例：

```bash
sudo install -d -o root -g root -m 0700 /etc/cpa-rust-gateway/credentials
sudo sh -c 'umask 077; printf "mgmt_%s" "$(openssl rand -hex 32)" > /etc/cpa-rust-gateway/credentials/management-key'
sudo sh -c 'umask 077; printf "csrf_%s" "$(openssl rand -hex 32)" > /etc/cpa-rust-gateway/credentials/management-csrf'
sudo sh -c 'umask 077; openssl rand -out /etc/cpa-rust-gateway/credentials/master-key 32'
sudo sh -c 'umask 077; openssl rand -out /etc/cpa-rust-gateway/credentials/backup-key 32'
sudo sh -c 'umask 077; openssl rand -out /etc/cpa-rust-gateway/credentials/client-key-pepper 32'
sudo sh -c 'umask 077; openssl rand -out /etc/cpa-rust-gateway/credentials/grok-build-cache-key 32'
```

Key Material 与 SQLite State 必须分别备份：没有匹配 Key Ring 的数据库备份无法打开加密 Credential；
只有 Key 没有数据库也不是完整运行时备份。不要把值写入 Compose、环境变量、命令行参数、镜像或 Git。

### 3.3 State

可写 State Directory 至少包含：

- `control.sqlite3`：控制面、加密 Credential、事件和运维状态；
- `backups/`：已审查的 Backup Artifact；
- 执行 Restore Staging 时的 `restore-target.sqlite3`。

一个 State Directory 只能由一个进程使用。应放在可靠的本地持久化文件系统上；执行在线备份时需要
按 SQLite 规则处理数据库/WAL/SHM，并在操作者控制的流程中同时保护外部 Key。

## 4. 原生源码构建

### 4.1 Linux 依赖

Debian/Ubuntu 示例：

```bash
sudo apt-get update
sudo apt-get install --yes \
  build-essential ca-certificates clang cmake curl git libclang-dev libssl-dev \
  nodejs npm pkg-config ripgrep
```

安装固定 Rust Toolchain，并使用 `web/prism/.nvmrc` 指定的 Node 版本。若发行版 Node 太旧，建议使用
nvm、fnm 或 Volta。

全新主机先从 rustup 官方 bootstrap 安装 `rustup`，再安装固定 Toolchain：

```bash
command -v rustup >/dev/null || \
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
    https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
source "$HOME/.cargo/env"
```

### 4.2 编译固定 Revision

```bash
git clone https://github.com/Ricardo121380/cpa-rust-gateway.git
cd cpa-rust-gateway
git checkout <TAG_OR_COMMIT>

rustup toolchain install 1.97.1 --profile minimal --component rustfmt --component clippy
npm --prefix web/prism ci --ignore-scripts --no-audit --no-fund
npm --prefix web/prism run check

export GATEWAY_RELEASE_REVISION="$(git rev-parse HEAD)"
export GATEWAY_RELEASE_RUST_VERSION=1.97.1
export GATEWAY_RELEASE_TARGET="$(rustc +1.97.1 -vV | awk '/^host:/ {print $2}')"
CARGO_INCREMENTAL=0 cargo +1.97.1 build --locked --release --package gateway

./target/release/gateway --help
```

Rust Build 会调用 Prism Build，并内嵌精确的四个已审查 Asset。如果没有安装
`web/prism/node_modules`，或 generated management client 与 Contract 漂移，Cargo 会主动失败，
不会打包旧 UI。

### 4.3 本地 Smoke Test

仓库提供 synthetic、loopback-only Smoke Test：

```bash
./scripts/test-p12-02-serve.sh
```

它会创建临时 synthetic key，启动两个 Listener，检查 `/healthz`、管理面隔离和内嵌 UI，然后删除
临时状态；不会访问真实 Provider。

## 5. Docker 源码构建

### 5.1 构建当前宿主架构

```bash
docker buildx build \
  --file deploy/docker/Dockerfile \
  --build-arg CPAR_REVISION="$(git rev-parse HEAD)" \
  --build-arg CPAR_VERSION=source \
  --load \
  --tag cpar:local \
  .
```

这是社区/源码镜像，不继承私有 release workflow 的 Sigstore Signature、SBOM 或不可变 Receipt。

### 5.2 多架构镜像

BuildKit Builder 能执行两个平台时：

```bash
docker buildx build \
  --file deploy/docker/Dockerfile \
  --platform linux/amd64,linux/arm64 \
  --build-arg CPAR_REVISION="$(git rev-parse HEAD)" \
  --build-arg CPAR_VERSION=source \
  --output type=oci,dest=cpar-source.oci.tar \
  .
```

优先使用原生 Builder。QEMU/Emulation 可以构建社区镜像，但不等于项目签名 Release 的证据强度；
正式流程会在每个架构的原生 Runner 上构建并执行 Smoke Test。

### 5.3 准备宿主目录

Runtime Image 使用数字 UID/GID `65532`。State Mount 必须对该身份可写；Credential Mount 必须可读、
可遍历但不可写。

```bash
sudo install -d -o 65532 -g 65532 -m 0700 /srv/cpar/state
sudo install -d -o 65532 -g 65532 -m 0500 /srv/cpar/credentials
# 安全创建六个文件后：
sudo chown 65532:65532 /srv/cpar/credentials/*
sudo chmod 0400 /srv/cpar/credentials/*
```

### 5.4 Linux Docker 运行

```bash
docker run -d --name cpar \
  --restart unless-stopped \
  --network host \
  --read-only \
  --user 65532:65532 \
  --cap-drop ALL \
  --security-opt no-new-privileges:true \
  --memory 768m --cpus 2 --pids-limit 512 \
  --ulimit nofile=65536:65536 \
  --tmpfs /tmp:rw,noexec,nosuid,nodev,size=16m \
  --mount type=bind,src=/srv/cpar/state,dst=/var/lib/cpa-rust-gateway \
  --mount type=bind,src=/srv/cpar/credentials,dst=/run/cpar-credentials,readonly \
  cpar:local serve \
  --data-listen 127.0.0.1:18180 \
  --management-listen 127.0.0.1:18181 \
  --state-dir /var/lib/cpa-rust-gateway \
  --credential-dir /run/cpar-credentials

curl --noproxy '*' --fail http://127.0.0.1:18180/healthz
```

因为 CPAR 有意只绑定容器 loopback，所以 Linux 必须使用 Host Networking。Bridge Mode 的
`-p 18180:18180` 无法访问容器内 `127.0.0.1`。Docker Desktop Host Networking 不是已审查生产路径。

## 6. Docker Compose

```bash
export CPAR_IMAGE=cpar:local
export CPAR_STATE_DIR=/srv/cpar/state
export CPAR_CREDENTIAL_DIR=/srv/cpar/credentials
export CPAR_REVISION="$(git rev-parse HEAD)"
export CPAR_VERSION=source

docker compose -f deploy/docker/compose.yml config
docker compose -f deploy/docker/compose.yml up -d --build
curl --noproxy '*' --fail http://127.0.0.1:18180/healthz
```

Compose 有意不包含 `ports:`，且因为 `network_mode: host` 仅支持 Linux。它使用 Read-only Root FS、
删除 Capability、设置资源限制，并分别挂载 State 与 Credential。`docker compose down -v` 不会备份
或删除这些 Bind Mount；显式删除或覆盖 `CPAR_STATE_DIR` 才是破坏性操作。

## 7. 原生 Linux + systemd

### 7.1 安装 Binary 与账号

```bash
sudo groupadd --system cpa-gateway 2>/dev/null || true
sudo useradd --system --gid cpa-gateway --home-dir /var/lib/cpa-rust-gateway \
  --shell /usr/sbin/nologin cpa-gateway 2>/dev/null || true

revision="$(git rev-parse HEAD)"
sudo install -d -o root -g root -m 0755 "/opt/cpa-rust-gateway/releases/$revision"
sudo install -o root -g root -m 0755 target/release/gateway \
  "/opt/cpa-rust-gateway/releases/$revision/gateway"
sudo ln -sfn "/opt/cpa-rust-gateway/releases/$revision" /opt/cpa-rust-gateway/current
```

按前文创建 root-owned Credential，再安装和验证已审查 Unit：

```bash
ruby scripts/check-p12-02-systemd-unit.rb
sudo install -o root -g root -m 0644 deploy/systemd/cpa-rust-gateway.service \
  /etc/systemd/system/cpa-rust-gateway.service
sudo systemd-analyze verify /etc/systemd/system/cpa-rust-gateway.service
sudo systemctl daemon-reload
sudo systemctl enable --now cpa-rust-gateway
sudo systemctl status cpa-rust-gateway --no-pager
curl --noproxy '*' --fail http://127.0.0.1:18180/healthz
```

Unit 会创建 State/Runtime/Log Directory，通过 systemd 只读 Credential Directory 传入 Key，在无
Capability 身份下运行，并应用文件系统、Kernel 和 Syscall Hardening。

## 8. 首次启动与配置

空数据库也可能通过 `/healthz`，但在操作者完成以下步骤前无法推理：

1. 打开 loopback Prism UI 或 Management API；
2. 创建 Draft Config Version；
3. 配置 Egress Policy、Upstream、Endpoint、Credential 与 Exact Binding；
4. 创建 Public Model、Route/Candidate、Access Group 和 Client Key；
5. Validate 并 Publish Config Version；
6. 重启 CPAR，组合 Active Immutable Runtime Graph；
7. 先验证 `/v1/models`，再对目标 Provider/Channel 做一次有界请求。

远程管理必须使用 Tunnel，而不是公开管理端口：

```bash
ssh -L 18181:127.0.0.1:18181 operator@your-server
```

然后在本机打开 `http://127.0.0.1:18181/admin-ui/`。详细资源顺序、Opaque ID 台账、Canary 和
Rollback 规则见 `docs/p12-rollout-runbook.md`。

## 9. 反向代理与 TLS

使用 Caddy、Nginx 或其他已审查反向代理终止 TLS，并且只代理数据面。仓库 `deploy/caddy` 是带
占位域名的模板，不是可直接复制的生产配置。

关键边界：

- 不得代理管理端口 `18181`；
- 禁止改变 SSE Framing 的响应 Buffer/Compression；
- 不要把全局 Caddy `servers` Timeout 影响到其他站点；
- 保留 WebSocket Upgrade Header；
- 显式配置 Upstream Connect/Header/Idle Timeout；
- 切流前分别验证 `/healthz`、JSON、SSE、WebSocket。

## 10. 升级、备份与回滚

1. 记录当前 Binary SHA、Config Version、Database Schema 与 Key Version；
2. 创建文件系统/SQLite 一致的备份与 Hash Receipt，Key Material 单独备份；
3. 构建、验证 Successor Revision，不立即替换 `current`；
4. 停止 CPAR，保留旧 Database，切换 `current` Symlink 或 Image Digest，再启动；
5. 检查 `/healthz`、Management Preflight、`/v1/models` 和一次有界 Canary；
6. 验收失败时停止 Successor，恢复旧 Binary/Image 和兼容 Database，并验证 Rollback Receipt。

没有匹配 Migration/Restore Contract 时，不要跨不可逆 Schema/Key 变更降级。Management Restore 会
写入 `restore-target.sqlite3`，不会静默覆盖 Live Database。

## 11. 签名私有 Release Artifact

`.github/workflows/release-artifact.yml` 需要手动触发，当前为 Linux amd64/arm64 生成保留 14 天的
私有 Artifact。每个 Bundle 包含 Binary、OCI Archive、CycloneDX SBOM、Manifest、Sigstore Bundle
和 Receipt；它不会发布 GHCR 或 GitHub Release。

导入前，必须根据预期身份验证解压后的 Artifact Bundle：

```bash
ruby scripts/p12-release-artifact.rb verify \
  --artifact-dir <EXTRACTED_ARTIFACT_DIR> \
  --revision <EXACT_40_CHARACTER_SHA> \
  --rust-toolchain 1.97.1 \
  --target <x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu> \
  --version 0.1.0 \
  --require-signature \
  --require-receipt

ruby scripts/p12-release-artifact.rb inspect-oci \
  --artifact-dir <EXTRACTED_ARTIFACT_DIR> \
  --revision <EXACT_40_CHARACTER_SHA> \
  --rust-toolchain 1.97.1 \
  --target <x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu> \
  --version 0.1.0
```

Ruby verifier 会检查封闭文件集合、摘要、Build Metadata、架构、OCI Layout、SBOM 和 Receipt。
`--require-signature` 会要求 Sigstore 文件存在且被 Receipt 摘要覆盖，但它本身**不会**执行密码学
签名验证。安装 Cosign 后，必须再使用 Artifact 之外预先固定的身份独立验证 Manifest：

```bash
cosign verify-blob \
  --bundle <EXTRACTED_ARTIFACT_DIR>/artifact-manifest.sigstore.json \
  --certificate-identity \
    'https://github.com/Ricardo121380/cpa-rust-gateway/.github/workflows/release-artifact.yml@<EXPECTED_REF>' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  <EXTRACTED_ARTIFACT_DIR>/artifact-manifest.json
```

必须把 `<EXPECTED_REF>` 替换为从独立可信来源确定的 Release Ref。不能仅因为下载包内的
`signing-identity.json` 声称某个身份就接受它，否则攻击者可以同时选择 Artifact 和所谓的信任根。

正式 `gateway-image.oci.tar` 是 OCI Archive，不是 Docker Archive。使用 OCI 工具导入，例如：

```bash
skopeo copy \
  oci-archive:gateway-image.oci.tar \
  docker-daemon:cpa-rust-gateway:<REVISION>
```

`docker load` 只适用于 Docker Archive，不适用于正式 OCI Layout。使用前必须同时完成结构/摘要
验证与独立 Cosign 验证。

## 12. 故障排查

| 现象 | 检查项 |
|---|---|
| Cargo 提示缺少 Prism dependency | 运行文档中的精确 `npm --prefix web/prism ci ...` 命令。 |
| 容器启动但宿主无法连接 | 确认 Linux `--network host`；Bridge `ports:` 与 loopback-only bind 不兼容。 |
| Credential 被拒绝 | 检查精确文件名、普通文件、大小/前缀、Permission 且不能是 Symlink。 |
| `/healthz` 正常但不能推理 | Publish 完整 Config Version、签发 Client Key 并重启。 |
| Management 返回 not found | 检查 Management Key、loopback peer/origin 和精确 Config Version Header。 |
| SSE 被缓冲 | 禁止 Proxy Buffer/Compression，并检查 Idle/Read Timeout。 |
| Cursor/Revision 冲突 | 从第一页重新执行有界读取，或重新读取当前 ETag/Revision。 |
| Restore 后数据库无法解密 | 恢复匹配的外部 Master/Backup Key；不要猜测或直接替换 Key。 |

## 13. 明确不在本文授权范围内的事项

本文不授权真实 Provider Probe、账号注册、Autoreg、公开管理面、自动 Credential 修复、生产 Grok
Web Clearance、Kubernetes 或未支持架构。这些工作必须有独立 Change Request、证据和回滚计划。
