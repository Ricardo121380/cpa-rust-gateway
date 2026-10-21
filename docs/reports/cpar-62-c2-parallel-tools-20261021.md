# CPAR #62 C2 剩余部分 — 有界受控 ParallelTools 多调用闭环

日期：2026-10-21（Asia/Shanghai）。**本轮 C2 本地实现与公共受控验收 PASS_BOUNDED。**
同一响应中的两个独立函数调用、交错 SSE、整份终态与适用 Usage 完成屏障、两个结果回传、
后续依赖工具轮和问答，均经过公开 HTTP listener、鉴权、实际 compiler/factory 和 Build adapter。
不可变代码候选：`a786cc72c4b7880ae8ed35de6a9c0ae788f38a82`。

| 证据层 | 结果 | 限定范围 |
| --- | --- | --- |
| 本轮 C2 实现／公共受控验收 | PASS_BOUNDED | HTTP Responses → 实际 GrokBuildResponses → Canonical/CanonicalBridge → 自有 loopback 上游 |
| 最终 Full | 45/45 PASS，exit=0 | 候选＋18 项明确继承覆盖；1635 passed / 0 failed / 17 ignored，130 suites |
| 公共专项 | 12/12 PASS，187 条回执 | 同一不可变候选及输入；另有 2 个底层专项在 Full 中通过 |
| Standards | PASS_STATIC，0 actionable | 独立只读审查；完整运行检查另列 |
| 本轮 C2 Spec | PASS_BOUNDED；独立静态审查 0 actionable | 本地切片范围，不提升为整票或实际客户端成功 |
| 整票 #62 | OPEN / PARTIAL / BLOCKED | 原样 Codex 组合正向、固定服务资格、#61 等仍独立 |
| 原样 Codex 0.161.0 | 严格 runner FAIL，exit=1 | HTTP 400；receiver/Attempt/Usage/ledger=0；六项成功判据全 false |
| 实际原版 Pi 0.84.3 | PASS_BOUNDED_CONTROLLED | 既有 serial 工具、显式历史、失败、同 PID 取消；不是实际 Pi 并行批次验收 |
| 固定 Build 生产激活 | BUILD_ACTIVATION_BLOCKED | stock ledger 未改；固定 profile 1.0.50 没有新增资格 |
| 真实 Provider／远端 CI／发布部署 | NOT_RUN / NOT_RUN / NOT_RUN | 没有真实账号调用、push、PR、合并或发布 |

## 1. 实际基线、规则与保留

