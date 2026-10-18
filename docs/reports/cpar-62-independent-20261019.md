# #62 独立准备：实际 Codex / 原版 Pi CLI → 标准 Build Responses

执行日期：2026-10-18 至 2026-10-19（Asia/Shanghai）。**切片 PARTIAL；#62 保持 OPEN / BLOCKED。**
#61 的 cost 仍为 **NOT_ADMITTED**，其正式依赖、全部验收条件与用户等待一手依据的决定保持。
本报告只证明固定 CLI、固定配置、合成身份和标准响应形状下的本地受控行为。

## 起点与授权

实际基线为 `codex/prism-v4-delivery` / `1743dd0801ac5f99fd80a98253d341205b3a465b`，没有回退。
已读根/祖先规则、CONTEXT、issue tracker、相关模型身份、Usage 留存与流生命周期 ADR/契约，
以及 [#61 定向材料](cpar-61-targeted-evidence-20261018.md)和[续作报告](cpar-61-continuation-20261018.md)。
#58/#61/#62 最新正文、全部评论和原生依赖已读回；#62 的唯一 blocker 仍是 #61。
本轮只认领 #62，并获准追加独立准备结果；没有修改 #61 或其他工单。
[基线回执](assets/cpar-62-independent-20261019/baseline-summary.json)固定 55,696 个既有项。
既有文档修改、未跟踪材料和历史失败均保留，不属于本轮提交。
[保留核对](assets/cpar-62-independent-20261019/preservation-final.json)证明 55,636 个常规文件、10 个符号链接
逐项不变；50 个目录项仅核对目录元数据，不宣称递归核验嵌套仓库后代。唯一原有代码文件只增加 `mod cli;`。
完整原始路径/状态库存留在私有 run；公开库存以路径 SHA256 绑定，避免把历史路径名称写入公开材料。

## 实际客户端身份

| 对象 | 固定实际入口及运行结果 | 加载身份和边界 |
| --- | --- | --- |
| Codex CLI | `/opt/homebrew/bin/codex` → `@openai/codex/bin/codex.js`；实际 `codex-cli 0.161.0` | macOS arm64；平台可选包解析为 MODULE_NOT_FOUND，wrapper 使用本包 `vendor/aarch64-apple-darwin/bin/codex`；不是仅引用 npm 声明 |
| 原版 Pi CLI，本轮推理入口 | `/opt/homebrew/bin/pi` → `/opt/homebrew/lib/node_modules/@earendil-works/pi-coding-agent/dist/cli.js`；实际 `0.84.3` | CLI 私有树中的 `@earendil-works/pi-ai`、agent-core/client/protocol/tui 均为 `0.84.3`；实际 ESM trace 确认加载该 SDK 的 `dist/api/openai-responses.js` 及其私有 OpenAI Node `6.40.0` |
| 包声明的 Pi bin | package.json 的 `bin.pi = dist/bundle/cli.js`；单独版本探针亦为 `0.84.3` | 与现存 shell symlink 不同；只验证身份，没有改链或用 bundle 替换本轮推理入口 |
| only-my-pi | `/Users/huangrui/.local/bin/pi` → 独立 only-my-pi 安装栈 wrapper | 不作为原版 Pi；没有执行其推理、修改安装栈或使用其 SDK 代替原版 CLI |
| Node | `/opt/homebrew/Cellar/node/26.8.2/bin/node`；实际 `v26.8.2` | 原版 Pi 的解释器；Codex JS wrapper 也由这一 Node 启动 |

安装文件的内容、mode、symlink 与解析目标在执行前后逐项比较；实际 ESM 加载路径单独留存。
Codex 所选合成模型 `grok-4.5` 没有内置客户端模型元数据，CLI 明确警告使用 fallback metadata；
这项实际条件保留，没有改模型、补猜 effort、删除工具或改客户端代码。

## 可复现入口与隔离

从本任务指定的 Asgard 仓库根目录运行；仓库须位于操作系统临时目录之外，以便外部写入探针有真实的禁止目标。
新 checkout 先按既有锁准备构建依赖（本机离线安装已验证）：

```sh
npm --prefix web/prism ci --offline --ignore-scripts --no-audit --no-fund
```

输出必须是操作系统临时目录下一个尚不存在的新目录：

```sh
/usr/bin/python3 scripts/acceptance/cpar62-cli.py --output "${TMPDIR%/}/cpar62-new-run"
```

入口会编译精确 gateway 测试二进制，固定客户端实际版本和安装树，再执行 opt-in 的真实 CLI 测试。
测试通常不会进入普通 Full 的默认执行；Full 会编译它并明确计为 ignored，实际执行结果单独列出。
默认入口验证限定准备结果，输出仍明确写出 Codex BLOCKED 和 #62 OPEN/BLOCKED。

额外的一秒快速释放探针可重复运行：

```sh
/usr/bin/python3 scripts/acceptance/cpar62-cli.py --require-prompt-release \
  --output "${TMPDIR%/}/cpar62-prompt-release-new-run"
```

这一秒来自原有测试 helper 的诊断窗口，不是 #58/#62 新增的产品 SLA。严格探针保留 FAIL；
有界释放与快速释放分别报告，不把延长观测后的释放写成一秒通过。

每个场景有独立 HOME、CODEX_HOME、Pi agent dir、会话/历史、凭据、临时目录和工作目录；环境只传白名单，
CPAR Key 和 Build 账号均由隔离 fixture 合成。Pi 保持 reasoning 能力，未设置 thinking off 或禁用工具。
只禁自动加载外部扩展、skills、项目规则、主题等隔离所需资源；默认 read/bash/edit/write 工具保持。
工具实际只读取临时 `seed.txt`，并在同一目录写入和读回 `result.txt`。
没有改全局 PATH、安装、用户配置、真实账号或其他项目。

所有测试二进制、driver、CLI 及其工具继承同一个 Seatbelt 边界：仅 loopback 网络；拒绝真实 `/Users`
目录读取及临时目录外写入。启动 CLI 前，IPv4 `192.0.2.1:443`、IPv6 `[2001:db8::1]:443`、
仓库文件的非截断写打开及真实用户目录的只读打开均实际返回 **EPERM**，否则停止启动。
实际 CLI 仍可连接真实 CPAR loopback 监听器。该证据来自内核限制及拒绝探针，不是只看 base URL 配置。

Codex 自定义 provider 的 `base_url`、`env_key`、`wire_api=responses` 与认证方式依据
[官方配置参考](https://learn.chatgpt.com/docs/config-file/config-reference)核对；运行事实仍以安装入口与实际请求为准。

## 生产链与测试替换边界

实际 CLI 进程 → 真实 Actix 公开 HTTP 监听器 → Client Key 鉴权 → production compiler/factory/路由及执行链
→ 实际 leased Build adapter → 既有测试 socket handoff → 真实 UpstreamClientPool → 独立 loopback TCP 上游。

[新测试入口](../../apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/cli.rs)
复用相邻 lifecycle、fixture、记录 writer、reader、ledger 和 transport。所有新增 Rust 都在既有 `cfg(test)`
模块下；生产程序没有新增开关、任意 endpoint 或兼容演示服务。唯一替换点仍是已构造固定 native request 的
受控 transport handoff；生产构造、解码、投影、模型绑定和生命周期不变，Build profile 继续为 1.0.50。

被动观测 middleware 转交 byte-identical 原始请求，保留实际控制；只记录安全 HTTP header 白名单，绝不保存 Authorization。
独立接收数在上游收到完整请求后记录，不能由 send 计数代替。响应体逐块转发并观测，未缓冲后伪装增量。
无凭据请求返回 401；给 Pi 请求增加已知不兼容 background 的独立负例返回 400，接收和 Attempt 增量均为零。
这条负例不作为 CLI 原始请求或成功样例；两个 CLI 自己发送的请求均未经改写。

## 实际请求与兼容矩阵

| 项目 | Codex 0.161.0 exec | 原版 Pi 0.84.3 RPC |
| --- | --- | --- |
| 模型、流 | `grok-4.5`、`stream=true` | 同左 |
| reasoning | `summary=auto`，**effort 缺失** | 实际默认 `effort=medium, summary=auto`；没有猜 budget/effort |
| include | `reasoning.encrypted_content` | 同左，完整交给生产 Build 请求构造；fixture 只提供原生 summary，不证明真实密文资格 |
| 工具 | function 工具之外，还发送 namespace `multi_agent_v1` 与 web_search | 原版内建 read/bash/edit/write，完整声明和参数保留 |
| 并行/选择 | `parallel_tool_calls=true`、`tool_choice=auto` | 未发送 parallel_tool_calls/tool_choice；没有把顺序两轮宣称并行通过 |
| 保存/续接 | `store=false`、prompt_cache_key；未发送 previous_response_id | 同左；完整显式历史回传；Build cache key 按已有 owner scope 派生，未原样泄露会话 key |
| 额外实际控制 | client_metadata 等照实留存；未删除 | max_output_tokens 实际默认 16384，保留到上游；未人为缩小预算 |
| 原始请求结果 | **FAIL / BLOCKED**：400 ClientRequestError，独立 receive/Attempt/Usage/ledger 均为 0 | **限定标准路径 PASS**：回答、两次关联工具循环、后续问答，共 5 个上游请求 |
| 失败 | 客户端唯一 turn.failed；未开始原生 SSE，原生失败场景 NOT_RUN/BLOCKED | 原生 failed + 已知 Usage → 公共唯一 response.failed、CLI error，无重发 |
| 取消 | 无可成立的流；NOT_RUN/BLOCKED | CLI 保持存活并 abort，Request Cancelled，约 15 秒后 socket/lease 释放；快速释放探针 FAIL，时序限制单列 |

Codex 的 namespace/web_search 被现有 Responses codec 的 function-only 工具契约拒绝；
summary-only reasoning 从静态源码看同样缺少当前 Thinking 表示所需的 effort；没有裁剪原始请求重发以绕过前一门槛。源码位置为
[decode_tool / decode_reasoning](../../crates/protocol-openai-responses/src/lib.rs)。
未通过删除字段、禁用这些工具、切换客户端实现或猜一个 effort 制造成功；
扩充工具种类或定义新的 reasoning 语义不属于可由本轮既有契约证明的最小修复。

Pi 在同一 RPC 会话中先回答，再完成 `read(seed.txt)` → 结果 `41\n` →
`bash` 计算/保存/读回 → 结果 `42\n` → 回答 → 后续问答。独立接收历史证明：
原生 summary 的 id/结构/文本、工具 call ID、完整参数、结果关联与顺序保留。
Pi 回传 function_call 时不重复原生 function item id，但 call_id 保持；不宣称所有字段整包 wire 保真。
标准成功公共流每次只有一个 completed，所有模型均为合成字面三段等值，Usage 为 input=20/output=7/total=27。

## 取消、失败用量与 exact Attempt 的限制

取消测试在实际 CLI 收到文本时观察真实 lease=1，再发送 RPC abort。收到 aborted 后继续观测连接和 lease，
等待期间检查进程未退出，观测结束后再采样同一 PID 的存活状态，随后才允许清理进程。
[结束存活证明](assets/cpar-62-independent-20261019/liveness-fixed/actual-cli/pi-cancel/cancel-client-alive)
与 abort-complete、连接/lease 观测共同证明 CLI 覆盖整个窗口；不能用进程退出或 server.stop 代替取消证明。
独立上游在约 15 秒收到关闭，lease=0，均早于 CLI 退出和停止 HTTP server；只收到 1 个请求。
公开流观测到一次合法 keepalive comment，没有伪造 completed/failed 或读取新 Usage。
HTTP/1.1 FIN 与合法写半关闭的歧义、Actix 持续响应以及既有 15 秒 SSE keepalive 与该观测吻合；
这是依据源代码和可见 keepalive 的解释，未以抓包断言具体 FIN 时序；不是 #61 source-drop 修复失败的证据，
不把提前取消合法 half-close 的策略作为本轮生产改动。
保留一秒探针的原始 RED，不能由有界释放替代快速释放资格。

失败场景的公共 response.failed 及 durable Usage/ledger 保存 20/7；Pi 的错误消息将用量显示为其默认 0。
这是实际客户端差异，不能将其默认零值当已验证零用量或真实零费用。未修改客户端，也未伪造成功终态补用量。

成功 5 次、失败 1 次、取消 1 次的 Request/Attempt/Usage/source-event/ledger 分场景核对。
已知 Usage 均绑定 exact Request/Attempt/owner，与 ledger source-event 一一对应、无重复，cost 保持 UNKNOWN。
取消未接收 terminal Usage，Usage/ledger 均为 0 行，不据此推出上游执行免费。
独立 background 负例增加 1 条失败 Request，不增加 Attempt、Usage、ledger 或上游接收；不会混入五次 CLI 请求。

通过实际 owner-scoped reader 核对：本轮标准成功/标准失败没有完整原生 Usage 加密 companion。
这就是现有能力边界，标 UNKNOWN/BLOCKED；#59 失败留存不能替代尚未实现的 #61 成功元数据留存。
没有给成功 fixture 添加 cost、未知 Usage、root/item/part 字段或实际模型别名，也没有重新搜索已无新材料的 cost 来源。

## #62 全票 AC 映射与剩余项

| AC | 本轮结果 |
| --- | --- |
| Codex 1 身份/隔离 | PASS，限定固定 exec / fallback metadata 模式 |
| Codex 2 实际工具闭环 | BLOCKED，原始请求在发送前拒绝 |
| Codex 3 控制与历史 | PARTIAL：真实请求/准确拒绝/零接收已取证，客户端必需工具及 summary-only 语义未兑现 |
| Codex 4 流/失败/取消/Usage/加密 | PARTIAL：原始 400/唯一失败通过；原生流、工具、取消 NOT_RUN/BLOCKED；成功加密依赖 #61 |
| Codex 5 实际模式资格 | BLOCKED，namespace/web_search 等实际行为未获资格，不能称客户端可用 |
| Codex 6 复现/范围 | PASS，本地限定配置与失败记录已交付 |
| Pi 1 入口/依赖身份 | PASS，原版 CLI/SDK/only-my-pi 分开 |
| Pi 2 隔离/公开 Build 工具闭环 | PASS，限定标准合成 SSE 与顺序工具模式 |
| Pi 3 reasoning/history | PASS，实际 medium/auto/include 与 owned summary 身份、call/result 顺序取证 |
| Pi 4 请求控制 | Scoped PASS，原始控制保留；已知不兼容负例零接收；不证明未发送的并行/服务端续接 |
| Pi 5 失败/取消/用量/加密 | PARTIAL：有界取消/唯一失败/网关计数与 ledger 已核对；快速释放、Pi 失败用量显示及 #61 成功加密仍有限制或阻断 |
| Pi 6 复现/范围 | PASS，仅本机固定原版 CLI，不外推其他发行形态 |

整票缺口继续保留；真实 Grok Build、真实模型关系、代表性原真实样本、桌面客户端与首版通过均 NOT_RUN/BLOCKED。
#61 的至少一个实际字段资格、成功 bytes/终态前完整加密确认和授权读回仍是正式前置。
没有改依赖、关闭 #62、改验收条件或推进 #63–#67。

## 验证与评审

六份新增/接入测试文件初始候选为 `062775a9`；最终代码候选为 `6490922141f64aa4222c907634b02502cffb11cd`。
没有生产协议、adapter、依赖或 endpoint 行为修改。Grok factory 公共定向回归为
**28 passed / 0 failed / 2 ignored**，包括旧 #59/#60/#61。实际 CLI opt-in 用例由单独入口执行，
不能用 Full 中的 ignored 替代其运行结果。Rust format、Python syntax、严格 Clippy、秘密扫描及空白检查均通过。

[首轮双轴评审及后续反例](assets/cpar-62-independent-20261019/review-round1.json)保留：
Standards 为 0 个 documented breaches、0 个 Fowler judgement findings；Spec 为 3 类整票完成缺口。
评审另指出取消判据的 P2 正确性漏洞：原驱动只在 aborted 时检查一次进程，之后可能把提前退出引起的释放误记为存活取消。
在同一真实 CLI 链路，发送 aborted 后主动 SIGTERM 结束 Pi，旧判据仍通过并写 `cli_kept_alive=true`；
修复后同一反例准确失败，错误为 `Pi RPC exited during cancellation observation`，不再声称整段存活。
这是**测试判据 RED/GREEN**，不是生产兼容修复或 Provider 证据；历史通过回执不再作为整段存活证明。
最小修复只增加等待期间的退出检查、观测结束的同 PID 存活证明，以及 Rust 对该证明的核对。
其他早期 harness 的事件索引、沙箱、计数、keepalive parser、语法与编译反馈均保留，未改写为生产 RED/GREEN。
必要的最小生产兼容修复为 **无**：Codex 发现项没有既有契约支持的安全最小放行。

最终不可变源码 `2fc4777abbac3e6b203323a0884d7a011308bac3`，tree `01bdff9c81bbfb8d30ce0a2f8da9eb4406a90946`：
**Full PASS 45/45；Rust 1542 passed / 0 failed / 14 ignored；400.288 秒**。
见 [源码与六文件哈希](assets/cpar-62-independent-20261019/liveness-fixed/full-liveness-source.json)、
[Full 步骤](assets/cpar-62-independent-20261019/liveness-fixed/full-liveness-steps.md)和
[汇总](assets/cpar-62-independent-20261019/liveness-fixed/full-liveness-result.json)。
同一提交、同一 Asgard 验证 checkout 随后执行两次实际 CLI，tracked/index 始终干净；
16 份既有用户文档 overlays 只用于验证，不进入当前分支增量。

[最终实际 CLI 回放](assets/cpar-62-independent-20261019/liveness-fixed/liveness-cli-replays.json)：
默认准备检查 **1 passed / 0 failed**；严格一秒探针 **0 passed / 1 failed**，失败原因恰为该快速释放诊断。
默认回放在同一 Pi PID 存活窗口内 **14,975 ms** 观察到 peer close=1、lease=0；严格回放为 **14,979 ms**。
两次均有观测结束的正向存活回执，之后才清理进程。安装树逐项未变，六份代码与 Full 哈希一致。
[实际请求/结果矩阵](assets/cpar-62-independent-20261019/liveness-fixed/actual-cli/compatibility-matrix.json)、
[客户端身份](assets/cpar-62-independent-20261019/liveness-fixed/actual-cli/client-identity.json)及
[严格探针](assets/cpar-62-independent-20261019/liveness-fixed/prompt-release-red/pi-cancel/cancel-diagnostic.json)分别保留。
默认准备检查 PASS 不意味着 Codex 可用、快速取消通过或 #62 全票通过。

旧源码 `28d1c42f` 的 Full 45/45、实际 CLI 回放、一秒探针 FAIL 和原始日志全部保留为历史证据，
不替代新代码候选的 Full 或加强后的整段存活证明。首次 Full 的临时 cargo wrapper 误套 soak guard、
临时目录 checkout 的外部写入探针失败、Asgard checkout 首次缺 Prism 构建依赖，均原样保留；
这些前置失败都未启动真实 Provider。按锁离线准备依赖后才执行当前同提交验证。

[历史归档](assets/cpar-62-independent-20261019/historical-validation-manifest.json)、
[首版归档](assets/cpar-62-independent-20261019/final-validation-manifest.json)和
[修复后归档](assets/cpar-62-independent-20261019/liveness-fixed/validation-manifest.json)
分别绑定字节、哈希与逐成员读回；修复后归档 96 个成员全部独立读回一致。
所有拟提交明文证据及解压安装库存均执行同一仓库秘密策略。Cargo compiler-artifact JSON 的文件名后缀触发 key 形状规则，
公开构建诊断只保留 message 分类及 stderr；原始 JSON 与 SHA256 留在私有 run，扫描器未修改。
巨量原始 ESM debug 亦保留私有 run，仓库保存实际加载路径、安装清单及 stderr 精确哈希。

固定基线 `1743dd0801ac5f99fd80a98253d341205b3a465b` 到候选 `368c9b92ed743f3f43700d1a59d6b2eb44359c47` 的复核结果见
[最终双轴评审回执](assets/cpar-62-independent-20261019/review-final.json)。Standards **PASS，0 条规范违规、0 条 Fowler 判断项**；
Spec **PARTIAL / BLOCKED，3 类整票完成缺口、0 项 scope creep，未发现最终增量新增行为缺陷**。
同轴最严重事项分别为：Standards 无；Spec 为 Codex 必需工具闭环仍阻断。原 P2 测试判据漏洞已独立确认封住。
复核后只补评审与远端读回文档，没有源码、fixture 或断言变动。
已追加[一条授权 #62 独立准备评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/62#issuecomment-6252691392)，
[独立读回](assets/cpar-62-independent-20261019/remote-result-readback.json)确认正文 SHA256 完全一致，
#62 仍 OPEN，唯一 blocker #61 的原生 ID 不变，#62 的 blocking 边也原样保留。
#58/#61 的正文、状态、标签、认领、全部既有评论与自身 updatedAt 前后相同；所有原生依赖 ID、方向及状态不变。
依赖 GET 中嵌入的 #62 对象仅 comments 从 0 到 1、updated_at 随本条评论变化，已逐字段绑定到这次写入窗口；
不能把这个引用对象变化写成 #61 被修改。#62 未改写正文或原 AC。
本轮分支提交只含自有代码/报告/资产，原有修改和未跟踪库存保留；#62 继续 OPEN / BLOCKED。
真实 Provider、真实账号/新预算、push、部署、发布与远端 CI 均 NOT_RUN。
