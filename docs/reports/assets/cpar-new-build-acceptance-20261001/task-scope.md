# Current task scope for #57 preparation

User requested implementation of “新构建真实渠道验收工具与环境准备”, based on
499896e6 report / 2e43ea4b business code, refreshed against actual current files,
GitHub #9, #39–50 and #57. This preparation can deliver independently; it does
not close C, #57 or #9.

Required: reuse existing live runner/test facilities; preserve historical smoke
receipt meaning; observe actual process/artifact/source SHA/checksum/instance/time
(an operator-supplied runtime commit is insufficient). Freeze exact channel,
endpoint, route/candidate, model, downstream protocol/mode, account/credential
revision, config revision, parameters and actual declared capabilities. Verify
the actual attempt; another successful target cannot substitute.

Required basics for Chat/Messages/Responses JSON/SSE: independent text multi-turn;
one tool cycle/result; two tool cycles/final answer; exact tool ID/name/arguments/
result/full ordered history; unique terminal/error/cancel/timeout/retry boundary;
native Usage and available exact ledger/attempt attribution. Validate declared
reasoning/parallel/stored/continuation/compact/WebSocket extensions individually.
“尚未读取声明时保持未知，不能把未知当成未声明。”

Public receipts distinguish LOCAL_SIMULATED, real new-build and production;
PASS/FAIL/NOT_RUN/BLOCKED, actual versions/environment/command/time/failure/evidence
paths, preserve unknowns, exclude account materials, secrets and sensitive bodies.
Use isolated local meaningful positives/negatives; reproduce defects before fix:
wrong target/unconfirmed version; incomplete history/arguments/results; false
success/truncation/replay; missing/misattributed Usage; uncompleted second cycle;
declared extensions uncovered. HTTP200/report existence alone cannot establish pass.

Read-only current checks within existing authorization: actual runtime, accounts,
routes, permission, egress and declaration inventory; separate immediately
acceptable combinations, missing environment/auth/route and software/protocol
blockers. Prior account/build results are historical only. No deployment,
service change, config publication, account recovery, permission changes or
inference is authorized automatically by this task. Complete independent prep
before proposing specific additional operations/impact/rollback.

Preserve Kiro native output-cap, Grok Web native multi-turn and Messages required
output-cap blockers. No large Provider rewrites, parameter removal, prompt
simulation, downstream-only truncation, disabling declarations or weaker
assertions. Only accepted exclusions are Grok Web tools unsupported and exact
Messages token counting unsupported; other non-equivalent combinations retain
their explicit status/specification.

Deliver code/commands, local positive/negative evidence, separate Standards and
Spec review, applicable quality gates at immutable tested source, current
readiness/blocked combination list, concrete ordered next real execution targets
and missing prerequisites, local commits and #57 evidence. Remote write/push/
close depends on explicit authorization; none was performed this round.
