# CPAR Linux ARM64 候选收口与新版六组合真实基础验收

Linux ARM64 **候选构建与验证 PASS**，本批构建配方／六组合驱动独立 **Standards complete PASS／Spec complete PASS**。新版真实基础验收 **FAIL，首个失败后已停止全批并退出**：Chat JSON 新 Pin PASS，首个基础请求 HTTP502 `UpstreamProtocolError`；其余五组合 NOT_RUN。新版完整基础组合 **0/6**、完整场景 **0/18**。本批没有修改产品源码，也没有失败后真实重跑。

本批新增预算意图 **2/60**（Pin1＋基础1），累计 **35/93**；网关／Pin 证据确认本批发送 **2**，历史累计 **34**。独立 Provider 接收数与实际费用均 **UNKNOWN**。剩余58个预算意图不续用。资源退出、生产保持及 Colima 状态恢复均 PASS；远端 CI、正式发布、正式部署和生产验收均 NOT_RUN。#47、#57、#9 保持 OPEN。

## 1. 基线、产品来源与本批范围

[实际基线](assets/cpar-build-six-20261011/baseline.json)为分支 `codex/prism-v4-delivery`、HEAD `5d7efdb0a108a2820939af9f39a568c58e611a3d`，与衔接信息一致；未执行回退。产品源码固定为 `abd454313f6a3707d6af5a5d653b731e2e54b08b`，既有工具提交为 `fe73140d64f7ed36afdfa86900d8546286e9ed09`。

主代理亲读适用规则、跨边界日志、GitHub tracker、[上批报告](cpar-output-limit-20261011.md)、[ADR-0104](../adr/ADR-0104-chat-output-limit-projection.md)、旧候选配方和失败回执、正式验收说明，以及引用的旧授权、身份连续性与退出计数规则。GitHub 初始三票均 OPEN，正文、评论、元数据和分页依赖已保存。[本批契约](assets/cpar-build-six-20261011/batch-contract.md)固定构建、调用、停止及交付边界；[授权记录](assets/cpar-build-six-20261011/user-authorization.json)继承明确批准的费用政策，次数和失败停止规则独立生效。

本批实施提交为 `5847fb224ff8f0f5ff532633c765ba08864015fd`。新文件只属于 `docs/reports/assets/cpar-build-six-20261011/` 及本报告；旧 remaining-five 驱动、旧 epoch、旧候选和所有失败回执保留。构建执行时工具 checkout 为 `5d7efdb0…`，配方由其后本批提交冻结；正式 entry／collector／audit 来源保持既有工具文件哈希。上述 checkout 均不冒充候选内嵌产品 source。

[保留核验](assets/cpar-build-six-20261011/preservation.json)确认任务起始 **42,766/42,766** 个脏路径原样保留：普通文件比较字节 SHA256，10个符号链接比较目标；含修改中的 `AGENTS.md`、未跟踪规则和其他历史内容。第一次辅助核验遇到目录符号链接退出，未签发结论；修正读取方式后完成全部核验。没有 reset、全局清理或宽范围暂存。

## 2. 隔离依赖与完整 release 构建

初始 `default` 和 `cpar-acceptance-20261010` 两个 Colima 环境均 Stopped。仅启动既有验收环境。原 builder 镜像固定为：

`sha256:436d5811a29917a624dd155d49501f6edb19d2ab62a1dcd308555923dea4b8e7`

既有 apt 缓存没有所需包，按授权从镜像配置的 Debian 官方签名软件源安装最小 `ruby` 依赖，使用 `--no-install-recommends`。共13个包、下载6552kB；Ruby为 `3.1.2p20`，`digest`、`open3`、`json` 加载通过。没有改动 macOS Ruby／全局开发配置，没有借用生产构建或新增远端服务。[依赖构建回执](assets/cpar-build-six-20261011/dependency-build.json)、[配方](assets/cpar-build-six-20261011/dependencies.Dockerfile)和 `dependency-provenance/` 保存包版本、来源 URI、deb SHA256及已验证 InRelease 摘要。独立后继镜像为：

`sha256:0fb5194f33ad4e1bc6eb9676f2dd8a16dc204dc2f4ab0747fe8fc5fbdc659250`

