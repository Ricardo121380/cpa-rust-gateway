# CPAR #62 C5 — native hosted web_search bounded local delivery (2026-10-21)

## Decision and qualification

PASS_BOUNDED。本轮实现原生声明 → 公共 listener/鉴权 → 生产 compiler/factory → 实际 leased Build adapter → 独立 loopback 上游 → 搜索项/阶段 → 带引用回答 → 后续显式历史。搜索由受控上游执行；CPAR 保留协议语义，不实现搜索或抓取 URL。

| Evidence layer | Result | Meaning |
| --- | --- | --- |
| 本地协议及受控完整链路 | PASS_BOUNDED | 本报告的闭合子集；不外推实际服务 |
| 固定 Build endpoint / profile 1.0.50 | BUILD_ACTIVATION_BLOCKED | 缺 cache-only false 的执行保证及完整链路资格；生产装配在 lease/send 前拒绝 |
| 原样 Codex | BLOCKED / strict success FAIL | 独立实际客户端成功门槛；不能以预期 HTTP 400 算兼容 |
| 实际 Pi 受控回归 | PASS_BOUNDED_CONTROLLED（既有用量/价格显示限制单列） | 仅说明本轮未破坏已支持的普通工具/显式历史/取消路径 |
| 真实 Provider / 远端 CI | NOT_RUN / NOT_RUN | 未调用真实账号、付费服务；未 push |
| 候选 / 发布 | 本地门禁范围见下文；发布资格 NOT_GRANTED | 本地 Full 不授予固定服务、三客户端或发布资格 |

#62 和首版三客户端完成条件不变；C1–C3、D1-A C4 是已实现前置。C6、ParallelTools、namespace 生产激活、Claude 新字段与 #61 不属于本轮实施。

## Baseline, ownership and immutable code

- 实际起点 `e16e120267c52b6dfb196df2976ea79a88f6ffe0`，分支 `feature/62-client-metadata`，初始索引为空；与给定预期一致。历史 `67db3304...` 用于定位，没有 reset/回退。
- 主实现提交 `c0196c242f2f0d709e87c4fdc4f86f71899d781f`；审查后共享纯阶段迁移判断提交 `0f451747e3252a8e20fffaf7b98de50f041a3c7a`。补充取消测试屏障提交 `60549c20f3762e8860ce7aed89400581550e5517` 后，必需安全补丁提交 `72464e7d1d4d716fad34affc154480288451477f` 是最终代码候选。
- 初始 18 项 dirty/untracked 逐文件备份。最终保留原始字节、权限、uid/gid；三份共改文档仅追加本轮后继说明，索引按 HEAD 加本轮后缀构建，未把继承改动纳入提交。其余既有历史材料完整保留。清单与核验见 [preservation-delivery-final.json](assets/cpar62-c5-20261021/preservation-delivery-final.json)。
- GitHub 只读快照：#62 OPEN，#61 OPEN 且为 blocker，本分支无 PR、remote branch 当时不存在。未改工单、工单依赖或远端 ref；完整快照保留在外部 evidence root。基线身份见 [baseline.json](assets/cpar62-c5-20261021/baseline.json)。
- Cargo manifest 和锁定依赖先冻结；只增加已有 workspace `url` 的纯解析依赖边及对应 lock edge，C5 功能没有新增包或升级版本。后续 Full 暴露 RUSTSEC-2026-0382，最小修复只将已有传递依赖 rustls-webpki 0.103.15 更新到 0.103.16，详见下节；其余包与依赖列表未变。HTTP 冷 client TLS roots 修复、C4 唯一目录修复和原 timeout 不变。

外部 evidence root：`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-c5-20261021`。原始源材料、全输入清单、18 项备份及未压缩日志仍在此；归档只包含本轮明确列举的非秘密证据。

## First-party wire contract and source applicability

来源于 2026-10-21T03:59 UTC 左右冻结；11 份完整内容的 URL、revision/date、字节数及 SHA-256 在 [source-manifest.json](assets/cpar62-c5-20261021/source-manifest.json)。官方文档无公开 revision/Last-Modified 时，记录为 null，以抓取时间和完整内容 hash 绑定，没有编造版本。

