# 首版三客户端候选支持清单（2026-10-23）

> **请求兼容性更新（同日 ADR-0125）：** 已实现 CPA 风格的 adaptive/effort、预算与缓存提示转换，
> 并完成本地公开 HTTP、JSON/SSE 和工具续轮验证。下面的 D2 和原始 Claude 状态是历史受测源码的记录，
> 不再代表当前请求入口必然拒绝 M1/M2/M5；当前源码的实际原版客户端与真实服务验收仍待重跑。
> 详见 [当前规则](../adr/ADR-0125-cpa-request-compatibility.md)。首版尚未 READY 的结论不变。

**产品范围已采用 Claude D2；首版尚未 READY。** 首版仍须 Codex、Claude Code D2、Pi
全部通过 Grok Build 完成约定闭环。[ADR-0123](../adr/ADR-0123-first-release-claude-d2-restricted-support.md)
落实本次选择；原始 Claude 的 adaptive/display、overall effort、block prompt-cache
继续作为 #58/#64 后续待办，既不删除，也不标完成。

本清单整理既有证据。本轮只核验规格、配置、引用和状态，没有新跑三客户端、Full、binary、
OCI、恢复矩阵或真实 Provider。历史报告的 NOT_DECIDED 与 RED 按原时间保留。

## 固定客户端与配置

| 客户端 | 受测版本、实际入口 | 配置与协议 | 已有证据及剩余条件 |
| --- | --- | --- | --- |
| Codex | CLI 0.161.0；`/opt/homebrew/bin/codex`；原生 `/opt/homebrew/lib/node_modules/@openai/codex/vendor/aarch64-apple-darwin/bin/codex` | 独立 HOME/CODEX_HOME；provider=cpar62，model=grok-4.5，wire_api=responses，requires_openai_auth=false，env_key=CPAR62_TEST_KEY；request_max_retries=0、stream_max_retries=0；原生 app-server --listen stdio:// | 原样实际客户端受控正常、磁盘 resume、失败与同进程 interrupt 见 [Codex 组合报告](../reports/cpar-62-codex-combined-20261022.md)；D2 同源码回归见下表。固定 Build 的 C1–C6/history/ParallelTools 等适用资格仍独立 BLOCKED；未资格负例保持400/零发送。 |
| Pi | 原版 CLI 0.84.3；`/opt/homebrew/bin/pi`；记录的实际脚本 `/opt/homebrew/lib/node_modules/@earendil-works/pi-coding-agent/dist/cli.js`；Node26.8.2，实际 `/opt/homebrew/Cellar/node/26.8.2/bin/node` | 独立 HOME/PI_CODING_AGENT_DIR；models.json 的 provider=cpar62、baseUrl=受控listener、api=openai-responses、model id=grok-4.5、reasoning=true；RPC；offline/no-approve/no-extensions/no-skills/no-prompt-templates/no-themes/no-context-files | 两工具/后续问答、失败保留已知20/7、活进程 abort 受控通过。失败界面零用量显示的既有局限保留，不等于 Provider 零用量。only-my-pi 未执行；不改变另一项目的Node政策。固定服务/真实 Provider 待验。 |
| Claude D2 | Code 2.1.302；`/Users/huangrui/.local/bin/claude` → `/Users/huangrui/.local/share/claude/versions/2.1.302`；arm64，0755；SHA256 `5262ce9de86243fed0d21edc39b271555d56780d0112e2b194439fe1c6662d37` | 独立 HOME/CLAUDE_CONFIG_DIR/tmp/work；--bare --print，stream-json 输入/输出，Read/Bash；Messages → GrokBuildResponses/CanonicalBridge；所有模型选择固定grok-4.5；**完整四项D2配置见下方** | **已采用，PASS_LOCAL_CONTROLLED**；首次 effort 本地拒绝后由客户端另发新Request。两工具、续轮、摘要/M4读回、永久失败与活PID取消均有证据。固定服务资格、#66与#67仍未满足；其他版本/平台/桌面形态不继承。 |
| 原始 Claude | 同一 Code 2.1.302、入口/二进制/哈希；无D2四项控制 | 原始adaptive/display、effort、block cache请求；相同隔离Messages/grok-4.5入口 | **BLOCKED_BEFORE_FIRST_RESPONSE**；两次400，独立receiver/Attempt/Usage/ledger/summary=0；有效M4仅Request证据。M3/M4已有独立局部契约，不能代替M1/M2/M5原始要求。后续须有准确固定Build等价依据、实现和独立实际客户端验收。 |

