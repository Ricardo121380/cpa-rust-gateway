# P13 全量发布方案 — Oracle Singapore(CPAR 生产)

| 项目 | 值 |
|---|---|
| 状态 | **待执行**。第 1–4 步已完成并留证;第 5 步授权由 operator 于 2026-08-29 给出;**第 6 步装机与重启由 operator 执行** |
| 目标 revision | `d75ab21b2c4f91f59c53e11087aa3a98caf1d9de` |
| 当前生产血缘 | `c02a689`(`codex/p12-deployment` 尖端,2026-08-09 发布) |
| 目标主机 | SSH alias `new-vps`(Oracle `ap-singapore-1` / Ubuntu 24.04 / aarch64) |
| 连接与红线 | [claude-code-oracle-singapore-vps.md](claude-code-oracle-singapore-vps.md) —— 本文件不放宽其中任何一条 |

---

## 0. 这次发布的真实范围(先看清再决定)

这**不是**一次前端发布。相对当前生产:

| | |
|---|---|
| 提交数 | 96 |
| 其中改后端 | 36 |
| 后端改动 | 89 个文件,**+46,190 / −2,447 行** |
| 数据库迁移 | **6 个(0014–0019)** |

**"只发前端"结构上不可能。** `c02a689` 的契约里 `listProviderEgressStatus`、`listOperationalUsage`、
`createCompatibleProxyPool`、`executeChannelPin`、`listOperationalBilling` **全部为 0** ——
Prism 的批 A–D 就是消费 P13 契约的那部分工作,它跑不到 P12 网关上。面板与后端由构造决定必须同发。

---

## 1. Artifact(已构建并独立验证)

| 项 | 值 |
|---|---|
| Workflow run | https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/33091046905 |
| 架构 | `ELF 64-bit LSB pie, ARM aarch64, GNU/Linux 3.7.0, stripped` |
| 二进制 SHA-256 | `714faebef0bfd97f1fde98303066196bb7a69e6dc884e5814d821b6bd29e9cbc` |
| SBOM | CycloneDX 1.5,290 组件,`2678472f3846463d439507c3087024ec2714d8ea4064ba09beae223dd8f93d72` |
| Sigstore | `cosign verify-blob` → **Verified OK**(在本机独立验过,不只信 workflow 自报) |
| 仓库验证器 | `p12-release-artifact.rb verify --require-signature --require-receipt` → `verified` |

**装机前必须在目标主机上重新核对 SHA-256**,复制过程本身也要验。

---

## 2. 数据库迁移 —— 本次发布的最大变量

启动即自动迁移,**没有单独命令、没有确认**:

```rust
// crates/gateway-store/src/control_plane.rs:1384
pub fn from_connection(mut connection: Connection) -> StoreResult<Self> {
    migrate(&mut connection)?;      // ← systemctl restart 就会跑
```

新增六张表:`0014_billing_ledger`、`0015_billing_materializer_checkpoint`、
`0016_routing_price_policy`、`0017_stored_responses`、`0018_stored_response_compactions`、
`0019_compatible_egress_pool`。

**两条实测结论,决定了回滚代价:**

1. **六个迁移里没有任何 `ALTER TABLE` / `DROP TABLE` / `DROP COLUMN` / `RENAME`** ——
   全部是新建表,既有 P12 表**一个字节都不动**。
   因此回滚只需换回旧二进制:旧版看到多出来的表不会受影响,**不需要跑 down 迁移**。
2. **每个迁移各自一个事务**,不是六个一起原子。
   所以中途失败会停在"已应用 N 个"的中间态 —— 这种情况**不要重试、不要继续**,直接走 §6 的库恢复。

---

## 3. 强制前置:备份

**在碰任何东西之前做,不做不许继续。** 沿用 P12-03 的 tar+哈希收据格式:

```bash
ts="$(date -u +%Y%m%dT%H%M%SZ)"
sudo install -d -m 0700 "/var/backups/cpa-rust-gateway/p13-deploy-$ts"

# 1) 控制面库的 SQLite 在线备份 + 完整性检查
sudo sqlite3 /var/lib/cpa-rust-gateway/control.sqlite3 \
  ".backup '/var/backups/cpa-rust-gateway/p13-deploy-$ts/control.sqlite3'"
sudo sqlite3 "/var/backups/cpa-rust-gateway/p13-deploy-$ts/control.sqlite3" \
  'PRAGMA quick_check;'          # 必须输出 ok

# 2) 现役二进制(回滚就靠它)
sudo cp -a /opt/cpa-rust-gateway/current/gateway \
  "/var/backups/cpa-rust-gateway/p13-deploy-$ts/gateway.previous"
sudo sha256sum "/var/backups/cpa-rust-gateway/p13-deploy-$ts/gateway.previous" | sudo tee \
  "/var/backups/cpa-rust-gateway/p13-deploy-$ts/gateway.previous.sha256"

# 3) 记下现役 revision(回滚时要指回它)
readlink -f /opt/cpa-rust-gateway/current
```

**`quick_check` 不是 `ok` 就停,不要发布。**

---

## 4. 强制前置:用生产库快照做 preflight

这一步能在**不碰生产进程**的前提下回答"新二进制吃不吃得下这个库、这份配置还编不编得出运行图" ——
2026-08-09 那次发布就是这么做的(见 `docs/reports/evidence/p12-codex-production-deployment-20260809.md`)。

