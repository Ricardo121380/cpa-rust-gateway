# CPAR 新构建真实渠道验收工具与环境准备

本轮准备交付 **PASS**：正式入口可运行，本地正反例和 Full 门槛通过，当前真实环境重新只读核实，
逐组合缺口及下一执行目标已落盘。**新构建真实验收 NOT_RUN；现在即可执行的完整真实组合为 0。
C 全批及 #57 继续 BLOCKED，父 #9 未完成。** 本轮真实推理调用、生产修改、远程回填/推送/关单均为 0。

## 版本、范围与既有证据

- 基线报告提交 `499896e6b5e9fff32b490c2c1aabbc2a99cc7c8f`；原 C 业务代码
  `2e43ea4bccd7c1253df7d5ceddfbc3f4e4bc61fb` 的修复和门禁沿用原有效证据。
- 工具实现 `64e5cf9ac8776173cae9eb22b0772e923867dcbd`；评审修复与本轮最终验证源码
  **`ff4b59edda6cd480782e036dbb7cecee762873be`**。两提交均引用 #57。
- 本轮重读实际 AGENTS、工单规则、CONTEXT/相关 ADR、质量门槛、C 收敛报告及其 preconditions、
  ledger、#57 归集说明；通过既有 gh CLI 重新读取 #9、#39–50、#57，14 项仍 OPEN，body 与
  前次输入一致、无新增评论。新输入归集在本轮资产目录；没有远程工单写入。
- 没有 Provider 适配重写、前端业务修改、依赖升级或权威管理契约/schema 变更。
  `scripts/check.sh` 增加工具回归门槛，已有同提交 cross-boundary FYI 与 trailer。
  用户修改的 AGENTS 及无关未跟踪内容保持原状。
- 文档归集提交不冒称重新执行 Full。完整测试版本以本段和运行回执为准，旧 C/D 的验收范围与
  版本不被本轮改写。旧 C 资产包原文件及校验清单保持完整。

## 可执行交付

复用 [原 C live runner](../../scripts/acceptance/cpar-batch-c-live.py) 的 `--new-build` 分支，
新增 [正式实现](../../scripts/acceptance/cpar_new_build_acceptance.py)、
[隔离回归](../../scripts/test-cpar-new-build-acceptance.py) 和
[完整命令/私有证据 schema](../../scripts/acceptance/cpar-new-build-acceptance.md)。
历史 smoke 入口仍标 `LIVE_EXISTING_RUNTIME`，不能替代新构建证据。

从项目根目录执行：

```sh
python3 scripts/acceptance/cpar-batch-c-live.py --new-build inspect \
  --host new-vps --sudo --out /absolute/path/to/current-environment.json

python3 scripts/test-cpar-new-build-acceptance.py

python3 scripts/acceptance/cpar-batch-c-live.py --new-build run \
  --plan docs/reports/assets/cpar-new-build-acceptance-20261001/next-krill-responses-json-plan.json \
  --runtime docs/reports/assets/cpar-new-build-acceptance-20261001/current-environment.json \
  --base-url https://cpar.142857142.xyz/v1 \
  --out /absolute/path/to/preflight.json

python3 scripts/acceptance/cpar-batch-c-live.py --new-build audit \
  --bundle /absolute/path/to/private/observations.json \
  --out /absolute/path/to/public-receipt.json
```

上述 `run` 没有 `--execute`，不发送推理。下一次必须重新 inspect；真实执行还需要
`--manifest <reviewed build-release manifest>`、完整冻结的 plan、当前 client secret 环境变量和具体授权，
之后才给 `run` 增加 `--execute`。模板/本轮具体计划中的未知字段有意保留 null，因此尚不能执行。

工具观察 `/proc/<MainPID>/exe` 的真实字节、嵌入 SHA、SHA256、实例、PID/start 和时间，并核对产物
manifest。实际每轮重新校验同一进程/产物；操作者传入的版本字符串不算运行证明。每个组合固定
channel、endpoint、route/candidate、实际模型、协议/流模式、账号/credential revision、config、
egress revision、参数与声明，Actual Attempt 必须全部相符；缺失 BLOCKED、矛盾 FAIL。

