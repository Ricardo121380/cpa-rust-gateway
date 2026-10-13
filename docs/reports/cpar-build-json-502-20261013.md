# CPAR-38 / #47：既有修复复核与新诊断资格停止

本批完成既有产品修复的来源核验、新公共入口回归和一次只读诊断资格检查。
真实诊断 **BLOCKED_BEFORE_PREPARE**：2026-10-13 09:58:28（Asia/Shanghai）
观察到同一批准 Build 材料剩余 2893.642 秒，低于既有准备驱动要求的
3600 秒。保留原断言并停止本批，远端 epoch、上传目录、临时进程、Pin 和
Chat 均未建立或执行。没有新增原生响应，旧 Chat JSON 502 的具体根因继续
**UNKNOWN**；这不是 NOT_REPRODUCED，也不是新一次 HTTP502。

实际基线为 `codex/prism-v4-delivery` / `201dbe2e283b9c07c85b6054779f53f4d368afd2`，
已包含[上一次 CPAR-38 局部交付](cpar-build-json-502-20261011.md)。给定的
`5a7826e8`、`abd45431`、`5847fb22` 检查点没有被用作回退指令。
本批没有重复实施已有修复或构建准备，没有新增产品字段转换、规则例外或前端修改。
约束见[本批契约](assets/cpar-build-json-502-20261013/batch-contract.md)，
初始 HEAD、用户 AGENTS 摘要和计数见[基线](assets/cpar-build-json-502-20261013/baseline.json)。

## 已确认的软件缺陷、候选解释与未知

亲自读取[原六组合报告](cpar-build-six-20261011.md)、所引用的批次契约、失败捕获、
诊断、完整两轴审查与退出回执，并读取当前 CPAR-38 报告、ADR-0100/0101/0105、
HTTP/Provider/Router/Usage/连续性契约及 GitHub #47/#57/#9 正文和最新进度。

已确认且已修复的三个软件缺陷仍在当前产品源码中：投影拒绝原来被通用错误抹去；
已确认 Request 准入后的部分错误没有关联头；Provider 已完成验证的 Usage 会在后续
投影失败时丢失。现有代码保留六种闭集拒绝原因及固定安全文字，统一附加已建立的
Request-ID，并转发已解码 Usage 快照至 HTTP 失败处理、原 Attempt 和幂等账本。
Request 身份未建立时不伪造关联，调用方 ID 不替代服务端身份。Attempt succeeded
仍可以与最终 Request failed 共存；未知计数、接收数和费用不填零。

本批沿实际 `chat_completions → execute_routed → start_grok_build → Build decode →
ProjectedEventSource → HTTP collection/encoding → SQLite events / billing` 复核。
已确认的边界和定位见实际源码：`runtime.rs` 的 ProjectedEventSource 转发 Usage 并保留
拒绝原因；HTTP handler 在确认准入后统一关联响应；HTTP pump 在源结束后保存已解码
快照。静态核对不是新真实样本。复杂 reasoning summary、phase、citation 等不可等价
内容仍只是旧 502 的候选解释，旧原文未捕获，不能确认具体致错字段。

既有 plain reasoning、text/tool 转换继续保真；rich native 内容在 Responses 保留，
在 Chat/Messages 无等价表示时明确拒绝。没有删除 native items、reasoning、citation、
signature、工具结果、call_id、顺序或历史，没有降低能力声明、4096 上限或测试断言。
没有新样本支持新增转换或产品规则决策。选定私有捕获仍默认关闭，只允许候选 source
与公共请求字节 SHA 同时匹配的一个 Chat JSON 合成请求；本批未启用真实捕获。

## 产品、候选和本地证据

[复用核验](assets/cpar-build-json-502-20261013/prior-evidence-reuse.json)确认：

