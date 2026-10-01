# new-vps 发布、回退与首批真实验收方案

状态：**READY_FOR_REVIEW / NOT_EXECUTED**。本轮授权包含实现、隔离验证、候选和方案；没有共享环境部署、服务启停、配置发布、账号恢复、权限修改或真实推理授权。以下变更步骤没有执行。签名发布及生产状态副本演练也是开放前置条件。

## 固定对象与当前事实

- 目标：`new-vps`，唯一服务 `cpa-rust-gateway.service`。03:28:32 CST 的[最新只读核验](production-service-final.json)确认 PID 499070，服务 active/running，实际 ELF 和 current 都在 `b0cf36e387b774439acc23830221389eb3ac74d0`；旧二进制 SHA256 `0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b`。
- unit 实际 ExecStart 为 `/opt/cpa-rust-gateway/current/gateway`，以 `cpa-gateway:cpa-gateway` 运行，状态目录 `/var/lib/cpa-rust-gateway`，数据/管理监听 `127.0.0.1:18180` / `127.0.0.1:18181`。Caddy 和这些监听值不在切换修改范围内。
- 当前 `control.sqlite3` schema 31，quick_check=ok，尚无 execution sidecar。aarch64/glibc 2.39，剩余空间约 31.3 GB。只读观察不是一致的可恢复备份。
- 拟替换源码：`80f5838902eb56be6dccc3ce047eb227d6c18e6c`，Rust 1.97.1，`aarch64-unknown-linux-gnu`，应用版本 0.1.0，候选 schema 34。
- 本地不可变候选目录：`output/cpar-execution-evidence-20261002/linux-arm64/80f5838902eb56be6dccc3ce047eb227d6c18e6c/`。二进制 `gateway-aarch64-unknown-linux-gnu` SHA256 `bd2e46ed6ab7174e2cfcdebe88ca7cba8cc1fb0b0ff46e9714469bc900cdadc0`；[manifest](artifact-manifest.json)文件 SHA256 `4cb4eac4e787eba6fe944b31f258f12af380a7179888a3216d75251aa00c90f7`。
- [P12 结构校验](artifact-verify.log)、ELF/嵌入 revision、实际受保护 `/admin/system`、OCI nonroot/read-only/network-none 启动和 410/410 隔离回归 PASS。最大 GLIBC 符号要求 2.34，小于目标 2.39；仍需目标主机动态库预检。
- 本地候选没有 Sigstore 签名/bundle/远程 workflow receipt；`signing-identity.json` 是预期身份声明，不能当成签名证明。[签名前置检查](artifact-signature-precondition.log)确实拒绝了缺少签名的集合。

## 发布前仍需完成的具体准备

1. 取得针对该 SHA 的远程 push / release workflow 授权，使用项目 `.github/workflows/release-artifact.yml` 生成 GitHub OIDC 签名和 receipt，验证 `--require-signature --require-receipt`。本轮未 push、未触发 CI。若远程重建字节与本地候选不同，将其当作另一候选：重新核对 manifest、ELF、源码、系统信息、受影响回归和状态演练，不能沿用本地二进制 hash 的验收。
签名流水线必须 checkout 精确 source80f58389。证据交付提交会成为新的本地 HEAD；不能对其 HEAD 运行流水线却继续称 source80f58389。可申请创建只指向该 SHA 的专用远程 ref，然后调用既有 workflow：

```sh
# 以下两条远程写入/CI触发均未执行，需具体授权
git push origin 80f5838902eb56be6dccc3ce047eb227d6c18e6c:refs/heads/codex/cpar-execution-arm64-20261002
gh workflow run release-artifact.yml --repo Ricardo121380/cpa-rust-gateway --ref codex/cpar-execution-arm64-20261002
```

该 workflow 现有 matrix 同时构建 x86_64 和 ARM64，审批需覆盖该完整 workflow；本任务消费 ARM64 artifact。实际 signing identity 应为该 ref 对应的 workflow URL，替换本地未签名集合的预期身份。按项目原 cosign verify-blob 核对 GitHub OIDC issuer/该精确 workflow ref，再执行 P12 require-signature/receipt。结构校验本身不是密码学验签。

