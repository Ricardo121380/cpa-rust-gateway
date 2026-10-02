# 固定候选远端签名验证与生产状态副本迁移／恢复演练

日期：2026-10-02。结论：**BLOCKED，尚不满足 READY_FOR_DEPLOYMENT**。已实际完成远端两平台签名构建、密码学验签、最终 ARM64 目标兼容与410隔离回归、一致生产状态私有副本、31→34迁移以及旧版本完整状态恢复启动。阻断是升级前就存在、现在生产仍保留的1条归档凭据 AEAD_INVALID_TAG；未豁免、修复或删除它。生产未切换，真实 Provider 调用0。

## 基线、固定对象与证据边界

读取[基线报告](cpar-execution-evidence-and-arm64-candidate-20261002.md)、其候选/验证/迁移记录及[发布方案](assets/cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)，核对当前规则、工单与Git状态。候选 source 固定 `80f5838902eb56be6dccc3ce047eb227d6c18e6c`；本轮开始报告 HEAD `a2c9e0743ab10097499bbef76a8f51db60043c69`。报告提交与发布源码分开。

[先行状态检查](assets/cpar-signed-state-rehearsal-20261002/baseline-check.json)未发现同source有效release运行，远端专用branch不存在，故推送准确source一次、dispatch既有release workflow一次；没有force push。[末次ref](assets/cpar-signed-state-rehearsal-20261002/final-remote-ref.json)仍为source80，[该branch流水线清单](assets/cpar-signed-state-rehearsal-20261002/final-branch-workflows.json)只有本轮release运行。已完成的410回归、迁移与恢复pristine对象经核验后复用，修正操作探针时只复验失败的启动/GET检查。

本轮没有修改产品源码、workflow、production unit/current/config/权限/账号；用户已有AGENTS.md及无关未跟踪内容保留。新增报告、值脱敏回执和已实际执行的操作配方；生产原始数据库、业务正文、钥匙及私有日志仅留在主机root0700任务目录，不进GitHub、不进入外部模型。

## 发布必要条件的实际结果

| 条件 | 结果 | 实际依据 |
| --- | --- | --- |
| 精确ref与远端构建 | PASS | checkout80，原native ARM64/x86_64完整matrix成功，Rust1.97.1 |
| 实际OIDC密码学验签 | PASS | 两平台cosign verify-blob Verified OK；固定issuer/identity；证书source/run/ref匹配 |
| 完整payload/manifest/receipt | PASS | 两平台各9文件、5个manifest payload；全hash/size及GitHubZIP digest一致；require-signature/receipt通过 |
| 最终ARM64目标兼容 | PASS | hash542a3c30，ELF64 LE AArch64；glibc2.39满足2.34；ldd无缺库；实际service UID启动 |
| 最终ARM64关键回归 | PASS | 410/410；223次loopback synthetic provider，real_provider_calls=0 |
| 一致生产私有副本 | PASS | 两库Online Backup，跨捕获窗口data_version/key/backup事件均无变化，持久key匹配LoadCredential；没有停写/停服 |
| 31→34迁移及原始数据 | PASS | 54个旧表的全部原列计数/HMAC保持；完整性/FK通过；新增历史证据继续Unknown |
| 候选真实serve/受保护GET | PASS | source80/hash542/schema34，UID995/GID986、独立网络/挂载/PID，8类受保护读取200 |
| 完整旧状态恢复与旧serve | PASS（恢复步骤、数据与启动） | 另一目录恢复两库、backups、6key、old ELF及部署元数据；两库启动前字节一致；旧hash0b516c31/schema31启动及同8类GET通过 |
| 全部历史加密材料正常 | **FAIL / BLOCKED** | ordinary40/41；nativeGrok5/5；归档1条AEAD_INVALID_TAG在升级前、迁移后、恢复后及当前生产仍存在 |
| 生产服务边界和收尾 | PASS | 同PID499070、old/current/unit/drop-in/key/监听未变；演练进程及端口无残留 |

机器可读判定：[release-readiness.json](assets/cpar-signed-state-rehearsal-20261002/release-readiness.json)。恢复步骤PASS只证明整套旧状态可恢复且可启动；既有历史密文异常没有变成PASS。“旧生产缺少新execution/capability字段”保留为部署后验证项。

## 远端构建、签名及完整制品