工作树：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`；
分支 `feature/62-client-metadata`；起始实际 HEAD 为
`702264588ade1521e841550bbfe772437e601b1b`，与输入一致。旧 C6 候选只定位历史证据，没有回退。
起始 index 为空，18 个既有 dirty/untracked 文件逐项备份原字节、mode、uid/gid 和 symlink。
没有 reset、clean、stash、整体暂存、清理历史失败或从旧 repo 目录开工。

读取适用 AGENTS、CONTEXT、issue-tracker/domain、cross-boundary 尾部、C6/C5/namespace 报告、
字段方案、ADR-0115 至 0118、BC-PROTOCOL-009/011/012 及必要 provider/router/core 接缝。
C1–C6 的既有本地结论和实际客户端限制保留，没有重做或改写历史报告。
本轮 [ADR-0119](../adr/ADR-0119-controlled-parallel-function-batches.md) 和
[BC-PROTOCOL-013](../contracts/BC-PROTOCOL-013-controlled-parallel-functions.md) 在行为实施前冻结；
旧契约、ADR 索引和 traceability 只追加后继关系。

三个共改文档按“原 HEAD＋本轮后缀”构建索引，继承 dirty 内容未进入提交。
其他原文件完全保持，三个文档的原字节也完整保留。
[preservation.json](assets/cpar62-parallel-20261021/preservation.json) 记录 18 项保留与独立暂存方式。
原始备份和包含继承差异的 patch 仅留在本地证据根，不混入本轮证据提交。

GitHub 起止只读核实：#62、#61 均 OPEN，#62 blocked_by 仍为 #61；本分支没有 PR／远端 branch。
develop/main 仍为 `c39bc07b1d53f831199f21cc5ae5062c987670b2` /
`c6a981b0f9eda09b0a2e58df11d272adf14f9552`。
[最终读回](assets/cpar62-parallel-20261021/github-final-readback.json) 未发现相应状态变化。
没有认领、评论、修改或关闭工单，没有改变依赖、remote ref 或生产配置。

## 2. 一手来源和冻结契约

[官方 function-calling guide](https://developers.openai.com/api/docs/guides/function-calling)
明确 false 一轮最多一个函数调用，forced specific function 为一次指定函数调用。
本轮补充冻结完整 guide，而非只依据 create schema。三份完整一手材料及检索元数据见
[source-manifest.json](assets/cpar62-parallel-20261021/source-manifest.json) 和证据归档。
HTTP Last-Modified 只作为服务器观测；未公布 publication/revision 的字段保持 null。

| 冻结材料 | Bytes | SHA-256 |
| --- | ---: | --- |
| Function-calling guide | 77573 | `ce460da5635ff6b235fd45bb94a425975fc438f5e47ba71705e7ebf689172ae1` |
| Responses create | 507870 | `9c4d505259ac72b879a3697f4f606d4131989ae686d204078b7b22e5638794d0` |
| Responses streaming events | 2092074 | `953d5dcf090a159603b31fade58a8e14c3174c874ed669a6fa916906abbd10c9` |

true、false、absent 分别保留到 Build body，不填默认值；null 和非 boolean 继续拒绝。
显式 true 同时要求实际 compiler 有效 ParallelTools capability、独立私有字段资格和自有 transport。
缺失控制不声称已知服务默认值。auto 允许零或多个；none 禁止调用；required 至少一个；
flat named 必须恰好一次且身份匹配。false 将函数数量限制到最多一次，required＋false 恰好一次。
flat named 的一次约束不随 true/absent 放宽，也没有推广为其他来源协议的新规则。

完整函数身份是 namespace 的 presence/value＋name；item id、call_id、output_index 各有职责。
不同 namespace 下同名函数合法，不能按裸 name 合并或改名。参数 delta 是暂态；arguments.done
确认一个完整对象，output_item.done 确认一个 item，response.completed 才确认整份终态。
各层身份／参数／原生输出位置互相矛盾、重复 item/call/index、截断、取消或错误 Usage 均不能
释放可执行完成。既有严格 JSON 对象、重复成员、大小界限和空对象规范保持。

没有发明通用序号连续性、默认值或事件顺序保证。现有 decoder 的“按 output-array 顺序注册
item”仍是明确的有界子集；已开始调用的参数可以交错，done 顺序可以相反。
公共 encoder 使用自己的序列；输入间隔序号不被补成推断值。
函数批次与 provider-hosted search 分开：C5 声明共存不赋予 hosted item 客户端函数结果或并行
执行语义。没有新增网关工具执行器、多 agent 调度器或 hosted search 执行器。

## 3. 最小实现及独立资格

- provider-grok 复用既有 item/call 状态、参数累计及 `pending_tool_completion` 队列。
  完成屏障由 `namespaces_qualified || search_enabled` 扩展为也包含独立 parallel qualification，
  同时拦住 semantic ToolCallEnd 与 function OutputItemEnd。整份成功终态、身份／参数确认、
  适用 Usage projection 和既有 durable confirmation 成立后，才能交付客户端可执行完成。
  第一条调用先到 done、第二条或终态失败时，第一条也不会漏出 arguments.done/item.done。
- Build builder／adapter 默认未资格；`controlled-test-support` 只携带有界 permission。
  gateway 新私有 cfg(test) flag 必须和自有 transport 共存；compiler profile 从 stock 派生，
  只在私有装配增加 ParallelTools，经实际 RouteCompiler 和生产 factory 进入链路。
  URL、请求、header、User-Agent、普通配置和 namespace/search/C6 等资格均不能授予它。
  其他 adapter、target、passthrough、非 HTTP／非 Responses 不继承此资格。
- Router 在既有 ToolExecutionConstraints／response projector 中落实来源 Responses 的
  forced named 恰好一次。存储续轮复用 C6 pinned candidate preflight，扩展到原生函数身份及
  parallel true，避免错误结果被 continuation scheduler 错报为 CredentialUnavailable 503。
  现在错误 namespace/name/call 结果准确地发送前 400，无额外 lease/send。
- core 历史、owner/model/route/candidate/credential lineage、TTL、加密 repository 和 Usage
  owner 不变；没有 SQL/schema、Canonical/payload-v1/AAD/key、依赖或锁文件改动。
  各独立资格的 boolean 是既有模式的延续；Clippy 只对相应内部结构作有理由的窄范围允许，
  全局 lint 和 `-D warnings` 保持。

[stock-ledger-binding.json](assets/cpar62-parallel-20261021/stock-ledger-binding.json)
逐字比较起始和候选 `p12_adapter_capabilities`，确认完全相同。
正常生产装配和 all-features 都没有调用私有 parallel grant，生产 Build 仍不含 ParallelTools。

## 4. RED 和中间失败保留

| 记录 | 实际结果与处理 |
| --- | --- |
| 初始公开 RED | 合法 parallel=true 的期望 200 断言得到 400，exit=101；独立 accepted=0、receiver=0，Request=1、actual Attempt=0。保留时点源码、7200 项原始输入清单、日志和实际执行 binary/hash |
| completion-red | 仅完成资格装配后，首个平面双调用正例可达；错误第二条终态仍漏出两条 arguments.done 和两条 item.done 后 failed，exit=101。修复原完成队列屏障；最终反例全部为零完成 |
| stored-result-red | 错误 namespace 的 stored 结果误报 503，期望 400 失败；receiver/Attempt 总数仍为第一轮的 1。扩展 pinned preflight 后准确 400 |
| parallel-suite-1 | 7/7 PASS；随后补齐 controls、Usage、跨模式、组合及存储反例 |
| parallel-suite-2 | 9 PASS / 1 FAIL：新测试误期望丢失 durable Usage confirmation 为 502；既有正确映射是 503 RecordingUnavailable，改为同时严格断言 503 与该错误码，产品行为未改 |
| parallel-suite-3 | 公共 12/12 PASS；随后固定代码候选并重新运行 Full／公共专项 |
| clippy-1/2/3 → 4 | 保留字段顺序、四项独立资格 boolean、无用 Result、分号及测试表达式反馈；第四次 `-D warnings` PASS，未放宽全局 lint 或 timeout |
| 回执导出旧缓存 | 一次命令遗漏显式 CARGO_TARGET_DIR，使用旧开发缓存；日志保留为 public-replay-old-cache，不作为最终绑定。显式指定本轮独立 target 后重新采集 12/12 |
| 保留检查器首轮 | 按 basename 错把 ADR README 后缀算到根 README，出现 AssertionError；无文件变更。改为三个确切路径后 18/18 原内容和元数据逐项通过 |

[red-binding.json](assets/cpar62-parallel-20261021/red-binding.json) 绑定初始失败到实际
`gateway-b314458fd9b519fb` 留存 binary，SHA-256
`d598f4d0868f50b9132bd3b2e413f5ffc5387f771bf4eac99d9440b859410f95`。
不同阶段同名 artifact 的 hash 不混用。原始 RED 没有改成 expected-400 的兼容性 PASS。
本轮没有提高 timeout、删客户端字段、关闭工具、弱化断言或无差别重试直到绿色。

## 5. 公共边界验收回执

[controlled-receipts-final.json](assets/cpar62-parallel-20261021/controlled-receipts-final.json)
包含最终候选 187 条结构化回执。计数是场景回执，不是 187 个 Rust test。

| 回执 | 数量 | 核验内容 |
| --- | ---: | --- |
| 五请求完整历史链 | 32 | flat 不同函数／不同 namespace 同名 × 显式／存储 × JSON／1、7、31 字节 SSE × 正／反 done 顺序 |
| 独立双 flat 初始主链 | 1 | 同一响应两调用，receiver/Attempt/Usage/ledger 各一次 |
| C1–C6＋parallel 组合 | 2 | JSON/SSE；C4 本地加密精确读回而非转发，C5 声明独立保留，C6 input id 和完整 namespace 精确到 receiver；不是实际 Codex |
| controls 数量／选择 | 28 | 14 场景 × JSON/SSE；true/false/absent、auto/none/required/flat named，包含同名 forced 第二次调用 |
| 发送前拒绝 | 40 | 20 类 × JSON/SSE；资格缺失、错型/null、selectors、缺失／重复／错误结果；独立 accepted/receiver=0，actual Attempt/Usage/ledger=0 |
| 发送后失败 | 77 | 22 类 SSE fault × 1/7/31 bytes，4 个未知 Usage JSON/SSE，5 个 JSON 冲突，2 个丢失确认；一 receiver/Attempt，无重试／可执行完成／伪 completed／成功存储根，续轮 404 |
| stored 多结果反例 | 5 | 缺失／重复／错误 call/name/namespace，准确 400；receiver/Attempt 总数保持第一轮 1 |
| FIN/RST 取消 | 2 | 第一调用上游已到 done，再真实第二调用 delta → lease=1 → 断开 → 独立上游关闭 → lease=0；清理前观测，存储根不存在 |

每个五请求正例中，第一响应同时产生互不依赖的 seed=19 与 seed=22；两个结果按 call_id 反序
回传，下一轮确认合计 41；后续才请求依赖这些结果的 combine，返回 42 并完成后续问答。
独立并行和依赖前轮结果的调用分别构造。完整 effective history 逐轮对比 receiver，保持 C6 的
user/developer/system id、role/content part 顺序、reasoning/function/message 原生身份和 output 顺序。
stored 正例独立重开 repository，核对完整 messages、稳定 lineage、exact owner，foreign owner 不可读。
每组五 Request／receiver／Attempt／Usage／ledger；每响应 token 20 input、7 output、inclusive measured。
多调用没有重复计费或计为多个 Attempt；未知计数保持 null，费用与 native success companion UNKNOWN。

FIN 和 RST 的最终释放观测分别为 8ms／7ms，fast probe 均通过，且独立 peer close=1、lease=0。
这是本机观测上界，不是生产延迟保证。未读到的取消 Usage 为 UNKNOWN，不因未落 Usage 行推断上游零用量。
四项未知 Usage 负例包含 cost_in_usd_ticks／未知 counter，精确保留拒绝材料并加密读回；
cost_in_usd_ticks 仍 NOT_ADMITTED，#61 未放宽。
跨 adapter/mode、Chat/Messages、无任一独立资格和正常装配另有公共零发送断言；底层专项直接确认
semantic ToolCallEnd 与 native OutputItemEnd 都在原完成队列内，没有只测外层 JSON 标签。

## 6. 最终不可变候选与验证绑定

代码提交 `a786cc72c4b7880ae8ed35de6a9c0ae788f38a82`，Git tree
`4c665480722e60366fb735f3f29e3045f3fb416d`。只提交本轮 23 个 owned path；报告与证据为后续文档提交。
独立 validation worktree：
`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar62-parallel-full-20261021-a786cc72`。
全新 target：`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-parallel-20261021/target-full-final`。

[full-binding.json](assets/cpar62-parallel-20261021/full-binding.json) 明确列出 18 项继承覆盖文件、
原始／应用／candidate blob hash、mode 和 symlink。此次是候选＋明确覆盖层的 Full；bare-tree Full
未另行执行。源码工作树、validation Full 前后及公共专项／实际 CLI 后的 7206 项输入清单完全相同，
SHA-256 `c2add60c02b5e847d2578cf84058f9c101c48a0d90812b277bc6db726b211f34`。

| 绑定 | SHA-256 |
| --- | --- |
| Cargo.toml | `1b1a37ce8d8fcafa51d379ffae2ff33fff091c5a48c0f3458e47b99c12b22355` |
| Cargo.lock | `36564c0219707741d79ecb37d9fc4ee620ce2fe6801fdbe09b32fb4c8687c858` |
| rust-toolchain.toml | `8e390d6a0838315f972690f46ef8bae8b7ecc9ee6c1ed70140ef852869c2482e` |
| Full 原始日志 | `ed1d52f1a7f25f015034403b94464c748d7302f9e8961e31489180197b265a48` |
| 最终公共专项日志 | `2feb96d255c943a47a267378c9e28dd61efefbb6624c7a2616745e08606b5639` |

最终命令均在上述 validation worktree，显式设置本轮 CARGO_TARGET_DIR；Full 另设置
CHECK_REPORT_PATH 指向本地 evidence/full-check.md：

```sh
./scripts/check.sh full
cargo test --locked -p gateway --all-features public_build_parallel_ -- --nocapture
python3 scripts/acceptance/cpar62-cli.py --require-prompt-release --require-codex-success --output <new-OS-temp-directory>
```

[Full 逐项回执](assets/cpar62-parallel-20261021/full-check.md) 为 45/45 PASS，596.63s、exit=0；
[Full 汇总](assets/cpar62-parallel-20261021/full-summary.json) 为 1635 passed / 0 failed / 17 ignored。
覆盖 C1–C6、普通 Responses/Pi、serial、失败、取消及项目 required checks；ignored 不冒充已运行。
实际 CLI 另行显式运行。rustc/cargo 1.97.1，Node 26.8.2、npm 11.19.1、Ruby 2.6.10；
RUST_TEST_THREADS、RUSTFLAGS、CARGO_BUILD_JOBS、test opt override 均未设置。
Clippy、RustSec、dependency policy、crate boundaries、source policy、doc links、contract refs、
secret scanner 和 whitespace 均 PASS。

[final-public-verification.json](assets/cpar62-parallel-20261021/final-public-verification.json)
记录 Full 和公共专项各自实际 gateway binary/hash；workspace feature unification 的 binary
与 package 专项不同，未把 binary hash 冒充相同。
[validation-inputs-binding.json](assets/cpar62-parallel-20261021/validation-inputs-binding.json)
将全部源码输入、manifest/lock、原样 runner 及 CLI binary 关联到同一代码候选。
后续报告提交只作静态检查，不把代码 Full 改绑到文档提交。

## 7. 原样客户端隔离回归与剩余条件

沿用未修改的 `scripts/acceptance/cpar62-cli.py`，启用两个严格 flag。
macOS Seatbelt 对 IPv4／IPv6 非 loopback、真实 home 读取、临时目录外写入均实际返回 EPERM。
独立临时 HOME／CODEX_HOME／Pi 配置仅用合成账号和 loopback；未读取真实凭据，未调用 Provider。
客户端安装 12564 个文件／link 前后比较一致，only-my-pi 未执行。
[actual-cli-summary.json](assets/cpar62-parallel-20261021/actual-cli-summary.json) 记录全部版本、
可执行文件 hash、隔离探针、场景 accounting、取消时间线与原始回执位置。

Codex shell version 为 0.161.0，实际 native binary hash 为
`12ac11d2c7eee27cfae34393986d7b7c9ed0dea537cb749831cdd7033893e6de`，与安装冻结清单相符。
本机没有 optional platform npm package，launcher 使用其既有 vendor fallback；没有替换 executable。
Pi shell／declared-bin 均 0.84.3，Node 为 v26.8.2。

- **Codex 严格成功 FAIL**：实际 CLI exit=1；没有回答工具任务和后续问答，没有五次增量 200、
  五次独立接收及准确 accounting，没有两个正确关联结果和 result.txt。六项判据全 false。
  公开 HTTP 400、ClientRequestError、receiver/Attempt/Usage/ledger=0，只是发送前拒绝证据。
- **Pi 既有受控回归 PASS_BOUNDED**：三个 turn 均 stop，五次 HTTP 200、receiver／Attempt／Usage／
  ledger 均 5，真实工具产物 result.txt 精确为 `42\n`。失败为 error、1 Attempt/Usage/ledger、
  无伪 completed；Pi 对失败 usage 仍显示全零，持久化准确 token 20/7，既有限制未改。
- **Pi 同 PID 取消**：真实 delta → lease=1 → abort → 独立 peer close=1 → lease=0；观测上界 8ms，
  fast probe 通过，观测结束与清理之前 PID 21020 仍存活。没有以进程退出或 server stop 代替释放。
  取消没有可确认的 Usage；标准行计数 0 不等于 Provider 零用量。

underlying preparation test exit=0 与外层 strict runner exit=1 分开：前者包括旧 expected-400 oracle，
后者要求真实 Codex 成功，不能互相覆盖。此次公开请求原样带 parallel=true、multi_agent_v1 namespace、
web_search、client_metadata 和三条非 assistant 输入 id；见
[field observations](assets/cpar62-parallel-20261021/actual-codex-field-observations.json)。
字段存在不证明某一个检查先触发，泛化 400 也不足以锁定单一根因。
现有 actual-CLI 场景仍调用未组合资格的 lifecycle_fixture，namespace/search/C6/parallel 未共同装配，
没有在本轮将其旧拒绝 oracle 改写成未完成的正向资格。

**进入下一步的本地协议软件前提已具备。** C1–C6 与 ParallelTools 的公共合成组合正例已通过；
下一步可以在相同隔离下装配完整独立资格，保留原样 Codex 全部字段和严格六项判据，使用真实
exec_command/write_stdin 工具闭环、相关后续轮、失败与取消来做组合正向验收。
这项 actual-Codex 正向装配与验收本身尚未完成，不能从公共合成正例或 Pi serial 结果推断成功。

仍独立未解决：固定 Build source→release→serving profile 1.0.50 的字段及 ParallelTools 资格；
#61 native success companion／成功材料的独立证据；整票 #62 的实际客户端组合正向与其他验收条件。
真实 Provider、远端 CI、发布均 NOT_RUN。费用 UNKNOWN；cost_in_usd_ticks NOT_ADMITTED。

## 8. 审查和证据交付

[independent-reviews.json](assets/cpar62-parallel-20261021/independent-reviews.json) 为两个干净、
单次只读子代理对 `70226458..a786cc72` 的 23 path 独立审查：Standards hard violations=0、
actionable smells=0；本轮 Spec missing/partial=0、wrong implementation=0、scope creep=0。
主代理点验关键资格与完成屏障，最终验证由主代理负责；子代理没有修改代码或替代运行验收。
Standards／本轮 Spec PASS 不覆盖整票 #62 OPEN/BLOCKED。

[evidence.tar.gz](assets/cpar62-parallel-20261021/evidence.tar.gz) 保留 RED、中间失败、最终 Full、
公共专项、原样客户端受控观测、冻结官方材料及逐文件输入清单。
[archive-manifest.json](assets/cpar62-parallel-20261021/archive-manifest.json) 逐成员独立读回与 hash，
并对解压内容和明文 assets 使用项目原 secret regex 扫描。
首次扫描命中输入清单中旧报告文件名的 `sk-` 子串；核实为路径误报，
仅按确切 member／行号／整行字节允许，并在 manifest 保留命中、原因与行 hash，未放宽 regex。
归档不含真实凭据、原始请求 headers／客户端配置、SQLite、安装文件、巨大 binary 或继承 dirty patch。
完整原始材料及 binary 留在本地
`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020/output/cpar62-parallel-20261021`；
实际 CLI raw receipts 位于 summary 中记录的独立 OS temp 目录，未清理。
报告／traceability／assets 的后继文档提交只包含本轮增量，原 18 项内容继续保留。
