# B-02 final independent review closure

总范围：`6883f47ea89332563b6ac90ce42c077387b0f9c5...15babae64154f8ff5fa1055f856d11640385633a`。
共 28 个文件。所有探子只读、单轮、`fork_turns=none`；主线程等待终态后修改、验证和裁定。
初审完整 24/24 的原文及 Spec FAIL 保留在 [初审](review-initial.md)。复核不虚报重读全部 28 个文件：
初审 24 个文件中 20 个 Git blob 未变，4 个修改文件及 4 个新增资产属于 8 文件增量。
[覆盖清单](review-coverage.json)列出实际集合。

## Standards

主代理组合裁定 **PASS，COMPLETE 28/28**：初审 24/24 + 增量 5/8 + 新探子缺口 3/3，
必要 helper／相邻连接已补齐。初审无硬违规；一项 Duplicated Code 判断性观察已关闭。
最终 0 确定硬违规、0 未关闭判断性观察。以下保留两个复核终态，不把中断结果记为 PASS。

第一次复核终态 **PARTIAL**：已完整读 decoder、tests、contract、manifest 的全部增量和初审原文，
覆盖 5/8；通过 Git blob 比较确认另外 20/20 未变。其 JSON 投影误把数组当对象，报
`AttributeError: 'list' object has no attribute 'items'`，因此三份执行 JSON 和 helper 邻接未核验。
未执行测试或修改。主代理未复用该探子，另派下面的有限缺口核验。

缺口探子终态 **complete**，原文：

缺口覆盖 **COMPLETE 3/3**，另完成指定 helper／邻接核验。以下为本轮事实；已有 5/8、初审 24/24 覆盖沿用前报告，不声称本轮重读 28 文件。

- `docs/reports/assets/cpar-b02-20261008/full-ee65-pass.json:8,16,25-27`：对象结构正确；历史 source 为 `ee65e01279be71695140591d446f8d1c0db5689a`，exit 0，报告 **45/45 PASS**；日志 Rust tests 汇总 **1469 passed／0 failed／12 ignored**。`role` 原文：“Does not prove the later source checkpoint.” 此证据不能归给最终 15baba。
- `docs/reports/assets/cpar-b02-20261008/review-null-red.json:19,22,25,28-29`：exit 101；内嵌测试源码与 ee65 Git blob 的差异确实只有新增 case 名及 `frames[index]["logprobs"]=Value::Null` 两行。测试源码 SHA256 为 `784d630d3008421d10973e4b778bdfd49d104d3b86604f484f7f72c7baeb0606`；旧产品源码 SHA256 为 `69d15e381cb97252c66c7f4757ee8dbb9eac50ce8f9e4a6da72159706dae0bf7`，均重新计算一致。失败日志明确标记 `null-done-logprobs`，随后出现 `event: response.completed`；结果 0 passed／1 failed，红例来源可信。
- `docs/reports/assets/cpar-b02-20261008/review-fix-executions.json:1,21-28,51-58`：根节点为两元素数组。green exit 0／18 passed；clippy exit 0，命令包含 `--all-targets --all-features -- -D warnings`。虽 `source_head` 仍为 ee65，两条 receipt 的源文件哈希均等于最终 15baba Git blob、当前文件及 `validated-source-files.json:8-9`：测试 `4944f57a544ddeffad8d9ed3db3402ab84721587c0e728abb66aa7a99969ad8b`；decoder `6d0d6c6313ee106804c9329f78cfb3e3d9c628722d69b9ac9dd9faeefde84926`。

`apps/gateway/src/runtime/batch_c/http/responses_done.rs:770` 已统一调用 `assert_public_failure(frames, kind).await?;`；`:255-316` 保留旧 public／SQLite／GET 断言，并强化为恰好一个 failed、owned continuation 拒绝、received／requests 各 1。`:124-128` 将实际 handler 观测 append 后重开 SQLite；事件来自 `batch_c.rs:46-49,210,240` 的真实 sink，未构造预期结果。Succeeded Attempt／Failed Request 与 bootstrap Failed 原义符合 `BC-PROTOCOL-001-openai-responses-adapter.md:169-172`；`gateway-http-actix/src/lib.rs:3222-3240,1650-1677` 保持失败不形成成功存储／WS root。