[release run36956561884 / attempt1](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36956561884)完整成功；[job/step记录](assets/cpar-signed-state-rehearsal-20261002/workflow-run.json)、[实际checkout/toolchain](assets/cpar-signed-state-rehearsal-20261002/checkout-and-toolchain-excerpts.log)、[workflow日志](assets/cpar-signed-state-rehearsal-20261002/workflow.log)（[仅尾空格规范化及原hash](assets/cpar-signed-state-rehearsal-20261002/public-log-normalization.json)）。ARM64为ubuntu-24.04-arm、x86为ubuntu-22.04；原workflow SHA256 `03ed6991517e65ac210bbddd244997ad2c4937dcc64042963be4037d1eacbf8a`，没有workflow修改。授权副作用为准确branch、构建matrix/cache、GitHub OIDC/Fulcio/Rekor签名及14天artifact上传，没有部署步骤。

issuer `https://token.actions.githubusercontent.com`；精确identity `https://github.com/Ricardo121380/cpa-rust-gateway/.github/workflows/release-artifact.yml@refs/heads/codex/cpar-execution-arm64-20261002`。本地cosign3.1.2实际验证两份bundle及manifest，CI cosign2.6.4也实际签名/验签。[签名核验回执](assets/cpar-signed-state-rehearsal-20261002/signed-verification.json)及[ARM64证书](assets/cpar-signed-state-rehearsal-20261002/linux-arm64-certificate.txt)表明source OID1.3.6.1.4.1.57264.1.3=固定SHA，repo/ref及run/attempt1一致。P12既有验证器require-signature和require-receipt均PASS；identity声明及结构校验没有被当成密码学证明。

[GitHub两份artifact](assets/cpar-signed-state-rehearsal-20261002/github-artifacts.json)实际下载且ZIP digest匹配：ARM64 id11206417516，`cec88d27aa0168a2c56c3c93b927d5cd5e59f80db84c30f0cb0ddb4d39949c76`；x86 id11206397516，`09344d74ee61a92fd05002d58e10cc1ff15d0b392b1b7aefd8f561e41ca202c3`。GitHub保留到2026-10-16约02:48UTC。完整本地集合在 `output/cpar-signed-state-rehearsal-20261002/linux-arm64/` 与 `linux-amd64/`；大binary/OCI/ZIP不进入报告提交，小manifest/SBOM/metadata/identity/signature/bundle/receipt归档在本证据目录。

下面为完整9文件清单，口径是文件字节SHA256，与解析后canonical JSON摘要分开。

### linux-arm64

| 文件 | Bytes | SHA256 |
| --- | ---: | --- |
| artifact-manifest.json | 1016 | `7e6c9b59b3d2715692cd8178662ca8532836c784db5b833c6c4ee1ae0aa81653` |
| artifact-manifest.json.sig | 96 | `d8a59220b5d6940df177c6fb69dcea4c3c4ab377018250494afb584f5c1f7039` |
| artifact-manifest.sigstore.json | 10029 | `e39ac6e5d0098117f89e1c09d7a9aacaf1c8a6043efdf249fac8ded0f46841ce` |
| artifact-receipt.json | 1546 | `6fce27b33ca0e21e1d48c17e393f5a585739de4f7f3f76ffd366cf88d3917ae0` |
| gateway-aarch64-unknown-linux-gnu | 19500320 | `542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87` |
| gateway-build-metadata.json | 261 | `ee8a7c474ba03646444c53dabd24691e050169704668a9521f3db0b824fbd403` |
| gateway-image.oci.tar | 41024512 | `a52cf98495ec60b824c590c2ac0a38fea0136196327d4147fc285a43fad4cefb` |
| gateway-sbom.cdx.json | 209278 | `7dc683785a261ba33f5f66dd34093cf2c87a89211ed4ac711b53b9266b734947` |
| signing-identity.json | 304 | `02b3b040d755b6ca69192aabcc3193c62c9019de16e3cf1c18026eb7f8ac6605` |

### linux-amd64

