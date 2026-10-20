# #64：实际 Claude Code 的 Messages 独立受控切片

日期：2026-10-20（Asia/Shanghai）。当前结论：**PASS_BOUNDED_OBSERVATION / 实际工具闭环 BLOCKED**。
#64 仍 OPEN / PARTIAL / BLOCKED，保留 #64 → #63 → #61 的原生依赖。用户明确授权从已验证的 #63
受控实现开始实际客户端验收，但这不解除 #61/#63 的整票条件。`cost_in_usd_ticks` 继续 NOT_ADMITTED；
缺失用量和费用 UNKNOWN。真实 Provider、收费账号、发布、部署和 #65–67 均未执行。

## 基线与工作区保留

已核实 #63 本地与远端 `feature/63-build-messages-summary` 的 HEAD 均为
`0d13e485fca189fefd2801b0d2c72c7a991a45df`，包含最终功能提交
`0d196a66d60cbdbb5c5441a158e5d2f33dda7966`。主仓库仍为 #62 分支的
`098c92a51ac549a38c8f15fd265e0fb379de2c77`；历史 SHA 仅定位，没有回退。
本轮建立 `feature/64-claude-code-messages` 和独立工作树
`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar64-claude-code-messages-20261020`，不更新 develop/main。

主仓库原有 9 tracked + 81 untracked、#63 原有 9 tracked + 7 untracked，两个 index 均为空。
字节、模式、symlink 与 HEAD/分支已冻结；#63 的 16 份必要文档/规则 overlay 原样复制并登记，未当作
本轮产品增量提交。主代理亲自阅读 AGENTS、CONTEXT、架构、crate boundaries、质量规则、ADR 索引、
ADR-0040/0100/0108/0112/0114、BC-OBS-007 与相关协议契约；读取 #58/#61/#63/#64 正文、评论和原生依赖。

## 实际安装、运行时与 OS 隔离

| 身份 | 核实值 |
| --- | --- |
| 入口 | `/Users/huangrui/.local/bin/claude` |
| 实际安装 | `/Users/huangrui/.local/share/claude/versions/2.1.302` |
| 文件 | Mach-O 64-bit executable arm64 |
| SHA-256 | `5262ce9de86243fed0d21edc39b271555d56780d0112e2b194439fe1c6662d37` |
| 隔离内运行时 | `2.1.302 (Claude Code)` |
| 实际请求 SDK 标识 | `x-stainless-package-version: 0.128.0`，声明的 runtime `v26.3.0`（不是独立的 engine 版本查询） |

冻结安装当前字节到独占的 OS 临时目录，再运行该相同文件，不换版本、不安装或升级。
每次运行前后核对实际入口、目标文件和冻结副本。驱动为
[`scripts/acceptance/cpar64-claude.py`](../../scripts/acceptance/cpar64-claude.py)。
运行时测试进程、控制器和实际 CLI 分别采用明确环境；CLI 使用独立 HOME、CLAUDE_CONFIG_DIR、缓存、
工作目录及临时目录，只带合成 Client Key。真实用户会话、配置、凭据文件均没有由 harness 读取。
CLI 的内置组件清单仍可出现在 init 中；它不是读取用户插件的证据。

版本/帮助检查阶段完全禁网；实际任务中的客户端及所有工具子进程继承 Seatbelt，只允许本次公共 listener
的 localhost 端口出站；拒绝其他网络，
拒绝 `/Users`、`/Volumes`、任务外临时目录和 Claude 的系统管理配置路径读取，拒绝任务外写入与
securityd lookup。独立 IPv4/IPv6、另一个 loopback 端口、真实用户目录 open、任务外合成凭据 read/write
及实际 shell 子进程的负例均要求 EPERM/拒绝；独立负例接收器 accepts=0，合成 sentinel 保持。
从不以 `--bare` 或环境变量代替这些 OS 证据。