一次组合自动执行 7 请求：文本两轮，单工具周期及最终回答，两个工具周期及最终回答。
Chat/Messages/Responses × JSON/SSE 共六种，检查完整有序历史、工具 ID/名称/参数/结果、第二周期
新 ID、实际响应模型、唯一终态、原生 Usage/源八字段/持久 Usage/账本和 exact source-event 归属。
遇首个失败/缺证据即停止；不自动重试、重定向、恢复账号或修改环境。

九种错误/截断/取消/超时/重试边界，以及按真实声明展开的 reasoning、parallel、stored、continuation、
compact、WebSocket 各细项，通过现有受控设施或获授权的操作者采样后导入 audit。工具会检查原始
wire、实际发送/attempt、资源释放、所有权/资源/操作等证据；它不自动注入生产故障、重启服务或
执行跨所有者删除。**缺受控样本 NOT_RUN；声明未知 BLOCKED；不从空对象推定 false。**
目前六份本地基本回执是 `basic_status=PASS`、整体 `NOT_RUN`，不能当作整组/扩展真实通过。

公开回执仅投影版本、参数、Usage、检查、类别、时间、证据路径和摘要身份；原始正文只在内存/
受保护私有 bundle。无效身份与任意 failure 对象不会原样公开。退出 0=PASS、1=FAIL、2=BLOCKED/NOT_RUN。

## 本地验证和两轴评审

[Full](assets/cpar-new-build-acceptance-20261001/full-check.md) **45/45 PASS**，退出 0，
2026-10-01 **15:22:12–15:25:56 UTC**，源码固定 ff4b59ed。
[精确运行回执](assets/cpar-new-build-acceptance-20261001/full-run-receipt.json) 保留命令、工具链、时间、
HEAD 和被测四个源码文件的校验值。运行开始时唯一 tracked 用户差异为 AGENTS；业务/工具源码无未提交差异。
Full 包含严格 Clippy、Rust tests、前端现有公共门槛、供应链与秘密扫描及新工具 **24/24** 回归。
无需重复单独 Fast（Full 已含公共步骤）；EgoLite NOT_RUN，本任务没有浏览器/界面行为修改。
交付材料的 [docs 门槛](assets/cpar-new-build-acceptance-20261001/docs-check.md) **7/7 PASS**；
最终 staged whitespace/JSON/引用/manifest 单独核对。为满足仓库 whitespace 规则，归档日志仅删除
行末水平空白；[规范化记录](assets/cpar-new-build-acceptance-20261001/archive-normalization.json)
保留修改前后 hash 与行数。spec Markdown 仅规范 EOF，原工单 body 仍保留在 JSON 输入。

六种组合在真实 loopback HTTP server 上各发送 7 次，共 **42** 个基础正例请求。六种停止反例各覆盖
六组合，共 **36** 个负例请求；每个只发一次：错误 endpoint、缺 revision、HTTP503、错误响应模型、
错误运行版本、缺每轮 runtime。另两次请求验证重定向不重发及慢速流 deadline。
以上 **80** 是最终测试循环推导的 synthetic HTTP 请求数，不是整个会话/诊断重复执行的总数，
真实 Provider 次数仍 0。其他内存反例覆盖全部目标字段、未知版本/声明、工具参数与完整历史、
第二周期、伪/重复终态、终态后输出、截断、Usage 丢失/差异/错归属、账本 source-event、取消/超时等。

[两轴评审记录](assets/cpar-new-build-acceptance-20261001/review.md) 保留 initial FAIL 及限定关闭范围：
Standards 1 项校验重复漂移、Spec 4 项假通过/泄漏问题全部先复现再修复。
关闭复核分别 PASS；Spec 复核限定四个 finding 及直接回归，未冒称穷尽所有扩展或 C 全规格。
[固定旧源码反例回执](assets/cpar-new-build-acceptance-20261001/review-red-receipt.json) 使用 64e5cf9a
工具与 ff4b59ed 测试重现失败；最终绿色正式证据以 ff4b59ed Full 为准。
早期探索日志仅作诊断，未标为 immutable source acceptance。