| 文件 | Bytes | SHA256 |
| --- | ---: | --- |
| artifact-manifest.json | 1014 | `ba43074a1ac94e5ee8b36b61e401872c4dd7a55adaf15e64cd68cbe5211a3c7c` |
| artifact-manifest.json.sig | 96 | `e5f902c32c7c2c3622b599d1b1761a1027346146b04dbc1c6d1b94f3d95e02a3` |
| artifact-manifest.sigstore.json | 10029 | `f7c785dc2bbe895329fb3116cf96c4262512053fea227358988ef730f1913689` |
| artifact-receipt.json | 1545 | `69a67b4744e6c6af27b35f656ad1c2c083ea7b4209c775bb29b7abd7cdde8f2c` |
| gateway-build-metadata.json | 259 | `0773bb509eef2ebea6ed7d0230ef998cb93978b733ead7a26d24858ee2b540d3` |
| gateway-image.oci.tar | 41806848 | `66af1a98a53140706966501e1ced2f24c4dd2b07c42fe69c8783fea99bc9daa3` |
| gateway-sbom.cdx.json | 209276 | `5dfdb7d777708fd3e127b2f4925ff1d1aea53d20cabab318d8e43b31c3536cc3` |
| gateway-x86_64-unknown-linux-gnu | 22762776 | `d2ffc57b95690c841167cd5a7150b8570cbaf51266d24a88f29e7fb165091790` |
| signing-identity.json | 304 | `02b3b040d755b6ca69192aabcc3193c62c9019de16e3cf1c18026eb7f8ac6605` |

最终ARM64 binary为 `542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87`，与同source本地unsigned的 `bd2e46ed6ab7174e2cfcdebe88ca7cba8cc1fb0b0ff46e9714469bc900cdadc0` 不同。未声称跨环境可逐字节重建，也未继承unsigned二进制的验收；最终对象在new-vps重新完成下面全部检查。

## 最终ARM64目标兼容及回归

[目标兼容](assets/cpar-signed-state-rehearsal-20261002/target-compatibility.json)：ELF machine183、ELF64 little endian，嵌入source/Rust/target精确匹配；`libstdc++`、`libgcc_s`、`libm`、`libc`和aarch64 loader全部解析，max GLIBC2.34≤target2.39。已隔离运行help，候选实际受保护 `/admin/system` 又核对version0.1.0/Rust1.97.1/source80/target/schema34。

[410完整回执](assets/cpar-signed-state-rehearsal-20261002/native-http/receipt.json)及[汇总](assets/cpar-signed-state-rehearsal-20261002/signed-binary-regression.json)：实际签名ELF、SQLite、loopback TLS及真实SOCKS fixture；三协议JSON/SSE、Request/Attempt/Usage/ledger来源、错误/取消/超时/重试、材料/config/node变动、在途隔离、protected pin和正式collector链均通过。223次仅到private network内synthetic Provider，真实调用0。[固定源码配方输入](assets/cpar-signed-state-rehearsal-20261002/target-inputs.json)记录真实commit/root-tree对象及4个验收脚本hash；目标只传输所需脚本，不冒充完整remote checkout，CI的完整checkout证据独立存在。

## 一致生产副本的具体依据

[只读前核验](assets/cpar-signed-state-rehearsal-20261002/production-before.json)：服务active/running、UID995/GID986、PID499070；进程和current均旧b0cf36e3/hash0b516c31；schema31控制库为WAL，admin-auth为delete/user_version1；两库完整性/FK无错误；六把持久钥匙均0600并与systemd进程内已加载材料相同。空间约31GB。

任务根 `/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884` 为root0700。[capture-2回执](assets/cpar-signed-state-rehearsal-20261002/backup-receipt.json)给出已实际成功窗口02:47:53.274953–02:47:53.398746 UTC，0.123784秒。两库通过受支持的SQLite Online Backup API从各自read transaction取得一致数据库（涵盖已提交WAL）；各自独立readonly observer的同连接data_version在整个窗口保持control2→2/admin1→1。六key的stat/hash保持、inotify未见key或备份档案写入/重命名/权限事件，持久/已加载key匹配，生产进程未变。

