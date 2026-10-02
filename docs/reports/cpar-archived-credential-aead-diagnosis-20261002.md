# 单条归档凭据 AEAD 上下文诊断、隔离修复与发布影响

后续进展：[用户授权后的 AC-01 生产修复与 fresh-copy 复验](cpar-archived-credential-production-repair-20261002.md)已经 PASS，历史材料阻断解除；总体仍等待发布时核对、完整备份及独立部署授权。下文保留 `a4034b12` 诊断交付时的“生产未修复”状态与原始证据。

日期：2026-10-02。**诊断与隔离方案验证 PASS；生产发布仍 BLOCKED。** 脱敏对象 `AC-01` 的密文完整、可在其直接父配置的加密上下文中认证，但与当前子配置的 AAD 不匹配。旧 ELF 与签名候选 ELF 都实际复现此异常；在新副本中仅重新封装该行 `ciphertext` 后，两版原生读取、导出和配置验证均通过。生产未修复，未部署、未真实推理，C／#57／#9 未关闭。

## 基线与本轮边界

起始 HEAD 为报告提交 `6bd5ea4a`；后续本地 log 与 [#57 实时只读回执](assets/cpar-archived-credential-diagnosis-20261002/issue-57-live.json)没有新的实施进展。读取[上一轮主报告](cpar-signed-artifact-and-production-state-rehearsal-20261002.md)、[原发布判定](assets/cpar-signed-state-rehearsal-20261002/release-readiness.json)、密文审计、漂移、旧新源码、捕获／迁移／完整恢复记录与[既有发布方案](assets/cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)。遵循实际 AGENTS、领域 ADR 与凭据边界；保留已有 AGENTS 修改及无关未跟踪内容。

候选源码仍为 `80f5838902eb56be6dccc3ce047eb227d6c18e6c`，签名 ARM64 ELF SHA256 仍为 `542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87`；旧源码 `b0cf36e387b774439acc23830221389eb3ac74d0`、旧 ELF SHA256 `0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b`。实际读字节核对两个 ELF，再用原物执行，不构建、重签、dispatch 或推送新候选。

所有原数据库、密钥、定位、密文及瞬时解密材料仅在 new-vps 的 root0700 任务目录处理。检索从原私有 locator 开始，只扩展到同 Credential ID 的 9 个版本记录、直接父记录、精确关联、捕获集合内备份，以及目标／父记录的当前生产只读核对。14 个保留 `.sqlite3` 候选先查最小格式和精确身份；不跟随 archive 软链。只对有意义的最早父版本副本、最早含目标的副本进一步认证；未全面解密历史备份。

本轮只新增报告及操作配方，产品源码、前端、schema、workflow 均未改变。两个公开源码探子未在有界等待内交终态，收紧后中断，过程结果未作为结论依据；关键源码阅读、所有私有操作和最终验证由主代理完成。

## 结论、可信程度与已排除项

[精确记录检查](assets/cpar-archived-credential-diagnosis-20261002/inspection.json)确认：配置 lifecycle=`archived`，凭据 status=`active`、kind=`oauth_json`、credential revision=2、config revision=0。不是“停用凭据”。封装为 format1、4166 bytes、24-byte nonce、16-byte tag、key_version1，结构有效。

根因的**可证部分，可信程度 HIGH**：密文绑定了直接父配置的 AAD，却被存放在子配置记录中。父子 Credential ID、Upstream ID、kind、status、revision、key_version 相同，完整密文逐字节相同，且真实 parent_id 相连。该密文在父 AAD 下 PASS、子 AAD 下 AEAD_INVALID_TAG；其他 7 个同身份上下文都不能打开这份密文。当前 AAD 是 domain `cpa-rust-gateway/control-plane/credential-aad/v1` 后依次追加 UTF-8 配置 ID、Credential ID、Upstream ID，每段有 32-bit big-endian 字节长度。

**强支持的形成机制**是：复制父配置凭据时保留了原密文，未按新 config_version_id 重新封装。无法确定执行者、具体 SQL／导入脚本或首次异常写入的准确时刻；没有目标的创建资源审计、归档转换审计或原始操作日志。把这部分留为来源追溯缺口，不把“复制时漏封装”的操作过程写成已捕获事实。

已排除：探针误报、普通／native 格式混用、密文截断／未知版本、nonce/tag 切片错误、当前全局 Master Key 错误、把归档与停用混淆、错误的父子／Upstream 关联，以及本轮候选迁移新增破坏。父 AAD 下能认证完整密文，说明该封装在其原上下文中保持认证完整性。[独立原生对照](assets/cpar-archived-credential-diagnosis-20261002/native-cross-validation.json)进一步排除只有 Python 构造／键映射能复现的情况。

[源码比较](assets/cpar-archived-credential-diagnosis-20261002/native-source-comparison.json)实际核对旧／候选：`secret_store.rs`、`control_plane_service.rs`、`credential_pool_compiler.rs` 完整文件相同，`open_credential_for_export` 函数相同。相关真实读取结果也一致，没有发现候选扩大该异常影响。未把全部产品行为都推断成相同。

## 时间与来源证据

所有下表时间按用户时区 CST；JSON 原回执保留 UTC。

| 事件／保留来源 | 时间与已知事实 | 证据强度／限制 |
| --- | --- | --- |
| 直接父配置和凭据创建审计 | 2026-08-03 16:10:22 | 版本和身份来源可追溯 |
| 更早正常的父记录版本 | 备份文件 mtime 2026-08-05 20:56:53；kind=bearer、revision0、key_version1，可认证 | 不是当前 OAuth revision2 密文；禁止以它直接覆盖目标 |
| 父记录 OAuth 变化 | 2026-08-09 22:17:18 与 22:20:25，两个 `credential_oauth_rotated` 审计 | 是 OAuth 凭据材料变化，不能称为 Master Key 轮换 |
| 最近可追溯正常读取迹象 | 父记录 `credential_exported` 审计 2026-08-09 22:26:15、22:26:40、22:27:11 | 导出边界在认证后写审计；该审计没有逐次密文 hash |
| 子配置 AC-01 创建／发布 | 创建 2026-08-09 22:43:33，发布审计 23:00:50 | 有父关系及发布记录，没有可追溯创建命令／归档转换审计 |
| 最早保留的同密文异常副本 | 备份文件 mtime 2026-08-31 21:31:07；schema13，目标与当前 capture 密文相同；目标 AAD 失败、父 AAD 成功；父记录也逐字节匹配 | mtime 是保留文件的时间元数据，不能作为签名的首次损坏时间 |
| capture／迁移／原完整恢复 | 2026-10-02，schema31→34 及恢复31 | 三份 pristine 的同一失败密文及全部54张原表均保持 |
| 本轮当前生产只读核对 | 2026-10-02 17:15:51 | 目标与父记录全部列均与 capture 相同，6把持久／已加载／捕获 key 匹配 |

来源：[有限备份清单](assets/cpar-archived-credential-diagnosis-20261002/history-inventory.json)、[历史及当前核对](assets/cpar-archived-credential-diagnosis-20261002/history-and-live-check.json)、[精确历史审计](assets/cpar-archived-credential-diagnosis-20261002/targeted-history-attribution.json)。原来“不知道能否恢复”已经收敛为：**当前一致捕获集合中直接父记录是匹配身份、版本、材料、密钥与加密上下文的可信恢复来源，8月31日保留副本提供独立同材料佐证**。若需要补全责任／准确首发时刻，最小输入是 8月9日22:20–23:01 CST 对此父／子配置的原始创建、复制或 SQL 操作日志；不影响已验证恢复方案的选择。

## 脱敏影响与原生验证

当前生产关联：目标有 1 个完全启用的 Endpoint/Credential/Upstream 绑定、关联 3 个历史 Route Candidate；当前 active 配置不是目标，也没有同 Credential ID；目标不是当前一阶 rollback predecessor；没有该目标 refresh state；native Grok links 表为0。同 Credential ID 有146条历史 Attempt，但 Credential ID 是版本内身份，旧记录缺精确配置执行证据，不能把这146条归因于 AC-01。

这使当前 active 路由和账号池不会选中 AC-01，现有启动和活动配置保持正常。历史异常仍可通过显式归档材料操作触达；列表／概要成功不代表材料可认证。

每次 native 试验使用新派生状态／6 key 副本、旧或签名原 ELF；复用上一轮 namespace runner，独立 mount/net/PID/IPC/UTS、chroot，服务 UID995/GID986、CapEff/CapBnd0、NoNewPrivs。生产状态与宿主 credentials 路径不可见，只有独立 loopback，无外部路由，保留地址连接被拒绝。后台续期／发现线程可以运行，但真实外发受内核阻断。导出正文只在本机进程内瞬时比较，未落盘、未进入工具输出或外部模型。真实 Provider 调用0。

| 实际受保护操作 | 原始副本：旧／候选 | 修复副本：旧／候选 | 影响判断 |
| --- | --- | --- | --- |
| 配置详情、Credential 概要 | 200 / 200 | 200 / 200 | 只读元数据不能证明 Secret 可读 |
| 目标 Credential metadata | 500 / 500，`management_internal_error` | 200 / 200 | 原生精确 AAD 读取确实失败；修复后恢复 |
| 目标 one-time export | 500 / 500 | 200 / 200 | 修复后输出与父记录完全相同；正文不保留 |
| 父记录 metadata／export | 200 / 200 | 200 / 200 | 独立读取成功的相同身份来源 |
| 归档配置 validate | 503 / 503，`management_lifecycle_unavailable` | 200 / 200 | 本配置完整运行材料准备受异常阻断；修复消除此失败 |
| 直接 fork 归档配置 | 409 / 409，revision conflict | 409 / 409 | 原生 fork 仅接受 active source；没有绕过归档保护 |
| 直接 publish 归档配置 | 409 / 409，lifecycle conflict | 409 / 409 | 归档版本不能直接发布；修复没有放宽生命周期权限 |

四组 native 运行、每组9个请求，共36个操作均取得预期结果；这不是36次推理。具体源码边界：`control_plane_service.rs:211` 构造 AAD；`secret_store.rs:392` 验证格式、选择版本、认证；`management_mutation_service.rs:1160` 按精确记录身份打开材料；`management_resources.rs:7048/7110` 是 export／metadata；`configuration_edit.rs:73` 只接受 active source，mutation 的 `configuration_edit.rs:16` 正常 fork 会逐条打开并用新 AAD 重新封装。Credential inventory 的身份投影在解密失败时使用未知显示投影，不能被当成认证成功。

整库备份／恢复保持原密文，不修正行 AAD。原失败包恢复启动仍成功，而上述历史读取仍失败，说明整套启动 PASS 和历史材料 PASS 是两个条件。若将 AC-01 作为未来历史激活／运行材料准备对象，必须重新校验；当前一阶回滚不是它。没有执行历史配置激活、账号重新授权或真实 token 有效性检验。

## 隔离最小修复、迁移与回退

[修复回执](assets/cpar-archived-credential-diagnosis-20261002/repair-receipt.json)：从 capture-2 创建新的 `repaired-pristine/`。核对目标／直接父的身份、parent、kind、状态、revision、key_version及密文字节；认证父材料后生成全新24-byte nonce，用**目标自身完整 AAD**重新封装同一明文。只有 `upstream_credentials.ciphertext` 的1行1列变化。未填空、替换账号、删除记录、改变状态或伪造认证结果。

其他53张原表全部原列 count/HMAC 保持，凭据表除目标 ciphertext 外所有值和行数保持；admin-auth 库字节相同，6把 key 字节相同，config lifecycle/revision、credential kind/status/revision和关联保持。两库 integrity/FK 通过。AEAD 审计变为 ordinary41/41、archived32/32、active3/3、draft6/6、nativeGrok5/5。

[定向迁移／恢复及 CAS 回执](assets/cpar-archived-credential-diagnosis-20261002/migration-and-rollback-receipt.json)：另一新副本通过候选真实 store-open 执行31→34，54张旧表原列相对于**修复基线**全保持，46份材料全通过。CLI audit 后续仍因空 capability view 报 `missing_endpoint_capability_profile`、exit1；只将 store migration 判 PASS，不把整个 CLI 判成功。独立的 repaired-candidate serve／validate 使用真实持久视图通过。

旧 ELF 用新的完整 state/key 派生副本实际启动，旧 schema31及目标材料读取／导出／配置验证通过。另保留新的 `repaired-restore-package/`，两库在启动前与 repaired-pristine 逐字节相同，包含6 key、backups、old ELF、unit/current依赖。此包只做静态包核对；它自身没有单独再启动，实际旧启动证据明确来自 `repaired-old/` 的相同状态/key派生副本，原完整恢复模型继续复用上一轮证据。

在另一个 `repair-cas-roundtrip/` 中，只回写保存的原 ciphertext 就恢复了54张表原始逻辑基线；重新应用修复 ciphertext 又恢复54张表修复基线；用过期预期密文再次写入命中0行并 rollback，所有数据保持。原始失败行、capture-2、migration-entry-probe、restored-pristine及原 key 全部保留，原 capture 控制库文件 SHA256 也未变。**副本修复 PASS 不是生产修复 PASS。**

## 发布门槛与明确处置建议

原[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57)要求“受影响既有账号、停用意图、权限、连接、自定义路由、Usage、账本与历史兼容；必要迁移有验证和回退说明”，并要求未部署保留 NOT_RUN、不得擅自修改父规格范围。原[release-readiness](assets/cpar-signed-state-rehearsal-20261002/release-readiness.json)将 `all_historical_credential_envelopes_readable=FAIL` 列为停止条件；[发布方案](assets/cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)要求“resolve and reverify archived credential readability”。

本次证据区分：升级新增损坏已排除；已有归档 AAD 异常已确认；本次签名候选的迁移与同路径异常处理没有新增损失。但生产 AC-01 的 metadata／export／validate 影响仍存在，门槛没有被满足。故新[发布判定](assets/cpar-archived-credential-diagnosis-20261002/release-readiness.json)仍 **BLOCKED / PRODUCTION_REPAIR_NOT_EXECUTED**。

推荐路径是申请对 AC-01 进行单字段生产重新封装，然后复核该材料门槛。具体对象、前置、单事务修改、验证、停止条件与单字段回退见[生产最小处置申请](assets/cpar-archived-credential-diagnosis-20261002/production-repair-proposal.md)，方案已在副本验证，尚未实施。它不需要账号重新授权、Master Key 轮换或产品源码变更，也不包含部署许可。

如果用户选择暂缓修复，保留 BLOCKED 和原异常记录即可。如果希望调整发布门槛，必须明确接受上述历史 metadata／export／validate 的已知损失，并由用户决定；本轮没有推荐或自动应用豁免。删除、填空、更改停用／成功状态会损失历史完整性且不能证明恢复，未作为通过方案。

复用原签名、完整制品、410隔离回归、source80 Full45/45与两轴评审证据；仅补充受本单条处置影响的 native／AEAD／数据／迁移／CAS检查。修复若获批并完成，发布前仍需 fresh inventory、一致新备份、切换时完整离线 pre-upgrade 状态及明确部署授权。真实渠道预算、pin、C各组合及 #57 总体验收继续独立。

## 交付与下一项工程工作

[证据与配方索引](assets/cpar-archived-credential-diagnosis-20261002/README.md)、[#57 本地归集](assets/cpar-archived-credential-diagnosis-20261002/issue-57-evidence.md)、[结构化归集](assets/cpar-archived-credential-diagnosis-20261002/issue-57-evidence.json)。公共产物经 Python/JSON语法、证据语义／链接、docs门槛、secret scan及限定 diff 检查；[交付回执](assets/cpar-archived-credential-diagnosis-20261002/delivery-static-receipt.json)绑定实际结果。产品未变，不重跑完整 Rust build/Full 或签名演练。

本轮完成标准已经达到：异常可靠定界、历史和当前影响有证据、可信来源与最小修复经过独立原生和副本回退验证，剩余是明确的生产单字段处置授权。下一项为**获批后执行 AC-01 生产重新封装与 fresh-copy 复验**，完成后再判断材料门槛；不是直接切换、真实推理或关单。