```bash
# 独立目录 + 独立端口,与生产完全隔离
sudo install -d -m 0700 /tmp/p13-preflight/state /tmp/p13-preflight/creds
sudo cp "/var/backups/cpa-rust-gateway/p13-deploy-$ts/control.sqlite3" /tmp/p13-preflight/state/
# 凭据目录:复制现役 credential dir(root-owned,勿打印内容)

sudo /path/to/gateway-aarch64-unknown-linux-gnu serve \
  --data-listen 127.0.0.1:18280 \
  --management-listen 127.0.0.1:18281 \
  --state-dir /tmp/p13-preflight/state \
  --credential-dir /tmp/p13-preflight/creds
```

**四项必须全绿,任何一项不绿就不要发布:**

| # | 检查 | 期望 |
|---|---|---|
| 1 | 进程起得来,不 panic | 日志无 `migration`/`schema` 类错误 |
| 2 | `curl 127.0.0.1:18280/healthz` | HTTP 200 |
| 3 | `curl -H '<生产 client key>' 127.0.0.1:18280/v1/models` | HTTP 200 **且模型列表非空** —— 这证明 P12 时代的 active Config Version 在 P13 下仍能编译出运行图 |
| 4 | 快照库的 `PRAGMA quick_check` | 迁移后仍为 `ok` |

第 3 项是**本次发布最关键的单点**:P13-07 改过路由/候选的编译路径,如果现役配置在新版下编不出来,
网关会起得来但**什么都路由不了**。preflight 不做这一项,等于把这个风险直接带上生产。

跑完**立刻清理**:停 preflight 进程、删 `/tmp/p13-preflight`(里面有生产库副本与凭据副本)。

---

## 5. 装机(operator 执行)

```bash
revision=d75ab21b2c4f91f59c53e11087aa3a98caf1d9de

# 校验后再装
sha256sum gateway-aarch64-unknown-linux-gnu   # 必须 714faebe...9e9cbc

sudo install -d -o root -g root -m 0755 "/opt/cpa-rust-gateway/releases/$revision"
sudo install -o root -g root -m 0755 gateway-aarch64-unknown-linux-gnu \
  "/opt/cpa-rust-gateway/releases/$revision/gateway"
sudo sha256sum "/opt/cpa-rust-gateway/releases/$revision/gateway"   # 复制后再核一次

sudo ln -sfn "/opt/cpa-rust-gateway/releases/$revision" /opt/cpa-rust-gateway/current
sudo systemctl restart cpa-rust-gateway          # ← 迁移在这一刻发生
sudo systemctl status cpa-rust-gateway --no-pager
```

**发布后立即验(顺序不要变):**

```bash
curl --noproxy '*' --fail http://127.0.0.1:18180/healthz                  # 200
curl --noproxy '*' -H '<生产 client key>' http://127.0.0.1:18180/v1/models # 200 且非空
sudo sqlite3 /var/lib/cpa-rust-gateway/control.sqlite3 'PRAGMA quick_check;'  # ok
curl --fail https://<生产域名>/healthz                                     # 公网 200
```

再跑一次暴露检查,确认管理面**没有**被顺手带上公网:

```bash
./scripts/p12-07-verify-exposure.sh   # 第 7 项:四条管理路径均非 200
```

**Caddy / DNS / 防火墙一律不动。** 本次发布不含流量切换。

---

## 6. 回滚

**触发条件(任一即回滚,不要现场调试):**
`/healthz` 非 200 · `/v1/models` 非 200 或列表空 · `quick_check` 非 `ok` ·
公网健康检查失败 · 迁移中途失败 · 管理面出现在公网。

**回滚(只换二进制,不动库):**

```bash
sudo ln -sfn "$(cat /var/backups/cpa-rust-gateway/p13-deploy-$ts/previous-revision)" \
  /opt/cpa-rust-gateway/current
sudo systemctl restart cpa-rust-gateway
curl --noproxy '*' --fail http://127.0.0.1:18180/healthz
```

因为六个迁移全是新建表(§2),旧二进制**不需要 down 迁移**就能正常跑。

**只有在迁移中途失败时**才需要恢复库:停服务 → 用 §3 的备份覆盖
`/var/lib/cpa-rust-gateway/control.sqlite3` → 换回旧二进制 → 启动 → `quick_check`。

---

## 7. 我(Claude Code)没有做、也不会做的事

- **没有连过那台机器。** 上面每一条命令都是给 operator 的,不是我执行的记录。
- **不执行装机与重启** —— handoff §6 明确 "Only then may an *operator* install the artifact and restart the CPAR unit",
  且其禁止清单包含 `systemctl restart`。
- **不碰** Caddy / DNS / 防火墙 / Docker / Autoreg / Provider 请求 / Channel Pin。
- **不读也不打印**任何凭据、数据库行、请求响应体。

发布完成后如果你要我做 Prism 的真机验证,那是**另一件事**:按 handoff §4 建
`-L 18181:127.0.0.1:18181` 隧道,管理凭据由你带外提供,我只做只读渲染核对,
并按 handoff §9 的格式回报(`REMOTE_MUTATION_COUNT` / `PROVIDER_REQUEST_COUNT` / `SECRET_OUTPUT_COUNT`)。

---

## 8. 我无法从本地消除的三个不确定

1. **现役 active Config Version 在 P13 下能否编译出运行图** —— 只能靠 §4 preflight 第 3 项回答。
   这是本次发布的头号风险。
2. **P13 是否改变了既有流量的数据面行为** —— 后端 36 个提交里含协议与调度改动,
   本文件没有能力代替 P13 自己的 Delivery Gate 证据。
3. **拓扑归属** —— 旧 `jakarta-vps` 上 CPAR 进程仍 active(handoff §1)。
   **不要**据此停任何一台,也不要假设公网流量当前落在哪台。
