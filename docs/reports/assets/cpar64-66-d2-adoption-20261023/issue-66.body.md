## Parent

[产品规格 #58：CPAR Rust 网关目标与三客户端首版范围](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58)

## What to build

维护者得到绑定同一不可变源码与客户端配置的候选、审查和运行资格材料，可以准确判断是否满足后续真实验收前置条件。

覆盖 #58 用户故事 8, 12, 13, 14, 15, 24, 35, 47, 48, 49, 50, 51, 52, 56；验收场景 T11, T12。

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

- [ ] 三客户端受控闭环和新证据恢复结果齐备；固定同一候选源码、受影响配置及客户端版本，所有更改均有对应行为与范围。
- [ ] 在该不可变候选上完成适用 Task/Phase/Fast/Full 及 Standards/Spec 审查；历史 Gate、不同源码或仅 helper 通过不能替代。
- [ ] 生成可对应的 source/binary/manifest，并完成原流程要求的 reader、egress 和隔离系统启动/退出资格；不擅自选择新的部署拓扑或变更生产服务。
- [ ] 核对版本支持范围、摘要展示限制与客户端实际必需能力；已知语义缺陷或必需条件未知时不得给出合格结论。
- [ ] 逐项记录远端 CI、候选制品与系统证据的实际执行状态；缺失条件保持 BLOCKED/NOT_RUN，不能把某个测试绿灯代替整项资格。
- [ ] 本票完成是后续真实验收的候选前置资格，不要求真实 Provider 已通过，不发布或部署；这样与下一票不存在相互等待的依赖环。

## Blocked by

- #62：Codex 与 Pi 的 Responses 受控工具闭环
- #64：Claude Code 的 Messages 受控工具闭环
- #65：诊断证据的进程重启与升级恢复