Pi 的 package 声明 bin 是 `dist/bundle/cli.js`，与回执记录的实际 `dist/cli.js` 分列；不把它们
混写成同一个已核验入口。版本/入口记录见 [Codex/Pi identity](../reports/assets/cpar64-d2-candidate-20261022/codex-pi-client-identity.json)、
[Claude identity](../reports/assets/cpar64-d2-candidate-20261022/d2-client-identity.json)。这些是受测时的冻结身份，
本轮没有重新执行安装文件确认其当前版本。客户端构造复用 [cpar62-cli.py](../../scripts/acceptance/cpar62-cli.py)
和 [cpar64-claude.py](../../scripts/acceptance/cpar64-claude.py)，配置中的 key 仅来自合成fixture。

Claude D2 必须同时采用：

```sh
DISABLE_PROMPT_CACHING=1
CLAUDE_CODE_DISABLE_THINKING=1
CLAUDE_CODE_MAX_RETRIES=0
CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1
```

[可复制局部配置](grok-build-messages.md)使用占位凭据、清空继承环境、独立目录及注释掉的客户端命令，
不会自动发请求。不得遗漏普通重试或非流式fallback设置。省略thinking不证明模型停止推理；网关不删字段、
不伪造上游错误、不透明重发。Messages不展示Grok summary，但完整加密留存和工具续轮要求保持。

## 产品选择与各层资格

| 层级 | Codex | Pi | Claude D2 | 原始 Claude |
| --- | --- | --- | --- | --- |
| 产品选择 | 首版必需 | 首版必需 | **ADOPTED**，本轮ADR-0123 | M1/M2/M5后续义务，未完成；不属于已选D2配置 |
| 实际客户端受控验收 | PASS_LOCAL_CONTROLLED，版本/零重试/受控资格限定 | PASS_BOUNDED_CONTROLLED，版本/RPC配置限定 | PASS_LOCAL_CONTROLLED，版本/四变量限定 | BLOCKED_BEFORE_FIRST_RESPONSE；准确拒绝PASS不等于兼容 |
| binary/OCI生命周期 | 共享交付binary历史144/144、原样OCI历史290/290；**不是客户端执行** | 同左 | 同左；未在该binary/OCI运行D2 | 同左；生命周期绿灯不能解除原始请求阻断 |
| 固定Build服务资格 | BUILD_ACTIVATION_BLOCKED | BLOCKED，须核对适用字段/profile/模型/续轮 | BLOCKED，D2采用未授予资格 | BLOCKED，原始控制仍无准确准入依据 |
| 三客户端真实Provider验收 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN；不纳入D2结果 |

Codex的namespace子工具和hosted search在实际组合场景中 **NOT_CALLED**；parallel=true是字段共存，
实际两个工具跨响应依赖执行。单独受控ParallelTools/JSON/SSE证据不改称实际客户端已调用所有能力。
Claude D2未发送context_management，不能把M3与其他字段共存的契约正例改称实际调用证明。

## 证据来源和不可变源码