- [Responses create/input/output](https://developers.openai.com/api/reference/resources/responses/methods/create)、[input-items](https://developers.openai.com/api/reference/resources/responses/subresources/input_items/methods/list)、[streaming](https://developers.openai.com/api/reference/resources/responses/streaming-events) 定义声明、原生项、动作、显式输入和阶段字段；[web search guide](https://developers.openai.com/api/docs/guides/tools-web-search) 说明 false 的 cache-only 语义、缺省 true 及 preview 的区别。
- 固定公开 Build 树 revision `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8` 的 [tool_overrides.rs](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/tool_overrides.rs)、[Responses history](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation/responses.rs)、[stream consumer](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/stream/responses.rs) 佐证 hosted 声明、原生历史和阶段使用；其旧消费者的 `find` 命名没有被猜成新 wire alias。
- 当前 CPAR 固定代码仍使用 `https://cli-chat-proxy.grok.com/v1/responses`、profile 1.0.50。上述公开树的 SOURCE_REV 和版本注入机制没有建立可核验的 source → release → 实际 serving profile 关系；也没有提供 false 的执行保证。客户端源码不等同服务资格，当前 stable 指针未被用来切换 profile。

新增 Accepted 后继 [ADR-0117](../adr/ADR-0117-native-hosted-web-search.md) 和 [BC-PROTOCOL-011](../contracts/BC-PROTOCOL-011-native-hosted-web-search.md)，旧 Accepted 决策没有重写。精确支持/拒绝决策另见 [field-decisions.json](assets/cpar62-c5-20261021/field-decisions.json)。

| Contract surface | Bounded behavior |
| --- | --- |
| 声明 | 顶层唯一 `{"type":"web_search","external_web_access":false}`；两个成员必需且类型精确；不删除或改写 false |
| 拒绝项 | true、缺省、null/非 bool、重复键/声明、未知键/额外选项、preview/dated/其他未开发 hosted 类型；namespace 子项仍只允许 function |
| 上界 | whole tools 256 KiB、128 顶层节点、既有 namespace/256 function 上界；hosted 声明 256 B；原生项 64 KiB、ID 512 B、总 output 256 项；query/pattern 4096 B、queries/sources 各 128、URL 8192 B |
| 动作 | search 的可选 query、queries、sources；open_page 的可选 nullable url；find_in_page 的 pattern 与可选 nullable url。闭合键集合；无 `find` 猜测 alias |
| 身份和阶段 | 独立 web_search_call ID、原顺序/output_index；in_progress → 可选 searching → completed；阶段可整体缺省，出现后必须完整确认；done/terminal 的键、值及缺失状态一致 |
| 引用 | 复用已有 annotations/URL citations；保留文本、字符位置、URL、title、sources 的实际内容和顺序；不补标题、不改索引、不抓取 URL；sources 与 citations 分别保留 |
| 控制 | 复用 auto/none/required/已有 flat named function；required 可由 hosted 满足，none/flat named 禁止 hosted 输出；不支持强制 hosted/namespace selector；ParallelTools 独立 |
| 显式历史 | 已完成 top-level 原生项按原位置重编码；只接受闭合的 sole assistant OpaqueContent 封套，顶层入口设置标记；拒绝嵌套、混合、未完成、跨类身份重复；无需伪造 function_call_output |
| Presence | action 缺失、query/queries/sources 缺失或显式空集合、URL 缺失/null/string、空 title 均分别保持；不把 null 自动变为缺失或补字段 |

## Source locations

| Responsibility | Source |
| --- | --- |
| 声明/混合工具集合 | [Responses codec](../../crates/protocol-openai-responses/src/lib.rs)、[ResponsesToolSet](../../crates/protocol-openai-responses/src/tool_set.rs) |
| 闭合动作、历史、阶段 | [web_search.rs](../../crates/protocol-openai-responses/src/web_search.rs)、[native output](../../crates/protocol-openai-responses/src/native_output.rs)、[annotations](../../crates/protocol-openai-responses/src/annotations.rs) |
| Router 控制和转换 | [tool_controls](../../crates/gateway-router/src/tool_controls.rs)、[response_transform](../../crates/gateway-router/src/response_transform.rs)、[protocol_transform](../../crates/gateway-router/src/protocol_transform.rs) |
| 生产准入/实际 Build 编码解码 | [gateway runtime](../../apps/gateway/src/runtime.rs)、[Build adapter](../../crates/provider-grok/src/build_responses.rs)、[inference](../../crates/provider-grok/src/inference.rs) |
| 公共受控闭环 | [search.rs](../../apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/search.rs)、[negatives](../../apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/search/negatives.rs)、[failures](../../apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/search/failures.rs) |
| 既有 payload v1 加密读写 | [stored_response](../../crates/gateway-store/src/stored_response.rs) |

## Production composition and minimal representation

ResponsesToolSet 保存整个混合工具数组和原顺序，普通 flat-only 路径保持既有表示。Router 用完整工具集合检查请求、输出和工具控制；hosted 搜索不计作额外客户端 function，也不绕过普通 call/result 关联。

生产 compiler/factory、Build builder 和 decoder 各自保留 C5 资格门槛。正常生产分支没有 search 资格；拒绝发生在 project_candidate、实际 lease/send 之前。受控正例通过私有 `cfg(test)` 组合资格、已有依赖测试接缝和自有 loopback transport 使用实际 Build adapter。namespace 与 search 两个布尔资格相互独立，均不授予 ParallelTools。stock `all-features` 没有取得资格的生产调用点；没有 User-Agent、header、请求字段、普通配置或 URL override 激活通道。

阶段复用空 TextDelta 的闭合 protocol 扩展，原生项复用 OutputItemStart/End；公共 SSE 保留真实阶段观察并生成自己的连贯 sequence_number。JSON 不合成未观察到的阶段。hosted item 完成及同一响应的普通 function 可执行完成，均等到 terminal 一致性和标准 Usage 准入后再释放。

没有 SQL migration、Canonical 枚举字段或 payload-v1 shape/AAD/key/TTL 变更。新增语义在既有 RawExtensions/OpaqueContent 封套内；旧 reader 能保留封套不代表旧执行器会准入。`hosted_search_extensions_reopen_in_payload_v1_without_presence_loss` 验证写入、重开、活动密钥轮换、owner 隔离、replay 仍为 v1；未重跑 #65 全恢复矩阵。

## Public positive and negative evidence

[controlled-receipts-final-candidate.json](assets/cpar62-c5-20261021/controlled-receipts-final-candidate.json) 和归档的 `search-final-candidate.log.gz` 绑定最终候选 72464e7d 及其新 lock。60549c20 的 `grok-regression-3.log.gz`、更早的 `search-suite-final.log.gz` 和相应 summary 均作为各自时点证据保留，没有改绑资格。9 个 public tests 全过；JSON 与 SSE 的 1/7/31 字节碎片分别运行。每个正例三次独立请求，核对 receiver 的原样声明、三种动作、回答及 citations、阶段次序、后续搜索问答、第三轮不带当前 search 声明的合法显式历史。

每组得到 3 receiver / 3 Request / 3 actual Attempt / 3 Usage / 3 ledger，20 input、7 output、inclusive measured provenance、准确 lineage，lease 最后为 0，费用 None/UNKNOWN。混合 flat lookup 与 namespace 同名函数另有 JSON/SSE 三轮，搜索身份、function/namespace 身份和结果关联独立保留。

| Negative or failure surface | Observed assertion |
| --- | --- |
| 未获资格、非法/重复/未知/超界声明与非法历史 | 168 组前置拒绝 receipts；四种 namespace/search 资格组合，JSON/SSE；receiver=0、Attempt=0，writer 无 Usage/ledger |
| 输出/引用/终态 | 56 组已发送后的拒绝 receipts；item 身份冲突、无声明/none/named 下 hosted、非法 action/status/URL/citation、阶段重复/倒退/关联错误及 done/terminal presence 冲突；准确失败且不重发 |
| 未知 Usage | JSON/SSE 两种原始样本含未知 web_search 计数或 cost_in_usd_ticks；实际 Attempt 的加密失败 Usage 原文读回完全一致；已验证 20/7 的既有账本保持，费用 UNKNOWN |
| 混合可执行工具 | 在未知 Usage 场景中普通 function 与搜索同时存在；无 function arguments.done、无 function/search output_item.done、无 response.completed |
| root 搜索统计 | 保留非零 tool_usage 及 unknown/null 字段的完整受控样本，JSON/SSE 均 BLOCKED；不删字段制造正例，不承诺新增整响应加密证据格式 |
| 截断和 EOF | 唯一 failed terminal/无成功、不重发；缺失 Usage 不补零、费用 UNKNOWN |
| FIN/RST 取消 | 先收到真实增量、lease=1，再 abort；独立 peer 在 cleanup 前观察 close，lease=0；显式再次请求能复用容量，取消请求无伪造 Usage/费用 |

统计和费用是独立语义。purpose-built 正例仅包含已经审查的标准 Usage，没有从真实或受控失败样本中删除未知字段。不会从 action、sources、citations 推算搜索次数或价钱。#61 的 cost_in_usd_ticks 继续 NOT_ADMITTED；即使标准 token ledger 已验证，Provider/search 完整费用仍 UNKNOWN。

## Retained RED, failures and fixes

所有失败日志与后续结果分别保留，未覆盖为绿色：

1. `red-public.log`：合法声明经公共边界得到 HTTP 400，receiver=0、Attempt=0；预期正例 200 的断言失败。`red-search.rs` 保留该时点测试源码。两次 RED executable 的原时点 hash 均未采集，身份为 UNKNOWN；后来留存 binary 副本仅记录留存时 hash，不冒充原时点可执行身份。
2. `search-suite-1.log`：5 pass/4 fail，暴露 Router 对原生空文本阶段载体的重复识别缺口；在现有 native 元数据入口修复。`search-suite-2.log`：8 pass/1 fail，剩余 RST probe 只设置 zero-linger 未 drop socket；修复 probe，保持 timeout 原值。第 3/4 轮 9/9，最终轮 9/9。
3. `mixed-completion-red.log`：构造搜索和普通 function 共存、terminal 含未知 Usage；公共流先发 function_call_arguments.done 的断言失败。完成屏障扩展到该混合场景的普通 function；原始未知 Usage、失败状态和无可执行完成均保留。
4. `affected-crates-1/2.log` 保留新增私有参数漏传和旧 part_annotations_null 诊断阶段回归；补足显式 false，并把最终文本引用范围校验放回应有阶段。`affected-crates-3/final.log` 604 pass/0 fail/7 ignored。
5. `clippy-1` 至 `clippy-4` 保留行数/Debug 字段、测试私有参数和未启用 reqwest json feature 等检查失败；分拆现有方法、补字段/参数、测试用 body 编码修复。第 5/6 轮通过。未放宽项目 lint、断言、超时、账号或模型。
6. Standards 的纯阶段迁移谓词重复由最终小提交收拢到现有 web_search 模块；主代理逐行核验谓词等价和两侧关联校验保留。第一次 fmt-check 的格式反馈已修正；最终候选专项通过。
7. `full-1-failed.log` / `full-1-failed-check.md`：首轮独立 Full 在旧取消用例失败，gateway 278 pass/1 fail/4 ignored。7112 个源码输入前后完全一致。`disconnect-reproduction-1.log` 单独复现；第 2 轮诊断定位 extra=true/reset=true、peer closed、lease=0，而 RequestFinished=Failed。被拒 Usage 已经关闭上游，因此原测试用 peer close 作公共 body 取消屏障不足，释放 receipt 会与 RST 处理竞态。60549c20 先在原 1 秒窗口确认公共 Cancelled，再释放 receipt；没有改生产代码、timeout 或放宽断言。第 3 轮 Grok 全组结果见最终验证。
8. `grok-regression-2.log` 保留屏障补丁一次上下文误命中导致的私有 receipt 未定义编译失败；定位后纠正至目标测试，第 3 轮重新完整执行。该编译失败没有作为任何绿色证据。

原始文件 allowlist、hash 和压缩内容核验见 [evidence-manifest.json](assets/cpar62-c5-20261021/evidence-manifest.json)。不归档账号、密钥、数据库、target 或继承文档正文；既有历史证据没有改写。

## Required supply-chain gate remediation

第二轮 Full 的 Rust 为 1613 passed / 0 failed / 17 ignored，但 `cargo deny check` 因 [RUSTSEC-2026-0382 的固定源码](https://github.com/RustSec/advisory-db/blob/e6f94f3760d5e6c54c1b05f204002263bc025524/crates/rustls-webpki/RUSTSEC-2026-0382.md) 阻断，故该轮完整结果为 FAIL（RustSec audit step 未执行）。公告日期 2026-10-20，受影响的旧锁为 rustls-webpki 0.103.15；[维护者修复 PR #546](https://github.com/rustls/webpki/pull/546) 明确提供 0.103.16 的 distrusted SPKI 路径拒绝补丁。

完成必需门禁所需的最小增量为 `cargo update -p rustls-webpki --precise 0.103.16`。72464e7d 只改 Cargo.lock 的该 package version/checksum，保留 source 和依赖数组；无 broad update、advisory ignore 或版本门槛放宽。修复不是 hosted search、服务资格或 client 控制的新能力。固定一手材料的 URL/revision/date/hash 见 [dependency-advisory-manifest.json](assets/cpar62-c5-20261021/dependency-advisory-manifest.json)，精确锁差异见 [dependency-delta.json](assets/cpar62-c5-20261021/dependency-delta.json)。初次按 PR merge revision 查最终编号路径得到 404；该编号在后续 e6f94f commit 分配，最终材料只引用成功冻结的 revision。

独立只读探子确认仅该 package 变化，冷 HTTP roots 目录在基线和最终候选的 tree hash 均为 `7233bd293636803f2499c10939c286bbc1087430`，所有 timeout 和 advisory ignore=[] 不变。其指出单独 audit 的四行 stdout 未含退出码；主代理持有该调用的工具完成回执 exit=0，最终 Full 仍单独执行并记录 audit 步骤。此处不以缺少失败文字推断成功。新候选重新运行 Full 和实际客户端回归，未继承旧锁的资格。独立差异核验见 [dependency-review.json](assets/cpar62-c5-20261021/dependency-review.json)。

## Final verification binding

最终完整 Full 绑定代码 `72464e7d1d4d716fad34affc154480288451477f`，没有复用旧 manifest/lock、旧依赖测试资格或开发 target。验证工作树 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar62-c5-full-20261021-72464e7d`；全新独立 target `/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-c5-20261021/target-full-final-3`。显式携带并核验 18 项必要覆盖；实际 7112 个 tracked/untracked 输入按文件 hash、mode、symlink 逐项冻结，执行前后清单完全相同。

```sh
CHECK_REPORT_PATH=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-c5-20261021/full-3-check.md CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-c5-20261021/target-full-final-3 ./scripts/check.sh full
```

结果 **45/45 PASS，exit=0**；Rust **1613 passed / 0 failed / 17 ignored**，130 suites。默认并发，未设置 RUST_TEST_THREADS、RUSTFLAGS、CARGO_BUILD_JOBS 或 test opt override；没有提高原 timeout。17 ignored 不作为真实 Provider 或 ignored actual-client 的执行证据；实际客户端另按隔离 runner 明确执行。

| Frozen input | SHA-256 |
| --- | --- |
| Cargo.toml | `1b1a37ce8d8fcafa51d379ffae2ff33fff091c5a48c0f3458e47b99c12b22355` |
| Cargo.lock | `36564c0219707741d79ecb37d9fc4ee620ce2fe6801fdbe09b32fb4c8687c858` |
| rust-toolchain.toml | `8e390d6a0838315f972690f46ef8bae8b7ecc9ee6c1ed70140ef852869c2482e` |
| 全 7112 项清单内容（前后相同） | `a4f6a17792dce91acd7df8efb00beae2120d536ccd8f624a65241f0665e6d527` |

[full-3-check.md](assets/cpar62-c5-20261021/full-3-check.md) 是完整 45 项逐步回执；[full-3-binding.json](assets/cpar62-c5-20261021/full-3-binding.json) 列出每项覆盖和独立装配；[final-verification.json](assets/cpar62-c5-20261021/final-verification.json) 保存源树、日志/回执 hash、实际 test binary hash 和执行后观测的 advisory-db revision。完整前后清单保留于外部 evidence root，未把继承文档正文或全部历史文件名再包装入 Git。

前两轮各自保持 FAIL；第三轮从固定的新候选完整重跑，未把后续单项绿色拼接成 Full。开发期四个受影响 crate 的专项为 604 passed / 0 failed / 7 ignored，开发期 Grok 全组为 76 passed / 0 failed / 3 ignored，最终候选 C5 public 专项为 9/9（[详细回执绑定](assets/cpar62-c5-20261021/final-public-verification.json)，同一候选 manifest/lock；workspace 与 package 选择生成的 test binary 分别记录 hash）；第三轮 Full 重新覆盖全部这些源码。旧锁上的开发期专项不充当新锁的独立资格。报告和证据提交位于代码候选之后，只增文档与证据，其静态门禁另验，不把 Full 改绑到后续报告提交。

## Actual client regression

采用最终代码候选 `72464e7d` 的既有未修改 runner `scripts/acceptance/cpar62-cli.py --require-prompt-release --require-codex-success`，使用独立 OS 临时目录和合成账号。Seatbelt 实测 IPv4/IPv6 非 loopback 连接、真实用户目录读取和越界写入均 EPERM；客户端身份检查及安装前后 bytes/mode/link 清单相等。未执行 only-my-pi 分支程序、未改已安装客户端或 runner。版本：Codex 0.161.0、Pi 0.84.3（shell 与声明 bin 一致）、Node v26.8.2。

- Pi standard：5 个独立 receive / Request / Attempt / Usage / ledger，5 个 HTTP 200 增量响应，3 个 RPC turn stop；两轮相关工具和后续问答、真实受控文件结果 42 已核验。另有 1 个负例 probe，故 finished=6，不能误算为第 6 个成功上游请求。
- Pi failure：1 receive，1 failed terminal，无 completed；网关保留已收到的 20/7 Usage 与准确 ledger。Pi 本身的 message_end 显示零用量是既有显示限制，不能用它推断零上游用量。成功 Pi 的零价格显示也不成为计费依据，网关费用 UNKNOWN。
- Pi cancel：真实 delta → lease=1 → abort → 同一 PID 仍活着 → 独立 peer close=1 → lease=0，均在服务/进程 cleanup 前观测；本次 abort-to-release 观测上界 14 ms，原 1 秒诊断探针通过。这不是新增产品 SLA。取消没有收到终端 Usage，网关未造计数或费用。
- 原样 Codex：HTTP 400、receiver=0、Attempt=0；实际 success gate 六项全 false。因此 Rust preparation/Pi 验证 exit=0，而带 `--require-codex-success` 的严格外层 runner **exit=1 / FAIL**；不把该失败改名为兼容性 PASS，也不删字段、关工具或更换客户端。

受控 actual-client 结果和原始 receipts hash 见 [actual-cli-final-summary.json](assets/cpar62-c5-20261021/actual-cli-final-summary.json)。此回归验证普通 Pi 路径，不授予真实客户端 hosted-search、固定 Build 或 #61 成功元数据资格。完整原始临时目录已在 summary 中记录，归档不包含认证配置、数据库或密钥。


## Standards and Spec

两路独立 read-only review 审查 `e16e1202...c0196c24` 的 29 个本轮路径、契约和证据，继承 dirty 不计入变更。后续增量见 [final-delta-review.json](assets/cpar62-c5-20261021/final-delta-review.json)，取消测试屏障由主代理逐行复核（生产代码未变、原 1 秒窗口和 Cancelled 断言保留且提前到回执释放前）。记录见 [independent-reviews.json](assets/cpar62-c5-20261021/independent-reviews.json)。

| Review | Result | Disposition |
| --- | --- | --- |
| Standards | PASS；0 hard violation；此前 1 判断项已解决 | 重复阶段迁移谓词已由 0f451747 收拢；主代理直接复核、专项及最终 Full 验证 |
| Spec (本轮 C5) | PASS_STATIC_BOUNDED / 0 actionable | 审查时最终 Full、实际 Pi 为待补证据；最终结果单独列上，不把静态审查冒充运行结果 |
| Spec (整票 #62) | PARTIAL/BLOCKED | 本轮不消除独立生产/客户端/计量条件 |

## Minimal missing service evidence and remaining boundary

固定服务激活至少需要：适用版本和日期的 serving 声明，明确覆盖固定 proxy/profile 1.0.50 的 external_web_access:false cache-only 执行保证；或可独立验证的 source/release/service 映射链。材料还须覆盖 native declaration、search/open_page/find_in_page、阶段和 JSON/SSE 终态、citations/sources、显式历史及适用工具控制，并交代实际全部 search Usage/root statistics 的含义与计量。当前公开客户端树与本地 loopback 无法替代这些材料。

原样 Codex 还保留 C6、实际 ParallelTools、namespace 生产激活、#61 等独立条件。没有删除实际客户端字段、关闭工具、降级客户端或切换 preview 来制造资格。没有真实 Provider、远端 CI、push、工单变更、合并、发布或部署。