## 当前真实条件与下一顺序

详细 [前置条件/目标清单](assets/cpar-new-build-acceptance-20261001/real-channel-preconditions.md)、
[实际 inventory](assets/cpar-new-build-acceptance-20261001/current-environment.json)、
[主机 DNS/TCP/TLS](assets/cpar-new-build-acceptance-20261001/egress-probes.json)、
[逐组合清单](assets/cpar-new-build-acceptance-20261001/combination-readiness.json)。

15:22:19 UTC 新只读观察：`new-vps` / `cpa-rust-gateway.service` / `instance-20260726-2136`，
PID 499070，实际仍运行 **`b0cf36e387b774439acc23830221389eb3ac74d0`**，SHA256
`0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b`。
管理 active config 为 `kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75 / rev-1`；观察期间稳定，
不能把它等同于每个历史 attempt 的 loaded graph/lease revision。账号刷新可能独立进行。

| 渠道 | 当前条件 | 下一动作/阻断 |
| --- | --- | --- |
| openai-compatible / Krill | gpt-6-sol Responses endpoint/candidate 启用，账号 runtime available，管理 revision 0，有 route grant | 共通构建/声明/精确 revision 条件齐后第一目标；gpt-5.5 旧 candidate 指向 Chat 且已 disabled，不能替代 |
| grok.build | grok-4.5 route 启用；三个账号仅一个 runtime available，其 native account revision 96 | 第二目标；固定 available 账号，补 executing credential revision 和出口证据 |
| codex | gpt-5.6-terra route 启用，运行时账号 expired | 账号授权/恢复未执行；reasoning override false 保留，不改声明取得通过 |
| grok.console | grok-4.20-0309 route 启用；两账号 credential_unauthorized，session absent | 专用认证/会话恢复未授权，不能以界面 active 代替 runtime availability |
| kimi-coding | 账号 runtime available、管理 revision 847，binding 启用但 route_ids 空 | 缺专用 route/model/grant；动态 revision 必须执行前刷新 |
| anthropic-compatible、claude、grok.official、grok.web、kimi-api、kiro | 当前配置无对应账号/endpoint/route | 需具体材料、授权与环境；Web/Kiro 软件阻断独立保留 |

五个当前目标域名 TLS 探测 PASS，仅主机直连，无 Provider HTTP/inference 请求；网关实际代理路径、
权限、模型可用性未据此宣布通过。公开模型 capabilities 均为空，candidate override 是配置线索，
effective 六项声明仍未知。历史记录只读采集证实实际 Attempt 缺六项精确身份字段。

下一轮首先补 value-free exact execution revision 观察与实际声明/loaded graph/egress 证据，再构建并
核实 Linux ARM64 候选与 manifest、取得指定目标部署/必要重启授权后切换。首轮固定 Krill
`gpt-6-sol @ p12-12-production-krill-responses-endpoint`，顺序 Responses JSON→SSE→Chat JSON→SSE→
Messages JSON→SSE，各模式 7 请求并逐轮停止；基础后补受控边界及实际声明扩展。其后 Grok Build
`grok-4.5` 同顺序。其他账号/路由恢复另按具体授权推进，不能借用前两个渠道的成功。

Kiro 原生 hard output cap / Messages 必填上限、Grok Web 原生多轮 / Messages 上限，以及条件性
Grok 富元数据不等价桥接继续 BLOCKED。只保留已确认的 Web 工具不支持、Messages 精确 token
counting 不支持两项规格限制。台账覆盖原 198 基础格（186 applicable BLOCKED，12 Web 工具
inapplicable NOT_RUN）、24 Kiro capped 格、12 条件性元数据格、49 扩展准备项；执行真实格 0。

[本轮 #57 本地归集](assets/cpar-new-build-acceptance-20261001/issue-57-evidence.md) 和
[资产校验清单](assets/cpar-new-build-acceptance-20261001/evidence-manifest.json) 是下一轮起点。
release artifact、部署、远程 CI、真实推理、生产及 #57/#9 关闭仍未完成；本轮准备独立交付。
