# CPAR #62 C6 — 非 assistant 输入消息 id 原生保真与续轮闭环

日期：2026-10-21（Asia/Shanghai）。**C6 本地受控切片 PASS_BOUNDED。**
完成 decode → Canonical → Router → 实际 Build body → JSON/SSE 完成 → 显式历史／既有存储续轮。
不可变代码候选：`8b192fd99a6d33e0517bcf12eb3f8a8ffec8f343`。

| 证据层 | 本轮结果 | 适用边界 |
| --- | --- | --- |
| C6 本地实现与受控验收 | PASS_BOUNDED | 三角色原值、历史、工具续轮、隔离与准确拒绝 |
| 必需 Full | 45/45 PASS，exit=0 | 同一候选＋明确列出的继承文档覆盖层；1621 passed / 0 failed / 17 ignored |
| Standards | PASS_STATIC，0 actionable | 两轴独立审查中的 Standards；最终 Full 另列 |
| 本轮 C6 Spec | PASS_BOUNDED；独立静态审查 0 actionable | 不扩大为整票或实际服务资格 |
| 整票 #62 | OPEN / PARTIAL / BLOCKED | 实际 Codex、固定服务与并行控制资格、#61 等仍独立 |
| 原样 Codex 0.161.0 | BLOCKED；严格成功 runner FAIL / exit=1 | HTTP 400、独立 receive/Attempt/Usage/ledger 全 0；六项成功判据全 false |
| 实际原版 Pi 0.84.3 | PASS_BOUNDED_CONTROLLED | 普通工具、显式历史、失败、同 PID 取消；既有用量展示和 companion 限制保留 |
| 固定 Build 生产激活 | BUILD_ACTIVATION_BLOCKED | C6 无 serving 资格，不新增生产能力 |
| 真实 Provider / 远端 CI / 发布部署 | NOT_RUN / NOT_RUN / NOT_RUN | 未发生对应操作，不授予发布资格 |

## 1. 实际基线与原有内容保留

