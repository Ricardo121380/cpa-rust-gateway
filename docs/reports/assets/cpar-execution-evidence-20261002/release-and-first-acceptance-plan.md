# new-vps 发布、回退与首批真实验收方案

最新复核（2026-10-05）：[发布就绪复核报告](../../cpar-release-readiness-and-controlled-deployment-preparation-20261005.md)、[逐项门槛](../cpar-release-readiness-20261005/release-readiness.json)与[完整可执行配方](../cpar-release-readiness-20261005/controlled-deployment-runbook.md)。

状态：**BLOCKED / UNRESOLVED_SOURCE_SPEC_FINDINGS_AND_ACCEPTANCE_CONTRACT_GAPS**。A／B原PARTIAL静态覆盖已补齐，但候选有13项P2规格问题；B ADR结构与本地验收矩阵／revision合同冲突尚待处理。Krill新gpt6-sol路线唯一Key已于10月4日到期，首批真实验收另有权限前置。当前不能称“只等部署授权”。

AC-01修复及历史材料可读性仍PASS，10月5日只读重验普通41/41、归档32/32与native5/5；两平台各9文件与原验签集合一致，签名、45/45 Full、ARM64410回归及历史迁移／完整恢复证据保留。现场仍旧sourceb0cf36e3/schema31/PID499070。本轮无产品代码修改、生产写入、服务停启、部署、迁移、配置／账号／权限变化、推理或GitHub写入。

以下保留原固定对象、历史演练和freeze/execute入口；新的现场／权限／停止条件和完整操作以10月5日配方为准。新配方包括获准停服后才取的最新完整offline备份、恢复验证、候选启动时的有界数据准入门控、完整state/auth/六key/old制品回退和新写入保全决策。临时nft门控是未来具体部署授权需覆盖的新增主机状态，本轮未设置，不修改unit/Caddy。产品修复产生新候选时重新绑定source/hash、安装路径、检查和配方，不能复用当前签名证明新源码。

## 固定对象与当前事实

| 对象 | 固定值 / 实际证据 |
| --- | --- |
| source SHA | `80f5838902eb56be6dccc3ce047eb227d6c18e6c`；报告初始 HEAD `a2c9e0743ab10097499bbef76a8f51db60043c69`，后续报告提交也不是发布源码 |
| 专用远端 ref | `refs/heads/codex/cpar-execution-arm64-20261002`；本轮从不存在状态创建，只指向上述 source；没有 force push |
| release CI | [run 36956561884 / attempt 1](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36956561884)，原完整 matrix 的 native x86_64 与 ARM64 均 SUCCESS；实际 checkout 为上述 source |
| ARM64 制品 | `gateway-aarch64-unknown-linux-gnu`，19,500,320 bytes，SHA256 `542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87`；Rust 1.97.1 / aarch64-unknown-linux-gnu / 应用 0.1.0 / schema 34 |
| 已签名 manifest | [完整集合与文件 hash](../cpar-signed-state-rehearsal-20261002/download-and-file-inventory.json)；[ARM64 manifest](../cpar-signed-state-rehearsal-20261002/linux-arm64/artifact-manifest.json) 文件 SHA256 `7e6c9b59b3d2715692cd8178662ca8532836c784db5b833c6c4ee1ae0aa81653` |
| 签名身份 | issuer `https://token.actions.githubusercontent.com`；identity `https://github.com/Ricardo121380/cpa-rust-gateway/.github/workflows/release-artifact.yml@refs/heads/codex/cpar-execution-arm64-20261002`；证书 source OID 和 run/attempt 均匹配；[实际密码学及严格回执检查](../cpar-signed-state-rehearsal-20261002/signed-verification.json) 两平台 PASS |
| 目标服务 | `new-vps` / `cpa-rust-gateway.service` / `cpa-gateway:cpa-gateway`，UID995/GID986；生产数据、管理监听仍为 `127.0.0.1:18180` / `127.0.0.1:18181` |
| 旧版本 | current 与进程均 `/opt/cpa-rust-gateway/releases/b0cf36e387b774439acc23830221389eb3ac74d0`；旧 binary SHA256 `0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b`；schema31；PID499070，起始 2026-09-28 09:23:42 UTC |
| 目标兼容 | [ELF / ldd / 隔离 help](../cpar-signed-state-rehearsal-20261002/target-compatibility.json) PASS；ELF64 LE AArch64，最大 GLIBC2.34，目标2.39，动态库无缺失；[候选实际服务启动](../cpar-signed-state-rehearsal-20261002/candidate-startup-receipt.json) PASS |
| 生产后核验 | [03:48 UTC 只读核验](../cpar-signed-state-rehearsal-20261002/production-after.json)及[私有末次审计](../cpar-signed-state-rehearsal-20261002/final-private-audit.json)：PID/current/旧 hash/unit/drop-in/六把持久 key 均匹配；两端口未变；无演练 gateway 残留。空闲 31,000,924,160 bytes |

