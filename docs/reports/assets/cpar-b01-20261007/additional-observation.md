# O-01: ordinary Responses JSON phase admission

状态：**OPEN_OUT_OF_SCOPE**，不列入原 13 个 P2 的已修复统计，不改规格或工单。发现于合成 B-01 fixture 初次试跑；尚未作独立两轴评审或完整 JSON/SSE 反例矩阵。

起点 `ee521c82dad311f8e82e15f0f63f1e4f5189b35f` 的普通 `openai-compatible.responses` JSON 上游返回一条带 `phase:final_answer`、两个 output_text 片段的 completed assistant 消息；消息没有 `metadata`／`internal_chat_message_metadata_passthrough`。第一次公开请求返回 **502 StreamTruncated**，尚未进入第二轮。两片内容、引用及 logprobs 与最终 B-01 fixture 相同。只移除可选 phase 的合成试验后，第一轮完整通过并触发第二轮 400，因此此问题与多文本历史拒绝独立。

当前源码 `crates/protocol-openai-responses/src/upstream_response.rs` 的 `validate_proven_message_metadata`（原行 1390–1400）把单独 phase 也视为 paired-turn metadata，要求 `phase=final_answer` 后进入 `validate_proven_turn_metadata`；后者要求两份 turn_id 对象同时存在且相等。`decode_message` 在发出完成结果前调用该检查，runtime 将 JSON 解码失败映射为 BootstrapTruncated／公开 StreamTruncated。

`phase` 是官方可回传 output-message 字段，参见 [OpenAI output-message parameter](https://github.com/openai/openai-python/blob/main/src/openai/types/responses/response_output_message_param.py)。当前 `native_output.rs` 和 Responses request admission 允许 commentary/final_answer，其他原生 Grok decoder 的既有公开回归也能保留此字段。因此普通 JSON 分支的附加要求值得后续单独修复。

本轮不需要此字段来触发 B-01；主闭环使用合法且可被旧输出路径接受的 id/status、两片 annotations 和 null/empty logprobs。另将现有三类 Grok metadata 回归扩展为两片，以验证当前可输出和回传的 phase 路径。生产代码没有删除、伪造或重写 phase；此观察未被隐式修复。

初次试跑命令为 `cargo test --locked -p gateway --bin gateway responses_json_multipart_output_replays_exactly_through_public_http -- --nocapture`，2026-10-07 21:17:36–21:17:47 +08:00，退出 101。其日志 SHA256 为 `cfb2ad185d960c6ad528522ce3d335617de2ed9e49fc7366620ff53ca5e54e3b`；[原始合成试跑日志](phase-trial.md)随本地证据保留。该执行不计 B-01 RED。后续最小任务应独立复现 phase-only JSON/SSE，区分可回传 phase 与仅属于上游 envelope 的 paired turn metadata，再保留原闭集校验和匹配检查。