实际工作树为 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`，
分支 `feature/62-client-metadata`，起始 HEAD 与给定交付一致：
`4a6a83d1be5617329932b7fba691c2bfbd72a7a3`。旧候选 `72464e7d...` 仅定位 C5 证据，
没有回退。没有从旧的 repo/cpa-rust-gateway 开工，没有 reset、clean 或 stash。

先读取实际 AGENTS、CONTEXT、issue-tracker/domain、cross-boundary 尾部、字段方案、C5 报告、
BC-PROTOCOL-009/011、ADR-0115/0116/0117 及其必要接缝。18 项既有 dirty/untracked 文件先备份
原字节、mode 和 symlink；起始 index 为空。三个共改文档只追加 C6 后继，索引按 HEAD＋本轮后缀
构建，没有把继承内容纳入提交。其他原文件逐项保持。历史 C5 失败和本轮中间失败全部保留。

GitHub 只读快照：#62、#61 均 OPEN；#62 原生 blocked_by 仍为 #61；#61 的 #59/#60 依赖均 closed。
本分支没有 PR 或远端 branch。remote develop/main 为 `c39bc07b...` / `c6a981b0...`。
没有认领、评论、改写或关闭工单，没有改变依赖、remote ref 或生产配置。

保留核验见 [preservation.json](assets/cpar62-c6-20261021/preservation.json)。
本轮契约：[BC-PROTOCOL-012](../contracts/BC-PROTOCOL-012-native-input-message-id.md)；
Accepted 后继：[ADR-0118](../adr/ADR-0118-native-input-message-id.md)。二者在行为实施前冻结。
既有 Accepted ADR 未被重写；BC-PROTOCOL-009、ADR 索引和 traceability 仅追加本轮后继。

## 2. 公开来源冻结与资格边界

10 份完整一手源码／metadata／schema 保存在本地证据根和受控归档。
[source-manifest.json](assets/cpar62-c6-20261021/source-manifest.json) 记录 URL、revision、已公布
source date、抓取日期、字节数及 SHA-256。官方文档未公布的 revision/date 为 null，未编造版本。

Codex rust-v0.161.0 annotated tag `7e21416b38834816c224ea0dfd135c3de94b2f15` 指向
`979011409de0a60b52f179721948e65531d26144`；commit 日期 2026-10-06T22:34:37Z，tag 晚一秒。
[models.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/protocol/src/models.rs#L1021)
的 Message.id 为 Option<ResponseItemId>，存在时序列化；
[common.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/codex-api/src/common.rs#L288)
把 input 定为 Vec<ResponseItem>；
[responses.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/codex-api/src/endpoint/responses.rs#L83)
直接编码该 request 并 POST /responses。

| 冻结材料 | SHA-256 |
| --- | --- |
| Codex models.rs | `59c29630f8351fdfdab51fc48e4b468e9749bbff4a043984c57470b49da7fe40` |
| Codex common.rs | `052151729693439be83e0af587078d67275794b6db860bd6541c3470ab767244` |
| Codex responses.rs | `4f2fff3da4ca3ca9fcfa40ada8e98a0229af296094e03febb4d823f32ef932c8` |
| 当前 create schema markdown | `9c4d505259ac72b879a3697f4f606d4131989ae686d204078b7b22e5638794d0` |
| input_items markdown | `1f58a68b72193fdfdf1053236a0117af5d613d5ec51cc2e20a90d0a4fa23281e` |
| Build conversation/responses.rs | `0e7325bcdaf5fe140d1d4d1d8642c38138f37e8bee2f9ed8edd840d46315d483` |

本次 [create schema](https://developers.openai.com/api/reference/resources/responses/methods/create)
为 507870 bytes，hash 与旧快照不同；以本次完整内容为准。其非 assistant message 分支仍未列 id。
[GET input_items](https://developers.openai.com/api/reference/resources/responses/subresources/input_items/methods/list)
返回 id 不证明调用方可在 POST 自定 id。

[固定公开 Build 树](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation/responses.rs#L194)
revision `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`，commit 日期 2026-09-29T16:57:09Z，
非 assistant EasyInputMessage 不发送此 id。客户端代码不证明服务端接受／冲突／续轮语义，
也没有建立到固定 proxy/profile 1.0.50 的 source→release→service 关系。
C6 因此采用明确的本地兼容契约，不宣称已获固定服务资格或通用 POST 标准字段资格。

## 3. 冻结的有界契约与最小实现

| 项目 | C6 行为 |
| --- | --- |
| 角色 | 新准入仅 user/developer/system 的 id；既有 assistant 规则保留 |
| 类型／边界 | 存在时为 1–512 字节 ASCII graphic（0x21–0x7e）字符串；不 trim、改写、截断、哈希、拼前缀或替换 |
| Presence | absent 原样缺失；null、空字符串、错型、超界、空白／控制／非 ASCII 拒绝；JSON 转义拼写可变化，解码后的字符串值相同 |
| 闭合形状 | duplicate JSON names（含相同值及转义同名）、未知同级／nested 扩展、非 assistant status/phase 拒绝；不新增 content 能力 |
| 消息内容 | 保留 role、位置、各 content part 顺序和原文；空 text 允许，空 content array 拒绝 |
| 单次有效历史 | non-assistant/assistant/function/reasoning/hosted 原生 item id 全局唯一，等值重复也拒绝；call_id 是独立关联空间 |
| 跨请求 | 独立自包含请求可合法重放同一 id；不引入全局注册、去重或幂等服务 |
| 显式／存储续轮 | 原输入→上轮完整原生 output→本轮 input 的顺序保留；本轮重复此前 id 预拒绝，不合并／覆盖／去重 |
| 权限 | id 不产生 owner、route、授权、幂等或 item_reference 查询权 |
| 接收方 | 原生字段送达受控 Build receiver；没有借用 C4 本地诊断消费删除它 |

已有 decode_message_item 保存在 CanonicalMessage.extensions；新纯
[input_message_ids.rs](../../crates/protocol-openai-responses/src/input_message_ids.rs) 只拥有闭合形状、
资格需求检测及既有 ID 格式校验，Router 的 assistant/function ID 检查复用相同谓词。
Build 复用 flush_message_parts/insert_extensions 原样编码，没有独立身份框架。

生产 candidate projection 检查实际 GrokBuildResponses、Responses source/target、HTTP 和
Canonical/CanonicalBridge。非测试构建固定无 C6 资格；gateway 私有 cfg(test) 装配＋自有 loopback
transport＋既有 controlled-test-support 构造器才给予局部资格。Build builder 默认独立拒绝。
请求、header、User-Agent、普通配置或任意 URL 都不提供资格；C6 与 namespace/search/ParallelTools
相互独立，不顺手放行 status、phase 或其他扩展。

既有 stored_response_continuity clone 上轮输入。C6 在完整历史进入 exact pinned candidate 后
提前复用 project_candidate，避免重复 ID 被 continuation scheduler 误归为无 credential 的 503；
现在是发送前准确 400。真实 owner、route、credential revision 和不可重试规则仍由原边界维护。
没有新增第二套历史 parser、schema、Canonical 字段、payload-v1/AAD/key/TTL/reader 改动或依赖升级。
每次 stored 正例独立重开加密 repository，逐条比较完整 messages 和 lineage；未重跑 #65 全恢复矩阵。

## 4. RED、中间失败与公共受控回执

真实 HTTP listener／鉴权／生产 compiler-factory 边界先复现合法 id 请求 HTTP 400。
独立 accepted/receive=0，Request=1，Attempt=0，Usage=0，ledger=0，预期 200 的断言 FAIL、exit=101。
保留原时点测试源码、完整失败日志、执行后立即留存的 binary/hash 和 7177 项输入清单；没有把 RED
改写成预期 400 的绿色测试。

| 历史记录 | 原结果及处理 |
| --- | --- |
| c6-suite-1 | 新测试 raw string 少结束符，编译 FAIL；修正测试语法，保留日志 |
| c6-suite-2 | 4 PASS / 2 FAIL；通用 stored helper 注入不适用的 capability override 导致 RouteAccess；改用既有 C4 repository 装配，未改 native 能力 |
| c6-suite-3 | 4 PASS / 2 FAIL；合并历史重复 ID 错报 503，新增 C6 pinned preflight 修为 400；另修正“每响应 upstream_response_id 应改变”的测试预期，全部稳定 lineage 成员仍严格比较 |
| c6-suite-4/5 | 6/6，随后补齐 adapter/mode/builder 后公共 7/7；最终候选再独立 7/7 |
| clippy-1/2 | 首轮保留行数、分号、词法 lock scope 反馈；资格应用收拢到原 transport profile owner，stored 断言聚合，lock 在 await 前结束；第二轮 PASS，没有放宽 lint 或 timeout |

[controlled-receipts-final.json](assets/cpar62-c6-20261021/controlled-receipts-final.json) 为最终候选的
308 条结构化回执；[final-public-verification.json](assets/cpar62-c6-20261021/final-public-verification.json)
绑定候选、manifest/lock、源输入清单、实际 test binary 和日志 hash。7 个公共测试＋1 个 builder
专项均在最终 Full 中 PASS；公共专项随后采集原始 --nocapture 回执，仍是同一候选。

- 三角色原值 receiver、多 content part、相同完整历史跨请求合法重放；1 和 512 字节边界包含全部 ASCII graphic 转义字符、absent id、空 text。
- 8 组五轮链：显式／存储 × JSON／1、7、31 字节 SSE。两次相关工具读取 41、以 previous=41 计算 42、正确回传结果并完成后续问答；每轮输入 id/role/order/content、原生输出和实际模型一致。
- 每组 5 Request / 5 receiver / 5 actual Attempt / 5 Usage / 5 ledger；20 input、7 output、inclusive measured；Usage→Attempt→Request 与 ledger source identity 精确核对，最终 lease=0。
- 258 条标记输入／资格负例，另有跨协议、其他 adapter、passthrough 及跨 item 类型反例；独立 receiver 和实际 Attempt 为 0，writer/ledger 明确读回而非从缺失猜零。
- mixed history 与 assistant/function/reasoning/hosted 共存时，仅各自资格齐备才能通过；单独 C6 不授予其他能力。call_id 可合法等于一个 message id，仍按独立关联规则处理。
- 存储续轮 12 条负例：同值／改内容／跨类型重复 id→400；foreign owner→404；不同 route→409 continuity_unavailable；不可用 credential revision→503 CredentialUnavailable。没有新增 receiver 或 Attempt，也没有转投备用 candidate。

标准 token ledger 不证明完整 native success companion；后者仍 UNKNOWN/#61。cost_in_usd_ticks
仍 NOT_ADMITTED，真实 Provider 费用 UNKNOWN。没有未观测值被写为零。

## 5. 不可变候选上的完整验证

独立工作树 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar62-c6-full-20261021-8b192fd9`，
全新 target `/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-c6-20261021/target-full-final`。
18 项继承文档／规则覆盖层明确列在 [full-binding.json](assets/cpar62-c6-20261021/full-binding.json)，
全部 7181 tracked/untracked 输入逐文件 hash/mode/symlink 在 Full 前后以及实际 CLI 后保持相同。
这是“候选＋明确覆盖层”的 Full，不把继承文档冒充提交内容。