依赖准备和产品构建分开。产品干净 `git archive abd45431…` SHA256 为 `f21bec53d792eda65452537f5b190ec4bd037b0f6fe0dca61d42bead856a748d`。保留本批两次启动失败：第一份 Dockerfile 在编译前因 BuildKit 解析本地 digest 尝试 registry 而失败；第二次因 Colima 未挂载外部卷而无法建立 bind mount，容器未创建。对应 `candidate-build.json`、`candidate-build.log.gz`、`candidate-mount-failure.json`、`candidate-build-container.log.gz` 原样保留，未从失败尝试提取产物。

最终[实际配方](assets/cpar-build-six-20261011/candidate-build.sh)从后继 digest 创建自有容器，`--pull=never --network=none`，通过 `docker cp` 输入归档。重新完整执行原 `scripts/build-release.sh`，包含 Ruby 规范化脚本；Linux ELF 按脚本规则输出 `macho-uuid: skipped non-Mach-O artifact` 并正常返回，随后 `release-build: ready`。再进行 strip、ELF、动态依赖和 `--help` 检查。[完整回执](assets/cpar-build-six-20261011/candidate-build-container.json)记录138.23秒、docker attach退出0、容器退出0；[成功日志](assets/cpar-build-six-20261011/candidate-release.log.gz)解压后 SHA256 为 `90fb24812fd9776be1634bd856cf24ba1c25b81e14b4145da15c43e87be0379a`。

构建回执中的 `artifact_extracted=false` 是构建完成、提取前的时间点；其后的[候选验证](assets/cpar-build-six-20261011/candidate-validation.json)才证明提取、内嵌元数据和 manifest。未覆盖这一历史回执。

| 候选项目 | 实际验证 |
|---|---|
| 产品 source | `abd454313f6a3707d6af5a5d653b731e2e54b08b`，与干净归档及唯一内嵌字段一致 |
| 目标／格式 | `aarch64-unknown-linux-gnu`；ELF64 little-endian，e_machine 183/AArch64 |
| 内嵌 Rust | `1.97.1` |
| binary 大小／SHA256 | 19,500,176字节；`f505ed6e71b3dea47b940175d39274605d4b0721597cf2841e65ce7aa536b473` |
| 动态依赖 | libstdc++.so.6、libgcc_s.so.1、libm.so.6、libc.so.6；AArch64 loader |
| 可运行性 | 隔离 Linux 与 new-vps 上 ldd 解析、`--help` 均退出0 |
| manifest 原始文件 SHA256 | `b51ea8085b87bc2d9b942e7ee57e84e4e015aa53fc4409ba7a3e8d9489b7e583` |
| manifest 规范化对象 SHA256 | `79d4425355c4aed8300c54d28aaee872cfb5d1860acf135eef9b2ba54f30537c` |

[manifest](assets/cpar-build-six-20261011/artifact-manifest.json)与[metadata](assets/cpar-build-six-20261011/gateway-build-metadata.json)均匹配实际 binary。原始文件和排序紧凑 JSON 对象是不同哈希输入；启动回执使用前者，正式 audit 使用后者。候选只是两个 payload 的未签名验收 manifest，完整签名 P12 发布包未生成。候选文件留在本地 `output/cpar-build-six-20261011/candidate/`，并有 new-vps 保护根中的自有副本。

## 3. 驱动冻结、本地检查与独立审查

本批自有 `tools/remote-*.py` 从旧 remaining-five 驱动派生，顺序固定为 Chat JSON、Chat SSE、Messages JSON、Messages SSE、Responses JSON、Responses SSE。历史预算33／确认发送32，新增 Pin6＋基础54＝60、累计预算93，均由断言固定；旧21／50／71、旧 lease 和旧路径未沿用为本批绑定。发送前占用预算，操作前持久化停止锁存，失败／未知即停，重复运行与上限拒绝。HTTP本地拒绝也占预算，发送证据不完整时 actual 保留 null；资源退出结论与发送对账独立。

[本地检查](assets/cpar-build-six-20261011/local-checks.json) **35/35 PASS**，覆盖六组合次序、预算先占用、FAIL/UNKNOWN/BLOCKED全批停止、重入、上限、HTTP400、部分 wire 保留、未知 Pin、失败或缺失 Attempt、资源与计数分离。正式 entry、collector、audit 未修改；remote upload14个文件及 epoch13个关键文件分别独立逐字节读回，见[远端文件核对](assets/cpar-build-six-20261011/remote-byte-verification.json)与[epoch核对](assets/cpar-build-six-20261011/epoch-byte-verification.json)。