远端制品与本地 unsigned 候选 `bd2e46ed6ab7174e2cfcdebe88ca7cba8cc1fb0b0ff46e9714469bc900cdadc0` **字节不同**。最终采用对象明确为 `542a3c30…`；其 ELF、版本、动态库、410 回归、生产副本迁移和新旧启动均已重新实际验证。旧本地集合只保留为历史证据。

## 本轮已完成的准备与尚未解除的停止条件

1. [远端 workflow 和实际 checkout/toolchain](../cpar-signed-state-rehearsal-20261002/checkout-and-toolchain-excerpts.log)、完整两平台制品、Sigstore bundle、签名及 receipt 均已归集。对指定 identity/issuer 执行 cosign verify-blob，再执行既有 P12 `--require-signature --require-receipt`。各 payload hash/size 与 manifest 一致，ZIP digest 与 GitHub API 一致。预期 identity 文件没有被当作密码学证明。
2. 主机任务根 `/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884` 为 root:root 0700。完整已验证制品在 `artifact-542a3c30aae7/`；此前超时传输的 `artifact/` 是失败现场，禁止用它安装。
3. [一致在线副本](../cpar-signed-state-rehearsal-20261002/backup-receipt.json)在 `capture-2/`：两库 SQLite Online Backup API，WAL 由该机制处理；全捕获窗口中两个同连接 data_version 均不变，key/备份目录 inotify 静默，key stat/hash 未变且持久 key 与进程 LoadCredential 完全相同。窗口为 02:47:53.274953–02:47:53.398746 UTC，0.123784 秒；未暂停写入或停服。这证明该实际窗口的跨库/key 一致性，不宣称两个独立备份操作天然跨库原子。
4. [迁移](../cpar-signed-state-rehearsal-20261002/migration-receipt.json)在 `migration-entry-probe/`，31→34 实际通过。53 个旧控制表及1个认证表全部旧列 count/HMAC 保持；schema_migrations 只对原31条校验，新增32–34单独核对。新 execution sidecar=0、新 native import receipt=0，旧 Usage/accounting 仍 Unknown。CLI audit 在完成真实 store migration 后因空 CLI capability view 报 `missing_endpoint_capability_profile`；原日志保留，不能称 audit 整体成功。随后实际 serve 使用持久视图启动并完成受保护 GET。
5. [完整旧状态恢复](../cpar-signed-state-rehearsal-20261002/restore-receipt.json)在另一目录 `restored-pristine/`：两库启动前逐字节匹配 capture-2、全部原表旧列一致、六把持久 key 匹配、旧 binary hash 核实，schema31/integrity/FK通过，旧 serve 和受保护读取通过。[备份档案/属主/模式/软链及恢复依赖](../cpar-signed-state-rehearsal-20261002/complete-private-restore-audit.json)实际核验通过。没有执行 migration down。
6. 启动分别使用上述 pristine 状态的派生副本。mount/net/PID/IPC/UTS 独立，chroot 只见 task 状态、受保护 key 和公共运行库；真实 UID995/GID986，effective/bounding capability=0、NoNewPrivs；独立 `127.0.0.1:28180/28181`，无外部路由、保留地址连通测试被内核拒绝。后台续期/发现线程存在，但其外部网络全部被阻断；只在派生副本产生 credential_refresh_state 更新，原迁移和恢复状态保留。真实 Provider 调用0。
7. **历史密文可读性 PASS（授权生产修复后）**：[原升级前审计](../cpar-signed-state-rehearsal-20261002/crypto-before.json)保留40/41及AEAD_INVALID_TAG；原迁移／恢复保持同一异常的证据不改写。后续仅修复AC-01的ciphertext，[新生产处置回执](../cpar-archived-credential-production-repair-20261002/post-repair-validation.json)为普通41/41、归档32/32、active3/3、draft6/6、native5/5，旧／签名候选18项原生操作及54表迁移保持通过。原失败locator、密文、状态与新完整pre/post集合留在受保护task根；没有删除、换账号、修改逻辑revision或增加发布例外。
8. 活跃生产在运行期间的 refresh state、1份 upstream credential 的 ciphertext/revision 和管理审计发生[可观察漂移](../cpar-signed-state-rehearsal-20261002/production-logical-comparison.json)。配置、权限、路由、原生账号、历史Request/Attempt/Usage/ledger的旧列比较均保持；不要宣称活跃生产整库字节冻结。副本证明仅绑定捕获时点；任何未来切换须重取当前 inventory 与完整 pre-upgrade 备份。