这组额外稳定窗口检查使两库与key可对应同一已提交状态；不是把两次独立backup称为天然跨库原子。SQLite对在线备份的一致快照保证及data_version的同连接比较语义见[官方backup说明](https://www.sqlite.org/backup.html)与[PRAGMA data_version](https://www.sqlite.org/pragma.html#pragma_data_version)。[合成守卫探针](assets/cpar-signed-state-rehearsal-20261002/sqlite-consistency-guard-probe.json)实际确认另一个连接的commit能被observer发现。若窗口有变化则保留失败副本并拒绝放行，未以live主文件copy替代WAL备份。生产未暂停写入、未停服。

私有副本覆盖控制库、认证库、必要backups、6持久key、old ELF、unit/drop-in合成文本、old current及恢复ownership/mode元数据。历史archive软链不跟随复制，避免读取/写入其生产目标；[31项archive元数据/bytes校验](assets/cpar-signed-state-rehearsal-20261002/complete-private-restore-audit.json)为21file、8directory、2symlink，bytes/软链目标与原档案一致，原owner/mode在副本中已恢复。DB文件hash和54个表的计数及HMAC在回执；HMAC随机key和所有原始行只留主机，不公开低熵业务值的逐行摘要。

## 迁移、原数据与Unknown

[实际迁移回执](assets/cpar-signed-state-rehearsal-20261002/migration-receipt.json)绑定source80/hash542。在独立mount/net/PID/IPC/UTS中，用原服务UID通过候选真实store-open迁移path执行31→34。CLI audit在迁移完成后用其空Catalog/capability view重建populated图时报 `missing_endpoint_capability_profile`，exit1和原日志留私有目录；仅store迁移标PASS。随后候选实际serve和全部受保护读取通过，未把CLI错误吞掉当作整个命令成功。

迁移前后53个控制旧表+1个认证表的全部原列，经每行canonical类型编码、长度分隔、排序后HMAC-SHA256逐表比较，计数与HMAC一致；schema_migrations对原31行比较，新增版本32–34单独核对。两库integrity_check=ok、foreign_key_check=0。重点计数如下；完整逐表结果见回执。

| 状态/关联 | 升级前 = 升级后原数据 |
| --- | ---: |
| config_versions | 38 |
| upstreams | 59 |
| upstream_endpoints | 75 |
| upstream_credentials | 41 |
| endpoint_credential_bindings | 56 |
| access_groups | 38 |
| access_group_routes | 97 |
| client_keys | 38 |
| model_routes | 88 |
| route_candidates | 88 |
| 原生Grok账号 | 5 |
| 历史native导入batch | 9 |
| 历史event | 2553 |
| 历史request recording | 66 |
| ledger | 728 |
| admin_account | 1 |

原event2553包含Request926、Attempt789、Usage739、terminal99；ledger728。原config/权限/路由/禁用意图/绑定、native账号关联、历史receipt字段、Usage和账本source全部原列保持。grok_account_links为空这一实际数据没有伪造关联。新增gateway_attempt_execution=0，native_account_import_receipts=0；旧账本的usage evidence全为provenance/input_accounting Unknown。旧batch updated_count的新DDL默认0只作为存储默认，不证明历史实际更新数为0，不创造新证据。

[密文审计](assets/cpar-signed-state-rehearsal-20261002/crypto-before.json)使用准确source AAD、匹配master-key和实际XChaCha20-Poly1305认证；公开HChaCha vector自校验通过，已安装cryptography/OpenSSL执行AEAD。Native Build按实际binary persisted格式解析，其余native材料按相应格式验证；过期/reauth状态保持，不发起恢复或续期。普通凭据40成功/1失败，5个native全通过。失败为archived配置中的1份（当前active3/3、draft6/6均成功），明确AEAD_INVALID_TAG；[old/new密钥与AAD实现](assets/cpar-signed-state-rehearsal-20261002/legacy-crypto-source-comparison.json)完全相同。迁移后及恢复后结果相同，证明异常早于升级、不是迁移新增损坏；它仍是发布所需“全部历史材料正常”的FAIL。

## 完整恢复与实际启动隔离

`restored-pristine/`从capture-2在另一目录恢复整个必要状态和匹配key、old binary、unit/current/ownership依赖，两库启动前文件SHA256与捕获时相同，schema31、全部原列计数/HMAC及integrity/FK通过。[完整恢复回执](assets/cpar-signed-state-rehearsal-20261002/restore-receipt.json)和[old启动回执](assets/cpar-signed-state-rehearsal-20261002/old-restored-startup-receipt.json)绑定已核实old hash0b516c31、sourceb0、schema31；实际启动成功，不是仅SQL down或旧symlink。

新旧分别在pristine状态的派生启动目录执行；[候选启动](assets/cpar-signed-state-rehearsal-20261002/candidate-startup-receipt.json)与old启动均返回healthz200和8类受保护GET200（system、38 config versions、Client Keys、Access Groups、5 active routes、5 native accounts、4 operational pools、9 provider pools）。active identity及管理rev-N投影与私有DB核对，明确不把management ETag推算为execution revision。

实际隔离由root先创建私有namespace并使mount propagation private，mount只含task副本/keys/公共运行库，然后chroot并以真实UID995/GID986、CapEff=0、CapBnd=0、NoNewPrivs运行。独立loopback28180/28181，无external route，保留地址外部连接被拒绝；生产state/credentials路径在进程根中不存在。续期/模型发现线程仍存在，但任何真实外发均被内核阻断；只在startup派生副本观察到credential_refresh_state变化。pristine迁移后状态、恢复状态及所有失败现场保留，新旧测试进程正常结束。

[末次只读生产核验](assets/cpar-signed-state-rehearsal-20261002/production-after.json)及[最终私有审计](assets/cpar-signed-state-rehearsal-20261002/final-private-audit.json)确认old PID/起始/current/hash/unit/drop-in/key/监听未变，无task进程/端口残留。活跃生产未停服，期间[逻辑比较](assets/cpar-signed-state-rehearsal-20261002/production-logical-comparison.json)观测refresh state、管理审计及1份upstream credential的ciphertext/revision变化；其他原列（配置、权限、路由、native、历史请求/Usage/ledger等）仍匹配capture。[漂移评估](assets/cpar-signed-state-rehearsal-20261002/production-state-drift-assessment.json)只输出列名/数量，不输出原值，并确认失败归档密文仍与snapshot相同。不能将仍在运行的生产库称为字节冻结，也不能拿本轮snapshot替代切换时新备份。

## 失败保留、验证及交付

[操作配方/失败索引](assets/cpar-signed-state-rehearsal-20261002/README.md)记录staging父目录、archive软链、首轮大包传输超时、native存储格式、公共OpenSSL配置、UID映射/loopback权限、unsupported bwrap选项、GET缺少config header及management rev-N断言等准备错误。失败目录/回执保留；修正的是本任务操作配方，没有改candidate/workflow、跳过签名或改source凑通过。410成功回归不重复，成功迁移/恢复对象复用；失败启动探针在新派生副本上复验，最新结果PASS。

固定source80的既有[Full45/45及两轴评审](cpar-execution-evidence-and-arm64-candidate-20261002.md)按原scope继续有效；它不是远端release workflow全量测试。产品源码和workflow没有变化，本轮交付执行docs静态门槛、JSON/Python配方语法、证据source/hash/状态交叉检查、秘密扫描及staged diff核验；具体结果见[交付静态记录](assets/cpar-signed-state-rehearsal-20261002/delivery-static-receipt.json)和[docs检查](assets/cpar-signed-state-rehearsal-20261002/delivery-docs-check.md)。不重复Rust Full或真实Provider调用以替代本轮验证。

## 具体部署与首批验收申请

已更新原[发布/回退/首批方案](assets/cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)，固定new-vps/cpa-rust-gateway.service、old source/hash/schema31、final source80/hash542/schema34及拟安装 `/opt/cpa-rust-gateway/releases/80f5838902eb56be6dccc3ce047eb227d6c18e6c-542a3c30aae7/`。完整已验9文件位于task的 `artifact-542a3c30aae7/`，此前partial `artifact/`禁止采用。

当前不执行切换。先对私有失败定位作另行授权的诊断/恢复并通过全部材料门槛；随后申请该具体服务的有界stop/start、current原子切换、自动迁移与受保护只读核验，失败时保留迁移后现场并整套恢复最新pre-upgrade state/key/old release再启动。切换前须新inventory、一致新副本/旧启动复验及停服后的完整离线备份；不改unit/Caddy/账号/权限策略。停机时间本轮未实测，维护影响明确为服务停止到新/旧启动核验完成的窗口。

首批顺序为Krill gpt-6-sol Responses JSON，随后Grok Build grok-4.5 Responses JSON。每格拟1次protected pin+7次formal basics，最多8次真实推理，预算尚未授权；fresh actual lease/config/egress/scoped capability从部署后pin/GET获取。当前旧构建缺这些字段不是签名前/切换前真实请求门槛。JSON之后SSE/其他协议/声明扩展按独立冻结和预算推进。Kiro cap、Grok Web native multi-turn/cap、unknown relay onward仍单列，不新增能力例外、不修改冻结C标准。

## #57 本地归集与剩余事项

[GitHub #57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57)只读核对为OPEN，[本轮归集](assets/cpar-signed-state-rehearsal-20261002/issue-57-evidence.md)与[结构化状态](assets/cpar-signed-state-rehearsal-20261002/issue-57-evidence.json)收集了本轮实际远端签名/目标/状态演练证据。没有GitHub评论或关单，C/#57/父#9均未关闭。

剩余：归档材料阻断关闭并重验；fresh生产备份/明确切换授权；实际新版本部署核验；真实pin/能力冻结；三协议JSON/SSE、声明扩展及受控边界；Kiro/Grok Web独立缺口；按各工单收口。**BLOCKED是本候选当前部署就绪判定；本轮实际验证已完成，不代表部署、真实渠道验收或项目完成。**