[审查冻结](assets/cpar-build-six-20261011/review-freeze.json)限定 `5d7efdb0…5847fb22` 的38个拥有文件。[两轴最终回执](assets/cpar-build-six-20261011/review-final.json)：

| 审查轴 | 结论 | 证据边界 |
|---|---|---|
| Standards | complete PASS；硬问题0、非阻断 heuristic1 | 静态审查、归档／binary／manifest、精确内嵌元数据、成功日志、38文件及规则哈希 |
| Spec | complete PASS；未闭合发现0 | 构建来源、14上传文件、35门槛回放、旧断言保留、严格授权连续性、未知发送语义 |

两个 Standards 辅助核验先后因 Python3.9 辅助 API 不存在及辅助命令转义错误返回 BLOCKED；初始回执保留，后续独立一次性探子完成缺口后才签发 complete PASS。非阻断意见是失败日志与成功回执名称容易混淆，本报告明确对应关系并保留原文件。审查结论仅覆盖候选／驱动实施，不表示真实基础成功。

[复用核对](assets/cpar-build-six-20261011/prior-evidence-reuse.json)验证既有23文件哈希一致；产品目录与 `abd45431…` 相同。因此复用该产品独立 clean-checkout **Full45/45 PASS**、既有产品 Standards／Spec，以及本地六组9/9、190/190公共参数与矩阵、354/354共享回归、54/54验收工具回归。没有重做无变化的全仓检查；新配方与驱动另审。该初始复用回执的 `pending` 只描述当时新审查尚未开始，现由上述最终审查回执闭合。

## 4. 真实运行身份与资格

使用既有 new-vps 和保护根 `/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010`，新 binary 目录 `bin/abd45431-six-20261011`、私有 epoch `private/abd45431-six-20261011`。[只读准备](assets/cpar-build-six-20261011/readiness-initial.json)、[自有副本准备](assets/cpar-build-six-20261011/prepare-receipt.json)、[启动](assets/cpar-build-six-20261011/start-receipt.json)分别记录资格及绑定。

同一批准 Grok Build 账号／授权来源已自行正常续期：上批材料135，本批材料136；严格非空身份摘要不变，变动仅在允许的正常刷新字段，连续范围 floor77/current137，密钥版本和来源字节匹配。本批主动 refresh／重新授权／替换账号均0。实际池和新 Pin 单独证明 lease137，没有从材料revision推算 lease，也没有沿用旧 Pin 或旧会话资格。

| 冻结维度 | 本批证据 |
|---|---|
| source／binary／manifest | 与第2节完全一致，远端实际字节独立核对 |
| 临时 unit／进程 | `cpar-build-acceptance-20261010.service`；PID3287370，process_start663051947 |
| config | `cpar-build-config-20261010`／revision9 |
| endpoint／route／candidate | `cpar-build-endpoint`／`cpar-build-route`／`cpar-build-candidate` |
| model | upstream/public/response 均 `grok-4.5` |
| 账号／材料／lease | 账号 opaque evidence `5beff1b6…`；native136／actual137 |
| Client Key／egress | 单一 `ff7ef703…`；direct egress `69aa38e9…`，完整摘要见 Pin/audit |
| 有效期 | expires_at_ms `1791711342393`；运行前新鲜资格成立 |
| 能力声明 | reasoning、stored、continuation、compact、websocket=true；parallel=false |
| 独占及出口 | 单账号池、单 enabled candidate、max_attempts=1；guard映射；无竞争进程；后台OAuth/refresh/目录任务禁用 |

启动后 **65.59秒**审计零新增，管理读取未触发出口；运行观察及 capability 声明在新 Pin 前后重取。没有上游目录／额度查询、第二 Client Key、故障注入或扩展真实调用。

## 5. 新版矩阵、首个失败与诊断边界

[新 Pin](assets/cpar-build-six-20261011/chat-json-pin.json)于 `2026-10-11T07:32:19.341Z` 通过，Attempt1、upstream_sent=true。随后使用正式公共路径、entry及audit；正式 collector 保持接入，但该首请求在 HTTP 错误处停止，未形成成功样本的完整 Usage／ledger 关联。

