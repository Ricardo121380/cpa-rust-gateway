# B-01 固定源码两轴独立评审

固定起点：`ee521c82dad311f8e82e15f0f63f1e4f5189b35f`。修复源码：`939028d63519126f16252c3e72d3e1d5ace7ca75`。

使用 code-review Skill。两名探子分别只读完成 Standards 和 Spec，均为单轮、`fork_turns="none"`，主代理等待全部终态后收敛。逐轴结论及覆盖记录见 [review.json](review.json)。19/19 文件包括本轮四个 Rust 文件、一个行为契约以及十四个报告／快照／RED 资料；未排除测试或 RED patch。主代理沿上述 file:line 点验实际输出构造历史、真实 HTTP receiver 记录及纯 Text 准入条件，无新增发现。

下面保留两份探子终态原文，分别记录静态结论，不合并为单一 verdict。原文的待办状态是评审返回时的检查点状态；后续 Full 及最终收口结果见 [完整报告](../../cpar-b01-responses-multipart-history-20261007.md)。两名审查者均未亲自执行测试，执行结果由主代理的独立回执记录。

## Standards

Standards verdict：**PASS（静态审查）**。Coverage：**COMPLETE，19/19**；硬标准违规 **0**，可行动 heuristic **0**。

范围为 `ee521c82dad311f8e82e15f0f63f1e4f5189b35f...939028d63519126f16252c3e72d3e1d5ace7ca75`：完整覆盖四个 Rust 文件、一个行为契约、十四个报告／快照／RED 资料，包括 `red-tests.json` 解码后的完整 patch。标准依据已读实际 `AGENTS.md`、quality-gates、执行规则 1.1–1.5、ADR 索引、ADR-0100/0101、BC-ROUTER-004 和 CONTEXT。

已核实事实：

- `crates/gateway-router/src/protocol_transform.rs:987–992` 只恢复 Responses assistant 纯 Text 数组准入；`:585–593` 仍先检查消息／每片扩展及角色，`:910–911` 仍拒绝空内容，共享工具历史 helper 未变。符合 ADR-0100:12–21 的执行前校验及 :31–38 的原生表示保留；现有 ADR 已覆盖，未发现需要新增架构决定的改动。
- `apps/gateway/src/runtime/batch_c/http/responses_history.rs:202–249` 使用实际返回 output 构造下一轮，并断言实际 HTTP 接收的完整 input；`:135–165` 检查 SSE 分片索引、顺序和唯一 completed；`:275–355` 为十种反例设置双模式、零 handoff／HTTP 接收／Attempt 断言。测试位于 `apps/gateway/src/runtime.rs:504–505` 的 `#[cfg(test)]` 模块。
- `docs/contracts/BC-ROUTER-004-protocol-transform-admission.md:66–75` 明示既有 ADR 对早期限制的替代关系，列出三个实际存在的回归函数，符合 `docs/quality-gates.md:18–21` 的契约证据链要求。
- `docs/reports/assets/cpar-b01-20261007/development-verification.json:3–7` 的四份源码哈希全部匹配固定修复提交；RED patch 哈希及 RED 产品源码哈希亦匹配。主报告 :3、:41–52 明确检查点、Full 待执行、发布 BLOCKED 和原 source80 绑定，未将定向结果扩大为正式交付。

本探子未运行测试。Full 与最终文档收口仍是主代理的待办步骤；依据 `docs/06-development-plan.md:163–167` 完成后才能推进本地状态，本静态 PASS 不替代其执行结果。

## Spec

Spec verdict **PASS（本切片静态符合性）**；coverage **COMPLETE，19/19**。缺失／部分发现 **0**；scope creep **0**；错误实现发现 **0**。核验范围是固定 `ee521c82...939028d6` 的全部变更及指定相邻调用链。

以下 `A` 指 `docs/reports/assets/cpar-b01-20261007/`，`H` 指 `apps/gateway/src/runtime/batch_c/http/responses_history.rs`。

- 规格要求“公共三协议的非流式／流式文本多轮正确传递”（`A/issue-27.md:13`）、“同协议及可等价桥接验证输出和历史回传”（`A/issue-31.md:20`）。修复仅增加 Responses assistant 纯 Text 数组准入：`crates/gateway-router/src/protocol_transform.rs:987–992`。空内容、逐片元数据、角色、工具关联及能力检查仍分别在 `:910`、`:585–593`、`:342–345`、`:1013`；共享工具 grammar `:950–964` 未变。
- `H:202–222` 从第一轮实际返回的完整 output 构造第二轮；`H:237–249` 断言实际 HTTP 接收的完整历史、两次接收及目标模型，保留两片顺序、id/status、独立 annotations、null/empty logprobs。接收记录发生于 `apps/gateway/src/runtime/batch_c/http.rs:131–134`，实际网络发送在 `apps/gateway/src/runtime.rs:5513`。[官方 input union](https://raw.githubusercontent.com/openai/openai-python/main/src/openai/types/responses/response_input_param.py)包含 output-message。
- SSE fixture `H:28–91` 的事件、索引和序号符合所查[delta](https://raw.githubusercontent.com/openai/openai-python/main/src/openai/types/responses/response_text_delta_event.py)／[done](https://raw.githubusercontent.com/openai/openai-python/main/src/openai/types/responses/response_text_done_event.py)形状；`H:135–165` 逐项断言四次分段 delta、两片完整 part 和唯一 completed。phase 双片回传另由 `apps/gateway/src/runtime/batch_c/grok.rs:309–362` 覆盖。
- `H:276–355` 的十类风险负例均断言 400，以及 handoff、Attempt、实际接收为零，符合 `A/issue-27.md:21`“有零上游调用证据”。

运行证据仅核验记录，未亲自执行测试：JSON/SSE RED 均在第二轮返回 400（`A/red-json.md:12–15`；`A/red-sse.md:11–14`）；GREEN 与相邻回归见 `A/execution.md:18–148`。四个源码哈希、RED patch／源码及全部保存日志哈希均匹配。

Full、另一审查轴及最终收口仍待主代理完成；报告正确标为 NOT_RUN（`docs/reports/cpar-b01-responses-multipart-history-20261007.md:41–50`），尚未 RESOLVED_LOCAL。O-01 独立保留，其他 B 项、C／#57／#9 与发布状态未扩大。
