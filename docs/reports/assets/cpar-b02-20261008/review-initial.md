# B-02 independent review, initial round

固定范围：`6883f47ea89332563b6ac90ce42c077387b0f9c5...ee65e01279be71695140591d446f8d1c0db5689a`。
两个互相独立、只读、单轮、`fork_turns=none` 探子均给出 complete 终态。
以下保留评审原文；其 Full 回执路径是初审当时路径，准确的 ee65 执行快照另保存在
[Full ee65](full-ee65-pass.json)。本轮发现属于 B-02 内的遗漏，不增加原 13 项 P2 的计数。

## Standards

Standards **PASS（文档硬规则）**；硬违规 **0**，判断性 smell **1**。范围仅为
`6883f47ea89332563b6ac90ce42c077387b0f9c5...ee65e01279be71695140591d446f8d1c0db5689a` 的 B-02 修复。

覆盖 **COMPLETE 24/24**：完整审查全部 diff hunks、15 份证据资产；内嵌重复测试源码逐差比对，回执日志按工具已检查事项去重核验。另核验：

- `apps/gateway/src/runtime.rs:1912,4869,7265`：路由、真实 decoder bootstrap/source。
- `crates/gateway-http-actix/src/lib.rs:1615,3184`：WS root 注册与持久化／失败路径。
- 同目录 `request_observation.rs:160`、`stored_response_continuity.rs:145`。
- `crates/gateway-core/src/gateway_event.rs:289`、`crates/protocol-openai-responses/src/native_output.rs:585`。
- `apps/gateway/src/runtime/batch_c/http/responses_history.rs:19`：B-01 相邻回归入口。

已见事实：`upstream_response.rs:568` 的 `push` 将错误永久置为 `Rejected`；`:508` 的 `confirm_item_parts` 校验确认快照；失败路径不进入 `lib.rs:1656` 的 completed cache 或 `:3230` 的完成持久化。符合 BC-CORE-003:48“terminates once”、BC-RESP-002:44 和 BC-RESP-004:82 的成功根限制。`apps/gateway/Cargo.toml:41` 明确新增依赖仅属 dev；边界 allowlist 同步。当前 AGENTS 的额外 17 行仍在未提交差异中。

**判断性观察：possible Duplicated Code。**
`apps/gateway/src/runtime/batch_c/http/responses_done.rs:255` 的 `assert_public_failure` 与 `:740` 的 `contradictory_native_done_confirmations_fail_public_storage_and_continuation` 重复相同公共请求、失败观测和 GET 检查。关键原文在 `:256/:758` 均为 `let peer = Peer::start_scripted(false, None, Some(chunks(&frames)))?;`，`:283/:781` 均调用 `assert_persisted_outcome(&fixture, true)?;`。推断：相同检查已有 helper，重复体可能增加同步维护成本。标准出处 `/Users/huangrui/.codex/skills/code-review/SKILL.md:46`：“the same logic shape appears in more than one hunk”；`:41` 明定启发式“never a hard violation”。

Full 回执静态核对为 ee65、45/45、1469 passed／0 failed／12 ignored；本探子未执行测试、格式化或修改文件。结论不扩展为整个 B 批、Phase DONE、真实 Provider 或生产验收。

## Spec

Spec **FAIL**；覆盖 **COMPLETE 24/24**。已核对完整固定 diff、指定契约及相邻 runtime/source、HTTP 存储、WS root、Request/Attempt、native encoder、B-01 回传路径；排除隔离副本及无关用户文件。

**(a)/(c)：1 项 P2，字段类型校验仍有缺口。**

要求：“校验支持事件的字段集合、类型及必要字段”；“非法或矛盾事件触发明确失败，不能继续形成成功结果”。

- **事实**：`crates/protocol-openai-responses/src/upstream_response.rs:896`，`text_done` 对存在的 `logprobs` 接受 `null` 或空数组。已指定官方快照 `output/cpar-b02-20261008/official-streaming-events.html:1`，锚点 `#response.output_text.done`，定义为 **“logprobs: array of object”**，没有 nullable 标记；快照哈希与 `official-source.json:5` 一致。本地 `native_output.rs:585–593` 的兼容依据是 done **省略** logprobs，并不发送 null；最终 **part** 可保留 null 是另一字段位置的规则。
- **静态推断的触发/影响**：合法 delta 后，收到同文的 `output_text.done`、`logprobs:null`，再收到一致的 item-done/completed，当前分支不会报错，仍可形成成功结果及存储/WS 根。现有 `responses_done.rs:402` 仅覆盖非空数组反例，未覆盖该类型错误。
- **必要最小补齐**：保留本地 done 省略字段的兼容；字段存在时按已核实的事件定义只接受空数组。增加该输入的公开 handler 失败、持久化缺席及调用次数反例；保持最终 part 的 null/empty 保真规则。

**(b)：未发现其他确定的 scope creep。** 五类内容矛盾、关联/阶段、确认后追加、永久拒绝及同 read 阻断实现已核对：`upstream_response.rs:568,852,913,939,1097,1231`；失败不提交存储/WS 根沿用 `gateway-http-actix/src/lib.rs:3230,3283,1656,1696`。

执行证据仅作静态核验：targeted 18/18、邻接 224 passed/1 ignored；`full-receipt.json:7` 绑定 ee65，45/45、1469 passed/12 ignored，日志及报告哈希相符；本人未运行测试。结论仅覆盖 B-02；其余指定发现未修，发布仍 BLOCKED。

初审汇总：Standards 0 硬违规、1 判断性观察；Spec 1 项 P2。两轴独立，初审 Spec FAIL 未被 Full PASS 覆盖。