| 新版组合 | 新 Pin | 文本多轮3 | 单工具2 | 连续工具4 | basic／formal |
|---|---|---|---|---|---|
| Chat JSON | PASS | **首轮502；执行1，成功0；余2未运行** | NOT_RUN | NOT_RUN | **FAIL／FAIL** |
| Chat SSE | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN／NOT_RUN |
| Messages JSON | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN／NOT_RUN |
| Messages SSE | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN／NOT_RUN |
| Responses JSON | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN／NOT_RUN |
| Responses SSE | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN／NOT_RUN |

[矩阵 JSON](assets/cpar-build-six-20261011/real-batch-matrix.json)、[执行摘要](assets/cpar-build-six-20261011/chat-json-basic-summary.json)、[独立正式 audit](assets/cpar-build-six-20261011/chat-json-audit.json)保留原结论。正式 audit 对整个文本场景写 `NOT_RUN/execution_stopped`、executed_turn_count=1；表中“首轮502”描述已发生的执行失败，不将未完成场景改标成功。正式 entry与独立audit均退出1。

首个基础请求在 `2026-10-11T07:32:31.708683+00:00` 发往 `POST /v1/chat/completions`，保持 `grok-4.5`、`max_completion_tokens=4096`、`stream=false`，只有文本多轮场景的第一条 user 消息。返回124字节 HTTP502，`gateway_error/UpstreamProtocolError`，wire SHA256 `dd3e643d6103a6e312a200642baca754f08f3e2b2b399356dbc7adc64063d9c9`。[失败摘要](assets/cpar-build-six-20261011/chat-json-failure-capture.json)记录时间、字节摘要及事件。原始请求、响应 wire 和运行日志只保存在0700/0600私有目录；[私有索引](assets/cpar-build-six-20261011/private-capture-index.json)公开摘要，不提交会话内容或凭据。

响应没有 `x-request-id`。唯一串行窗口找到 `p1-request-433c99cf8f57471b800505f66e1ae4f2-0`：Request1、Attempt1（succeeded/completed）、request_finished1（failed，3010ms），新增Usage0、账本0。该关联是时间窗口推断，不冒充精确 wire ID 绑定。自有数据库继承的各1条旧事件和1条旧账本单独扣除。

[主代理定位与证据边界](assets/cpar-build-six-20261011/chat-json-diagnosis.json)逐文件核对产品 source：非流式 Build 在返回 buffered source 前已经完成HTTP/JSON/native解码；AttemptSucceeded记录发生在下游投影／收集／编码之前，因此它和最终 Request失败可以同时成立。`ProjectedEventSource` 会把投影拒绝变为 `UpstreamProtocolError`。严格 native projector 对复杂 reasoning、phase、citation 等不可保真内容有明确拒绝路径。

只运行一个现成离线案例：`cargo test --offline --locked -p gateway --bin gateway grok_native_metadata_survives_public_responses_and_rejects_lossy_bridges -- --nocapture`，**1/1 PASS**，19.96秒，真实请求0，见[回执](assets/cpar-build-six-20261011/native-offline-reproduction.json)及[日志](assets/cpar-build-six-20261011/native-offline-reproduction.log.gz)。该测试使用合成原生复杂元数据，验证 Responses保留原始items而Chat/Messages拒绝有损桥接。它复现现有拒绝边界，**没有精确重放此次真实上游响应**；上游原始响应未捕获，不能认定某个具体字段是本次根因。现有Full／输出上限简化fixture通过不能覆盖这一真实内容缺口。

后续最小产品范围是：确定真实原生响应形状及无内容泄露的具体投影拒绝原因，核对 pre-header error的Request-ID与失败Usage证据，裁定保真Chat/Messages桥接及原生回归。不能通过丢弃items／reasoning／citation、改请求上限或降低断言绕过失败。本批不实施产品修复、不再调用真实Provider。

旧 `1b6023f52f4c00ea6d8296524d306c141e4edae7` 产品的 Responses JSON/SSE 各9/9仍按[原批](cpar-build-batch-20261011.md)和[remaining批](cpar-build-remaining-20261011.md)保留，历史完整组合2/6、场景6/18。旧SSE失败、旧Chat400及其修复证据也保留。历史通过不计入新版0/6。

## 6. 请求计数与资源退出

[分项对账](assets/cpar-build-six-20261011/request-accounting.json)：

