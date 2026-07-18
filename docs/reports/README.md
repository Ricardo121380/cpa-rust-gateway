# Verification reports

本目录保存 Task、Phase Gate、基准、差分、安全和部署验证证据。

## 命名规则

- Task：`p<phase>-<task>-short-title.md`。
- Gate：`g<phase>-gate-report.md`。
- 基准：`benchmark-YYYY-MM-DD-short-title.md`。
- 安全：`security-YYYY-MM-DD-short-title.md`。

## 报告最低内容

- 计划版本、Task/Gate、日期和环境。
- 改动范围与对应 Matrix/Contract/ADR。
- 执行命令及退出状态。
- 可复查结果和已知限制。
- 失败、偏差、回滚与后续任务。

报告不得保存 Secret、Cookie、Authorization Header、原始 Cache Key、生产 Body 或未脱敏日志。需要原始材料时，只记录受控外部位置和不可逆摘要。

## 已完成阶段

- [G0 阶段门禁报告](g0-gate-report.md)
- [G0 干净检出完整门禁日志](g0-clean-full-log.md)
- [G0 可复现构建日志](g0-reproducible-build-log.md)

## 已完成任务

- [P1-01 Request context and errors report](p1-01-request-context-errors.md)
- [P1-02 Canonical request report](p1-02-canonical-request.md)
- [P1-03 Canonical event state machine report](p1-03-canonical-event-state-machine.md)
- [P1-04 Bounded canonical stream report](p1-04-bounded-stream.md)
- [P1-05 OpenAI Responses adapter report](p1-05-openai-responses-adapter.md)
- [P1-06 Deterministic Mock Provider report](p1-06-deterministic-mock-provider.md)
