# B-04 independent review record

Fixed base: `08812db2373583b8a725b80071efe75b69f9d7ae`. No reviewer edited files or ran
tests/Clippy. Main agent owns all changes and execution. Five interrupted rounds are recorded
in `review-interruptions.json`; they are not PASS and provide no complete-coverage conclusion.

## Standards

Agent `/root/b04_standards_scoped`, target `42fae997ef7c82f7e611e97f22da8b5a79634233`:

> Standards：FAIL（1 项文档登记缺口）。未发现待修源码硬违例或 actionable smell。
> 完整覆盖 25 files：14 Rust、11 文档，不计原有 dirty AGENTS/未跟踪资料。
> [P2] 新 ADR 未登记需求追踪索引。规则 `docs/adr/README.md:28` 要求在需求追踪索引登记；
> 新决策 `ADR-0103:7,33`、`event_store.rs:259` 改为 Attempt 级 Usage 身份，diff 只有 ADR
> README 索引项。`docs/traceability.md` 搜索无 ADR-0103/B-04/bootstrap。应登记
> B-04 → ADR/Contract → 实现/测试 → 报告。
> 历史过程偏差单列：development-plan:326,330 要求探索前等级和数值预算；Task card:12–21
> 明认未做到并给剩余预算。不能补记为先行合规，不是当前源码修复项。
> 12 项 smell 未形成需修 finding。driver 方法和 decoder accessor 有实际生产消费者，跨层身份
> 改动有因果，Option 槽与有序 Request 计数无新增生产无界集合。fixture 有实读断言、真实
> writer/worker/账本、重复确认及可用 fallback。ADR 明示旧 binary rollback 不兼容。
> 未运行 tests/Clippy/Full；未核验 staged secret；IN_PROGRESS 报告未虚称完成。

Agent `/root/b04_standards_recheck`, target `640f4a188336326b58c85266182d8faabad5d725`:

> PASS（Standards 复核）。新增范围仅 `docs/traceability.md` 六行；固定 base 共 26 文件。
> 14 Rust blob 均与 42fae997 相同。原 25 文件及 smell 判断沿用上轮，本轮完整覆盖新增文档。
> 原 P2 已关闭：traceability:225 登记完整链，五个相对链接、四个 Contract 在 HEAD 存在，
> 三个测试符号存在。新增行 IN_PROGRESS、仅本地 B-04、不关闭 #32/Batch B/#57/发布。
> 历史过程偏差保留；本次 PASS 不消除此偏差。

## Spec

Agent `/root/b04_spec_bootstrap`, target `42fae997ef7c82f7e611e97f22da8b5a79634233`:

> PASS（静态）。完整覆盖指定 10 个源码 diff：runtime、batch_c、http、bootstrap_usage、
> responses_done、responses_order、execution_identity、router、Chat/Responses decoder；
> 完整读取 ADR-0100/0103、BC-OBS-001，核对 B-04/B-03 与 #32 Usage。不是全 25 文件结论。
> runtime:6058,7352 在 begin 失败取已校验 ID/Usage，保留错误；4661 清槽，4763 take，无新增
> 上游 read/补采。router:1455–1468 先确认准确失败 Attempt，再确认 Usage，失败阻止 fallback。
> bootstrap_usage:464–467 是实际 read 字节比较，http:163–167 无 sleep。负例、snapshot、
> duplicate、same Response ID 的各 Attempt 独立归属都有覆盖。Responses 保留 NonRetryable，
> 实际 received/handoff 计数；B-03 EOF 正对照保留备用成功。无 completed/stored/续接成功根。
> Messages 是先 handoff 的后续失败，output placeholder 未冒称观测；成功/后续失败/取消保持
> 原 observer。管理使用测试 facade，ADR 不代表完整部署/重启。
> 未审 Store/control、未执行 Full/Provider/生产，交给其他 scope 与主代理。

Agent `/root/b04_spec_control`, target `640f4a188336326b58c85266182d8faabad5d725`:

> PASS，零 finding。完整覆盖两个 control 源码、12 文档 diff、ADR-0100/0103 与实际 writer/
> worker/protected read 测试部分。materializer:275–303 不同 explicit Attempt 独立接受，同
> Attempt 冲突、legacy 混合、missing ID、Request 不匹配均 InvalidLineage；349–359 原 source ID、
> 精确账号及完整 UsageSummary。control:1702–1796 按 Request/Attempt 去重，exclusion 按 Request
> 计数，1424–1444 常量内存同组 request_count=1、observations=2。SQL 排序包含 Request ID。
> 测试真实 queue/writer/BillingWorker，facade 调生产只读 reader。报告如实 IN_PROGRESS、未知
> 完整度、Full/部署/重启/Provider 限制，不扩大到 #32/#57。
> 未执行 tests/Clippy、未审 Store/bootstrap 全 diff；属分工排除，无指定覆盖缺口。