| 口径 | 历史 | 本批 | 累计 |
|---|---:|---:|---:|
| Pin预算意图 | 7 | 1/6 | 8 |
| 基础预算意图 | 26 | 1/54 | 27 |
| 推理预算意图总数 | 33 | **2/60** | **35/93** |
| 网关／Pin证据确认发送 | 32 | **2**（Pin1＋基础Attempt1） | **34** |
| 独立Provider接收 | UNKNOWN | UNKNOWN | UNKNOWN |
| 实际费用 | UNKNOWN | UNKNOWN | UNKNOWN |

55次管理 GET 单独计数：readiness2、prepare2、start6、Pin资格检查44、exit1；Pin POST1已计入新增推理预算。后续 SQLite／proc／文件读回没有管理HTTP或Provider调用。历史上游目录请求1单独保留，本批上游目录／额度／主动refresh／重试／fallback／失败后重跑／扩展均0。`unpriced/null`只来自继承账本，不是本批免费或费用成功的证明。

[退出回执](assets/cpar-build-six-20261011/real-exit-receipt.json)在 `07:32:59Z` 记录资源退出与发送对账分别 PASS：只停止本批临时实例，MainPID0、原PID不存在，19180/19181无listener，停止后5秒审计不增。网络审计只包含允许的DNS/连接事件，不能用于推算Provider接收数。[独立停机读回](assets/cpar-build-six-20261011/post-exit-readback.json)确认 stopped=true、审计摘要不变、其余五组合目录不存在、208个历史文件哈希保留和私有权限0700/0600。

生产PID499070、process_start551409035、unit/binary/current/Caddy SHA、活动配置 `kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75/rev-1` 均保持；重启数仍0。[端口前](assets/cpar-build-six-20261011/production-ports-before.json)与[端口后](assets/cpar-build-six-20261011/production-ports-after.json)同为18180/18181、原PID及原socket inode/fd。没有触碰生产service/current/config/Caddy/端口或切换账号。

[Colima退出](assets/cpar-build-six-20261011/colima-exit.json)确认default与验收环境均恢复原先Stopped；旧镜像、旧容器及新的完整构建容器保留。磁盘上新配方是本批文件，不更改当前会话已加载的规则。

## 7. 分层结论与交付状态

| 层次 | 结论 |
|---|---|
| Linux ARM64完整构建／候选验证 | PASS；exit0，source/binary/metadata/两种manifest摘要及远端字节匹配 |
| 本批Standards／Spec | 各complete PASS；只覆盖冻结的候选／驱动实施 |
| 本批本地检查 | 驱动35/35 PASS；补充既有离线案例1/1 PASS，LOCAL_SIMULATED |
| 产品Full／既有本地矩阵 | 按source及文件哈希复用；没有新增真实资格 |
| 新版六组合真实基础 | FAIL停批；0/6完整组合、0/18场景 |
| 旧产品历史基础 | Responses两组9/9；历史2/6组合、6/18场景，身份独立 |
| 真实扩展及整体规格 | NOT_RUN／未完成；不以Pin或basic代替 |
| 临时资源退出／生产保持 | PASS；不等于生产验收 |
| 远端CI | NOT_RUN；未push |
| 正式发布／签名发布包 | NOT_RUN |
| 正式部署／切生产 | NOT_RUN；仅授权验收目录复制及临时实例运行 |
| 生产验收 | NOT_RUN；原生产持续运行 |

[文档检查](assets/cpar-build-six-20261011/docs-check.md) **7/7 PASS**：链接、契约引用、计划状态、canary、Caddy约束、tracked secret scan及whitespace。本批报告与证据另检查 staged diff，未引入凭据文件或原始会话；[最终本地验证摘要](assets/cpar-build-six-20261011/final-validation.json)分列新检查和来源一致的复用项。

GitHub同步在本报告和证据就绪后更新三个既有进度评论，保留历史前缀、原AC、元数据和依赖，不新增同内容评论、不关闭工单。同步与独立读回回执在交付时补齐。

下一项工作先处理本次Chat JSON真实502的原生响应证据和保真桥接缺口，再重新确定后续真实基础授权。即使将来基础六组合全部通过，首项声明扩展仍是 `visible_reasoning/reasoning_history` 的真实样本，随后还有各适用协议的stored、continuation、compact、websocket及受控故障／恢复等缺口；parallel未声明，不伪造覆盖。本批在归档与GitHub同步后结束，不自动进入这些后续范围。