```sh
CHECK_REPORT_PATH=/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020/output/cpar62-c6-20261021/full-check.md CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-c6-20261021/target-full-final ./scripts/check.sh full
```

[Full 逐步回执](assets/cpar62-c6-20261021/full-check.md)：**45/45 PASS，exit=0**。
[full-result.json](assets/cpar62-c6-20261021/full-result.json)：Rust **1621 passed / 0 failed / 17 ignored**，
130 suites。包括 C1–C5／普通 Responses 的既有回归、C6 新增 8 项；未将 ignored 实际客户端或真实
Provider 用例计入已运行。实际 CLI 另行执行。未设置 test threads、RUSTFLAGS、build jobs 或 opt override，
未提高 timeout。RustSec、dependency policy 均 PASS，Cargo.lock 原值未改。

| 验证输入 | Identity / SHA-256 |
| --- | --- |
| 代码候选 Git tree | `8fcf7adebd53801245f6cecf2a489c401c1918a8` |
| Cargo.toml | `1b1a37ce8d8fcafa51d379ffae2ff33fff091c5a48c0f3458e47b99c12b22355` |
| Cargo.lock | `36564c0219707741d79ecb37d9fc4ee620ce2fe6801fdbe09b32fb4c8687c858` |
| rust-toolchain.toml | `8e390d6a0838315f972690f46ef8bae8b7ecc9ee6c1ed70140ef852869c2482e` |
| 7181 项输入清单 | `e1207f0d2a22174ca8ee9bd17eec7e5a0045254d2e74644e712da4575a7ccbea` |

