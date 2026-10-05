# new-vps 受控切换与完整回退操作配方（2026-10-05）

**当前执行门槛 BLOCKED。本文件与操作附件没有执行。** 准入前须先处理 [A／B 审查发现](review-results.md)，冻结修复后的唯一候选并补齐其必要检查。以下配方绑定当前 source80/hash542；修复产生新 source／制品时，必须同步变更所有 pin、hash、安装目录、恢复 helper 常量及相关验证，不能用当前 Full／签名覆盖新源码。

这是 [原发布与首批验收方案](../cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md) 的当前详细操作补充。审批前事实与维护窗口操作分开。本轮没有停服以取“最新离线备份”，既有 capture-2／capture-post 是历史演练材料。

## 唯一目标、影响与职责

目标主机只有 new-vps，目标应用服务只有 cpa-rust-gateway.service。执行身份 root；服务身份 cpa-gateway:cpa-gateway / UID995/GID986。

| 对象 | 固定值 |
| --- | --- |
| 旧源码／schema | b0cf36e387b774439acc23830221389eb3ac74d0 / 31 |
| 旧 release | /opt/cpa-rust-gateway/releases/b0cf36e387b774439acc23830221389eb3ac74d0 |
| 旧 ELF SHA256 | 0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b |
| 新源码／schema | 80f5838902eb56be6dccc3ce047eb227d6c18e6c / 34 |
| 新 ELF SHA256 | 542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87 |
| 新 release | /opt/cpa-rust-gateway/releases/80f5838902eb56be6dccc3ce047eb227d6c18e6c-542a3c30aae7 |
| 完整 ARM64 签名集合 | /opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884/artifact-542a3c30aae7 |
| 数据／管理监听 | 127.0.0.1:18180 / 127.0.0.1:18181 |
| 控制状态／认证库／归档 | /var/lib/cpa-rust-gateway 整个目录，含 control.sqlite3、admin-auth.sqlite3、WAL／SHM／journal、backups |
| 持久密钥 | /etc/cpa-rust-gateway/credentials 整个目录；LoadCredential 使用的六个名称见下文 |
| 窗口备份根 | /var/backups/cpa-rust-gateway/80f5838902eb56be6dccc3ce047eb227d6c18e6c-542a3c30aae7/UTC |
| 操作工具 | [approved-window-operations.py](approved-window-operations.py)；[恢复依赖 hash](recovery-tool-hashes.json) |
| 临时数据准入表 | nft inet cpar_release_80f5838，只限制本机非 root 进程访问 127.0.0.1:18180 |

维护会暂时中断业务入口与管理入口；数据准入门控持续到批准的开放或回退时点。Caddy 可能对维护期间的新数据请求返回502，客户端不得自动重发以凑通过。没有测量此次1.37GiB状态复制、校验及新鲜恢复的停机耗时，不能给固定停机秒数。批准时明确维护开始、截止、成功后的开放范围；超出截止或验证未完成即按有界回退处理。

本方案新增的 nft 临时门控属于未来部署授权必须明确覆盖的共享主机状态变化，**当前授权没有执行它**。不改 unit／drop-in／Caddy 配置，不停其他服务，不发布配置，不更改账号或权限。Caddy 当前实际为非 root 的 caddy 身份，nft 已安装；操作人 UID0 可访问 healthz 和管理 GET，不能借这个通道发起未授权推理。

## 审批前已完成的独立准备

- 当前进程／current／旧 hash／schema31／unit／密钥／AC-01／权限／账号／出口只读核对：[生产摘要](production-readonly.json)、[完整脱敏复核](host-readonly-audit-reconciled.json)、[库存](current-environment.json)。
- 两平台签名集合、manifest、receipt、payload 和 GitHub ZIP digest 一致：[制品复核](artifact-verification.json)。密码学 PASS 复用完全相同字节的10月2日 cosign 结果；未重签。
- 旧／候选完整状态恢复、31→34原54表旧列保持、旧新服务实际私有启动的既有演练仍有效；新鲜窗口备份须重做相同检查。
- 恢复工具四个文件与既有演练完全同 hash，[现场依赖](recovery-and-admission-dependencies.json) PASS。原 helper 使用真实 UID995/GID986、独立 mount/net/PID/IPC/UTS、零外部路由，秘密和日志留主机内。
- 当前完整 state 1,436,528KiB、旧 release18,664KiB，可用约28.3GiB。空间预算采用至少8份完整state加旧制品和2GiB余量，当前约需13GiB；未来按实际增长重算。预复制、恢复、候选验证、失败快照和 displaced-live 都不删除。备份父目录0755只容纳 root0700窗口子目录。
- 配方已完成 AST／内容／链接／脱敏检查；capture／verify／preserve／restore 新窗口实际运行均 **NOT_RUN**，必须在获准维护窗口内执行。不能把静态检查当成新鲜备份恢复成功。

