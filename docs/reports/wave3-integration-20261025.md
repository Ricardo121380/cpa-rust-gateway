# #70/#71 共同集成、数据库安全与优化票验收

日期：2026-10-25（Asia/Shanghai）。状态：**PASS_LOCAL_INTEGRATION**。
八张优化票已进入同一受测后端。完整门禁、数据库负例、默认构建实际进程和双轴审查通过。
GitHub 同步结果在本报告末尾单独记录，不由本地 PASS 推定。

## 固定身份

| 对象 | 身份 |
|---|---|
| 共同起点 | `7c16070bb66f7f3008022f7870b5d1f96b63f167`；包含前两批六票和安全维护 |
| #70 原分支末端 | `24253e0e1c63fa6a286f78160d944da1b8f23cfa`；原受测 `731c318320ae6f9df69cc77a708ea67c468a2695` |
| #71 原分支末端 | `14f9a9e3c15a03765136fc5cecfaa3b28baa3b1b`；原受测 `2dc8c87c9c8f7c78db4c368d62932be5b4573e82` |
| 本轮受测代码与驱动 | `e06ac169fb8217b00530d9f4e70cfefdd594a513` |
| 受测 tree | `adb66414ca866f915fcd0a398952d290dc9914fd` |
| 工作分支 | `feature/parallel-70-71-integration` |
| 工作区 | `worktrees/cpar-parallel-70-71-integration-20261025` |
| Cargo.lock SHA-256 | `3b0c82a72bea2fba0b9de42ff4fa6083d08b42fb2a7ba1a33d007807079de445` |
| 默认构建 gateway SHA-256 | `5a41b79488c39e23c84eb163c43d8084eef9c6db5546897090fe444466b3a3af` |
| 固定 CPA | v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683` |

代码提交后才运行最终 Full。所有7241个受测文件的内容/模式已冻结；联合验收后逐项核对零漂移。
默认 gateway 通过 `cargo build --locked -p gateway --bin gateway` 构建后复制冻结；并非 Rust 测试可执行文件。
Full 的 workspace all-features 可执行文件也跑过225项，但最终进程证据使用上表的默认构建。
helper测试程序只承担已有 operator backup、受控价格来源和私有解密核验；实际管理、公开请求和恢复采用由 gateway 进程承担。
后继报告提交只更新文档和脱敏证据，不将文档提交冒称为重跑过的代码候选。

## 集成与安全修复

- 以 #70 为起点合入 #71，保留两个父链、独有回归、ADR0133/0134、请求规则与路由/亲和接线。
  11个重叠路径经逐项核对；OpenAPI同时保留 Route策略、Explain的selection_policy及Endpoint request_policy。
- 保留原迁移1–40，路由/亲和为0041，请求规则顺延为0042，当前schema42。外部依赖版本未改变，
  Cargo.lock只纳入 #71 已有的 gateway→protocol-anthropic workspace边。
- `gateway-store`统一检查已知40/41/42的结构身份。共同40与 #70 standalone41允许升级；
  #71 standalone41、两种结构混合及已覆盖的缺列形态拒绝，不能只凭版本数字通过。
  这是一组针对本次碰撞的身份检查，不是对任意数据库损坏的完整证明。
- backup预检对不兼容历史返回compatible=false，两个restore入口拒绝且不发布目标。
  不自动重写 #71 独立数据库的版本，不降回40丢弃其中规则；原binary/artifact/keys继续保留。
- 非空或未知请求策略不能降级删除；`{}`及空rules/headers仍允许。整次downgrade在一个SQLite事务中完成，
  后续guard失败会撤回前面的列删除与迁移记录修改。成功执行的更早历史down迁移仍保留其原有破坏性语义，
  不将schema downgrade当常规配置回滚或无损恢复。

后继决定：[ADR-0135](../adr/ADR-0135-integrated-schema-identity-and-atomic-downgrade.md)。
原决策正文和独立分支结果保留，当前支持说明已指向新迁移链。

## 本轮动态验证

| 验证 | 结果与边界 |
|---|---|
| Full | **44/44 PASS**；137组Rust结果合计 **1751 passed /0 failed /23 ignored**；fmt、Clippy、serve、agent回归、文档/契约、秘密扫描、deny与audit全部通过 |
| 数据库专项 | **6/6 PASS**：真实历史40、路由41升级；错误规则41/混合/缺loss列拒绝；两种备份读入口拒绝；非空策略保护；跨步回退原子性 |
| 显式备份专项 | **7/7 PASS**，含默认忽略的容量测试；错key、篡改、chunk顺序/长度、旧格式、已有目标和规模边界 |
| 默认gateway联合进程 | **225/225 PASS**，schema39→42、非空配置/规则/亲和/价格/历史、管理恢复、新进程实际采用、合法续接 |
| Usage查询与导出进程 | **517/517 PASS**，同一默认gateway；查询/冻结批次/CSV/JSONL/权限/取消/限制与独立计算 |
| 额度历史进程 | **21/21 PASS**，schema42；实际退出/新进程读回，6次额度读取；查询与重启不新增上游、不造Usage/ledger |
| 附加保全 | 原13条gateway_request_recording逐单元保持；恢复库quick_check=ok、FK无错误、前缀完整1..42 |

Full运行环境为独立 `output/wave3/target` / `output/wave3/tmp`、jobs2、test_threads4。
实际进程使用独立临时目录、生成的合成凭据、动态loopback端口和出站OS限制；后台Provider任务禁用。
原始日志、固定二进制、完整输入清单保留在本工作区 `output/wave3`；摘要含其hash，不发布私钥或原始业务材料。

## 联合场景实际证明了什么

同一配置含普通compatible Endpoint规则和独立Build Endpoint。公开Build重复请求真实建立并复用同账号绑定；
管理fork/publish后重新选取当前合格账号、改变fingerprint并记录NULL-loss reselection。独立receiver核验native认证，
普通Endpoint的header没有泄入Build。随后在同一运行代发普通请求，核验default/override/filter及解密header实际发送。
这些记录由生产请求产生，不再通过直接INSERT制造亲和成功。

多候选RR/WRR完整周期、fill-first、租约饱和、失败排除、跨Key与严格stored/compact/WS约束仍由同源Full中的
#70公共socket专项覆盖；三个协议规则/回滚、并发revision、原生count追加headers和估算零发送由 #71/#68 专项覆盖。
新增stock场景只有一个合格Build账号/候选；不能据此声称stock多账号份额或多真实代理出口已验。

容量沿用30天、25000条完整lineage /100000 events /40000 quota的冻结分布。
旧库升级按所有原列投影比较原始值与密文；相同schema42的恢复继续比较完整业务表和新增列。
非空price review的source/content digest、原目录/旧ledger、冻结导出、encrypted model_thinking和新header密文均保留。
新进程既读回配置和受保护历史，也执行带原对话历史、当前thinking及规则/header的合法续接；跨Key请求拒绝且零发送。

本次v2 artifact为 **98,565,677 bytes**（约94MiB）；创建10.373秒，helper OS high-water RSS为11,452,416 bytes。
受保护HTTP预检10.361秒、恢复10.293秒。这些只是本机受控测量，没有生产SLA或所有128MiB数据分布资格。
并发快照、恢复SIGKILL中断、有限文件写入失败和受控ENOSPC沿用既有检查；原库和无覆盖目标语义保持。

## 八票验收与后续边界

[75项逐条映射](wave3-integration-20261025-evidence/acceptance-map.json)保留GitHub原AC文字、原受测源码和报告；
不把旧Full失败改为通过。下表链接各自独立证据，本轮共同结果见上表。

| 工单 | 原独立交付 | 当前边界 |
|---|---|---|
| #68 | [计数](issue-68-count-tokens-20261025.md) | 上游报告/精确本地/估算分离；实际客户端计数仍在#64或渠道票复验 |
| #69 | [控制与命名](issue-69-controls-model-names-20261025.md) | 冻结模型与CPA范围；不是全模型逐字节等价 |
| #70 | [策略与亲和](issue-70-routing-build-affinity-20261025.md) | Build普通亲和；严格lineage保持；不推出全Provider会话框架 |
| #71 | [规则与headers](issue-71-request-rules-upstream-headers-20261025.md) | 三类普通compatible adapter；专用Grok/Kiro非空策略仍拒绝 |
| #72 | [统计与导出](issue-72-usage-query-export-20261025.md) | 有限冻结快照；不替代加密备份、不重算历史费用 |
| #73 | [价格版本](issue-73-price-preview-20261025.md) | 合成公开格式来源；真实来源可用性/价格准确性未据此获证 |
| #74 | [额度历史](issue-74-quota-history-20261025.md) | 已接观察源；Kimi为projection证据，非真实Kimi请求 |
| #75 | [容量恢复](issue-75-history-backup-capacity-20261025.md) | 本次非空schema42恢复已采用；不是物理断电/填盘或全规模资格 |

真实Provider、当前实际Codex/Pi/Claude、固定Build资格、远端CI、签名release和部署本轮均NOT_RUN。
cost_in_usd_ticks仍NOT_ADMITTED；额度、Token Usage、费用保持不同口径。
#61–#67和父项义务继续存在；优化票完成不表示首版READY，不解除#64→#66依赖或原始Claude后续义务。

## Standards

初轮与修订后独立规范审查均0项新增违规、0项可操作启发式发现。
检查范围为共同集成增量及重叠接线，原独立分支审查按原身份保留。

## Spec

初轮2项P2已修复：缺loss列的schema40曾能通过兼容判断；缺少同一运行代规则与Build亲和联合证据。
分别新增真实RED→GREEN回归和stock公开请求/发布/恢复场景。第二轮独立审查0项遗留。

初始数据库专项1 PASS/4 FAIL、缺loss列新增RED、Full01主动中断143、Full02的Clippy失败101全部保留。
Full03是本轮完整成功执行，不把补跑片段拼成一次Full。
规范轴0项遗留；规格轴0项遗留。

## 保全与远端同步

[保全核验](wave3-integration-20261025-evidence/preservation.json)：五个来源工作区的HEAD、分支、原始index字节、
dirty状态、文件bytes/modes/uid/gid及既有缺失路径均未变化。未重置、清理、stash或整体暂存原工作区。
集成提交保留两个父链；develop/main尚未改变。历史检查覆盖1299个新增可达blob（63,823,378 bytes），
现有秘密规则未发现匹配；该检查不等于通用隐私或安全证明。

**GitHub同步：待此报告与当前入口提交后，发布已审查的单一feature分支并按75项AC逐票同步；
尚不在此文档提交中宣称远端关闭。** 后续独立读回会记录正文、状态和原生关系。

机器摘要：[summary.json](wave3-integration-20261025-evidence/summary.json)；
[Full](wave3-integration-20261025-evidence/full.md)；
[默认进程225项](wave3-integration-20261025-evidence/process.json)；
[双轴审查](wave3-integration-20261025-evidence/review.json)。
