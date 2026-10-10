# Grok Build 隔离实测、必要修复与证据收口（2026-10-10）

**本批临时实例创建、Linux ARM64 构建、账号隔离、后台阻断、真实请求及退出已实际执行。真实验收仍 BLOCKED：模型目录 GET 成功一次，首个 Responses JSON Pin 的管理 HTTP200 携带失败回执，上游闭合诊断为 HTTP426／Unrecognized；基础推理 0/54。没有重试、fallback、换模型或真实重跑。**

最新本地产品源码为 `1b6023f52f4c00ea6d8296524d306c141e4edae7`；实际真实运行源码为此前的 `f346a03f912b150006a288c38087550bb3d04cb0`。后者的真实失败没有被新版本的本地通过替换。当前源码尚未 push，没有对应远端 CI、签名发布制品或生产验收。

## 承接范围与授权

本轮入口工作区为 `codex/prism-v4-delivery`，隔离实测开始基线 `82a1f69861300456cca7df779ee07949d043c84b`，原产品基线 c1b304d0。用户已确认 Build、4.5／4.6／4.7、既有账号逐一核对、既有 new-vps、费用不设金额上限，并明确回复“批准创建并运行临时隔离实例”。[具体运行授权](assets/cpar-build-live-20261010/runtime-authorization.json) 与[前阶段账号／费用授权](cpar-build-acceptance-authorization-20261010.md)分别保留。

执行对象为 new-vps 的 `/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010`（0700）、临时 `cpar-build-acceptance-20261010.service` 和 loopback 19180／19181。仅复制单个已授权 Build 账号必要的私有材料与匹配密钥；密钥、账号原材料、完整响应和私有日志留在 VPS 保护目录。生产 unit、current、配置、Caddy 和18180／18181保持原样。

本批最多一目录 GET、六次必要 Pin 加54次基础，费用不限额；retry／fallback／extension／fault probe／rerun／OAuth／quota query 为0。不限金额只改变货币门槛；费用上界仍 NOT_PROVEN，不改变身份、参数、协议、输出终态、归属、Usage、账本和失败停止规则。

[前置收口报告](cpar-preacceptance-local-20261010.md)中 Web／Console 专属页面13项、未知写回读与保存／应用区分、历史独立复核，以及 Kiro／Grok Web 具体取证方案继续按原来源复用，本批未重做。专属页面是 stock 嵌入管理页＋生产管理 HTTP＋synthetic loopback，真实推理0；其最终页面快照额度15与停止后总接收额度18不是同一计数范围。已完成 T12／#57 AC02／E-05 的既有 bounded 结论和历史 not-found／PARTIAL记录均保留。

## 三个账号与实际请求范围

[当前来源账号核对](assets/cpar-build-live-20261010/source-build-eligibility-100607.json)逐一检查了全部三个 Build 身份。核对不等于成功推理，也没有对已知过期身份发送探测请求。

| 脱敏账号前缀 | 当前本批观察 | 处理 | 真实 Pin |
| --- | --- | --- | ---: |
| fc78df8341bd | native rev0，enabled／active，已过期 | BLOCKED_ACCOUNT_EXPIRED；不自动refresh／重新授权 | 0 |
| 50d8e3f4fc86 | native rev98，enabled／reauth_required，已过期 | BLOCKED_REAUTH_REQUIRED_AND_EXPIRED；不解除状态 | 0 |
| 5beff1b67bd3 | native rev133，enabled／active，未过期 | 最小单账号副本，实际 Pin lease rev134 | 1，失败 |

[选择性副本回执](assets/cpar-build-live-20261010/selective-copy.json)区分 source schema31与新空 state schema34；保留所需账号密文及匹配 key version1，生产写入0，不复制生产 Request／Attempt／Usage／账本历史。原生保存revision133、运行投影revision134与实际 Pin lease134分别记录；投影本身不是实际执行证明。

隔离配置为 `cpar-build-config-20261010`／rev9，provider `cpar-build-provider`、endpoint `cpar-build-endpoint`、route `cpar-build-route`、candidate `cpar-build-candidate`，模型 `grok-4.5`，adapter `grok.build.responses`，固定 `https://cli-chat-proxy.grok.com/v1`。单账号池、单 enabled candidate、max_attempts1、all_active scope、专用 Client Key和单 route grant；普通 credential binding0。[配置回执](assets/cpar-build-live-20261010/configure.json)及[修复后重启核对](assets/cpar-build-live-20261010/repair-restart.json)保留精确对象。