## 停服前停止条件与在途处理

1. 13个P2源代码发现未修复、B文档审查未收口、相关本地矩阵／规格冲突未处理，或新候选身份没有重新绑定：停止。
2. 旧进程/hash/current/schema、unit、六密钥、AC-01、活跃配置／账号／权限／路线、制品及工具不符：冻结新事实并重审，停止。
3. 空间不足，其他 state／key 持有者不明，签名或恢复依赖缺失，无法保证所有业务与管理写入者暂停：停止。
4. 由维护负责人安排客户端暂停新请求；本任务没有通知其他人。只读 GET 检查所有普通／native pools 的 active_leases=0、无待处理恢复／执行、无数据监听 established 连接；未知或仍有在途则继续等待或退出窗口，不强杀在途推理。
5. 正常 systemctl stop 发送当前 unit 的 SIGTERM。旧版和候选都启用 Actix 信号处理，SIGTERM 停止接收连接并优雅关闭，应用 shutdown_timeout=30s，unit TimeoutStopSec=45s。源码依据 deployment.rs:286–317、343–381，锁定 actix-server2.6.0 信号处理源码；**本轮没有实测生产 SIGTERM drain**。长流必须先自然结束，不依赖30秒强制截断。
6. stop 后检查 MainPID=0／inactive、ExecMainStatus=0，未发生 SIGKILL、日志无 event flush／worker stop失败，required事件无新增 quarantine；任何异常停止切换，保全当前旧状态并由负责人决定恢复旧服务。禁止把杀进程后裸复制判为一致备份。

## 获准窗口内的固定操作

以下由同一个 root shell 在 new-vps 执行；先有具体维护授权。工具上传和目录创建也在该授权窗口内进行，本轮未上传新的操作工具。先将本报告的 approved-window-operations.py 与 recovery-tool-hashes.json 放入下面 root0700窗口目录并校验其报告附件 hash。

~~~sh
set -euo pipefail
umask 077
CPAR_WINDOW_UTC=$(date -u +%Y%m%dT%H%M%SZ)
CPAR_TASK=/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884
CPAR_RELEASE=/opt/cpa-rust-gateway/releases/80f5838902eb56be6dccc3ce047eb227d6c18e6c-542a3c30aae7
CPAR_BACKUP_BASE=/var/backups/cpa-rust-gateway/80f5838902eb56be6dccc3ce047eb227d6c18e6c-542a3c30aae7
CPAR_WINDOW=$CPAR_BACKUP_BASE/$CPAR_WINDOW_UTC
test ! -e "$CPAR_WINDOW"
install -d -m 0700 -o root -g root "$CPAR_BACKUP_BASE" "$CPAR_WINDOW"
~~~

安装仅创建唯一新目录，不覆盖同名 release；9文件从完整 artifact-542目录复制，禁止使用之前超时的不完整 artifact/。以下安装之前使用 [artifact-verification.json](artifact-verification.json) 与 manifest／receipt核对完整9文件，binary hash必须同上；目录0755／gateway0755／root属主，artifact内容不重写。

~~~sh
test ! -e "$CPAR_RELEASE"
install -d -m 0755 -o root -g root "$CPAR_RELEASE"
cp -a "$CPAR_TASK/artifact-542a3c30aae7" "$CPAR_RELEASE/artifact"
chown -R root:root "$CPAR_RELEASE"
chmod 0755 "$CPAR_RELEASE/artifact"
install -m 0755 -o root -g root \
  "$CPAR_RELEASE/artifact/gateway-aarch64-unknown-linux-gnu" "$CPAR_RELEASE/gateway"
echo '542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87  '"$CPAR_RELEASE/gateway" | sha256sum -c -
~~~

若同名 release 已存在，退出本段，先逐文件核对并明确复用；不能盲目删除／覆盖。旧 release 保留。

达到上述零在途及暂停写入条件后才停止 gateway：

~~~sh
systemctl stop cpa-rust-gateway.service
systemctl show cpa-rust-gateway.service \
  -p ActiveState -p SubState -p MainPID -p ExecMainCode -p ExecMainStatus -p Result
~~~

确认旧服务干净停止后，再设置临时门控，此时没有在途可被门控中断。先要求下列 table 不存在；若已有同名表，不自行删除。规则拒绝所有非root发往数据端口的包，因此候选启动后普通代理请求仍无法进入；管理18181、Provider外出443、其他服务及端口保持原规则。维护人员暂停所有root数据客户端，root只允许只读health核验。