归档密文问题已在获批单字段范围内解决并重验，原材料门槛保持且已满足。10月5日新复核确认总体同时受源代码规格问题、本地验收／契约缺口和首批Key前置影响；不能仅归因于inventory、offline备份或授权。旧构建缺失新的execution/capability字段属于**部署后** pin/GET核验事项，不是要求切换前已有新构建真实请求。

## 原切换模型（最新精确操作与授权范围见10月5日配方）

申请对象为：`new-vps` 的唯一 `cpa-rust-gateway.service` 从上述 b0cf36e3/schema31 切换到已签名 source80f58389/hash542a3c30 的 release，允许该服务一次 stop/start、31→34 自动迁移、只读迁移核验，以及失败时停止候选并完整恢复 pre-upgrade 状态/key、切回旧 release 后启动。配置发布、unit/Caddy 修改、权限/账号状态修改和真实推理不包含在此申请；真实验收预算另行批准。**当前 BLOCKED，不执行此申请。新的临时nft准入门控及数据恢复决定以10月5日配方末段的具体范围为准；没有把它当作已授权动作。**

拟安装不可变 release 为 `/opt/cpa-rust-gateway/releases/80f5838902eb56be6dccc3ce047eb227d6c18e6c-542a3c30aae7/`，其 `gateway` 是最终 ARM64 payload 的同 hash 副本，`artifact/` 保存完整9文件及签名/receipt。以 source+artifact hash 标识安装对象，避免与同 source 的 unsigned 构建混用。

操作顺序：