六组合计划为 Responses／Chat／Messages×JSON／SSE；每组合文本多轮3、单次工具2、连续工具4，共9次。拟冻结输出控制为对应协议的4096字段；实际未进入基础请求，不能声称这些参数的实时终态／费用语义已验证。扩展只读完整 serving declaration，不自动展开扩展测试。

## 必要修复、来源与验证

### 后台任务显式禁用

原 serve 启动会触发 startup refresh，并安装自动 refresh／catalog worker；本地最小修复提供 `--background-provider-tasks enabled|disabled`，默认 enabled 保持原行为，disabled 同时跳过启动刷新和整个 RuntimeReload worker subscription，后续发布不会重建 worker。只影响显式隔离模式，没有弱化普通服务默认行为。

进程回放使用 stock binary、公用 HTTP、独立接收文件和 synthetic TLS 上游。固定 f346a03f 在两个模式分别观察65秒、配置发布／重启／再次发布：default refresh2／catalog2，disabled refresh0／catalog0；每模式 public inference1＋JSON/SSE Pin各1。三个拒绝控制均0新增接收。[固定源码回放](assets/cpar-build-live-20261010/pin-fixed-source-background.json)、[运行回执](assets/cpar-build-live-20261010/pin-fixed-source-background-run.json)不是真实 Provider 证据。明确 synthetic OAuth seed3在冻结后台观察基线之前。

### 单进程网络与私有目录边界

自有 preload guard只允许固定 Build DNS及其缓存IP的 TCP443、loopback和Unix；OAuth／其他DNS和其他远程 connect拒绝，审计文件异常失败关闭。初次独立Spec发现README的TCP声明未检查socket类型：保留[旧 UDP RED](assets/cpar-build-live-20261010/guard-udp-red.json)，补 SO_TYPE=SOCK_STREAM及SO_PROTOCOL=IPPROTO_TCP，再以[原生 GREEN](assets/cpar-build-live-20261010/guard-tcp-green.json)和[独立补审](assets/cpar-build-live-20261010/review-guard-closeout.md)收口。

这是可信 stock 进程的 getaddrinfo／connect审计接缝，不是系统调用或 TLS 沙箱；DNS／connect计数不等于服务端HTTP接收数。动态green不覆盖全部远程TCP／Unix／audit-fault组合。正式候选还单独核对/proc maps、环境、实际参数、proxy环境未设置和65秒平坦审计。

临时 unit采用 UMask0077、NoNewPrivileges、ProtectSystem、ProtectHome、PrivateTmp／Devices及仅自有 state／private／audit写入路径。生产父目录为另一个service uid的0700；最初capability空集导致loader／state读失败，原[bootstrap失败](assets/cpar-build-live-20261010/bootstrap-initial-failure.json)保留。只对临时 unit授予必要 CAP_DAC_READ_SEARCH（CapEff4），没有修改生产父目录权限、unit或服务。

### 原生 Pin 读取实际 serving snapshot

目录 GET 成功后，旧候选的正确 Pin 输入仍 HTTP400；保护事件及独立 loopback复现证明该错误在发送前，public control能发送一次，Pin接收0。静态断点定位到未物化的 registry图，Pin没有读取实际 scheduler serving snapshot。最小修复保留一次 `Arc<RouteSnapshot>`供route、model及权限检查，不删除检查，不改变配置／generation／lease守卫和max_attempts1。

[旧Pin失败](assets/cpar-build-live-20261010/pin-presend-400.json)、[synthetic RED](assets/cpar-build-live-20261010/pin-synthetic-red.json)、[GREEN](assets/cpar-build-live-20261010/pin-synthetic-green.json)和[两轴补审](assets/cpar-build-live-20261010/review-pin-closeout.md)分别保留。fixture另有不含tools／reasoning的plain-text应答修正；最新固定源码回放证明成功Pin接收各1、权限／revision／model拒绝接收0。

