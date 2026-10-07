# B-02 fixed scope

日期：2026-10-08，Asia/Shanghai。固定起点：`6883f47ea89332563b6ac90ce42c077387b0f9c5`；上次产品源码 `939028d63519126f16252c3e72d3e1d5ace7ca75`；分支 `codex/prism-v4-delivery`。实际 HEAD 与用户提供的基线一致，没有后续 B-02 修复。

本轮只处理 [B-02/P2 原发现](../cpar-release-readiness-20261005/review-results.md)：[#27](issue-27.md)、[#31](issue-31.md)、[父规格 #9](issue-9.md)中的 Responses SSE 完成确认校验和失败闭环。状态在本地 Full 与两轴审查结束前为 IN_PROGRESS，不替代历史 Phase 计划。

用户明确指定的测试边界为公开 gateway 请求、受控 loopback SSE 上游、合成账号和临时 SQLite，以及实际保存、续接与 WebSocket。沿用生产 Client-Key 鉴权、route compiler、leased executor、provider builder、上游 SSE decoder、协议投影和 HTTP/WS writer。HTTP 请求由 Actix test service 分派；WebSocket 使用真实 loopback RFC 6455 server/client。普通 Responses 的 `stored_responses:true` 使用既有闭集 candidate opt-in；WebSocket 是该 adapter 已有能力。测试不会关闭这些能力，也不会赋予其他渠道新能力。

主代理亲读基础文档和即将修改的确切代码；探子只定位外围调用链与既有测试，只读、单轮、fork_turns=none；派发后主代理等待终态，修改、判断和最终验证由主代理完成。两轴 code-review 也使用互相独立的只读探子。

不修改 B-03 最终 item 顺序、B-04 bootstrap Usage、B-05 密钥集合、E-05 revision/same-grant 规格冲突或 O-01 phase-only JSON 准入。A-01/B-01 保留既有 RESOLVED_LOCAL。不修改前端、生产、真实账号或 Provider；不推送、发布、部署、生成候选签名、写入或关闭 GitHub 工单。原 source80 制品与演练维持原绑定；项目发布 BLOCKED。

工作区起始 4277 个用户修改／未跟踪文件的完整哈希清单仅保留本机，交付前复核；AGENTS.md 不纳入提交。

最终收口：B-02 **RESOLVED_LOCAL**；最终修复源码 `15babae64154f8ff5fa1055f856d11640385633a`，本地Full45/45、两轴组合COMPLETE28/28且PASS。初审失败与复核PARTIAL均保留并补齐。正式远端Phase、发布候选签名／制品和生产验收不继承原source80；发布仍BLOCKED。
