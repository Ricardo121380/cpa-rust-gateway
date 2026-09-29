# CPAR A 批次本地交付与验收 — 2026-09-30

## 范围与结论

- 来源：[实施规格 #9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)，A 批次 CPAR-01–17 / #10–26。
- Checkout：`codex/prism-v4-delivery`；固定起点 `9fc4ceb4e12aab9c60658869706828a7d5d8312f`。
- 用户明确授权：**授权 Codex 完成本次 A 的前后端**。本批次包含 `web/prism/**`，跨边界记录见 [cross-boundary-log](../cross-boundary-log.md)。
- 本地实现、定向 Rust/TypeScript 检查和 EgoLite 合成环境验收已执行。最终完整门禁：**PASS，44 项全部通过**。
- 真实 Provider 授权／续期／计费调用、真实账号写入、生产发布和远端 CI：**NOT_RUN**，本次授权不包含这些操作。

## 工单映射

以下 PASS 表示受影响实现的本地验收；不表示所有真实渠道或生产验收通过。

| 工单 | 实现 | 本地行为证据 | 状态 |
|---|---|---|---|
| CPAR-01 / #10 | 准备、保存、绑定、应用分开反馈；丢响应先回读 | configurationTask 测试；Ego `import-all-failed`、`import-connection-failed`、`import-publication-unknown`、`import-response-lost` | PASS |
| CPAR-02 / #11 | 全分页待应用差异与恢复账号 review；revision/active/lifecycle CAS | configurationLifecycle/Task 测试；Ego `merge-review`、`merge-conflict` | PASS |
| CPAR-03 / #12 | AI 提供商、账号、OAuth 独立导航；API 条目和账号分页计数过滤 | protected management inventory 测试；Ego API/OAuth 流程 | PASS |
| CPAR-04 / #13 | API Key 单表单；零匹配新建、唯一匹配复用、多匹配显式选定；保留停用 | apiProviderModel 测试；Ego `api-zero`、`api-one`、`api-multiple` | PASS |
| CPAR-05 / #14 | Codex/Claude 登记、绑定与应用；未知结果读取消费回执 | authorization_receipts HTTP；Ego `oauth-codex`、`oauth-claude`、`oauth-response-lost`，回调一次 | PASS |
| CPAR-06 / #15 | Kimi/Kiro 设备授权的未知结果与安全恢复；不重放消费动作 | protected device status HTTP；Ego `device-kimi-unknown`、`device-kiro-unknown` | PASS |
| CPAR-07 / #16 | Grok 原生保存与运行应用独立结果；显式继续应用 | native management HTTP；Ego `native-build-apply-failed` | PASS |
| CPAR-08 / #17 | 逐文件 mixed results、只重试未执行/拒绝项；同身份新授权更新原项并保留设置；精确批次回执 | native/ordinary duplicate import HTTP、atomic identity/store migration tests；Ego `native-mixed-batch` | PASS |
| CPAR-09 / #18 | 目录来源、时间、空成功、unsupported、失败保留成功、陈旧与分页部分加载；映射冲突恢复 | runtime/catalog/durable tests；真实 loopback HTTP 目录；Ego `catalog-matrix`、`catalog-mapping-conflict` | PASS |
| CPAR-10 / #19 | 默认关闭；所选 Key 的独立模型权限，保留其他模型和 sibling Key | 真实 gateway `/v1/models`、数据面与 Pin 拒绝/单次正例；Ego `pin-default-closed` | PASS |
| CPAR-11 / #20 | 主状态与授权/运行/额度/历史/调用事实分离；unknown 余额；精确成功范围与时间 | accountStatus/callEvidence 测试；Ego 成功回执关闭/重开与协议变化 | PASS |
| CPAR-12 / #21 | 续期分类、Disabled intent、late CAS；成功更新 runtime 与 restart-readable state | 普通四渠道 × 五种本地 HTTP refresh；Build 五种响应、停用 sibling 不 exchange | PASS |
| CPAR-13 / #22 | generation 在首次 dispatch、重试和内部重放前检查；在途 stream 自然完成 | native held transport 回归；真实 gateway A 在途、B 新请求、旧请求 503 不 fallback | PASS |
| CPAR-14 / #23 | 删除影响 digest、共享 endpoint/route/Key/历史保留 | credential_deletion/store HTTP；Ego `delete-shared-account` | PASS |
| CPAR-15 / #24 | 旧配置复活已删除账号前单独完整 review/token 与 durable audit | account_restoration/lifecycle tests；Ego 删除后 rollback review、确认和恢复快照 | PASS |
| CPAR-16 / #25 | 精确解除本地隔离；尚未验证；拒绝 Disabled/auth/quota/错目标，权限独立；请求与完成审计 | 真实 HTTP release 零 inference 与正反例；Ego `local-release` 确认、状态与拒绝反馈 | PASS |
| CPAR-17 / #26 | 显式 Key/目标/成本确认、严格一次 Pin；值无关会话回执；revision/config/build/instance 失效 | protected Pin HTTP；callEvidence 测试；Ego `pin-explicit-success` | PASS |

## 本地证据层

### 受控真实 gateway HTTP

运行入口：[cpar-batch-a-http.py](../../scripts/acceptance/cpar-batch-a-http.py)。本机临时 SQLite、随机合成管理/客户端/Provider 凭据及 loopback TLS Provider，启动本次独占的 `target/debug/gateway`，`finally` 结束进程。没有真实 Provider 或生产地址。值无关结果：[HTTP receipt](assets/cpar-batch-a-20260930/batch-a-http.json)、[Pin receipt](assets/cpar-batch-a-20260930/batch-a-pin.json)。

- 目录 metadata 请求与 inference 分别计数。真实成功→空目录→失败，零 current models 与隔离期旧行分离，失败保留最后成功；无目录实现的连接显示 unsupported。
- 新模型保持关闭；明确为一个 Key 开放后，另一 Key 与原有模型权限保留。公开 `/v1/models` 与数据面准入一致。
- A 的 semantic delta 已被客户端收到后停用 A 并发布；B 承接新请求，A 自然 `response.completed`。旧执行收到受控 503 后不为 B 分配 fallback。
- Pin 缺少 Key 或 Key 未授权均在 Provider 前拒绝。明确 Pin 成功恰好一次、一个 exact account/connection/model/protocol/build/instance。
- 本地 release 与拒绝不增加 inference 数；不绕过停用、明确授权失效、未到期 Retry-After 额度或 sibling Key 权限。resource audit 将请求、完成和观察分开。
- 受控 503 的 Endpoint 五秒冷却独立于 account-local release；测试让其自然到期后再建立额度/授权负例，没有重置未来额度窗口。

### Rust 高层本地验证

普通 OAuth refresh 使用真实四个 adapter 与 loopback HTTP 的 success、明确失效、HTML 403、429、5xx；覆盖 durable store、management readback、runtime pool/Health 和 restart 路径。Build 使用真实 native refresh/parser/worker、受控 HTTP transport 与相同持久化/装配边界。原生在途测试使用 held synthetic transport 与生产 Build lease/decoder。它们不是对真实服务或真实账号的验收。

其他关键测试包括：

- `native_reimport_updates_same_identity_and_reads_exact_committed_batch`
- `ordinary_duplicate_import_readback_resolves_actual_existing_account`
- `managed_import_observation_is_atomic_and_unchanged_import_preserves_newer_identity`
- `native_import_receipts_upgrade_and_rollback_preserve_disabled_accounts`
- `management_catalog_status_projects_success_and_failure_only_targets`
- `ingress_pinned_catalog_never_selects_from_a_later_publication`
- `kiro_partial_or_repeated_pages_never_return_a_successful_partial_catalog`

Provider discovery 不完整时按失败处理并保留最后成功。浏览器已读一页而后续页失败时明确显示加载不完整，不记录成成功的部分目录。

### EgoLite 产品验收

运行入口：[cpar-batch-a.mjs](../../scripts/acceptance/cpar-batch-a.mjs)，EgoLite TaskSpace 27 / p1，`127.0.0.1:5183` 的合成 Prism fixture。共 30 个 distinct scenarios；结果：[browser summary](assets/cpar-batch-a-20260930/browser.json)。最后核验 Pin 关闭/重开、解除隔离、目录状态/分页、映射冲突恢复、原生混合批次及普通丢响应导入，并补验四渠道绑定失败恢复、API 去重丢响应回读、原生连接零/一/多匹配及重建失败影响提示。浏览器后端是 fixture，不能据此宣称真实 Provider/生产通过。

## Review 与修复收敛

固定点审查分 Standards / Spec 两条只读探子。Standards 返回 complete；Spec 分别核验 CPAR-01–07 与 CPAR-08–17 的关键流程，两轮均明确为 partial，未逐行覆盖全部 manifest。后续只读核验检查修复后的精确 seams。根代理负责实现、判断、测试与最终 diff。没有将 partial review 描述为全仓无缺陷。

| 问题 | 修复与核验 |
|---|---|
| 生产 Pin receipt 新增 expect | 改为可分类的 serialization error；source policy gate |
| Build Pin 总 timeout 误报未发送 | 与正常完成路径一致，以 transport stage 补偿 sent 归因；静态独立复核及 Pin tests。不是证明字节确已抵达 Provider，未单独执行 45 秒 timeout 场景 |
| 删除 ADR 与兼容 HTTP optional review header 矛盾 | 明确 Prism 必须影响预览；既有直接 API 保留 version/revision 和 supplied-header stale 拒绝 |
| 原生同身份不能更新／重复未知无法核对 | managed revision update 保留 disabled/settings/creation lineage；schema 0032 receipt 与 import 原子提交；HTTP 与 mixed-batch Ego |
| 成功证据关闭丢失／重新授权使用缓存成功 | bounded current-session receipts；revision 进入 query key；refresh invalidates evidence；读取期间不宣称当前有效；unit、TypeScript、Ego scope/history |
| 身份 lookup 与提交后 snapshot 存在时序漏洞 | source identity 与 material 同事务绑定；unchanged 不覆盖既有较新 identity；atomic store regression |
| 空成功被 retained rows 掩盖 | current model count 只取 present-in-last-success，保留旧行但明确提示；service regression、真实 HTTP、Ego |
| 授权恢复可以跳过绑定并误报已连接 | 核对归属并补齐绑定后保持草稿，再核对新增修改后应用；5 个 unit 与 Codex/Claude/Kimi/Kiro 四个 Ego 负例，不再次消费授权 |
| 重新授权缺少身份依据仍覆盖原账号 | 需要可比较的 account ID 或邮箱；缺失/冲突返回显式 409，原材料、Disabled、绑定与 revision 保留；RED→GREEN 与受保护 Kiro HTTP |
| API 去重导入丢响应不能恢复实际 ID | 精确 actor/config/provider/import-ID/starting-revision receipt 解析已保存账号；Ego 一次导入并恢复原 ID，未新增第二个账号 |
| 原生接口配置绕过已有连接匹配 | 原生入口也完整读取连接，零/一/多匹配分别创建/复用/显式选择；三种 Ego 场景，保留原停用及连接设置 |
| 重建失败影响提示不足／回读按钮被页脚遮挡 | 显示新请求全局阻断；提供商及核对流程页脚不遮挡操作；Ego 重建恢复与 API 回读 |
| 内容相同目录重发布使已准入请求失败 | RCU 保留相同 RouteSnapshot Arc/游标；实际内容变化仍拒绝旧 pin；RED→GREEN 回归及真实新请求 |

## 数据与兼容边界

决策：[ADR-0099](../adr/ADR-0099-account-workflow-evidence-and-retirement.md)。schema 从 31 到 32；迁移增加 native batch receipt 与 updated count，不猜测历史批次归属。32→31→32 回归保留既有 disabled accounts、encrypted material 与设置；降级会失去新 receipt metadata，应按现有受支持迁移/备份过程处理。

Kimi/Kiro 等设备授权若不能提供可比较的身份依据，重新授权会拒绝覆盖原账号；首授和使用原 refresh token 的续期不因此改写连接。普通导入/OAuth receipt 是 bounded process-local，进程重启或过期后保持未确认，不能自动重放。调用历史只在当前管理员会话保留最新最多 100 个账号回执；退出/浏览器刷新后未保留的历史为未观测。Console/Web 若严格一次调用无法证明，则显式 unsupported，零请求。首次入库批次标记不因再授权而重写。OpenAPI 从权威 docs 单向同步至 Prism contract/client。

## 最终门禁

| 检查层 | 结果 | 范围 |
|---|---|---|
| `bash scripts/check.sh full` | PASS，44/44 项 | 前端双构建、Rust fmt/strict Clippy、全工作区 all-features、serve/agent 回归、source/crate/contract/docs/secret/依赖/RustSec |
| Rust 全工作区 | PASS，1405 passed，0 failed，12 ignored | 按 Cargo 输出汇总单元、集成和 doc-test；忽略项未执行，不计为通过 |
| Prism 完整 unit suite | PASS，64 files / 459 tests | 包含授权恢复 5 个回归、call evidence、状态与配置任务 |
| EgoLite | PASS，30 个 distinct scenarios | TaskSpace 27 / p1；本地 synthetic fixture，含四渠道绑定失败恢复与原生匹配 |
| Controlled gateway HTTP | PASS | 最终本地构建、独占临时 SQLite、loopback TLS Provider；显式 Pin 恰好一次，release 零 inference |

完整门禁收敛时，更新了旧测试对退役执行的预期（503），为原生合成 HTTP peer 显式设置 blocking read mode，及更新身份冲突的明确 409 断言。网关核心全套 163 passed / 1 ignored 与最终完整 gate 均通过。没有通过忽略失败测试或降低功能断言来收敛。

[完整检查逐步回执](assets/cpar-batch-a-20260930/full-check.md)；[frontend unit summary](assets/cpar-batch-a-20260930/frontend-tests.json)。用户既有 `AGENTS.md` 改动及其他未跟踪资产不属于本次提交。