~~~sh
if nft list table inet cpar_release_80f5838 >/dev/null 2>&1; then
  echo 'Existing maintenance table: stop and inspect owner' >&2
  exit 1
fi
nft -f - <<'NFT'
add table inet cpar_release_80f5838
add chain inet cpar_release_80f5838 output { type filter hook output priority -10; policy accept; }
add rule inet cpar_release_80f5838 output ip daddr 127.0.0.1 tcp dport 18180 meta skuid != 0 reject with tcp reset
NFT
nft list table inet cpar_release_80f5838 > "$CPAR_WINDOW/admission.private"
~~~

### 最新完整离线 pre-upgrade 与恢复验证

~~~sh
python3 "$CPAR_WINDOW/approved-window-operations.py" capture \
  --window "$CPAR_WINDOW" --authorized-maintenance
python3 "$CPAR_WINDOW/approved-window-operations.py" verify \
  --window "$CPAR_WINDOW" --authorized-maintenance
~~~

capture 在服务停止且没有其他state/key持有者时复制完整两个目录、全部存在的WAL/SHM/journal、所有backups、六个持久源key、完整old release、unit/drop-ins/current元数据。两次确认无写入者，全复制窗口覆盖一致性边界。cp --archive 保持属主、模式、mtime、ACL/xattrs和软链；内部manifest逐文件比对，fsync文件、目录及PASS回执后才继续。原始DB、密钥、低熵摘要和解密材料不离开主机。

六持久源名称是 management-key、management-csrf、master-key、backup-key、client-key-pepper、grok-build-cache-key；不能依赖停服后消失的 /run/credentials。复制整体凭据目录以保留可能的恢复依赖。

verify **从归档先还原到新的pristine派生副本再打开SQLite**，避免只读WAL访问修改归档SHM。先证明文件字节／元数据与归档相同；两库integrity/FK、schema31、所有原表旧列count/HMAC；旧hash ELF网络隔离实际启动并完成8项受保护管理GET；候选在另一派生副本真实迁移31→34，逐表旧列HMAC保持，普通及native AEAD全认证；候选隔离实际serve与管理GET通过。使用既有经过演练且hash固定的四个恢复helper。

既有 candidate admin audit 在迁移成功后可能以1退出并报告 missing_endpoint_capability_profile。只允许固定已解释模式，保存私有日志；schema34、逐表保持及真正serve成功缺一则失败。**不能写CLI audit PASS**。

fresh-backup-verification.json=PASS、所有隔离进程已停止、pre-upgrade归档保持原字节后才切换。验证失败保留现场、current仍旧，恢复旧服务的范围必须在维护授权内；不继续迁移生产。

### 原子切换与实际迁移

~~~sh
CPAR_NEXT=/opt/cpa-rust-gateway/current.cpar-next-$CPAR_WINDOW_UTC
test ! -e "$CPAR_NEXT"
test ! -L "$CPAR_NEXT"
ln -s "$CPAR_RELEASE" "$CPAR_NEXT"
mv -T "$CPAR_NEXT" /opt/cpa-rust-gateway/current
systemctl start cpa-rust-gateway.service
~~~

不执行 daemon-reload，不改变unit、Caddy或配置图。gateway真正serve路径负责生产31→34迁移，不能用CLI audit退出码替代运行加载事实。

## 候选启动后的放行核对

普通准入仍有门控。通过主机内原认证仅做GET／DB只读，不发送Pin或模型调用：

- systemctl active/running，无异常重启；实际 /proc/PID/exe 和 current 指向上述release，ELF hash/source/target/Rust/application与ARM64manifest相同；User/Group、LoadedCredential六源、权限／ACL、loopback两监听与原unit相同。
- /healthz成功；管理 /admin/system 和完整分页inventory、configuration、keys/access-groups/routes/account-pools/native-pools/protected models成功，HTTPS管理入口／CSP/origin维持原行为。字段不足、页不全、403或不匹配一律停止；不能以一个200放行。
- 两库integrity/FK、schema_migrations1–31原行保持、32–34各exact迁移行一致、当前schema34；原53控制表+1认证表旧列保持离线基线。后台refresh/catalog派生字段允许变化，但必须用精确身份、revision、审计／时序解释。未知变化不PASS。
- 原active配置／Disabled、账号identity／owner、bindings、自定义routes、Client Key和Access Group权限、egress、管理账号与密码库保持；普通与native材料及历史AC-01全部可读。旧Usage/accounting仍Unknown，不回填measured或0；历史Request/Attempt/Usage/ledger及关联不丢失，新sidecar不能凭空证明旧请求。
- 新 capability_evidence 的serving_snapshot/config identity和声明字段属于**部署后GET核对**；实际execution identity与lease/egress要在后续获批Pin及真实请求中证明。本阶段不要求先有新构建真实调用，不构成部署循环前置。
- 任一不符，暂停放行并进入完整回退；若已产生新的material、Request/Attempt/Usage/ledger、审计或授权写入，先保全，再作数据决定。