1. 重新核对远端精确 ref、已验证9文件集合及密码学签名，重新核对目标 old PID/source/hash/schema、unit/drop-ins/current、key匹配、空间和当前活跃配置/权限/账号/出口；若对象或状态漂移，先重新冻结并审查。安装只创建上述新 release：root属主、目录0755、gateway0755，artifact文件保持原字节；同名目录内容不同即停止，不原地覆盖。不运行失败/不完整 `artifact/` 目录。
2. 历史密文阻断已经关闭，fresh-copy新旧原生读取／启动复验已通过；切换前仍重核新inventory和当时状态。只有获得本节独立部署授权后才进入批准的维护窗口，停止进入新的验收/业务流量、等待在途请求自然结束。只停止gateway服务，不停止Caddy或其他服务，不靠修改Client Key、账号、能力或权限关闭窗口。维护影响为两loopback服务在stop→startup/迁移核验期间暂时不可用；本轮没有测量生产维护耗时，不给虚构停机时长。
3. 确认生产 PID 消失后，在 `/var/backups/cpa-rust-gateway/80f5838902eb56be6dccc3ce047eb227d6c18e6c-542a3c30aae7/<UTC>/` root0700 创建新的完整**离线 pre-upgrade**集合：control.sqlite3、admin-auth.sqlite3、存在的WAL/SHM/journal及必要状态/backups，六把持久源key，unit/drop-ins、current目标、old ELF和ownership/mode/symlink元数据。清楚记录捕获UTC与内部校验；不得把本轮较早的 capture-2 当切换时最新备份。
4. 系统 `LoadCredential` 六个持久源为 `/etc/cpa-rust-gateway/credentials/{management-key,management-csrf,master-key,backup-key,client-key-pepper,grok-build-cache-key}`。备份与恢复依赖持久源，不依赖 stop 后消失的 `/run/credentials/`。钥匙、认证库和控制库属于同一恢复集合。校验schema31、两库完整性/FK、原列HMAC、关键表计数和密文可读性；先在私有恢复派生目录用旧 hash ELF启动，确认备份可恢复，全部通过后才改 current。
5. 确认新 release payload 与 manifest/hash一致后，通过临时 symlink +同目录 `mv -T` 原子替换 current，然后启动 gateway。unit、原监听、Caddy、账号及活跃配置不改，无 daemon-reload。若实际 unit 已不同，停止重审。
6. 迁移核验：实际 `/proc/PID/exe` hash和embedded source、受保护 `/admin/system` source/target/Rust/version、current、新schema34、两库integrity/FK、身份/loopback监听、所有原列计数/HMAC与离线基线、配置/权限/停用意图/账号关联、5份原生Grok与历史import批次、历史 Request/Attempt/Usage/ledger 及Unknown行为；不得以一个HTTP200放行。原 Request/Attempt 的缺失新证据仍Unknown，新的真实执行身份须待部署后授权请求验证。

关键切换命令形态（尚未执行；所有停止条件、备份/恢复检查与新授权满足后才可运行）：

```sh
sudo systemctl stop cpa-rust-gateway.service
# 完整离线备份、新状态的旧版恢复验证和新release安装校验完成后
sudo ln -s /opt/cpa-rust-gateway/releases/80f5838902eb56be6dccc3ce047eb227d6c18e6c-542a3c30aae7 /opt/cpa-rust-gateway/current.cpar-execution-next
sudo mv -T /opt/cpa-rust-gateway/current.cpar-execution-next /opt/cpa-rust-gateway/current
sudo systemctl start cpa-rust-gateway.service
```

临时 symlink 若已存在，核对其所有权/来源和目标，不能盲目覆盖或删除。离线备份、恢复派生验证、安装和核验是上述命令之间的必要操作。

## 停止与完整回退条件

发布时历史凭据重新不可读或尚未完成fresh核对／完整备份／独立部署授权；source/ref/hash/target/toolchain/version不符；密码学验签或receipt不符；备份/key一致性、迁移、integrity/FK、原列数据、配置/权限/停用意图/账号或native材料失败；监听暴露扩大；异常重启；actual lease/config/egress/scoped capability未知或矛盾；Usage/账本source不匹配；正文或秘密泄漏。任一触发停止后续推理和放量，不重试凑通过、不改能力/权限规避。

回退在明确授权覆盖的范围执行：停止候选；先在root0700目录保存**完整迁移后/失败状态及新增Request/Attempt/Usage/ledger**，包括当时对应key和元数据；恢复刚刚验证过的整套pre-upgrade state、auth、六把keys、ownership/mode/symlink及必要部署元数据；原子恢复current到b0cf36e3旧release；用旧 hash ELF启动并复核schema31、完整性、原列数据/权限及加密材料。迁移后现场保留。本轮 restore receipt 验证了这一完整恢复模型；只切旧binary、migration down或仅改symlink都不能替代它。