2. 在授权的目标 staging 目录验证目标机 `ldd` 无缺失库及 `--help`，不连接生产状态。用生产数据库/状态的**一致私有副本**在隔离网络和独立端口完成 31→34 演练，核对已有 native Grok 账号、进口回执、加密材料可读性、账本、配置和权限。当前合成迁移覆盖 14 表，`grok_accounts=0`，不能代替生产 native 数据演练。本轮该项 NOT_RUN。
3. 验证完整备份恢复：状态目录、`admin-auth.sqlite3`、控制库及必要密钥目录、unit/drop-in、current 目标和旧 ELF。旧二进制在已恢复 schema 31 副本启动尚 NOT_RUN。校验旧 ELF hash，保留旧 release；仅重新指向旧二进制而留下 schema 34 不可回退。
4. 重新 GET 实际活跃配置/Client Key/权限/账号池/出口与有效能力，重新核对 PID、起始时间、SHA、schema 和空闲空间。当前草案中的管理 inventory 只指定目标，不能证明执行时 revision。若已漂移，重审方案与冻结目标。

## 获得部署授权后的一次有界切换

批准对象应明确为：将以上服务从旧 release 切换到已验证、已签名的候选，允许该服务一次 stop/start、31→34 自动迁移、受保护只读核验及必要的完整状态回退。真实推理预算另行明确；配置发布、权限修改和账号恢复不包含在此范围。

操作顺序：

1. 上传候选到 `/opt/cpa-rust-gateway/staging/80f5838902eb56be6dccc3ce047eb227d6c18e6c/`，按 manifest 核对全部 payload 与签名。文件 owner root，二进制 0755，release 目录不可原地覆盖；存在同名目录且内容不同即停止。拟安装路径 `/opt/cpa-rust-gateway/releases/80f5838902eb56be6dccc3ce047eb227d6c18e6c/gateway`。
2. 记录当前 unit/drop-ins/current/旧 ELF hash、实际进程和 UTC 时间。进入批准的维护窗口，停止新验收流量、等待在途请求自然结束；关闭窗口不能靠修改 Key 或模型声明。只停止 `cpa-rust-gateway.service`；不停止 Caddy 或其他服务。
3. 服务停止并确认 PID 消失后，在主机内 root-only 0700 目录 `/var/backups/cpa-rust-gateway/80f5838902eb56be6dccc3ce047eb227d6c18e6c/<UTC>/` 保存完整离线状态与 keys、unit/drop-ins 和 current 目标。用可恢复方式保留属主/模式/软链；核对 SQLite/WAL 一致性、备份 schema=31、quick_check、关键表行数和内部文件校验。不得只拷贝 live SQLite 主文件漏掉 WAL。
4. 系统 `LoadCredential` 的[六个持久源路径](production-credential-paths-readonly.json)在 `/etc/cpa-rust-gateway/credentials/`，均已只读核对存在/0600；备份在主机内保护保存、不输出内容。不依赖 stop 后消失的 `/run/credentials/cpa-rust-gateway.service`。恢复需同时匹配 master-key、pepper、cache-key 等材料，不能只恢复数据库。
5. 将校验过的候选安装到新 release。通过同一目录上的临时 symlink + `mv -T` 原子替换 `current`，指向新 release，再启动该服务。unit、监听地址、Caddy、账号和活跃配置不修改，因此无需 daemon-reload；若实际 unit 状态不同，先停止并重审，不自行改 unit。
6. 启动迁移后，只读检查数据 `/healthz`、受保护 `/admin/system`、进程 ELF SHA256 和嵌入 SHA、actual current、schema=34、quick_check、监听仍为 loopback、服务身份及权限、effective 声明和原有配置/账号。二进制、manifest、source、target、Rust、version 都必须一致；不得以单一 HTTP200 放行。

可审查的核心命令形态（尚未执行，备份/校验完成后才运行；路径已固定，UTC 备份目录由操作时记录）：

```sh
sudo systemctl stop cpa-rust-gateway.service
# 完整离线备份及 schema 31 恢复检查完成后
sudo ln -s /opt/cpa-rust-gateway/releases/80f5838902eb56be6dccc3ce047eb227d6c18e6c /opt/cpa-rust-gateway/current.execution-evidence-next
sudo mv -T /opt/cpa-rust-gateway/current.execution-evidence-next /opt/cpa-rust-gateway/current
sudo systemctl start cpa-rust-gateway.service
```