本报告和证据是后续仅文档提交，不把运行结果改绑到报告提交。

## 6. 原样 Codex 与实际 Pi

沿用未修改的 `scripts/acceptance/cpar62-cli.py --require-prompt-release --require-codex-success`，
在同一验证工作树、独立 OS 临时目录执行。Seatbelt 实测非 loopback IPv4/IPv6、真实用户目录读取和
临时区外写入均 EPERM；安装前后 bytes/mode/link manifest 相同。当前实测版本：Codex 0.161.0、
Pi shell/声明 bin 均 0.84.3、Node v26.8.2。

[actual-cli-summary.json](assets/cpar62-c6-20261021/actual-cli-summary.json) 绑定实际 executable、runner、
版本、隔离、原始回执 hash、严格门禁及具体观测。完整原始目录保留，没有把账户配置、密钥、数据库、
真实用户目录或原始 request metadata 归档进 Git。

| 实际场景 | 观测 |
| --- | --- |
| Pi standard | 5 HTTP 200 增量响应、3 turn stop、2 次相关工具、真实 result.txt 为 42；5 receive/Attempt/Usage/ledger，另 1 个预拒绝 probe 所以 finished=6 |
| Pi failure | 1 receive/Attempt/Usage/ledger；唯一 failed、无 completed；网关保留 20/7。Pi message_end 仍显示零用量是既有限制，不能据此推断零消费 |
| Pi cancel | 真 delta→lease=1→abort→同 PID 继续存活→cleanup 前独立 peer close=1、lease=0；本次 abort-to-release 观测上界 10 ms，原 1 秒 probe 通过；不新增 SLA，缺终端 Usage 不补零 |
| Codex standard | HTTP 400、CLI exit=1、receive/Attempt/Usage/ledger=0；保留原样 namespace/web_search/client_metadata/input id/parallel 等字段和客户端代码；没有关工具、删字段、换模型或猜 effort |
| 严格外层 runner | exit=1 / FAIL，六项成功判据全 false；Rust preparation/Pi 子验证 exit=0，不能覆盖严格 Codex FAIL |

原样 Codex 正向工具／失败／取消仍 BLOCKED。C6 本地原值链路完成不授予它的其他字段与并行资格，
也不把实际请求的通用 400 动态归因为某一个独立字段。Pi 成功完整 companion 和费用仍独立于本轮。

## 7. Standards、Spec 与下一项最小工作

[独立双轴审查](assets/cpar62-c6-20261021/independent-reviews.json) 固定比较
`4a6a83d1...8b192fd9` 的 17 个路径，既有 dirty/untracked 不计本轮差异。Standards：硬违规 0、
可操作判断项 0；C6 Spec：可举证缺项／错误／越界 0。主代理点验资格、builder、历史与续轮承重位置；
两路结果属于静态审查，最终 Full 和实际客户端回执另列。本轮 Spec 可在明确本地范围 PASS，整票 #62
仍 PARTIAL/BLOCKED，不能据局部绿色关闭工单。

下一项最小资格工作：取得明确适用于固定 proxy/profile 1.0.50、带日期／版本的 C6 serving 声明，
覆盖 caller-selected 非 assistant id 的接受、原值、冲突及显式／服务续轮语义，或取得可核验的
source→release→service 对应链。取得前保持生产拒绝；不需真实 Provider 调用来猜测。
原样 Codex 的正向场景还需独立的 namespace/search、实际 ParallelTools 和 #61 条件共同满足；
另一轮 expected-400 不能充当兼容成功。

本地完整证据根：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020/output/cpar62-c6-20261021`。
[证据清单](assets/cpar62-c6-20261021/evidence-manifest.json) 与
[精选完整归档](assets/cpar62-c6-20261021/verified-evidence.tar.gz) 按显式 allowlist 打包；原始来源、
全输入清单、18 项备份、历史失败和 RED binary 仍在本地，没有删除或覆盖。
本轮没有真实 Provider/付费账号调用、push、PR、工单写入或关闭、合并、发布或部署。