切换后若已有生产写入，不静默抹掉它们恢复旧库。先保留新状态和账本/请求窗口，明确待协调的数据范围和恢复决策，再执行获准回退。首批验收应在放开普通业务前进行；历史副本不能代替新写入的数据合并。生产账号恢复、权限/配置变更及多服务回退均在本申请外。

## 首批真实目标和预算草案

优先级 1：[Krill Responses JSON 草案](next-krill-responses-json-draft.json)：`gpt-6-sol`，route `route-0b9a2bb7-3f85-487a-bba2-0871bacc46ca`，candidate `candidate-95857499-eff9-4b29-8bb5-34e55dbabcb0`，endpoint `p12-12-production-krill-responses-endpoint`，Provider `p12-12-production-krill-upstream`，Client Key 证据 `4154a728ad5abcb01d21935755d1a35fe218a906449da23cc220848e9ee0705f`，账号证据 `cef2ecd2670e4737aad5c371a9720adcf36f9095d01e14e9c056e319aa480b9e`。

优先级 2：[Grok Build Responses JSON 草案](next-grok-build-responses-json-draft.json)：`grok-4.5`，route/candidate/endpoint 分别 `p12-12-production-grok-build-route` / `p12-12-production-grok-build-candidate` / `p12-12-production-grok-build-endpoint`，Provider `p12-12-production-grok-build-upstream`。拟用 Client Key 证据 `7b7960db5735b8850100838480b35002ca5e8953467bb04c740ce1a7f5d90127`；可用账号证据 `5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54`。其他两个过期 Build 账号保留，不恢复。实际 lease revision 不从 native row revision 推算。

10月5日[首批前置复核](../cpar-release-readiness-20261005/first-acceptance-preconditions.json)确认Krill唯一新route Key已过期，长期Key只有旧disabled route；Build目标账号／Key在只读快照可用，但actual lease revision仍待部署后Pin。以下10月2日权限结论只作历史：上一轮 03:28:32 CST 的历史[inventory](current-environment-final.json)显示这两个目标池为 available、权限包含目标 route；active inventory 仍为 `kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75` / ETag `rev-1`。有效能力观察七项均没有新来源字段，属于旧构建未知。旧构建与候选不符的[预检](current-runtime-preflight.json)为预期 FAIL / wrong_runtime_version，未使用 `--execute`，发送 0。

拟申请的首批预算：每个上述 JSON 组合 **1 次 protected pin + 7 次正式 basics，共最多 8 次推理**；不自动进入 SSE、其他协议、故障注入或扩展。JSON 成功后，SSE 与其余格按同样冻结/停止规则和批准预算推进。能力 true 所需扩展缺少样本仍 NOT_RUN，不更改声明；basic PASS 不关闭 #57。

## 冻结与正式命令

执行前在主机内通过现有管理认证 GET `/admin/client-keys` 和 `/admin/operations/account-pools` / `/admin/operations/provider-account-pools`，用 `SHA256("cpar-evidence:" + id)` 匹配上述 opaque ID，确认启用、可用及授权 route。原始 ID 只进入操作人私有 pin 输入；不读取或导出 Key secret，也不恢复账号。

授权 pin 的请求为 `POST /admin/operations/channel-pin`，请求头使用当前受保护管理认证、`X-Config-Version` 和当前资源 `If-Match`；body 对应以下闭合字段：

```json
{
  "provider_id": "p12-12-production-krill-upstream",
  "channel_id": "p12-12-production-krill-responses-endpoint",
  "route_id": "route-0b9a2bb7-3f85-487a-bba2-0871bacc46ca",
  "credential_id": "<在主机内匹配上述账号证据后的实际 ID>",
  "requested_model": "gpt-6-sol",
  "protocol": "openai_responses",
  "mode": "json",
  "client_key_id": "<在主机内匹配上述 Client Key 证据后的实际 ID>"
}
```

