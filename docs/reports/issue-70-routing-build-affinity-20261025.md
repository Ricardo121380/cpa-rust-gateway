# #70 生产路由策略与普通 Build 亲和：本地交付

结论：`PASS_LOCAL`。在既有独立 worktree 完成本票实现、公共 HTTP 到独立 receiver 专项、项目 Full、双轴 code-review 和本地提交。没有 push、合并、远端评论/关闭、部署、真实凭据或真实 Provider 调用。远端工单在本次读取时仍为 OPEN、0 comments；本报告不改写远端勾选。

## 固定身份与证据范围

- 工作区：`worktrees/cpar-70-routing-build-affinity-20261025`；分支 `feature/70-routing-build-affinity`。
- 初始实际 Git 根、分支和 HEAD 均已核对；从干净的共同基线 `7c16070bb66f7f3008022f7870b5d1f96b63f167` 开始。保留所有本票增量、历史日志和独立状态，没有 reset/clean/stash 或重建工作区。
- **最终动态受测 commit：`731c318320ae6f9df69cc77a708ea67c468a2695`；tree：`0ca2706201f55a1a6d12e89cb7b34fb3c23aeeb7`。** 最终报告/入口及一行错误注释修正属于此后的文档后继；不把后继 SHA 冒称为重新执行 Full 的代码身份。
- `Cargo.lock` SHA-256：`a3950a764ae2dcae4e04a8a32ada596f33ea105dbfda8e9714b3e32d3216cd6d`；没有修改依赖或锁文件。
- CPA 来源固定 v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`，对应 `selector.go` 的 `RoundRobinSelector.Pick`、`WeightedRoundRobinSelector.Pick`、`FillFirstSelector.Pick`、`availableAuthsFromPriorityBuckets`、`pickSmoothWeightedAuth`。这是对 CPAR 既有有界 schedule 的行为适配；没有复制 CPA 可变 accumulator 或静默转换其数字方向。
- Full 环境：Darwin arm64 debug；cargo jobs=2，Rust test threads=4；数据库/临时状态在本 worktree 的 `output/issue-70/state`，日志在 `output/issue-70/logs`，端口由 OS 动态分配。

## 策略和亲和语义

| 配置值 | 正式 serving 行为 | 兼容边界 |
|---|---|---|
| `smooth_weighted_round_robin` | 保留旧 Provider comparator：硬资格后，已知 quota、单独启用的 price evidence、load、升序 priority、降序 weight、稳定 ID | 旧 weight 末端比较仍不被称为份额分流；默认不变 |
| `round_robin` | 同 eligible tier 内 candidate 单槽轮转；endpoint 内 credential 独立 RR，忽略其权重 | 通过明确配置接纳；不改变旧默认 |
| `weighted_round_robin` | 同 eligible tier 内按 candidate 权重编译槽；credential 另按自己的权重调度 | candidate weight 不乘账号数；被排除槽先过滤，不把它们捐给后继 |
| `fill_first` | 选择稳定 ID 最先且能实际租用的 candidate/credential；不可用时退出并切换 | 不推进轮转游标；恢复后按当前候选集决定 |

CPAR 的 priority 始终 lower-is-better。显式策略先按 priority、quota confidence、单独启用的 routing-price evidence 分组，然后应用策略。Candidate 排序和 endpoint 内 credential 调度分别持有游标，复用相同账号槽和原子 lease 容量；legacy/RR/显式 WRR 不再相互推进游标。现存 `priority_failover` 可以读取，生产配置入口仍不支持启用它。

CPA `round-robin` / `weighted-round-robin` / `fill-first` 由操作者明确选择对应 snake-case 值；本票不新增自动配置导入器。显式 candidate 权重须为 1..=10,000；weighted tier 仍有既有 1024-slot 上限。保留/导入配置也在 snapshot compilation 检查，非法配置不能替换 serving generation。账单价格的获取/应用与启用 routing-price policy 保持独立。

普通 Build 仅通过已有 `prompt_cache_key`、HMAC identity 和 continuity store 接通软亲和。调用方 Key、Build Provider、上游模型及派生身份组成原有存储作用域；canonical public model、Config Version/revision、candidate、endpoint、credential revision、所选 transport/egress 哈希进一步限定绑定。原始缓存身份和 proxy 材料不写入绑定或日志。当前 native Build 只准入既有 direct egress，未扩张代理支持；合成 receiver 验证同一所选出口，不代表真实代理故障验收。

请求开始时在 blocking task 一次读取 request-local hints；scheduler 和实际 lease 循环不访问 SQLite。提示只有在当前候选层、credential priority、权限、协议/目录准入、健康、额度、停用、过期、revision 和原子容量复核后才可能生效；失败则按正常策略重选。成功 bootstrap 后写入实际绑定，15 分钟 sliding TTL，最多 4096 绑定；break 历史也限 4096。过期记录在成功写入时清理，超限按最早更新时间淘汰。并发 miss 可独立分配，最后成功 bootstrap 的事务决定下一次偏好；旧 generation fingerprint 不能被当前代接受。

每次替换与 break 记录同事务提交。expiry/egress 有精确闭合原因，其余使用保守的 `policy_reselection`，不虚构容量或并发竞争的唯一根因。未测得的 token loss 为 SQL NULL，不填零。Store 错误安全失败，bootstrap 后持久化失败不授权重试。严格 stored/compact/reasoning/WebSocket 和诊断 pin 不读取/更新软亲和，仍保持精确 lineage、无 sibling fallback。

## 逐项验收

以下 A1–A7 与 S1–S7 按当前工单两组验收顺序对应，均为本地实现/验证范围的 PASS；真实服务、客户端与发布资格另列。

| 项 | 本地结果与证据 |
|---|---|
| A1 复用 identity/store 并作用于 serving | PASS。`build_affinity.rs` 的 prepare/bind 和实际候选/lease hint 接线；重复身份公共请求命中实际 receiver 账号和出口 |
| A2 owner/Provider/模型/身份隔离，容量和过期 | PASS。公共跨 Key 与 HMAC 断言；fingerprint 包含规范化 public model 和配置命名空间；schema40→41 store 测试保留旧值并验证 4096 上限和 NULL loss |
| A3 建立、命中、失效、饱和和重绑，资格先行 | PASS。公共 expiry、实际 semaphore 饱和、health/revision、账号停用/重绑；沿用既有 quota、权限和原子 lease 检查，并由 Full 保留相关回归 |
| A4 精确 stored/compact/WS lineage | PASS。新增公共 stored lineage 负例拒绝且零新增发送，普通请求可随后健康切换；既有 compact/WS/reasoning exact 测试在 Full 中保留。新增 receiver 专项没有声称分别重做真实 compact/WS 客户端资格 |
| A5 并发、配置发布、revision 和账号退出 | PASS。12 请求实际容量竞争；管理服务真实 publish/rollback 后新 runtime adoption；credential material revision 更新与 disabled account compile-exclusion；原子 rebind/break |
| A6 公共请求到独立 receiver，重试与释放 | PASS。七项公共 TCP 测试；过期/跨 Key/同账号出口、一次 429 切换、语义输出后 EOF 不再发、成功失败后 active lease 均归零 |
| A7 仅 Build 已有接缝 | PASS。仅 typed Build ordinary driver 挂接；没有全 Provider 会话框架、Grok Web 原生续接修复或 #71 请求规则 |
| S1 candidate 与 credential 边界和旧 comparator | PASS。ADR0133/BC-ROUTE-010 逐项记录；保留 legacy quota/price/load/priority/weight 原断言，不把 tie-break weight 称为份额 |
| S2 RR/WRR/fill-first 与 CPA 同层来源 | PASS。生产 Provider-scoped 路径消费既有 schedule；endpoint pool 独立策略/游标且共用容量；固定 CPA 符号和 priority 差异明确 |
| S3 旧配置、默认和 priority 数字方向 | PASS。旧默认/priority 保留，显式策略独立编码；validate/publish/rollback 测试覆盖 legacy 与三种新值 |
| S4 资格复核、无跨 Provider/严格 continuation fallback | PASS。原 orchestrator 的 fresh 检查与 lease 所有权不变；新增失败排除、饱和、stored 精确负例；Full 保留权限、quota、停用与 Provider 范围回归 |
| S5 管理校验/发布/回滚/Explain 与非法边界 | PASS。受保护 CRUD、revision、闭合输入；RR/fill 10000 成功、10001 拒绝、WRR 1025-slot 拒绝且 active Arc 不变；Explain 三策略固定 starts 0–3、不推进实际游标 |
| S6 公共生产轮转/份额/fill/并发和候选集变化 | PASS。独立 cache 身份 RR 8 次 candidate 4/4、credential 4/4；WRR 16 次 candidate 4/12、credential 4/12；fill 8 次同一 candidate/credential，另测首账号/首 candidate 不可用退出、集合恢复 |
| S7 新配置契约和必要架构整理 | PASS。OpenAPI、ADR0133、BC-ROUTE-010、traceability/cross-boundary/roadmap 同步；复用 selector/affinity/lease；没有依赖升级或无关框架 |

七项公共测试位于 [routing_affinity.rs](../../apps/gateway/src/runtime/batch_c/grok/routing_affinity.rs)：

1. `issue70_public_http_policies_rotate_candidates_and_credentials`
2. `issue70_public_http_affinity_expiry_owner_capacity_revision_and_health`
3. `issue70_public_http_failover_and_postsemantic_no_retry`
4. `issue70_public_http_concurrent_capacity_releases_every_lease`
5. `issue70_public_http_stored_lineage_never_uses_soft_fallback`
6. `issue70_public_http_fill_first_and_candidate_set_changes`
7. `issue70_public_http_configuration_rollback_and_disabled_account_rebind`

另外五项新增定向测试覆盖控制面发布/回滚、管理 CRUD、迁移/容量、Explain 固定输入和混合策略游标，名称列在 [BC-ROUTE-010](../contracts/BC-ROUTE-010-production-strategies-build-affinity.md)。新增共 12 个 Rust 测试函数；不是把重复执行计成新增测试。

## 验证和 code-review

| 验证 | 最终结果 | 范围 |
|---|---|---|
| Full05 | **PASS 44/44** | Clippy、Rust、stock serve envelope、agent 多轮/流式、边界/文档/secret、deny/audit 等完整项目门禁 |
| Full05 Rust | **1738 passed / 0 failed / 23 ignored**，135 result sets | `cargo test --locked --workspace --all-features`；ignored 未当 PASS |
| 显式 encrypted_backup | **7 passed / 0 failed / 0 ignored** | 同一代码 `--include-ignored`，含 >128MiB 拒绝、v1/v2、错钥/篡改/已有目标、事务/中断边界；单列，不累加进 Rust1738 |
| HTTP crate 定向 | 254 passed / 0 failed / 4 ignored | 修正旧 Explain 两处精确断言后全 crate 通过；是 Full 的子集，不叠加计数 |
| 文档后继检查 | **PASS7/7**，Rust format 和精确 diff 通过 | [docs 收据](issue-70-routing-build-affinity-20261025-evidence/docs-final.md)；新增文档另经 staged secret scan，未重标为 Full05 |
| 权重边界 Red→Green | RED exit101；修复后 Control114、Router193 全通过 | 原 RR retained weight10001 接受反例保留；最终 Full05 覆盖修复 |
| Standards | COMPLETE，行为 finding 已全部修复；P3 注释随文档后继修正 | 全范围 surface/kernel，修复后 delta，最后权重四文件完整复核；途中路径错误的 partial 结果保留 |
| Spec | COMPLETE，0 open findings | 全范围及修复 delta 完成；逐项要求见上表 |

具体审查发现/修复分轴保留在 [review-first.json](issue-70-routing-build-affinity-20261025-evidence/review-first.json)、[review-second.json](issue-70-routing-build-affinity-20261025-evidence/review-second.json)、[review-final.json](issue-70-routing-build-affinity-20261025-evidence/review-final.json)。曾发现 replay INSERT 占位符回归、WRR/RR Explain 投影、fill-first 退出证据、configuration-diff SQL 夹具及混合策略游标干扰；已修复并验证。原有权限、price、ledger、UNKNOWN 断言保留，没有通过删除失败断言使门禁变绿。

### 历史结果保留

| 运行/对象 | 原结果及含义 |
|---|---|
| check01/02，targeted01/02/03/05，management02，http01，clippy02–05 | 编译、WIP/夹具、API method、旧精确断言或 lint 失败日志保留；其中 targeted05 的七项公共测试已 PASS，但同批最初管理断言失败，不能整体称 PASS |
| Full01 / f8cd0d5c | Clippy exit101，未得到完整 Rust 或 Full PASS |
| Full02 / e3ba8ecb | Clippy PASS；审查确认缺陷后仅停止本 worktree 自有进程，exit143；保留 INTERRUPTION 收据，不称 Rust/Full PASS |
| 冻结 e3 configuration-diff test binary | RED exit101（7 columns/6 values）；记录实际二进制 SHA 后修正 fixture 显式旧列名，保留原业务断言 |
| Full03 / 1475be29 | Rust exit101：旧 OpenAPI required-fields 未含 selection_policy；保留 FAIL，不用后继结果改写 |
| Full04 / db2023c8 | PASS44/44、Rust1738/0/23、显式备份7/7；属于权重上限最终修复之前的源码范围 |
| weight-bound-red | exit101，retained RR10001 在修复前被接受；原日志保留。weight-bound-green 及 Full05 为修复后的独立结果 |

全部原始日志在 ignored `output/issue-70/logs`，Full02 中断与 frozen SQL RED 收据位于同级目录。供应链重复版本 warning 保留；没有新增 ignore、跳过 gate 或升级依赖。

## 冻结产物和输入

[机器摘要](issue-70-routing-build-affinity-20261025-evidence/summary-05.json) 和 [Full05 44项收据](issue-70-routing-build-affinity-20261025-evidence/full-05.md) 纳入本地提交。完整源码/输入/二进制/日志清单位于本 worktree 的 ignored output，摘要保留绝对路径和 SHA-256：

| 固定对象 | SHA-256 / 作用域 |
|---|---|
| source-05.json | `518acf249d7d6d845e3dc6f1e78de89d8348a4fa1c8f5a84c6f597fd6915c849`；7218 tracked file hashes，Full 结束后逐文件检查 0 drift |
| input-05.json | `00c964c501cbb17d097c1dcb700cb43c20c808850a968e0199296f76fbd70b34`；测试入口、脚本、合成请求/权重/并发/fault/环境与 tracked closure；不保留临时随机秘密 |
| stock gateway-731c3183 | `a86e7e19ac0cb2313b116ba5270e81a94fb98e11eb1efa308843d25378dea860`；Full05 default-feature serve-envelope 正式进程构建 |
| gateway-tests-731c3183 | `0085b7e95da05b564e614cf6f3d5ae7d9f9dd112ee770a549da15fd7b17b42ef`；all-feature gateway 测试执行文件，生产 factory/公开 TCP + 本地独立 receiver；固定 origin handoff 和 alias 资格证明仅属测试 |
| test-binaries-05.json | `dac6d71719fb95fdfc116da21417118d8139fb00f4828602581b82d89e5c82ef`；115 个实际执行 Rust 目标的路径、大小和 SHA |
| logs-05.json | `28474752d87065db3afdfc907a3214ebc94455add74ac7c1c78d449199681d8e`；本地原始日志哈希清单 |

source-after-05.json 记录 Full/显式备份完成时输入文件无漂移。最终后继只更改报告/文档入口/一行 rustdoc，以 docs 门禁和精确 diff 验证；不更换受测二进制或把后继追溯标成 Full05。

## 共享接口、保留边界与集成注意

- Route/Snapshot policy 增加显式 WRR/fill-first，RR 正式准入；SQL 保留旧 policy 字段并在新 `selection_policy` 列编码新选择。管理 OpenAPI Explain 增加必需的 `selection_policy`；旧客户端若严格封闭字段，需按新契约更新。
- `AttemptDriver` 增加可选 request-local preferred credential hint；endpoint pool 增加明确 policy 选择和隔离游标；租约 ownership/retry/release 继续由原 orchestrator/pool 管理。
- ADR0133 / migration0041_routing_build_affinity / standalone schema41 增加绑定 fingerprint、credential revision、NULL loss 和闭合 policy_reselection 原因；down guard 拒绝会丢失新语义的回退。
- **#71 并行分支另有 migration0041_endpoint_request_policy / standalone schema41，已在共享 COORDINATION 和本票 reservation 中记录碰撞。两个 schema41 身份不同；禁止互相采用数据库。** 后续共同集成须从同一 schema40 前缀协调唯一迁移序列和恢复资格。本票没有修改 #71 工作区、导入其代码或假造占位迁移。
- #69 名称/控制/count 权限沿用已解析 canonical public model；#73 精确计价身份、旧 ledger、冻结导出和 price-review 证据无改写；应用价格不启用 routing-price policy。完整 Full 保留既有相关回归。
- #75 加密容器、空目标、key/version、事务及大小边界由现有实现和显式备份专项继续验证。**本票没有重跑 wave2 非空 review 大库的 198 项 stock 进程联合资格，不能把原 schema40 大库结果移植成新 schema41 的同等资格。** 原98,557,485-byte artifact、source/content digest 和所有回执按原身份保留。
- 含 `Some(model_thinking)` 的新 payload 被旧 strict reader 拒绝的可能仍存在；仅 down schema 或保留 v1 容器不保证旧 binary 可读。保留原 binary/artifact 和全部 Master Key 版本，不丢弃历史制造兼容。
- 本机合成调试结果不建立真实 Provider cache retention、真实 token loss/费用、固定 Build 激活、未修改 Codex/Claude/Pi、远端 CI、release/OCI/Linux 或部署资格。`cost_in_usd_ticks` 仍 `NOT_ADMITTED`；未放宽 #61–#67/首版 READY 条件。

## 本地提交

| 提交 | 内容 |
|---|---|
| f8cd0d5c01d5e6878ff1664172eecdb3f72f4fa9 | 显式生产策略与普通 Build 亲和 |
| e3ba8ecb354381380ee579218fc2b47b59cde7c6 | 修复加权 Explain/replay SQL，补 fill-first 公共证据 |
| 1475be2978d4aa56a455dd303e9d5398c76533bc | 隔离策略游标、固定 Explain 输入、保留旧 fixture SQL |
| db2023c8e6706a0b34c061a07aaff04ca11b1283 | 更新 Explain 精确 HTTP 契约断言 |
| 731c318320ae6f9df69cc77a708ea67c468a2695 | retained/imported 显式权重边界在 publication 前拒绝；最终动态受测代码 |

最终文档后继可由本分支 git log 解析；它不改变以上动态受测身份。仅本票 owned 路径进入提交，未执行远端写入。
