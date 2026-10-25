# #73 价格获取、差异预览与确认应用：本地交付记录

日期：2026-10-25（Asia/Shanghai）。**PASS_BOUNDED_LOCAL**：实现、受控验收、项目门禁与本地提交完成。远端工单未修改。

Baseline: `84fbfbf778b582f746c1cafe31a2ce58962e8f77` from the verified #68/#72/#74 integration.
Branch `feature/73-price-preview-versioning`, independent worktree
`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar-73-price-preview-versioning-20261025`.

Scope and semantics: [ADR-0131](../adr/ADR-0131-reviewed-price-source-versioning.md),
[BC-MGMT-021](../contracts/BC-MGMT-021-reviewed-price-source-versioning.md).

Fixed CPA reference: v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`.
The official tag and complete tree were checked; 97 management/redisqueue/SDK Usage Go files and both
management route files in the local pinned source copy matched their Git blob identities. Actual
comparison symbols: `GetUsageQueue`, `parseUsageQueueCount` in
`internal/api/handlers/management/usage.go`; `GetAPIKeyUsage` in `api_key_usage.go`;
`usage.PublishRecord`/`Plugin.HandleUsage`, `redisqueue.Enqueue`/`PopOldest` and
`TokenAccountingSchemaVersion=2`. The queue read consumes records. Within those verified sources
and management routes there is no price source/catalog-version/price-review service or
`ImportUsage`/`ExportUsage` symbol. This is a bounded comparison, not a full-repository negative claim.
This price workflow extends existing CPAR services rather than claiming a CPA implementation port.

No actual Provider, paid source, real credentials, remote write, push, merge, deployment or ticket
closure is authorized or performed. Test sources are injected synthetic public-format documents.
Pending preview state is ephemeral; applied review evidence is durable schema40. Existing six-rate
catalogs remain complete: missing rates stay null/blocked until explicitly completed by operator.
No historical repricing, complex tiers, optional-rate billing schema or routing-policy activation.

## 本轮源码与验收结果

- 实际受测源码：`5bbe4af62eb2cee3407c17ca0138d1c60c33822e`，tree `99b082602ef29e3c5f95256aba7044d48788c09f`。
- 本轮 Full：**44/44 PASS，exit0**，完成于 `2026-10-25T07:33:30Z`（上海15:33:30）。
- Rust：**1698 passed / 0 failed / 21 ignored**，134个结果集合；ignored不计通过。
- #72 正式 gateway 进程：**517/517 checks**，7 calls / 7 receives，均为本地受控TLS；exit0。
- Cargo.lock SHA-256：`a3950a764ae2dcae4e04a8a32ada596f33ea105dbfda8e9714b3e32d3216cd6d`，与共同基线完全相同。
- 固定进程 binary SHA-256：`ff45c4997ef847dad0dcc19a6c315a033c29ce4e27ca0cb90da25405b4614519`；Debug构建，嵌入上述受测SHA、Rust1.97.1和aarch64-apple-darwin。
- 固定binary本地路径：`output/issue-73/bin/gateway-5bbe4af62eb2`。
- `CARGO_BUILD_JOBS=2`，最终Full `RUST_TEST_THREADS=4`；各测试独立数据库、动态端口和本工作区target/TMPDIR。
- RustSec扫描378依赖，无漏洞；cargo-deny四类策略均通过。15组继承的重复版本warning保持，锁文件没有新增或升级，不添加豁免。
- Full与进程验收前后，受测文件/lock以及固定binary哈希保持；原始工作区干净。最终结果后继仅增加/更新docs，不重新归属旧失败。

[Full回执](issue-73-price-preview-20261025-evidence/full.md)、
[机器汇总](issue-73-price-preview-20261025-evidence/summary.json)、
[源码manifest](issue-73-price-preview-20261025-evidence/source.json)、
[运行后核对](issue-73-price-preview-20261025-evidence/source-after-full.json)、
[114份Rust测试binary哈希](issue-73-price-preview-20261025-evidence/test-binaries.json)、
[进程binary](issue-73-price-preview-20261025-evidence/binary.json)、
[517项脱敏进程回执](issue-73-price-preview-20261025-evidence/process72.json)、
[命令/输入/日志哈希](issue-73-price-preview-20261025-evidence/commands.json)、
[两轴审查](issue-73-price-preview-20261025-evidence/review.json)。

证据层级：本票来源为注入的受控合成文档；新增流程经过真实loopback管理HTTP，不是直接调用handler的伪结果。
#72的517项是正式gateway binary与本地TLS peer的独立实际进程验收，启动和重启都有OS localhost出站围栏，关闭后台Provider任务。
Full的114份Rust测试binary有哈希。保留的gateway binary属于之后显式嵌入SHA的#72进程验收；
Full默认build-metadata的启动/多轮进程步骤按源码与命令记录，未单独保留其gateway文件哈希，不能把两者binary哈希混用。
原始临时凭据、数据库、原始回执及日志保留在ignored output，不进入本报告证据目录。

## 失败和修复记录

1. 早期工作树编译诊断：回执转换参数借用类型、测试依赖未启用reqwest JSON便捷方法；均修复，未加依赖。
   Clippy的局部文档/控制流/测试字面量诊断也已修复；原日志和哈希保留。这些早期工作树没有冻结受测SHA，不冒充最终源码证据。
2. `426c9a9b`的首轮Full由主代理在Rust测试构建期间中断（exit143），原因是独立复核发现第二次读取准入状态码不一致。
   未在运行中修改源码。原`output/issue-73/full.md`、`logs/full-01.log`和中断原因保留为INTERRUPTED。
3. `0f7dc6ac`的第二轮Full在Rust测试失败（exit101）：新增review表已实际创建，但全表清单预期未补齐。
   原记录为FAIL；既有精确逐表断言保留，只增加`billing_catalog_reviews`这一项。存储专项随后110/0/2通过。
4. `5bbe4af6`的第三轮Full从头完整执行44/44；没有删除测试、跳过断言或降低门禁。
   该提交相对独立审查后的生产源码只差一行测试预期，主代理亲自核对；全部最终源码重新运行Full。

本轮没有读取真实凭据、验证真实models.dev可用性、调用真实Provider/收费来源、运行实际客户端/固定Build资格、
推送、合并、远端评论/关闭工单或部署。未知费用不变为0，`cost_in_usd_ticks`仍NOT_ADMITTED；
#61–#67、首版READY和容量/RSS门槛均未由本票解除。

## Acceptance against #73

| Requirement | Local implementation and evidence |
|---|---|
| Fetch does not overwrite prices | Reuses fixed public source and immutable catalog service. Refresh/preview do not insert catalogs or audit writes. Controlled source failure returns502 with catalog/revision unchanged. |
| Source/time/matching/units/diff/unknown | Preview records source SHA-256, observation time, explicit exact source→Provider/Channel/public-Model mapping, USD/unit, source nullable suggestions, operator completion, old effective version, add/change/unchanged and omitted tuples. Unknown models, unknown pricing/tier keys, currency/unit conflicts, unsafe numbers or inexact decimal scaling block application. |
| Confirm exact reviewed content/revision | Random retained handle plus content digest, actor, selected configuration and exact revision. Source is reread; all global catalogs are fingerprinted again inside the immediate insert transaction. Another draft's price import invalidates the preview. Changed digest/config/revision/source, reused catalog identity and duplicate confirmation conflict. |
| Atomic failure and retry | Controlled audit-trigger failure leaves catalog, review, revision and audit unchanged; the same retained confirmation succeeds after the failure is removed while inputs still match. Successful confirmation consumes the handle; repeated confirmation returns409 without another write. |
| Effective time and immutable history | Future/current effective time must remain at or after confirmation observation. Existing exact tuple / Attempt end time / lexical-ID tie break remains. Sparse omission explicitly retains the prior price. New catalog does not change existing ledger, start a materializer or activate a routing-price policy. |
| Traceability and #72 freeze | Real loopback HTTP applies a reviewed v3; all preexisting ledger cells remain identical. Subsequent synthetic Usage materializes against v3 (36 integer microunits), while the previous missing-price cost remains null. Both grains × CSV/JSONL exports captured before application remain byte-identical. Existing #72 HTTP assertions still show v1/v2 lineage after later purge. Applied source and content digests/reviewed mappings survive store reopen. |
| Management permission/audit/rollback | Existing Management Key, Origin/CSRF and draft mutation boundary reused; public/no-CSRF requests denied. Successful application records one billing_catalog_review_applied event in the catalog/revision/review transaction. Existing rollback copies rates into a new operator version, audits it and retains every predecessor. Successful price responses explicitly use no-store. |
| Separate future scope | Historical repricing, complex tier interpretation, nullable-rate catalog/pricing changes, actual source availability, real Provider prices/invoices, routing policy activation and capacity/RSS qualification remain outside this local slice. |

The complete six-rate catalog invariant remains. Missing source rates stay null and block application
until the operator explicitly supplies the missing dimensions in the proposal; no fallback zero is
inserted. Unknown model/tier/unit issues cannot be bypassed with those completions. Omitted entries
are absent from the new proposal but retain their effective older catalog under the existing sparse
lookup rule. Preview displays that effect rather than implying deletion or a zero price.

## Shared changes and integration notes

- `ManagementResourceHttpState`: server-owned PriceSource and finite pending previews; default is the
  existing fixed models.dev reader. Three additive routes: POST price-source/preview, POST
  price-source/apply and GET catalogs/{catalog_version_id}/review. Existing refresh gains observed
  source identity/digest and explicit unknown-model/precision issues.
- Mutation service and Store: reviewed import reuses catalog creation/revision/audit; global content
  comparison and review provenance join the same immediate transaction. Catalog enumeration now uses
  one short read transaction. Existing import and forward rollback API remain available.
- Schema40 / `0040_billing_catalog_reviews`: additive review table and registry. Existing catalog,
  ledger, Usage and quota tables are unchanged. The schema completeness test adds the new table to
  its exact sorted expected set; it still rejects extra/missing tables. Schema38 fixture upgrade to
  current verifies old event/ledger/catalog cells, retained migration39 and idempotent migration.
- The #68 HTTP fixture's schema assertion follows CURRENT_SCHEMA_VERSION, retaining the same assertion.
- Shared documentation: OpenAPI, ADR index, traceability and the affected roadmap row only. ADR-0131,
  BC-MGMT-021 and migration0040 were reserved before implementation. Later read-only checks saw
  parallel #69 ADR-0130 and #75 ADR-0132, and no competing0040 migration; integration must still recheck.
- Pending preview bounds:8 per process,10-minute TTL,512KiB evidence each; source body16MiB, mappings512,
  selected effective tuples/diff rows4096. Catalog capacity remains256. These are admission bounds,
  not measured RSS/performance qualification. Applied evidence persists; pending handles expire on restart.
- No global configuration or original feature workspace was edited; this work remains on its own branch.

## Standards

One initial P2 resource-boundary judgement was fixed: near-quadratic omitted-tuple searches on the
Actix worker became one effective-tuple BTreeMap with early4096 admission. Fingerprint/proposal work
runs under the existing finite blocking-read admission, including cancellation permit ownership.
Independent static fix verification closed it. A focused index test checks time, lexical ties and sparse
omissions. No remaining actionable Standards finding was reported within the changed-code scope.

## Spec

Two P2 findings were fixed and independently closed: missing no-store on successful price responses,
and second-stage read-capacity failures returning503 instead of the price contract's429. Real HTTP
assertions cover no-store; the second-stage mapping was additionally checked statically at its precise
error branch. Source failures remain502, malformed proposal limits400 and local storage failures500.
Review is static evidence; executed test/gate results are recorded separately below. Review did not
expand into a whole inherited-code/security audit. Final schema-test-only follow-up was checked directly
by the primary agent; production Rust is identical to the independently reviewed source.
