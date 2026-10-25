# #69 / #73 / #75 第二批本地集成

日期：2026-10-25（Asia/Shanghai）。**PASS_LOCAL_INTEGRATION**。三票已集成、专项和 Full 通过、代码审查完成；本地提交，不修改远端。

## 基线与受测身份

- 分支：`feature/parallel-69-73-75-integration`。
- worktree：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar-parallel-69-73-75-integration-20261025`。
- 上一批基线：`84fbfbf778b582f746c1cafe31a2ce58962e8f77`，保留为祖先。
- 最终受测代码：`6165a9f142e86e0a4240d4d6ab6d42b820028acf`，tree `1521d54ddaf625825fa7f8474844ec28d1dd2408`。
- Full02 启动于 `6a034382c5f46748a0050cb1464cf16b7f051b99`。至最终受测代码只修改 opt-in `cpar-wave2-integration.py` 的模型发现 oracle；所有 Rust、Cargo、迁移和 Full 使用的脚本相同。该最终 driver 单独完成 198/198。
- 本报告及当前入口是受测代码后的纯文档提交。最终文档后继 SHA 写入项目容器 `maintenance/2026-10-25-wave2-implementation-audit/INTEGRATION-RESULT.md`，作为 #70/#71 的新起点。

锁文件 SHA-256 保持 `a3950a764ae2dcae4e04a8a32ada596f33ea105dbfda8e9714b3e32d3216cd6d`。未升级依赖。
[机器汇总](wave2-integration-20261025-evidence/summary.json)记录 binary、日志 hash 和各层结果。
本地 maintenance 的 source-01/02/03、test-binaries、logs 清单保留 commit/tree、受控输入、工具链和原始失败身份；最终受测前后 7203 个受控文件及固定 binary hash 不变。

## 来源与差异处理

| 工单 | 冻结来源 | 纳入方式 |
|---|---|---|
| #69 | `741c13dc40854d4fdd2be5b67e5fcccc9c76fa3c` | 集成 worktree 起点，原分支保留 |
| #73 | `25f7b6687efa7fd517773f7bee70e9b7e98f59f9` | merge `6a81a254`，完整祖先可达 |
| #75 | `884645482feb6e2820c639042c4c794a024c24bb` | merge `085387da`，完整祖先可达 |

三条来源均未前进。保留 ADR0130/0131/0132、所有独有测试、旧 schema39 回执及历史 RED/FAIL。
文本冲突集中在 ADR 索引、traceability 和 roadmap，保留三票各自内容并更新当前入口。
management state、Alias/count、价格 handlers 和备份上传自动合并后通过本次测试；OpenAPI 来源操作和 schema 并集全部保留，共169 operations /150 schemas，AliasInput.prefix 保留。
迁移0039/0040及 registry 各唯一，最终完整前缀为1–40。历史测试中 migration39 的唯一性断言保留。

主克隆、旧 client-metadata、上一批集成和本批三个来源共六处，前后 HEAD、分支、status、index、diff及受控/非忽略文件内容摘要完全相同。保全清单位于本地 maintenance；没有 reset、clean、stash 或操作原成果分支。

## 本次实际验收

| 检查 | 结果 |
|---|---|
| 项目 Full02 | **44/44 PASS，exit0**；完成 `2026-10-25T08:44:51Z`（上海16:44:51）；[回执](wave2-integration-20261025-evidence/full.md) |
| Rust 全工作区 | **1726 passed /0 failed /23 ignored**，135个结果集合；ignored不计通过 |
| #69 公共 HTTP / count / controls / Explain | 本次 Full 覆盖 JSON/SSE、独立 receiver、名称与权限、工具轮次、发布/回滚；原分支PASS没有相加 |
| #73 价格及 #72 冻结结果 | 实际 loopback 管理 HTTP 覆盖 preview/apply/review 及失败围栏；旧 ledger 逐 cell、details/buckets × CSV/JSONL 字节保持测试通过 |
| #72 stock gateway 进程 | **517/517**，7 calls/7 receives，均本地受控 TLS peer；无真实 Provider |
| #74 stock gateway 退出/重启 | **21/21**，显式 expected-schema40及完整唯一前缀；零 Usage/ledger 伪造 |
| 备份显式专项 | **7/7**，包含默认 ignored 的大于128MiB拒绝测试；保留 v1 小备份、错钥、篡改、版本不兼容及已有目标负例 |
| schema40 联合大库恢复 | **198/198**，实际管理 HTTP 恢复后由新 stock gateway 采用；[逐项结果和表摘要](wave2-integration-20261025-evidence/process.json) |
| --fixture 版本负例 | schema40 binary配expected-schema39明确拒绝，不能输出错误版本PASS |
| 供应链 | cargo-deny四类策略PASS；cargo-audit扫描378依赖无漏洞；原重复版本warning保留，无ignore或门禁放宽 |

构建使用 Rust1.97.1、独立 target/TMPDIR、`CARGO_BUILD_JOBS=2`；Full02 `RUST_TEST_THREADS=4`。
进程使用动态端口、独立 synthetic 数据、关闭后台 Provider tasks，并验证本进程 OS 出站围栏。
固定进程 binary SHA-256：`4a2c32e59695e49b930a011a03c34a19356c82d4619283eafd14533e74cc6bc2`。
binary 路径中的 `6a034382c5f4` 是固定时身份；最终 driver 的后继只改 Python oracle，Rust内容相同。Full后续按默认features另构建的gateway与固定binary分别记录，不混用hash。

### schema39→40、非空review与大备份

由上一批实际 schema39 binary生成原冻结25,000条完整lineage、100,000 events、40,000 quota观察，再由真实 BillingWorker物化。保持30天、80% ordinary /10% failed native Usage /10% native Messages及原quota字段分布，不减少数据。
停服后保存旧业务表逐行摘要；本次程序升级到40，旧业务数据及密文字节保持，review新表初始为空。

随后通过真实socket管理HTTP的生产 preview/apply/review handler应用合成来源，stock gateway再授权读回。不是直接SQL伪造review；source由test-only helper显式注入，不调用models.dev。
大库中review为1条，source_digest/content_digest均非空且保留；catalog、ledger也非空。备份恢复前后的59张业务表投影一致，另有exact并发提交前缀（本次恢复10条，源共100条）。运行中checkpoint按其职责单独核验；额外比较原5条request recording的全部单元保持，恢复后的第6条是合法续接新增。未冒称全部原始表无例外相同。

- v2 artifact：**98,557,485 bytes**，SHA-256 `1344c5e6b75c892e1d24d466379c719840e7f4d4f67e781f0d301177bae20f18`；头明确为schema40。
- 创建：**11.426秒**；operator helper OS high-water RSS **12,091,392 bytes（约11.53MiB）**。
- 正常HTTP restore host OS high-water RSS **37,076,992 bytes（约35.36MiB）**；恢复后采用进程 **70,909,952 bytes（约67.63MiB）**。
- 另测并发提交、RLIMIT_FSIZE写失败、上传取消容量、SIGKILL恢复中断、空目标显式重试，原数据及未发布staging保留。

v2创建仍为store文件API，由operator helper调用。管理HTTP负责上传preflight/restore。没有新增生产CLI或下载接口；旧Vec创建入口仍为v1/原上限。

### 名称、计价身份与加密续接

管理HTTP保存并发布新模型、显式team prefix及原授权grant。请求 `team/public-wave2(high)` 使用解析后的 `public-wave2` 计价；上游收到 `gpt-5.5` 和high，覆盖合法body low。价格目录同时配置别名/上游名的异价decoy，仍选择正确public tuple而非误价/unpriced。
源库与恢复库额外SQL核验：`public-wave2 / wave2-reviewed /81 /exact`，`reasoning_tokens`仍NULL。测试显式设置等价output/reasoning费率，因此无需猜测未报告的reasoning拆分；81是合成目录的本地账本计算，不是真实Provider费用。

`store:true` 请求保存非空 `model_thinking=Effort(high)`；helper使用原Master Key认证解密，精确owner可读，foreign owner不可读。恢复前后公共GET、prefix配置、restricted模型范围及跨Key续接拒绝保持；读/拒绝不发上游。新gateway合法续接只发送一次，包含原question、answer、followup并采用当前low控制。原high保存在旧payload中，未伪称所有控制自动继承。
旧严格reader有deny_unknown_fields：回退schema40→39或保留v1容器，并不保证旧binary能读含Some(model_thinking)的新payload。应保留原binary、原格式artifact和全部Master Key版本，不下迁/丢弃历史来制造可读性。

## Standards

独立规范轴对本次合并区间审查，未发现需修复的实质违规或smell。重点检查模块职责、v2 chunk/AAD/EOF、取消后的upload admission及价格全局fingerprint/审计事务。静态范围与限制见[审查记录](wave2-integration-20261025-evidence/review.json)。

## Spec

首轮1项P2：`--fixture`跳过prepare导致expected-schema不受核对。`58bb1c2b`修复为每次current启动检查完整前缀及schema40表；动态负例通过，独立后继复核无新增实质问题。两轴分别记录，未把静态审查当动态PASS。

## 原失败与保留边界

build-01曾因test helper错误使用ThinkingEffort构造而失败，`cea0dd61`修复。Full01在fixture费率修正前由主代理终止（exit143），保持INTERRUPTED。
process-01原误预期81，实际因reasoning费率0且计数未知而正确产生partial60；`6a034382`只为新合成场景显式设置同价费率，原容量模板未改。
process-02原误预期discovery返回public名称； owning HTTP实际枚举精确上游名与prefix Alias，`6165a9f1`修正oracle并保留public计费断言。原失败、数据库、密钥、日志及原始回执保留在ignored `output/wave2/`，没有改成PASS。

此次结论限本机macOS调试构建、固定合成分布、受控来源及上游。128MiB为实现上限，未资格通过所有该尺度数据；选择性300条Usage导出、全窗口明确413、quota首屏200条，不是40,000条全量分页验收。文件大小限制/故障注入不是填满物理磁盘，SIGKILL不是断电。
备份不包含外部Master Key目录或独立admin-auth存储；本次私有材料另行保全。真实models.dev可用性、真实Provider/费用、未修改Codex/Claude/Pi、固定Build服务、远端CI、release/OCI/Linux及部署均NOT_RUN。cost_in_usd_ticks仍NOT_ADMITTED，#61–#67和首版READY门槛未解除。

没有push、远端评论/关闭、合入develop/main、部署或真实Provider调用。下一步仅给出从最终文档基线启动#70/#71的独立分支信息，没有开始新票。
