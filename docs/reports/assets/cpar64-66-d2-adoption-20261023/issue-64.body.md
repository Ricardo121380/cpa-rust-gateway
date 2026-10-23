## Parent

[产品规格 #58：CPAR Rust 网关目标与三客户端首版范围](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58)

## What to build

首版选用固定版本 Claude Code 2.1.302 的完整四变量 D2 配置，通过 CPAR 标准 Messages 使用 Build 装配；虽不显示 Grok summary，仍须执行并续接工具任务。原始 Claude 控制要求保留后续，不删除或标完成。

覆盖 #58 用户故事 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 30, 31, 32, 33, 35, 49, 56；验收场景 T01, T02, T03, T05, T06, T08, T10。

本票承接 #58 的已确认范围；实际开工须有用户明确指令。真实账号、收费请求、共享服务与发布部署分别遵守既有授权边界。

### 已确认的 Claude D2 首版配置（2026-10-23）

用户已经明确选择经验证的 Claude D2 配置作为首版 Claude Code 范围，无需再次询问是否采用。
固定客户端为 Claude Code 2.1.302，入口 `/Users/huangrui/.local/bin/claude`，解析到
`/Users/huangrui/.local/share/claude/versions/2.1.302`，SHA256
`5262ce9de86243fed0d21edc39b271555d56780d0112e2b194439fe1c6662d37`；使用隔离的
HOME/config/tmp/work、`--bare --print`、stream-json、Messages、字面 `grok-4.5`。
以下四项必须完整同时生效，普通重试和非流式 fallback 设置均不得遗漏：

```sh
DISABLE_PROMPT_CACHING=1
CLAUDE_CODE_DISABLE_THINKING=1
CLAUDE_CODE_MAX_RETRIES=0
CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1
```

省略 thinking 只描述客户端请求，不表示模型停止推理。首次含 effort 的请求由网关在发送前准确拒绝；
协商后的新请求由客户端自己重新认证并发起，拥有独立 Request；只有实际派发才有 Attempt，Usage/账本只按已验证证据记录。
网关不删字段、不伪造上游错误、不透明重发。Messages 不展示 Grok summary，但其完整加密留存、
item/part 身份与顺序、exact Attempt 授权读回、工具结果回传和续轮要求全部保持。

原始 Claude 的 adaptive/display、overall effort、block prompt-cache 要求保留为 #58/#64 后续待办，
不删除、不标完成；它们不属于已选 D2 配置。D2 已采用不证明原始配置兼容。
首版仍要求 Codex、Claude Code D2、Pi 全部通过 Grok Build 完成约定闭环。

产品采用、实际客户端受控验收、binary/OCI 生命周期、固定 Build 服务资格和真实 Provider 验收分别记录。
D2 的 PASS_LOCAL_CONTROLLED 绑定代码 `4015bd4542dbc52fde8984c14c01b34843ef3c59`；
产品制品仍为 `737fd040e983bef329be27b5cf48b4f6986a5772` + overlay，OCI 验收代码为
`f8d28c6b0f8f66e60cbc1359ea5aaf8fa896d920`。历史 binary144/144、原样 OCI290/290 以及
Full45/45（Rust1659/0/21）各自只证明原报告范围，本 binary/OCI 的三客户端执行仍 NOT_RUN。
固定 Build 服务资格仍 BLOCKED，#66 仍 OPEN，#67 未开始，首版尚未 READY。
#61 cost_in_usd_ticks 继续 NOT_ADMITTED，等待新一手依据；没有新材料不重复既有研究。

本地 `docs/adr/ADR-0123-first-release-claude-d2-restricted-support.md` 记录正式选择；
`docs/support/grok-build-messages.md` 提供占位凭据的局部配置，
`docs/support/first-release-candidates.md` 分列版本、证据与剩余条件。
ADR-0122 的独立实验、旧报告 NOT_DECIDED、原始配置与重试/fallback RED 均原样保留。
本决定不改变 native-field qualification 或 stock capability，不把 controlled-test-support 引入制品，
不关闭工单或解除原生依赖，也不构成真实账号、收费请求、发布或部署授权。

## Acceptance criteria

- [ ] 冻结当前实际 CLI 版本、入口、独立配置及请求行为；隔离用户凭据/会话/工作目录并证明外发限制，不用旧 Claude 版本或 synthetic executor 的历史结果代替。
- [ ] 经真实公开 Messages 监听器和生产 Build compiler/factory/执行链完成回答、两次相关工具循环、结果返回与后续问答；不得使用另一渠道作为替代。
- [ ] 观察并兑现实际必需的输出上限、thinking、tool choice、parallel 和历史语义；已知不兼容请求在发送前拒绝，独立受控接收数为零。不能删除字段、近似映射预算、伪造签名或修改客户端来通过。
- [ ] 确认客户端不展示摘要，同时完整加密摘要可按 exact Attempt 授权读回；工具续轮、文本含义、已知 Usage 和响应身份保持。
- [ ] 验证流式增量、失败、取消与必要启动请求；实际触发 token counting 时沿用明确不支持，不伪造精确值，并记录客户端实际处理结果。
- [ ] 以独立接收数证明无额外发送，交付可复现 CLI 兼容记录和可见限制；缺失的必需能力继续 BLOCKED，真实 Grok 和其他 Messages 消费者不继承本地 PASS。

## 原始 Claude 配置的保留义务（后续，均未完成）

- [ ] M1 adaptive thinking/display：取得适用于固定 Build 的准确语义/实现依据及实际客户端证明。
- [ ] M2 overall effort：保留原始 effort 控制要求；D2 的准确拒绝与客户端新请求不构成 effort 已兑现。
- [ ] M5 block prompt-cache：保留逐 block 缓存语义要求，禁用缓存的 D2 不构成原始要求已支持。

M3/M4 和 #63 的既有契约继续有效。实际 D2 省略 context_management，不把独立 M3 正例当作该客户端已调用。
原始配置仍 BLOCKED_BEFORE_FIRST_RESPONSE；历史原始请求400/零发送和对应RED不改写。
以上后续项不再作为已选D2配置的首版行为承诺，但#64整票及原生依赖仍未关闭。
本地受控结果只按其受测源码/配置记录，不能勾选固定服务或真实Provider条件。

## Blocked by

- #63：标准 Messages 的摘要留存与 JSON、SSE 工具闭环