推断：原 Duplicated Code 观察具有关闭依据；指定补核范围内 **0 确定硬违规、0 新判断性观察**，可支持主代理合并已有覆盖后裁定 B-02 Standards PASS。以上为回执静态核验，未运行测试、未读取进行中的最终 Full；release BLOCKED 边界不变。

## Spec

主代理组合裁定 **PASS，COMPLETE 28/28**：初审 24/24 + 独立修正复核 8/8，20/20 未变 blob
由复核点验。初审 1 项 P2（B-02 内 event-level null 漏洞）已用公开 RED/GREEN 修复，
最终 0 未关闭发现。以下为复核原文：

Spec **PASS（限定修正增量）**；增量 **COMPLETE 8/8**。已核对 `ee65e01279be71695140591d446f8d1c0db5689a...15babae64154f8ff5fa1055f856d11640385633a` 全部 hunks、四份新增资产及必要邻接函数。Git 逐文件内容比较确认初审另外 **20/20 未变**；结合 `docs/reports/assets/cpar-b02-20261008/review-initial.md:30` 的初审 24/24，组合覆盖为 **28/28**，并非本探子重新审查全部28文件。

**原 P2：RESOLVED。** 要求“校验支持事件的字段集合、类型及必要字段”。`crates/protocol-openai-responses/src/upstream_response.rs:896`，`OpenAiResponsesSseDecoder::text_done` 现在对存在的 `logprobs` 仅接受空数组；null、其他类型及非空数组均返回协议错误，省略仍兼容。官方快照 `output/cpar-b02-20261008/official-streaming-events.html:1`，锚点 `response.output_text.done`，原文为“logprobs: array of object”，无 nullable；快照 SHA256/字节数与 `official-source.json:5` 相符。

合法保真未收紧：`native_output.rs:585` 的 done 确实省略字段；`upstream_response.rs:1879` 的 `confirmation_part_text` 与 ee65 内容完全相同，保留最终 part null/empty。契约 `docs/contracts/BC-PROTOCOL-001-openai-responses-adapter.md:147` 明确区分两个位置。公开合法控制 `apps/gateway/src/runtime/batch_c/http/responses_done.rs:441` 删除 done 字段，同时在 `:510/:524` 比较完整终态和 StoredResponse，保留 `:167` 的 null/empty。

要求“非法或矛盾事件触发明确失败，不能继续形成成功结果”。新增 null 反例 `responses_done.rs:404`、另外四类矛盾 `:770` 均调用 `assert_public_failure(:255)`：无 completed、恰好1 failed、`UpstreamProtocolError`、SQLite append/reopen 后 RequestFailed、GET404、实际续接拒绝、network/handoff 均1；维持既有 AttemptSucceeded/source建立语义（`:136`）。decoder `push(:584)` 将该错误永久置为 Rejected。

证据静态核验：`review-null-red.json:19/:28/:29` 的 RED exit101、确切测试源码哈希及未改产品源码绑定相符；`:25` 显示错误输入曾形成 completed。`review-fix-executions.json:23/:53` 的两份 Rust 哈希均匹配15baba，GREEN18/18及Clippy exit0，日志哈希相符；`validated-source-files.json:3` 全部9项匹配。回执 source_head 仍记录提交前ee65，最终源码按内容哈希绑定。

新缺失、partial、不正确实现、scope creep：**0**，范围仅上述增量及必要邻接点验。`full-ee65-pass.json:8/:27` 只证明历史源码；未读取进行中的最终Full，未执行测试或修改。原13项P2计数不变；本结论不扩展为B批/Phase DONE，release仍BLOCKED。

两轴汇总：最终 Standards 0 硬违规／0 未关闭观察，Spec 0 未关闭发现。实际 Full 由主代理执行并独立记载；文档收口不扩增探子的源码覆盖。