旧私有 budget v1 保持 stopped及原字节hash；新版v2携带目录已消耗1、历史management presend rejection1、历史Provider inference0。只有证明旧HTTP400在transport前，才创建修复执行epoch；这不是重放真实失败或抹去历史计数。[修复重启回执](assets/cpar-build-live-20261010/repair-restart.json)保留这种区分。

### HTTP426后的本地版本漂移修复

实际 Pin 已进入上游路径。保护回执绑定 f346a03f、actual attempt、lease134、config rev9、direct egress及六项serving capability；同request的安全日志记录HTTP426、body_kind object、signal Unrecognized，未开始规范响应。这里只能认定失败，不能认定账号、额度、UA或版本为已证实原因。

公开查证确认CPAR的冻结CLI版本0.2.111已落后于[官方stable1.0.50](https://x.ai/cli/stable)，[官方发布记录](https://x.ai/changelog/build)日期为2026-10-06；官方固定源码说明version header参与proxy gating。由此推断版本门槛可能有关，426根因仍 NOT_PROVEN。[版本增量复核](assets/cpar-build-live-20261010/review-profile-closeout.md)仅对这一局部修复给出Standards／Spec PASS，不扩大到真实验收。

最新本地提交成对更新版本头及UA版本，保留现有固定linux／x86_64 profile和其他所有字段。修改现有请求oracle后先复现版本漂移，再通过原19项Build协议测试。[公开输入hash](assets/cpar-build-live-20261010/public-profile-source.json)、[RED](assets/cpar-build-live-20261010/profile-version-red-run.json)、[GREEN](assets/cpar-build-live-20261010/profile-version-green-run.json)分别绑定来源。官方公开main为1.0.45，其release pipeline注入shipping version；stable指针hash不是官方二进制hash，1.0.50发行二进制到该main的精确映射仍 NOT_PROVEN。

这是已证实的本地版本漂移修复；没有以新版本重跑真实Pin，不能写“426已修复”。

## 实际制品与正式门槛

| 来源／范围 | 验证与限制 |
| --- | --- |
| f346a03f实际运行源码 | 干净独立clone Full45/45，Rust1501通过／0失败／12忽略；默认development环境。[Full运行回执](assets/cpar-build-live-20261010/full-f346a03f-run.json)及lossless raw gzip保留 |
| f346a03f Linux ARM64 | 原生构建、ELF machine183；binary SHA256 `3f56f63b129a7d78725783ae037add87be69127014f880d0c4e76f40be64134f`，19,500,176bytes；[候选身份](assets/cpar-build-live-20261010/candidate-artifact-f346a03f.json)，实际/proc exe匹配 |
| f346a03f构建来源 | 5121 tracked中4916实际admitted文件逐字hash相同，205按`.dockerignore`排除；不声称全部tracked tree入镜。[来源证明](assets/cpar-build-live-20261010/native-source-f346a03f.json)保留前两次错误过滤范围的FAIL |
| 1b6023f5后继源码 | 干净独立clone Full45/45，Rust1501通过／0失败／12忽略；[固定源码回执](assets/cpar-build-live-20261010/full-1b6023f5-run.json)，前后clean；原19项Build oracle PASS。独立增量Standards／Spec另附 |
| 1b6023f5后继制品 | 同一原生builder、前候选精确base与四文件完整tracked delta构建；5122 tracked中4917 admitted文件hash一致、205排除；binary SHA256 `ea89d5c72628a871f7683f32d0b51a76dc9c588f7172c86345d1c989f1bd9063`。[候选身份](assets/cpar-build-live-20261010/candidate-artifact-1b6023f5.json)与[来源证明](assets/cpar-build-live-20261010/native-source-1b6023f5.json)；仅准备，尚未运行／发送真实请求 |
| 制品等级 | 本机临时native candidate＋P12格式metadata＋两文件hash manifest；不是签名P12 release、SBOM／OCI完整发布套件、远端CI或生产部署证据 |

初次Full Clippy失败、错误release环境下development identity测试失败及多次fixture／clock／snapshot读取驱动失败均保留。最新development Full不覆盖旧release环境的失败；正确固定来源的后继green只证明其实际执行范围。[原两轴审查](assets/cpar-build-live-20261010/review-initial.md)的Spec PARTIAL/P3也不被覆盖，新补审单独关闭该发现。[独立资产核验](assets/cpar-build-live-20261010/review-live-evidence.md)进一步区分旧候选通用manifest与f346a03f专用资产。

## 真实计数、退出与项目状态

[最终运行回执](assets/cpar-build-live-20261010/final-runtime.json)绑定UTC2026-10-10、实际源码／binary／guard、私有Pin／log／budget的hash及生产核对。它将cleanup PASS与real acceptance BLOCKED分开。

| 动作 | 实际数量／终态 |
| --- | --- |
| 手动模型目录 GET | 1，HTTP200；允许的4.5／4.6／4.7均出现在目录，选4.5；未重跑 |
| 历史management Pin admission | 1，HTTP400，已证明Provider inference0 |
| 修复后真实 protected Pin | 1 attempt，upstream_sent=true，上游闭合诊断HTTP426；失败 |
| 基础推理 | 0/54；首组合BLOCKED，其余五组合NOT_RUN |
| retry／fallback／model replacement／extension／fault probe／rerun | 各0 |
| 本批OAuth／refresh／automatic catalog／quota query | 各0；不把源生产服务正常后台算成本批 |
| 本批guard DNS／TCP connect | 各2（目录＋Pin路径）；仅连接层，server HTTP receive count不声明 |
| 基础Request／Attempt／Usage／账本资格 | NOT_RUN；Pin使用Noop serving event sink、management started审计与保护回执，不以event log0推断真实Pin没发送 |
| 费用金额／完整计费上界 | NOT_PROVEN；用户不限金额授权已承接，没有声称失败请求免费 |

临时PID2031518已停止，MainPID0，19180／19181无监听，停止后审计无新增。私有材料按用户要求保留，必要文件0600、任务目录0700；未删除生产或其他文件。生产unit仍active/running、PID499070、NRestarts0；unit／binary／current／Caddy hash、启动时刻及active配置kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75／rev1前后一致。核对针对这些不变量，不声称运行中的整库字节不变。

自有Colima profile `cpar-acceptance-20261010`的启动／停止只为本批构建，Docker active context保持default；最终停止回执另附。缓存与本机候选保留，不做prune、reset或全局清理。

#47、#57、#9保持OPEN；#57已勾AC02保留。历史198基础组合／186适用未执行、12个Web工具已接受不适用，以及49项扩展后继台账不因目录成功或Pin尝试而回写PASS。本批观察到单个Build候选的六项声明不等于49项扩展已验收。其他远端CI／发布／生产门槛仍NOT_RUN或原BLOCKED。

## 本地交付与 GitHub 同步

本批产品／执行支持提交分别为 f978e892（后台gate）、fb96f23c（局部assembly lint说明）、98febf05（fixture原子快照）、a3dc0c23（TCP-only guard）、f346a03f（Pin serving snapshot）、1b6023f5（公开版本漂移）。只提交自有路径，用户AGENTS.md字节保留；25,841个原有git-visible路径按基线lstat mode／size／mtime核对，不声称全内容hash或ignored nested覆盖。报告／证据后置提交与最终HEAD由output独立attestation记录，避免文件证明自己的提交hash。

GitHub仅给[#47既有进度](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57既有进度](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9既有进度](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)追加本批范围，并已独立GET读回。读回断言为 PASS：三个既有评论 ID 和历史正文前缀保留，未新增评论；三票正文、OPEN状态、labels、assignees和语义依赖保留；#57 的 AC02 勾选保留。依赖条目中的 `updated_at` 随评论 PATCH 正常变化，依赖编号与其余语义字段不变。公开摘要见 [github-live-after.json](assets/cpar-build-live-20261010/github-live-after.json)；没有 push、关闭或发布动作。

当前可独立进行的动作是审查本地后继候选与既有私有失败证据。若要重新验证真实Pin，需要单独扩展本批明确为0的rerun边界：仍固定同一专用实例、单账号、grok-4.5、单attempt、无fallback，复用已消耗的目录GET。任何新请求必须携带原失败计数，重新绑定实际候选进程和执行身份；公开版本变化不能代替成功Pin或Usage／账本证据。到期、失配、未知或新失败立即停止，不自动OAuth、续期或重新授权。