如部署授权只包含部署/只读验证，保持门控到批准窗口截止；届时没有继续维护／开放授权则完整回退，不能无限延长中断。若同次具体授权已包含恢复原有业务准入，且全部上述检查通过，可删除本任务唯一nft表恢复原规则，真实新构建验收仍NOT_RUN。**首批推理预算与正常业务开放分别由具体授权覆盖**。

~~~sh
# 仅在批准的开放时点，或旧版完整恢复并核验后
nft delete table inet cpar_release_80f5838
~~~

## 失败后的完整回退

1. 保留门控，停止候选并确认PID消失及全部写入者停止。unit45秒截止／SIGKILL／异常flush都作为故障事实记录，不自动抹掉。
2. 先 capture 完整 post-failure，包含最新state/auth/keys/backups、所有存在的journal与部署元数据。新增Request／Attempt／Usage／账本、加密stored／compact状态、执行sidecar及后台授权更新均包括在整目录里；不是只导出四张统计表。
3. 在root0600 data-decision.private.json记录实际新写入范围、时间、request/attempt IDs（留主机）、provider是否已扣量、授权材料是否轮换、是否已对客户端承诺保存、责任人批准引用。未知不能写none_observed；独立保存不是恢复后的自动合并。
4. 无新增业务／授权写入且有明确回退授权，可按下面配方恢复。若已有新写入，只在负责人明确批准“保留新窗口独立待协调，并恢复最新pre-upgrade”后恢复。不得静默删账、重发推理、重新扣费、重新创建已消费回执或将schema34混装进旧库。新OAuth刷新可能影响外部grant可用性，旧快照AEAD成功并不能证明Provider仍接受旧token；涉及材料重授权／外部修复另行授权。
5. restore 先确认state/key与window仍处于同一文件系统，再将当前state/key树移动到displaced-live私有现场（不删除），恢复整套pre-upgrade state＋认证库＋匹配六key＋元数据，校验完整old release或从其备份恢复，原子恢复current并fsync。当前device均2049；未来跨文件系统则停止，须修订保全配方。unit/drop-in若漂移，先停止并单独审查，不能暗中覆盖共享配置。
6. 旧版启动后核对旧source/hash/schema31、integrity/FK、管理入口、全部历史材料、配置／权限／Disabled与request/ledger基线；如仍失败保持门控并报告。成功后仅按获准范围解除临时门控，保留全部pre/post/failed现场等待数据协调。

~~~sh
systemctl stop cpa-rust-gateway.service
python3 "$CPAR_WINDOW/approved-window-operations.py" preserve \
  --window "$CPAR_WINDOW" --authorized-maintenance
# 数据负责人先在主机内创建mode0600的明确决定，字段见下段；不打印内容
python3 "$CPAR_WINDOW/approved-window-operations.py" restore \
  --window "$CPAR_WINDOW" --authorized-maintenance \
  --data-decision "$CPAR_WINDOW/data-decision.private.json"
systemctl start cpa-rust-gateway.service
# 完成旧版本、schema31、完整性、认证／数据／权限核验后才删除唯一nft表
~~~

data-decision必须具备 approved_restore_pre_upgrade=true、request_attempt_usage_ledger_and_auth_changes_preserved=true、非空approval_reference，以及new_write_disposition为none_observed或retained_separately_pending_reconciliation。这是未来维护负责人对已保存事实的明确决定；本轮没有生成伪造批准文件。

完整回退的目标是全部恢复集合，不是切旧binary/current或migration down。最新离线pre-upgrade取得之前，所有更早快照只作演练与历史证据，不能替代本次切换恢复点。

## 将来可批准的具体范围

解除阻断且重新绑定唯一候选后，可批准：new-vps上述单一gateway服务的一次受控停止/启动；唯一不可变release安装、current原子切换与自动31→34；root0700最新完整离线备份及网络隔离恢复/迁移验证；单一nft数据准入表的创建/删除；主机内脱敏只读验收；失败时停止候选、完整post保全、明确数据决定后的state/auth/keys/old release恢复并启动旧版。批准内容需写明窗口截止与成功后的业务开放范围。

真实Pin／7次basics每组、Key或权限修复、账号授权、配置发布、unit/Caddy改动、GitHub写入及关单都不在这份部署操作范围内。当前13个P2未解决，**不建议批准当前候选部署，也没有申请重复准备工作授权**。