| 范围 | 证据及边界 |
|---|---|
| 当前产品 / 构建来源 | apps、crates、build-release、Cargo.toml、Cargo.lock、rust-toolchain 与产品 source `bf822b14a62c21cf582a7539e3635f69bd8b7677` 的 Git 对象完全相同 |
| 本地候选 | Linux ARM64 binary `cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e`，19,565,744 字节；manifest 与内嵌 metadata 匹配，unsigned 两载荷候选 |
| 完整 Task / Phase | 复用冻结 source `c23d2c356ec0253d93e41ca94aac821ad167fcd6` 的 Task42/42、Phase45/45 PASS；gzip 文件与完整解压日志双摘要精确匹配；至当前 HEAD 只有 reports 文档变化 |
| 历史完整两轴 | 产品 / 驱动代码沿用原 review-final 和 review-delivery 的完整 Standards / 本批 Spec PASS；只用于未变实现，不能复用远端资格 |
| 本次公共回归 | **9/9 PASS**，真实 handler、runtime factory、Build decode/projector、SQLite 异步事件与 billing，仅上游 transport 为合成 |
| 本次驱动 / oracle | **18/18 + 54/54 PASS**，启动 Environment 注入、stopped guard、预算持久化、重放拒绝、精确 Usage/执行来源、上传边界 |

公共回归覆盖三协议 JSON/SSE 的等价成功与严格拒绝、错误原因脱敏、服务端 Request-ID、
完整/部分/缺失 Usage、重复事件去重、原 Attempt 与账本来源、选定捕获及私有权限。
完整结果和原始本地日志双摘要分别见[公共回归](assets/cpar-build-json-502-20261013/public-regressions.json)
及[驱动检查](assets/cpar-build-json-502-20261013/driver-local-checks.json)。它们均为
LOCAL_SIMULATED，真实 Provider 请求0，不能算入六组合验收或精确重放旧响应。

本批新增证据的 [docs 门槛](assets/cpar-build-json-502-20261013/docs-gate.json)
在冻结提交 `8873c7ba25dfa442a1049ea2a890ac8b30fb6ffc` 上 **7/7 PASS**。
独立[完整两轴审查](assets/cpar-build-json-502-20261013/review-final.md)
冻结在 `78acbcda00561a38d4964026fec362070a7a487b`：全部30文件，
Standards **PASS / 0发现**、Spec **PASS / 0发现**。
本批复用完整 Task/Phase，不因只有证据变化重做无变化的产品 Full 或构建。
远端 CI、正式发布、部署和生产验收均 NOT_RUN。

## 本次资格检查与永久停止

[只读实际观察](assets/cpar-build-json-502-20261013/readiness-root.json)记录同一账号
证据 `5beff1b6…`、native revision143、同一身份摘要、continuous floor77/current144，
变化字段仅见正常刷新集合，source pool available / active_leases0。
这些是观察，不是新实例的实际 lease 或 Pin 资格；没有主动刷新、重授权或替换。

材料 `expires_at_ms=1791859602393`，观察时剩余 **2893.642秒**。
既有 `remote-prepare.py:108` 要求 `expires_at_ms > now_ms + 3600000`。
[资格判定](assets/cpar-build-json-502-20261013/qualification.json)为
BLOCKED_BEFORE_PREPARE，本批在本地[永久锁存停止](assets/cpar-build-json-502-20261013/terminated-budget.json)。
没有实际执行准备驱动、创建远端 epoch、上传、启动、取得新 lease/声明或发送推理。
没有降低断言或等待自然续期后在本批再核资格。当前本批未用2意图随停止终止；旧58和
上一次未用2也保持终止。本次明确授权不延续旧 epoch。

| 计数 | 历史 | 本批新增 | 累计 |
|---|---:|---:|---:|
| 推理意图 | 35 | 0（Pin0、Chat0） | 35 |
| 网关 / Pin 确认发送 | 34 | 0 | 34 |
| Provider 独立接收 | UNKNOWN | UNKNOWN | UNKNOWN |
| 实际 Provider 费用 | UNKNOWN | UNKNOWN | UNKNOWN |