| 证据 | 固定身份及可证明内容 |
| --- | --- |
| 三客户端同产品输入受控记录 | [D2报告](../reports/cpar-64-d2-candidate-20261022.md)的受测代码 `4015bd4542dbc52fde8984c14c01b34843ef3c59`；[D2 strict result](../reports/assets/cpar64-d2-candidate-20261022/d2-run.json)、[Codex/Pi strict regression](../reports/assets/cpar64-d2-candidate-20261022/codex-pi-run.json)。每客户端正常路径均两次真实相关工具、五次执行和后续问答；失败/活进程取消另测。Claude正常6个Request含1次协商拒绝、5个receive/Attempt/Usage/ledger；失败2个Request含1次拒绝、1次执行；取消2个Request含1次拒绝、1次执行，Usage缺失保持UNKNOWN。 |
| Codex完整组合/磁盘续接 | [2026-10-22报告](../reports/cpar-62-codex-combined-20261022.md)按自身候选/source manifest绑定；D2阶段回归不改绑那次磁盘resume或未调用能力。 |
| binary本地资格 | [#66 local报告](../reports/cpar-66-local-candidate-20261022.md)：产品source=`737fd040e983bef329be27b5cf48b4f6986a5772` + 原overlay；Linux ARM64未签名六文件集，binary SHA256 `17c1fd546d1a8fbb7d310bc3f62a872f23b9d1b5b3f8195e383fc9f86162bdf2`；stock reader/serve/restart/exit/isolation为144/144。 |
| 受控证据复用依据 | [verification-reuse.json](../reports/assets/cpar66-local-candidate-20261022/verification-reuse.json)：相对4015bd45的849项产品Rust/lock/toolchain/SPA/build身份输入无diff，明确embedded revision不同；只复用历史controlled结果，不证明新Linux客户端执行。 |
| 原样OCI资格 | [#66 OCI报告](../reports/cpar-66-oci-runtime-20261023.md)：仍为737fd040+overlay原制品；验收代码 `f8d28c6b0f8f66e60cbc1359ea5aaf8fa896d920`；原样OCI serve/reader/同容器再启动/新容器采用/SIGTERM/cleanup为290/290。没有新建制品或推定真实客户端资格。 |
| 最新Full | 同OCI报告：f8d28c6b的Darwin独立源/overlay/全新target，45/45，Rust1659/0/21；不是产品binary源码的重跑。4015bd45的D2阶段Full另为45/45、1659/0/18。两者均为历史证据，本轮未重跑。 |
| 本轮交付起点 | `f43a08ece525d76c940f0195c7df3ab822ce779a`。本轮文档commit不改绑上述运行源码、binary或OCI；没有新的运行资格声明。 |

## 未满足的最小条件

1. **固定Build启用依据和实现资格。** 为实际所需字段、模型关系、profile/capability及连续性取得适用的一手证明，
   再按独立契约/授权落实生产资格。当前stock默认拒绝仍准确有效；不得把cfg(test)/controlled-test-support
   搬入制品，不得改native-field qualification制造通过。#61 `cost_in_usd_ticks`继续 **NOT_ADMITTED**，
   等待新一手材料，不重复已有研究或以零值、合成peer、D2采用替代依据。
2. **完成#66适用候选前置。** 保留#62/#64/#65原生依赖，绑定同一实际候选源码、完整配置与三客户端证据，
   对齐source/binary/manifest/reader/egress/系统资格及适用Gate、Standards/Spec要求。
   历史同产品输入复用需保留范围；binary/OCI的客户端执行仍NOT_RUN。远端CI、Sigstore/密码学验签及两份独立
   构建可重复性目前分别NOT_RUN，按实际交付流程要求处理，不能把hash核对当签名或reproducibility。
3. **另行启动#67的真实验收。** #66资格满足后，还须明确账号/环境/预算/新epoch，冻结serving与模型三段身份，
   取得适用fresh Pin；Codex、Claude D2、Pi全部经真实Grok Build完成闭环及证据核对。
   首个FAIL/UNKNOWN/身份矛盾停止该epoch，旧stopped epoch不恢复、不转移预算。完成适用交付说明后才判断首版READY。

本轮不启动#67、不读取真实凭据、不发送Provider请求。#64→#63→#61、#66→#62/#64/#65、#67→#66
均保留。#58/#64/#66 的 D2 文本来源见[原本地工单修订稿](../reports/cpar-64-66-d2-issue-drafts-20261023.md)；
2026-10-23 15:33 UTC 的只读复核已确认远端正文包含该 D2 决定，原修订稿的“未同步”属于当时状态。
远端尚未反映 ADR-0125 的本地请求兼容调整，差异与当前代码状态见[状态入口](../engineering-roadmap.md)。
本次没有远端写入；#9/#47/#57 的全范围义务及 native-v1 独立方向不变。