Agent `/root/b04_spec_store`, target `640f4a188336326b58c85266182d8faabad5d725`:

> FAIL 候选，1 项，完整覆盖两个 Store 源码及必要消费者，无指定 diff 未读缺口。
> 新按 explicit Attempt 分组（event_store:621–625），partial 校验回连仍只按 Request（678–680）。
> 同一 Request 的 A1 历史冲突 Usage 与健康 A2 Usage 使 A2 被拿到 A1 校验，decode_lineage:1470–1474
> 失败，不能仅排除 A1、返回 A2。违反 BC-MGMT-010:24–26 的 partial exclusions 和 ADR-0103:44–45
> 不同 explicit Attempt 不因共享 Request 冲突。其余 ID/legacy replay/unknown no-enrichment、排序、
> quarantine 指定检查静态 PASS。未执行行为测试，主代理需复现裁定。

主代理裁定：`partial-lineage-red-receipt.json` 在 `640f4a18` + 新测试上 exit 101，
0 passed / 1 failed，真实 `InvalidPersistedGatewayEvent`，确认该候选。最小修复令 partial
校验只回连相同 explicit Attempt；unknown legacy 仍按原有组校验，不放宽损坏历史。
人工插入的合法历史冲突 fixture 只证明 Store 表示及隔离；不声称它经过生产 writer。
GREEN：`partial-lineage-green-receipt.json` exit 0，1 passed / 0 failed；
`review-fix-store-regressions-receipt.json` exit 0，9 passed / 0 failed。
这些运行实际 HEAD 为 640f4a18 + 修复文件哈希，并未改标为提交后的 062f8281 运行。

本轮两项可修 finding 均已关闭。以下最终 delta 复核与逐 blob 证明共同收敛至最终源码。
轴结论分别记录，不把两轴混合排名。


## Standards final delta

Agent `/root/b04_final_standards_delta`, target `062f8281279b9bc6a0dc36b2859120e26e5f85dd`:

> complete. Standards delta：PASS，新增硬性规范 findings＝0。
> 完整读取 640f4a18...HEAD 两文件 diff 与必要上下文；固定 base 共 26 文件（14 Rust＋12 docs）。
> 逐 blob 比较确认其余 24 文件完全未变，可合并既有审查覆盖；未重新声称全量独立阅读。
> event_store:620,681 的 unknown_attempt 聚合及精确 Attempt join 限于现有读取事务；
> 无新增接口、schema、依赖或生产禁止构造，符合 quality-gates:38–41；十二类 smell 无新硬违例。
> 新测试 event_store:1853 验证健康 Attempt、排除计数与严格读取失败；BC-OBS-002:123 同名登记。
> traceability:225 维持 ADR 需求登记，未触及共享路径。
> 历史事前等级/预算缺失仍是过程偏差；此 PASS 不追溯消除。未执行 tests/Clippy/Full。

## Spec final Store

Agent `/root/b04_final_spec_store`, target `062f8281279b9bc6a0dc36b2859120e26e5f85dd`:

> complete. PASS：本轮静态 Spec Store 复核无新增 finding；旧 finding 静态关闭。
> 完整审查 base...HEAD 两 Store 文件、BC-OBS-002 全部 delta，640f4a18...HEAD 全部 delta；
> 亲读 ADR-0100 全 94 行、ADR-0103 全 80 行及 MGMT-010。限定范围 diff --check 通过。
> event_store:620 保留 unknown_attempt；681 按同一 explicit Attempt 回连，A1 冲突不再误配健康 A2。
> 695–696、1470–1476 仍逐条校验 lineage；unknown 历史仍扩大验证，malformed/missing 未放宽。
> event_store:259–272 为新 Usage 派生 Attempt 身份；364–384 保留旧 Request SHA/ResponseId 精确
> 重放并拒绝 unknown enrichment；916–924 保留读入校验。billing_ledger:87–94 只将同 Attempt 或
> unknown 的不同 payload 判为不可重试冲突，不隔离其他 explicit Attempt；625–631 幂等，
> 652–663 六 counters 与 evidence 保留。人工 SQL 回归只证明历史 Store 表示及隔离。
> tests、clippy、Full、Provider NOT_RUN；仅此静态覆盖，不声称审过其余文件。

最终源码双轴：**Standards PASS / Spec PASS**。完整 26 文件覆盖及每文件最终 blob 相等证明见
`review-coverage.json`。原始 FAIL 与中断记录保留；主代理执行 Full 的结果独立记录，不由静态
review 的 PASS 推导。证据收口报告/回执是后续文档增量，不伪装为这 26 文件的原始 review 内容。