该临时 symlink 若已存在则先核对所有权/来源，不盲目 `ln -sf` 或清理它。

## 停止与回退条件

错误 SHA/版本/target；签名未验证；迁移/quick_check 失败；旧数据、配置、权限或账号异常；端口暴露扩大；无法解密或 native 数据不兼容；异常重启；实际 lease/config/egress/capability 未知或矛盾；Usage 与账本 source 不匹配；出现正文/秘密泄漏。任一情况停止后续推理；不靠重试、改配置或关闭能力取得通过。

需要回退时只在明确包含回退的授权范围内执行：停止该服务；先完整保存 schema 34 的失败现场及新增历史/账本，禁止丢弃新记录；恢复经过验证的**整套** pre-schema31 状态和匹配 keys/metadata、属主与模式；再原子恢复 current 到旧 release，启动并验证旧 SHA、schema31、quick_check、健康与旧权限。

若切换后已出现生产写入，不静默丢弃写入以恢复旧库。保存完整新状态、明确账本与请求窗口影响，交由批准的恢复决策处理；计划内验收应在放开普通业务流量前进行，以控制此边界。migration 34 的 down 会移除执行 sidecar；该 SQL 回归不能替代生产完整备份，也不能用它抹掉新证据。

## 首批真实目标和预算草案

优先级 1：[Krill Responses JSON 草案](next-krill-responses-json-draft.json)：`gpt-6-sol`，route `route-0b9a2bb7-3f85-487a-bba2-0871bacc46ca`，candidate `candidate-95857499-eff9-4b29-8bb5-34e55dbabcb0`，endpoint `p12-12-production-krill-responses-endpoint`，Provider `p12-12-production-krill-upstream`，Client Key 证据 `4154a728ad5abcb01d21935755d1a35fe218a906449da23cc220848e9ee0705f`，账号证据 `cef2ecd2670e4737aad5c371a9720adcf36f9095d01e14e9c056e319aa480b9e`。

优先级 2：[Grok Build Responses JSON 草案](next-grok-build-responses-json-draft.json)：`grok-4.5`，route/candidate/endpoint 分别 `p12-12-production-grok-build-route` / `p12-12-production-grok-build-candidate` / `p12-12-production-grok-build-endpoint`，Provider `p12-12-production-grok-build-upstream`。拟用 Client Key 证据 `7b7960db5735b8850100838480b35002ca5e8953467bb04c740ce1a7f5d90127`；可用账号证据 `5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54`。其他两个过期 Build 账号保留，不恢复。实际 lease revision 不从 native row revision 推算。

最新 03:28:32 CST 的[inventory](current-environment-final.json)显示这两个目标池为 available、权限包含目标 route；active inventory 仍为 `kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75` / ETag `rev-1`。有效能力观察七项均没有新来源字段，属于旧构建未知。旧构建与候选不符的[预检](current-runtime-preflight.json)为预期 FAIL / wrong_runtime_version，未使用 `--execute`，发送 0。

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

从最终源码 checkout 根目录运行，`CPAR_ACCEPTANCE_PRIVATE_DIR` 指向操作人 mode0700 目录，`CPAR_ACCEPTANCE_ARTIFACT_DIR` 指向完整已验证候选目录，`CPAR_ACCEPTANCE_CLIENT_KEY` 由现有安全机制提供且不打印；这些变量不是新的授权。

```sh
python3 scripts/acceptance/cpar-batch-c-live.py --new-build inspect \
  --host new-vps --service cpa-rust-gateway.service --sudo \
  --manifest "$CPAR_ACCEPTANCE_ARTIFACT_DIR/artifact-manifest.json" \
  --out "$CPAR_ACCEPTANCE_PRIVATE_DIR/current-environment.json"

python3 scripts/acceptance/cpar-batch-c-live.py --new-build freeze \
  --plan docs/reports/assets/cpar-execution-evidence-20261002/next-krill-responses-json-draft.json \
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
