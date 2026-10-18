# 独立准备结果：保持 #62 OPEN / BLOCKED

依本轮明确授权，在 #61 未关闭时完成限定的 #62 独立准备。正式 `Blocked by #61`、原验收条件与 #61 的 cost **NOT_ADMITTED** 均保持；未重做无新增材料的 cost 搜索，也未将标准合成正例解释为成功元数据留存、真实 Grok Build 或首版通过。

- **固定身份**：实际 `/opt/homebrew/bin/codex` 为 Codex 0.161.0；实际 `/opt/homebrew/bin/pi` 指向原版 `@earendil-works/pi-coding-agent/dist/cli.js`，运行 0.84.3。Pi 包声明的 `dist/bundle/cli.js` 只做版本探针；没有修链或换入口。实际 ESM 证明私有 Pi AI SDK 0.84.3 / OpenAI Node 6.40.0。only-my-pi 单独辨识，未用于推理。
- **受控链路**：真实 CLI 进程 → CPAR 真实公开 HTTP/鉴权 → production compiler/factory/执行链 → 实际 leased Build adapter → 既有受控 transport handoff → 独立 loopback TCP 上游。仅新增 cfg(test) 测试及驱动，无生产 endpoint/协议放宽。HOME、配置、凭据、历史、工作/工具目录隔离；同一个继承的 Seatbelt 边界实际拒绝 IPv4/IPv6 非 loopback 连接、真实用户目录读取及临时目录外写入（EPERM）。
- **Codex：FAIL / BLOCKED**。未经删改的实际请求包含 namespace/web_search，被现有 function-only codec 在发送前返回400；独立 receive/Attempt/Usage/ledger 均0，唯一 turn.failed。reasoning 实为 summary=auto 而无 effort；该额外语义冲突仅作静态定位，未猜 effort 或裁字段重发。必需工具闭环、原生 SSE/失败/取消未成立，不能称客户端可用。
- **原版 Pi：限定标准路径通过**。保留实际默认 medium/auto/include、read/bash/edit/write 和控制字段；完成回答、read(seed=41) → bash(42并保存/读回) 两次相关工具循环及后续问答，共5个独立上游请求。summary 身份、call ID、完整参数/结果历史与顺序、增量流和唯一终态已核对。只证明顺序显式历史模式，不外推并行或服务端续接。
- **取消/失败/计数限制**：同一 Pi PID 从 abort 到观测结束保持存活，14,975ms 时上游关闭且实际 lease=0，随后才清理进程；一秒严格探针仍FAIL，不是新增产品SLA。测试判据曾遗漏整段存活核对，已以实际提前 SIGTERM 反例复现旧误通过并修复为准确失败，仅为测试判据 RED/GREEN。Pi 失败公共终态和 durable Usage/ledger 为20/7，客户端 error 却默认显示0，保留PARTIAL。独立接收与 Request/Attempt/Usage/source-event/ledger 分场景关联；取消无终态Usage不推出零费用。已知 background 负例400、receive/Attempt增量0，未混作原始CLI请求。
- **#61 前置仍 BLOCKED**：标准成功/失败 owner-scoped reader 的完整原生 Usage companion 为 None / UNKNOWN。#59 失败留存不能替代成功元数据完整加密、成功交付前确认及 exact Attempt 授权读回；未给成功 fixture 加 cost/未知字段。

验证：最终代码候选 `6490922141f64aa4222c907634b02502cffb11cd`；不可变验证源码 `2fc4777abbac3e6b203323a0884d7a011308bac3`（六文件哈希一致，16份既有用户文档 overlays 仅用于验证）。**Full 45/45；Rust1542 passed /0 failed /14 ignored；400.288s**。同源码实际CLI默认检查1/0；严格一秒探针0/1，原始失败证据保留。归档96成员独立读回一致，秘密扫描和静态检查通过。固定基线 `1743dd08` → 证据候选 `368c9b92` 的双轴复核：**Standards PASS，0规范违规/0 Fowler判断项；Spec PARTIAL/BLOCKED，3类整票缺口，0 scope creep、无最终增量新增行为缺陷**。

本地报告：`docs/reports/cpar-62-independent-20261019.md`；证据：`docs/reports/assets/cpar-62-independent-20261019/`。本轮只提交自有增量，原有 dirty/untracked 库存保留。提交尚未 push，因此上述源码/报告为本地可审查材料。

**#62 继续 OPEN / BLOCKED；依赖与原 AC 不变。未修改 #61 或其他工单，未推进 #63–#67。真实 Provider、真实账号/新增请求预算、push、部署、发布与远端CI均 NOT_RUN。**
