# #59 — Grok Build 失败请求的完整加密诊断闭环

日期：2026-10-18（Asia/Shanghai）。状态：本地实施、定向验证、修复后 Full 和固定候选双轴评审已通过。提交 SHA 与仅针对 #59 的 GitHub 回写/关闭独立读回见最终交付。

## 范围与基线

本轮仅实施 [#59](https://github.com/Ricardo121380/cpa-rust-gateway/issues/59)，父规格为
[#58](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58) T06/T07/T08。
基线分支 `codex/prism-v4-delivery`，HEAD `3369c730e18f095bb4d9e1b506061e985a475fb9`，
初始暂存区为空。初始已有 9 个跟踪文件改动与大量未跟踪历史材料；清单逐文件记录
49,022 个路径的内容哈希、mode 和大小。历史 SHA 仅用于比较。

通过 gh 重新读取 #59/#58 正文与全部评论（开工时均无评论）、#59 原生 blocker 与分配状态。
#59 为 OPEN、ready-for-agent、无 blocker、无人分配，随后按本轮授权认领。
只允许对 #59 回写及符合条件后的关闭；#58/#9/#47/#57、后续票、真实 Provider、push、部署和发布均不属于本轮操作。

根 AGENTS.md、docs/agents 的 issue/triage/domain 规则、CONTEXT.md、implement/TDD/code-review
及相关 ADR 已读取。原有脏改动保留；ADR 索引与 traceability 仅追加本票条目，提交时单独选取本轮增量。

本票属于跨 crate、公开失败与持久化模型的 L 范围。执行分为公共复现、精确切片与确认、授权 reader、故障与生命周期、完整检查与双轴评审。多轮返工来自已复现的公共失败反馈、SSE 收口、错误谱系隔离、周期 GC 与完整迁移清单；原 RED、编译反馈和 Full FAIL 均保留，不以耗时替代完成标准。

## 实际改变

1. Build JSON `/usage` 与 SSE terminal `/response/usage` 保留收到的原生对象切片；严格解析及
   已知计数校验仍先行。大整数、小数词法、缺失/null/0 与层级保持，未知扩展继续拒绝。
2. 普通 GatewayEvent 继续只含原有 UsageSummary。新敏感确认方法在现有有界队列同一槽中携带
   私有伴随项；现有 blocking SQLite writer 使用现有 SecretStore/MasterKeyRing/AEAD 加密。
   source-event 与密文事务提交之后才确认 Persisted，排队、超时或失败不算留存。
3. schema 35 新建 Attempt 级 `native_usage_evidence`。owner 从已准入 Request 解析；核对实际
   Attempt、source-event 与标准计数。AAD 绑定 owner/Request/Attempt/source/版本/生命周期。
   相同内容幂等、不同内容冲突、旧记录不补造，密文交换与篡改认证失败。
4. 公共失败帧之前确认已经解码的证据；录制失败反馈 RecordingUnavailable。私有失败但普通存储
   可用时，只在谱系合法的前提下保存已验证计数，不虚报私有留存，不重复 Usage 或账本。
5. 只读 owner reader 与实际 `gateway usage-evidence` 本机命令交付授权读回。输出仅写入
   canonical 0700 目录下全新、独占、no-follow 的 0600 文件；stdout/错误/普通事件不含原文。
6. 固定 30 天 read-time expiry，128 行有界 GC 接入现有周期 maintenance；事件与账本不随之删除。
   旧/新密钥按保留 key version 读取；非空加密整库备份/恢复与兼容 reader 单独验证。

记录格式、reader 操作及最低迁移/恢复条件见
[ADR-0109](../adr/ADR-0109-encrypted-failed-build-usage-evidence.md) 与
[BC-OBS-006](../contracts/BC-OBS-006-native-usage-evidence.md)。

## 复现、验证与原始失败

证据目录为 [assets/cpar-59-20261018](assets/cpar-59-20261018/)。所有调用使用隔离临时状态、
合成密钥/凭据、生产 compiler/factory/leased Build adapter 与公共 Responses handler。
独立 loopback HTTP 服务只在完整收到上游请求后计数；定向验证置于系统 loopback-only 网络限制，
非 loopback 合成目标被 EPERM 阻止。这里没有真实 Provider 接收/费用、远端 CI 或生产验收结论。

| 验证 | 已有结果与证据 |
| --- | --- |
| 原始公共缺口 | `public-red-confirmed.log`：JSON/SSE 原拒绝均成立，各接收一次，但完整记录为 0/2 |
| 原始录制失败缺口 | `fault-red-behavior.log`：私有录制失败未反映到公开结果；原证据保留 |
| 公共闭环 | `public-legal-failure-green.log`：JSON/SSE 精确读回、缺密钥及实际 SQLite 锁确认超时、拒绝保持、唯一合法 SSE 失败、重复交付单 Usage/source/ledger、价格空；拒绝后不继续读取 EOF |
| owner/密文/生命周期/CLI | `storage-index-green.log`：8 组通过，包括真实 reader 子进程、非空加密整库恢复、缺失 Attempt 隔离及超时后的既有 pending 事务恢复 |
| 原始维护缺口 | `maintenance-red.log`：没有后续流量时，原 worker 不清理新增的过期证据；`maintenance-green.log` 验证修复及保留未过期行 |
| 编译与静态检查 | workspace check 与最终 Clippy 通过；中间编译/Clippy 反馈保留 |
| 仓库 Full | 修复后最终 `full-after-review.log` 与 `full-after-review-steps.md` 全部 45 步通过；Rust 1526 通过、0 失败、13 忽略。首轮 schema 清单 FAIL 保留在 full-initial.log；已补齐新增表，未降低断言 |
| Standards / Spec | 首轮 Standards FAIL（P2）/ Spec PASS 保留；修复后固定候选 `58bbec33`：Standards PASS（0 硬违例，2 非阻断项，其中文档编号已修正）/ Spec PASS（7/7 AC，0 发现） |

首轮 Standards 的 P2 发现已在 `index-integrity-red.log` 实际复现：普通 reader 拒绝损坏的事件索引，而新 reader 错误接受。修复复用 BC-OBS-002 的现有 canonical decoder；`storage-index-green.log` 覆盖三类事件的关联/时间索引破坏与恢复。前置 `index-integrity-fixture-guard.log` 为 append-only 防护触发的夹具失败，单独保留。首轮结论见 [review-round1.md](assets/cpar-59-20261018/review-round1.md)。

早期编译错误、断言命名错误与中间静态检查失败属于本轮开发记录；不替代上述实际行为 RED，
也不改写先前真实失败、未知价格/模型关系或已经停止的 epoch。

## #59 验收逐项对照

| #59 验收条件 | 当前核对与边界 |
| --- | --- |
| 公共入口、生产装配、JSON/SSE、独立接收数及原失败 | 定向 PASS；每次受控服务接收 1，标准拒绝保留 |
| exact Request/Attempt/owner、完整原值/结构/精度/source/完整性 | 定向 PASS；reader 核对 raw_json 原切片及 complete_usage_object，未知计数不补零 |
| 复用 AEAD/key/有界 writer、授权读回、真正确认及隐私 | 定向 PASS；回读发生在 writer shutdown flush 前，普通事件和公开错误无原文 |
| 完整性、owner、幂等/冲突、交换密文、写入/确认失败 | 定向及 Full PASS；固定候选双轴评审 PASS |
| 已知计数、失败 Usage/source-event/ledger、费用 UNKNOWN | 定向 PASS；单 Attempt 单 source/ledger，原生价格不造账 |
| ADR、格式、TTL、reader、重开、旧 UNKNOWN、密钥和最低恢复 | 文档、8 组存储/恢复/CLI、周期 GC 与 Full PASS；固定候选双轴评审 PASS |
| 无字段放开、原失败不变、不实现新交付策略 | 定向 PASS；#60、native-v1 和三客户端可用性保持独立 |

全量初始路径核对：`preservation.json` 证明 49,020 个路径内容及 mode 完全未变；另 2 个文档保留原始全部字节，仅追加本票条目。`network-denial.json` 记录 loopback 可通、非 loopback 合成地址 EPERM。原始日志在本地保留，交付归档 [validation-logs.tar.gz](assets/cpar-59-20261018/validation-logs.tar.gz) 同时保存全部 `.log` 原文；[log-manifest.json](assets/cpar-59-20261018/log-manifest.json) 给出 SHA-256 清单。

## 收尾门槛

固定评审基线、候选、两轴结论与非阻断项处置见 [review-final.md](assets/cpar-59-20261018/review-final.md)。新契约编号改为 BC-OBS-006，消除与既有 BC-OBS-005 的歧义；仅修改本票文档引用。21 个产品文件与最终 Full 和评审候选完全一致。

提交前核对既有内容/mode 保留、本轮暂存 diff、文档/契约引用与 staged secret/whitespace；只提交本轮增量，再仅向 #59 回写并独立读回关闭结果。最终提交 SHA 和 GitHub 操作回执随最终交付提供，避免额外 docs-only 提交。真实 Provider、远端 CI、正式候选、部署和生产状态均为 NOT_RUN，本票完成不改变父规格和旧票的剩余义务。