最初运行因 Claude 默认使用 `/tmp/claude-501` 而在隔离外 mkdir 失败，HTTP 请求为零，这是 **harness
环境失败，不是协议 RED**。按[一方环境变量文档](https://code.claude.com/docs/en/env-vars)使用
`CLAUDE_CODE_TMPDIR` 将其内部目录指向本任务后继续；没有放宽任务外写入。原失败保留。

## 原始实际请求与兼容前沿

固定模式为 `--bare --print --no-session-persistence --verbose --output-format stream-json
--include-partial-messages --model grok-4.5 --tools Read,Bash --permission-mode bypassPermissions`，
首问为 `Tell me you are ready to inspect seed.txt.`。工具文件仅为临时目录的合成 `seed.txt`。
没有调整 thinking/effort/cache 来规避当前请求，也没有将 Grok 冒充 Claude。该配置使用官方
[bare/stream-json 接口](https://code.claude.com/docs/en/cli-reference)，不覆盖其他客户端模式。

实际 Claude → 真实 `/v1/messages?beta=true` → 生产鉴权、配置 compiler/factory、路由准入；
已装配实际 leased Build adapter、#63 transport、独立 loopback receiver、正式投影、加密 writer/reader 和计量。
实际请求在租约前即被拒绝，所以本轮实际 CLI **没有走完** leased adapter/上游/投影/摘要链。
独立受控协议探针和 #63 回归会真正经过该完整链，二者分开评价。这里不是 ScriptedClaudeExecutor；
也没有将只覆盖 Codex/Pi 的 #62 旧驱动标为 Claude 成功。

每次原始 request body 保存为 `.bin`，同时记录方法、完整 path/query、必要协议头多值字节、响应状态、
response headers、流 body 字节及 chunk 时间。认证只记来源数量，绝不保存值。恢复到生产 handler 的
payload 与收到的字节一致；重复 JSON member 的回归证明观察器没有将其重编码成合法请求。
这是原 body 与 HTTP 元数据证据，不声称保留原 TCP 分段、HTTP header casing 或 chunk framing。
独立 receiver 另有 TCP accepts、完整接收数及实际 native 请求原字节；不从 durable Attempt 缺失推断零发送。

| 实际字段/行为 | 当前证据、契约与结论 |
| --- | --- |
| `model: grok-4.5` | public / 主候选 upstream 配置均字面同名；实际请求被预拒绝，没有已选择 Attempt 或 native returned model 可验证 |
| `max_tokens: 32000` | 现有精确映射由独立 JSON/SSE 公共探针验证为 `max_output_tokens:32000`；不代表整个 CLI 请求可执行 |
| `thinking: {type:adaptive,display:updates}` | 第一次请求原样出现；无既有准确 Build 映射，BLOCKED |
| `thinking: {type:adaptive}` | 客户端收到 400 后自行去掉 display 的第二次请求；仍拒绝，未由 harness 删字段 |
| `output_config: {effort:high}` | 两次均出现。Anthropic effort 影响文本、工具与 thinking；不能当作已证明的 OpenAI reasoning effort 同义项，BLOCKED |
| `context_management.edits[0]` | `clear_thinking_20251015 / keep:all`。一方定义的是 thinking 历史保留；没有固定 Build profile 对应，BLOCKED |
| `metadata.user_id` | 合成配置生成的 device/session JSON 字符串被原样观察；解码保留，不取得跨协议准入，BLOCKED |
| block `cache_control:ephemeral` | system 两处、首 user block 一处；位置/缓存与用量语义不删除，现有 bridge 拒绝，BLOCKED |
| tool declarations | 实际 Read / Bash 及其 schema 保留；没有收到可执行 tool_use，实际工具执行与结果历史 BLOCKED |
| tool choice / parallel | 原请求未显式携带。独立公共探针验证既有 named tool + `disable_parallel_tool_use:true` 精确映射；不宣称实际并行已验收 |
| `anthropic-version` | 实际 `2023-06-01`；beta 全值及重复值顺序保留于观测，不据 beta 名称推定语义已实现 |
| HEAD / title / count_tokens | 本模式未触发，NOT_OBSERVED；不伪造启动请求或精确 token 数 |

第一方依据：[thinking](https://platform.claude.com/docs/en/build-with-claude/thinking-steering-and-cost)、
[effort](https://platform.claude.com/docs/en/build-with-claude/effort)、
[context editing](https://platform.claude.com/docs/en/build-with-claude/context-editing)。
它们定义 Anthropic 端语义，不构成固定 Build 服务的准确等价证明。本轮据此维持拒绝，没有批准新控制、
未知字段允许名单、数值预算近似、签名伪造或模型别名。直接 HTTP 取文的 403 与 web 工具提取证据分开记录。

实际两次 POST 均返回 400 / `invalid_request_error`；第二次是客户端 fallback，不是网关透明重试。
独立 TCP accepts=0、完整 receiver=0、Request/FailedFinished=2/2、Attempt/Usage/ledger/summary=0/0/0/0，
lease 始终为 0。该零接收由独立 receiver 确认；没有发生 lease=1→0，不能称为取消释放通过。
Claude 退出码 1，输出 `subtype:success` 同时 `is_error:true` 与 api_error_status=400。
驱动同时检查这些字段；客户端生成的零 Usage/total_cost 不当作源计数或免费证明，费用仍 UNKNOWN。

## 验收范围与未满足项

| #64 AC（正文原序） | 本轮结论 |
| --- | --- |
| 1 固定实际 CLI/配置/请求，隔离与外发限制 | PASS_BOUNDED：文件/运行时、OS/子进程负例和原始两请求已固定；其他 CLI 模式不继承 |
| 2 回答、两次实际相关工具与后续问答，经生产 Build | BLOCKED_BEFORE_FIRST_RESPONSE；没有实际工具执行证据，未代填结果或历史 |
| 3 兑现控制/历史，已知不兼容预拒绝且 receiver=0 | 准确拒绝 PASS；完整必需控制兑现 BLOCKED。独立精确控制正例单列为协议证据 |
| 4 不显示摘要、exact Attempt 加密读回、Usage/身份 | 实际客户端 BLOCKED_NO_ATTEMPT；无回答不能证明“不显示且续轮正确”。#63 受控读回回归单列 |
| 5 增量、失败、取消、必要启动/token counting | 实际 400 失败观测 PASS；成功流增量、真实客户端 in-flight cancel BLOCKED。未触发启动/计数明确记录；公共 socket/失败回归单列 |
| 6 独立接收证明无额外外发、可复现与限制 | 当前预拒绝 receiver=0 PASS；完整闭环仍 BLOCKED。真实 Grok/其他 Messages 消费者不继承 |

用户要求的两个实际工具循环、实际结果依赖、后续问答、实际每 Attempt summary 与计量，以及
客户端存活期间 delta→lease1→abort→upstream peer close→lease0，均未被当前预拒绝结果满足。
未启动一个会必然预拒绝的“取消场景”再把正常退出当取消通过，也没有提前 kill 制造 peer close。
不依赖实际成功请求的 #63 公共 FIN/RST、pending ACK、EOF/deadline、失败与资源释放回归继续执行。

## 验证身份与两轴评审

开发定向：`standard_responses::lifecycle` 为 23 passed / 0 failed / 2 ignored；其中新增三个非 ignored
公共测试覆盖实际字段的独立 JSON/SSE 预拒绝、32000/指定工具/串行精确映射，以及原字节/query/多值头/重复
JSON 拒绝的观察器回归。它们是合成协议探针，没有删除实际 CLI 请求字段以冒充兼容。
早期实际 CLI 用显式 `--diagnostic` 运行，观测 oracle PASS、compatibility BLOCKED。最终运行没有使用
`--diagnostic`：外层退出码 **1**（兼容 BLOCKED），内层 ignored 观测测试退出码 **0**，
`client_success:false`。诊断正常退出和 Rust 测试通过都没有升级成客户端兼容成功。

最终产品源码为 `6edb8085f9e850edc55e68d382ed97acbc54a0c5`，tree
`b12794e209bac5d7616151ef9a9143fae8b7fb60`。独立验证树在该提交上叠加原有 16 文件 overlay，
得到本地验证提交 `ad40a2e5f7f8ed05fd21917aabb329e617fac32b`，tree
`12c162ec4a30e0a8079f32ed46b6fd92889f88df`。验证目录为
`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar64-claude-20261020-06f779f24434`；
target 为 `/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-20261020/target-full-06f779f24434`。
目录名保留首次候选的短 SHA，但最终身份以上述提交、tree 和 manifest 为准。target 只在相同验证目录与
manifest 路径中复用，没有从其他工作树移入带旧 `CARGO_MANIFEST_DIR` 的可执行文件。

| 验证类别 | 最终证据与边界 |
| --- | --- |
| 独立 overlay Full | **PASS 45/45**；Rust **1575 passed / 0 failed / 15 ignored**，127 组；441.166 秒 |
| 最终实际 CLI（默认忽略，另行 opt-in） | 观测 **PASS_BOUNDED**，兼容/工具闭环 **BLOCKED**；实际客户端文件、配置、393 个源码/manifest/lock/driver 输入与 Full 候选一致 |
| Standards | **PASS**；独立初审及 identity 小增量补审均为 0 硬违规、0 Fowler 项；审查本身为静态只读 |
| Spec | **PARTIAL_BLOCKED**；未确认新增 bug / scope creep；完整工具、摘要与活客户端取消 AC 仍未满足 |
| 无既有 overlay 的纯产品树 Full | **NOT_RUN**；不能把 overlay Full 推广为纯提交 Full |
| 远端 CI / 真实 Provider / 发布 / 部署 | **NOT_RUN**；没有主动触发远端工作流或连接真实账号 |

Full 前后逐一复核 6980 个验证输入的字节、模式、symlink，差异为零，验证树保持 clean。
`Cargo.lock` SHA-256 为 `ee3d735f0f7aa364f3f4675e75a87751c8ad3784f0788ac244c4d391ce479cce`。
最终实际运行的 393 项输入又与 Full manifest 和产品工作树分别核对，无差异。实际 opt-in 与 Full 的
gateway 测试可执行文件因 Cargo feature 组合不同而分别记录 SHA，不声称二者是同一个二进制。
后续收尾提交仅增加报告和证据，不改变这些执行输入，也不声称对新增证据 metadata 重新执行了 Full。

首轮 Full 在旧候选 `06f779f24434f7de95426b05f33484a1c4e33a15` + overlay
（验证提交 `d28d25f3d7cd84eed8de77a3fcda5d046be0d8f9`）**FAIL**：一项既有断连测试在取消之前，
没有在原 1 秒启动窗口内观察到 lease=1 且 receiver=1。失败日志完整保留；相同原二进制单测重跑
通过（FIN 释放观测 7ms），未改变断言、超时或等待窗口，原失败原因 **UNKNOWN**。
最终 Full 的通过不抹去这条历史失败，也不据此宣称找到或修复了生产取消根因。

为导出逐项回执，又在最终 Full 的同一固定源码与原测试二进制上执行三个指定协议回归：
20 条 exact-owner 原生摘要授权读回、4 组 Request/Attempt/Usage/账本/独立 receiver 一致性矩阵、
JSON/SSE 各一条 32000/named/serial 精确控制探针，以及 Messages 公共 FIN/RST 释放回归均通过。
4 组各 5 请求是该合成矩阵自身的证据；实际 Claude 的独立计数仍为 2 个公共请求、0 个上游接收。
这些协议测试的 `actual_client:false` 明确保留，不替代实际工具执行、隐藏摘要续轮或活客户端取消证明。
#63 的 summary 例外继续仅适用于 Grok Build + Messages + CanonicalBridge；Responses、Passthrough exact、
其他 Provider/reasoning 与未知字段未新增例外。

## 可复现入口与证据

在本工作树运行下列命令会重新冻结当前安装文件、证明 OS 隔离，再执行单独的实际 CLI 观测。
`--output` 必须是尚不存在的 OS 临时目录；若入口已更新，记录并使用更新后的实际身份，不降级。
当前客户端的预期结果是外层非零退出与 `run.json` 中兼容 BLOCKED。

```sh
python3 /Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar64-claude-code-messages-20261020/scripts/acceptance/cpar64-claude.py \
  --output /private/tmp/cpar64-reproduce-new \
  --target-dir /Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-reproduce-target
```

若只需复核拒绝观测 oracle，可显式加 `--diagnostic`；它不验收成功工具闭环。独立验证 Full 使用
`CARGO_TARGET_DIR=<上述独立 target> CHECK_REPORT_PATH=<独立报告路径> scripts/check.sh full`，
源码、规则 overlay 与工具环境必须先按 manifest 核对；不将该命令解释为允许缺规则的工作树跳过检查。

- [源码、393 个执行输入和 16 个 overlay 的身份](assets/cpar-64-claude-20261020/source-manifest.json)
- [最终实际客户端配置、原始请求摘要与计数](assets/cpar-64-claude-20261020/actual-cli.json)
- [协议摘要读回、计量及精确控制回执](assets/cpar-64-claude-20261020/controlled-regression.json)
- [最终 Full 核验](assets/cpar-64-claude-20261020/full-final-receipt.json)与[45 步报告](assets/cpar-64-claude-20261020/full-final-check-report.md)
- [Standards](assets/cpar-64-claude-20261020/review-standards.md)及[Spec](assets/cpar-64-claude-20261020/review-spec.md)独立评审
- [原工作区保留核验](assets/cpar-64-claude-20261020/preservation.json)及[一方语义依据](assets/cpar-64-claude-20261020/primary-source-evidence.json)
- [158 成员原始 RED / 观测 / 失败 / Full 归档](assets/cpar-64-claude-20261020/verification-logs.tar.gz)与[逐成员 SHA-256 索引](assets/cpar-64-claude-20261020/archive-index.json)

归档包括原样请求/响应 `.bin`、必要协议头观测、隔离负例、客户端 stdout/stderr、生产路径计数与
初次/最终 Full 日志。认证值、用户配置/会话、二进制、密钥/数据库、临时 HOME/cache/work 及整页文档
提取不发布。完整源码路径和 Cargo artifact 清单保留本地，以 digest 登记；发布 393 项执行输入和
精确 overlay 身份。归档解压成员使用项目原 secret regex 与禁止路径规则逐一检查，再回读比对全部字节
和 SHA-256；158 项均通过。原主仓库 90 个脏文件及 #63 的 16 个脏文件字节、模式、symlink、HEAD、
分支和空 index 均保持；本轮产品提交不包含原 16 文件 overlay。

证据 metadata 增量完成后，文档链接（1230 文件）、契约测试引用（250 项）、计划状态、canary/Caddy
约束、owned staged secret scan 和 diff whitespace 静态检查均通过；393 个执行输入再次核对一致。
没有为纯报告增量重复 Full，也没有新增远端或真实 Provider 验收结论。

## 最小剩余工作

先为当前冻结客户端必需的 adaptive thinking/display、effort、上下文编辑、metadata 与 block cache controls
取得适用于固定 Build profile 的准确语义/契约和实现资格；当前一方 Anthropic 文档不能替代这个条件。
在此前提没有满足时继续准确预拒绝，不做字段剥离或近似映射。资格成立后，在同一生产装配路径补做真正的
普通/增量回答、两次实际关联工具、结果/后续历史、逐 Attempt 完整 summary 读回与计量，以及客户端仍存活
时的取消/断连观察。#61/#63 整票、真实服务 profile、真实 Provider 与发布门槛继续独立保留。