未知 `credential_revision` 不填 native/管理 revision；pin 的 `execution.identity.credential_revision` 才是冻结来源。管理 ETag 只用于管理请求并发保护，不填入整数 execution config revision。Pin 必须成功、single attempt、fresh、同一 runtime_build 和 Client Key；任何未知/漂移即停止。其 receipt 保存为私有 `channel-pin.json`。

从固定 source80f58389 的 clean checkout 根目录运行。`CPAR_ACCEPTANCE_DRAFT_DIR` 指本报告交付目录中 `docs/reports/assets/cpar-execution-evidence-20261002/` 的绝对路径；这些草案属于报告提交，不能假定它们存在于 candidate checkout。`CPAR_ACCEPTANCE_PRIVATE_DIR` 指向操作人 mode0700 目录，`CPAR_ACCEPTANCE_ARTIFACT_DIR` 指向完整已验证的 remote ARM64/hash542a3c30 集合目录（本地 `output/cpar-signed-state-rehearsal-20261002/linux-arm64/` 或安装 release 的 `artifact/`），`CPAR_ACCEPTANCE_CLIENT_KEY` 由现有安全机制提供且不打印；这些变量不是新的授权。

```sh
python3 scripts/acceptance/cpar-batch-c-live.py --new-build inspect \
  --host new-vps --service cpa-rust-gateway.service --sudo \
  --manifest "$CPAR_ACCEPTANCE_ARTIFACT_DIR/artifact-manifest.json" \
  --out "$CPAR_ACCEPTANCE_PRIVATE_DIR/current-environment.json"

python3 scripts/acceptance/cpar-batch-c-live.py --new-build freeze \
  --plan "$CPAR_ACCEPTANCE_DRAFT_DIR/next-krill-responses-json-draft.json" \
  --runtime "$CPAR_ACCEPTANCE_PRIVATE_DIR/current-environment.json" \
  --pin "$CPAR_ACCEPTANCE_PRIVATE_DIR/channel-pin.json" \
  --out "$CPAR_ACCEPTANCE_PRIVATE_DIR/krill-frozen.json"

python3 scripts/acceptance/cpar-batch-c-live.py --new-build run \
  --plan "$CPAR_ACCEPTANCE_PRIVATE_DIR/krill-frozen.json" \
  --runtime "$CPAR_ACCEPTANCE_PRIVATE_DIR/current-environment.json" \
  --base-url https://cpar.142857142.xyz/v1 \
  --out "$CPAR_ACCEPTANCE_PRIVATE_DIR/krill-preflight.json"

# 已获得具体真实推理授权且预检成功之后才加入 --execute
python3 scripts/acceptance/cpar-batch-c-live.py --new-build run \
  --plan "$CPAR_ACCEPTANCE_PRIVATE_DIR/krill-frozen.json" \
  --runtime "$CPAR_ACCEPTANCE_PRIVATE_DIR/current-environment.json" \
  --base-url https://cpar.142857142.xyz/v1 --execute \
  --out "$CPAR_ACCEPTANCE_PRIVATE_DIR/krill-basics.json"
```

Grok Build 使用对应草案与实际同范围 pin，输出文件改为 `grok-build-*`；不更换冻结账号以重试凑通过。JSON 后 SSE 应单独按 mode=sse 获取 pin、冻结；不能复用 json pin 冒充 sse 验证。正文和私有 wire bundle 留在操作人目录，报告只归集正式 value-free receipts。

公开响应 `model` 为 gateway 解析的 public model（`gateway-http-actix/src/lib.rs` 的 `resolve_public_model` 与 `TimeSource::response_metadata`）；因此草案 `response_model=public_model` 有源码依据，不再从 Provider inventory 猜测。执行记录中的 `upstream_model` 另行精确核对。

Kiro 输出上限、Grok Web 原生多轮及上限继续独立 BLOCKED；保持原有已接受限制。未知实际出口（例如 Web relay onward）仍阻断该组合，不新增例外、不降权限或关闭能力。
