# G1 phase gate report

| 字段 | 值 |
|---|---|
| 计划 | `v1.0` |
| Gate | `G1` |
| 日期 | `2026-07-19` |
| 验证分支 | `codex/g1-phase-gate` |
| 被测实现 Commit | `d4c469805d65ca0a2895e15fae86ba72414e5e11` |
| 结果 | `BLOCKED`（等待 `CR-P1-G1-001` 的用户批准） |

## 结论

P1-01 至 P1-09 均为 `DONE`，且每个 P1-03 至 P1-09 实现分支已推送并获得
GitHub Fast 与 Full supply-chain 两项成功结果。G1 不能被如实标为 `PASS`：其第二项
要求“任意 Chunk 切分得到相同 Canonical Event 序列”，但已冻结的 Canonical 契约把每个
`ToolCallArgumentsDelta` 保留为已提供的参数片段。不同的有效切分因此会得到不同数量和
内容的 delta 事件。

现有 P1-09 证明的是正确的 P1 语义：每个 Tool 的参数重组、最终 `RawJson`、Responses
SSE 的 `function_call_arguments.done` 和非流式 Function Call 输出相同。它没有、也不能在
不改变事件契约的情况下，证明原始 Canonical Event 向量逐项相同。P2 及后续 Phase 尚未
开始，服务器没有被修改。

## G1 条件与证据

| 条件 | 当前证据 | 结果 |
|---|---|---|
| `/v1/responses` 非流式和 SSE 均通过 Mock E2E | `gateway-http-actix` 的 `non_streaming_responses_uses_mock_through_router_and_bounded_transport` 与 `streaming_responses_emits_openai_sse_through_actix_body`；[P1-07 报告](p1-07-actix-responses-handler.md) | PASS |
| 任意 Chunk 切分得到相同 Canonical Event 序列 | [P1-09 报告](p1-09-tool-stream-property-tests.md) 明确说明不同 fragment schedule 可以包含不同数量的 `ToolCallArgumentsDelta`，只断言最终语义 projection；[BC-CORE-003](../contracts/BC-CORE-003-canonical-event-state-machine.md) 规定参数 delta 按 supplied fragments 保留 | BLOCKED |
| `EnterPlanMode`、`ExitPlanMode` 和普通无参数 Tool 输出 `{}` | `p1_09_tool_chunk_properties.rs` 的一字节回归，三个 Tool 均以显式 `RawJson("{}")` 进入 Canonical 边界，并校验 SSE 和非流式最终参数 | PASS |
| 客户端取消后没有遗留上游任务或无限缓冲 | `gateway-stream` 的背压、显式取消、consumer drop 测试；`gateway-provider` 的 pending pull/drop 测试；HTTP `dropping_an_unconsumed_sse_body_cancels_and_drops_the_source` | PASS |
| FirstSemanticEvent 前后重试状态可被测试明确区分 | `only_explicit_downstream_delivery_commits_the_retry_boundary` 与 `cancellation_keeps_fse_uncommitted_but_forbids_transparent_retry` | PASS |

## 已核验的任务分支与 CI

| Task | 已推送 Head | GitHub CI |
|---|---|---|
| P1-03 | `fbda3a8` | [29641552650](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29641552650) Fast + Full PASS |
| P1-04 | `2283677` | [29642965505](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29642965505) Fast + Full PASS |
| P1-05 | `edc0bbf` | [29644937742](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29644937742) Fast + Full PASS |
| P1-06 | `836e95d` | [29645853415](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29645853415) Fast + Full PASS |
| P1-07 | `a3095e3` | [29647710717](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29647710717) Fast + Full PASS |
| P1-08 | `43f51ad` | [29649217635](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29649217635) Fast + Full PASS |
| P1-09 | `d4c4698` | [29650649871](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29650649871) Fast + Full PASS |

## Pending Change Request

```text
CR-ID: CR-P1-G1-001
原因: G1 的字面“相同 Canonical Event 序列”与已冻结的 delta-fragment 契约冲突；
      BL-04 要求网络 Chunk 边界不影响语义，而不是要求保留物理分片的事件向量相等。
影响的 Task / Matrix ID / ADR: Plan G1、P1-09、BL-04、BC-CORE-003；不修改 P1
      的公开 Canonical Event 类型、Provider 接口或 P7 Kiro/EventStream 任务。
兼容性与迁移影响: 无 API、Schema 或数据迁移。仅将 G1 第二项精确为“任意已解码 Tool
      参数片段切分保持相同 Tool 语义 projection（call_id、name、最终 RawJson、SSE 与
      非流式输出）”。原始 bytes/EventStream 的任意切分一致性仍由 P7 验证。
测试与回滚变化: 保留 P1-09 固定/随机 seed、SSE/非流式和显式 {} 回归；批准后重跑完整
      门禁。回滚为撤销该 CR 与 Gate 状态文档更新；未创建阶段 tag 前不存在发布回滚。
用户批准: PENDING
计划版本变更: PENDING；批准后先更新计划元数据并记录 CR，再改变 G1 状态
```

## Review

- 复核了 `docs/06-development-plan.md`、`BC-CORE-003`、P1-03 至 P1-09 报告、每个任务
  分支的远端 Head，以及七个 GitHub workflow 的 Fast/Full job 结论。
- 复核确认本报告没有把语义 projection 的通过错误表述为 Canonical Event vector 相等，也没
  有通过文档变更放宽任何已批准的门禁。
- `ruby scripts/check-doc-links.rb` 与 `./scripts/check.sh fast` 在本报告变更后均通过。

## 未批准的替代方案

若坚持逐项 Canonical Event 向量相同，需要新增预 Canonical 的 Tool argument assembler：
它必须按 call 缓冲字节到 Tool End，再只发出一个完整 delta。这会改变事件颗粒度和 Tool
参数的流式可见性，并提前引入 Provider ingress 能力；按照计划 1.4，这是一项独立、范围更
大的 Change Request，不能为通过 G1 而隐式实施。

## 后续

在用户批准 `CR-P1-G1-001` 前，不创建 `phase-p1-complete` tag，不把 P1/G1 标为
`DONE`，不开始 P2，也不修改服务器。