管理只读 GET 独立于推理计数：资格观察2、成功退出核验1，共3。
先前 readback 失败均发生在管理 GET 之前。没有新 Request/Attempt/Usage/账本，
因为新实例未创建，不能用这点推断旧 Usage0 或账本0表示免费。
本批完整六组合 **NOT_RUN（0/6）**；旧 source `1b6023f5` 的 Responses JSON/SSE
各9/9仍仅证明旧身份的2/6组合、6/18场景；上批首轮502和所有历史失败保留。

## 退出、历史保存与操作限制

[独立退出读回](assets/cpar-build-json-502-20261013/remote-exit-final.json)为 PASS：
NOT_CREATED_VERIFIED；临时 unit MainPID0/not-found，19180/19181无监听或竞争进程，
本批 epoch/upload/audit 均不存在且5秒保持。旧停止 epoch 保持停止，334个历史冻结文件、
14个旧上传载荷摘要一致，保护根363个文件在退出窗口字节不变，私有树权限不变。
原诊断 epoch/捕获继续0700/0600、root owner、无symlink。三份更早 bootstrap 回执
的历史0644权限保留于0700父目录下；没有把它们改写成新捕获权限或修改历史文件。

生产 PID499070、restart0、进程起始、unit/binary/current、活动配置及今日初始 Caddy
摘要在本批保持；今日 Caddy 摘要与10月11日旧报告不同，已记录为本批前既有漂移，
未回退或写生产配置。[Colima观察](assets/cpar-build-json-502-20261013/colima-observation.json)
确认 default / acceptance 两 profile 为 Stopped，本批没有启动、停止或构建操作。

本批保留操作辅助失败：[失败记录](assets/cpar-build-json-502-20261013/operational-failures.json)。
首次只读 SSH 缺 sudo，PermissionError 出现在 ownership 读取时；补齐既有 sudo -n 后
取得观察。首次退出 wrapper 未正确引用 JSON argv；第二次新 checker 错把三份历史0644
回执套入全树0600断言。前两份失败回执、原提交脚本精确摘要和真实 stderr 私有文件保留；
核验随后按已观察历史基线完成，捕获和产品断言未放宽。候选核验器还曾错误查找 manifest
的 payloads 字段，依据实际 files/name 结构闭合，原 PARTIAL 不作 PASS。没有重建任何
失败原始日志，原生响应和凭据未进入公开证据。

## GitHub 与后续条件

三票初始均 OPEN，原进度各一条；依赖分别8/23/0，见
[初始三票摘要](assets/cpar-build-json-502-20261013/github-initial-summary.json)。
只向既有评论追加本次脱敏记录，保留历史前缀、正文/AC、state、元数据及分页依赖，
写入后独立 GET 核对；写入与读回结果单独封存。三票局部修复不满足整体关闭条件。
只提交本批拥有证据，用户 AGENTS 与无关文件保留，见
[保存核验](assets/cpar-build-json-502-20261013/workspace-preservation.json)；
不 push 或发布/部署。

重新开始先需新的明确批次授权，且材料在准备时剩余 **超过60分钟**，并覆盖后续静默
观察和资格冻结。采用已修复驱动的新自有 epoch/载荷，对新绑定做本地验证及完整两轴
审查；实际候选源码若变则依原配方重建。重新冻结 source/binary/manifest、进程/config、
route/model/Client Key、同授权账号材料、实际 lease、egress、声明和有效期。
不得沿用旧 Pin/lease/会话/预算，不主动刷新或新增连续性例外。

建议先另批 **最多2个诊断意图＝fresh Pin1＋PASS后原Chat JSON1**，任何结果停止。
新样本只证明新 epoch；未复现记 NOT_REPRODUCED，不能补样。证据支持的保真修复或
具体规则决策完成后，再单独授权基础矩阵 **最多60个新意图＝6个fresh Pin＋54基础请求**，
已声明扩展另批。这是请求预算建议，不是费用估算或本批继续发送资格。本批不进入下一轮。
