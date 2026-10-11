# CPAR Chat 输出上限修复、六组合本地验证与候选准备

本轮输出上限修复和限定本地验证 **PASS**；产品干净检出 **Full 45/45 PASS**；最终独立 **Standards complete PASS／Spec complete PASS**。原来剩余的 Chat JSON/SSE、Messages JSON/SSE 各完成基础 9/9 本地预演，Responses JSON/SSE 各完成受影响回归 9/9。全部使用合成账号和受控本地上游，真实 Provider 请求 **0**。

Linux ARM64 **候选未就绪／BLOCKED**：现有离线镜像已经完成 release 编译，但仓库要求的 `build-release.sh` 后续调用 Ruby 规范化脚本时，因镜像没有 Ruby 而退出 127。源码归档、固定配方、两次失败回执及复建命令已经准备；没有提取或签发新的 Linux binary/manifest。旧候选保留。#47、#57、#9 继续 OPEN；本地通过不关闭整票。

## 1. 当前基线、归属与保留项

本轮起点 `af7480b2da3edffc324d9a42d81f7d0990f50358`，分支 `codex/prism-v4-delivery`。实际核对了 Git、工作区和 GitHub；衔接 SHA 没有被用作回退指令。[基线](assets/cpar-output-limit-20261011/baseline.json)记录三票均 OPEN，以及旧产品三个定位文件与 `1b6023f52f4c00ea6d8296524d306c141e4edae7` 字节一致。主代理亲读适用规则、跨边界日志、上批报告、两个协议契约、ADR-0100 和相关基础文档。

本轮提交按实际作用分开：

| 作用 | 提交／证据 |
|---|---|
| 共享 router 现代输出上限、冲突验证、Rust 用例与契约 | `b1815a007c0fafad884fbbe4cf0bf06ea87754b5` |
| 同链 generic Channel Pin 源协议 namespace 修复；最后一笔产品行为改变 | `beb796a1cbc59aeae43e4ef3bf4933b4357aa460` |
| 产品交付源码；正式本地执行适配、用例与 ADR | `abd454313f6a3707d6af5a5d653b731e2e54b08b` |
| 候选配方补齐源码归档的结构性 Git 根 | `42a8ed3a031ca5d4b26b7b106a225a0598c36006` |
| 最终实施／工具 HEAD；本地证据读取实际内嵌 source | `fe73140d64f7ed36afdfa86900d8546286e9ed09` |

后两笔不改变 Rust、Cargo 或产品行为。后续报告及 GitHub 回执提交是文档证据，不改变上述审查和产品身份。[本地源码绑定](assets/cpar-output-limit-20261011/local-source-binding.json)记录实际 macOS 二进制 SHA256 `9bd53fdad88ae60f0d9a4e729588be4e982362b276b3f1118706836f885d5d67`，其唯一完整内嵌 source 是 `abd45431…`，而最终执行工具 checkout 是 `fe73140d…`。

[保留核验](assets/cpar-output-limit-20261011/preservation.json)：任务起始保存的 **37,255/37,255** 个已有脏路径字节未变，包括修改中的 `AGENTS.md`、未跟踪 `docs/agents/issue-tracker.md` 及所有无关文件。没有 reset、清理或宽范围暂存；只提交本批拥有的路径。未改前端、授权流程、数据库 schema、重试策略、计费算法或 capability 声明。

## 2. RED／GREEN 与参数裁定

[旧真实失败](cpar-build-remaining-20261011.md)及其 [capture 摘要](assets/cpar-build-remaining-20261011/chat-json-failure-capture.json)保持原状。本轮在实际网关进程公共 `POST /v1/chat/completions` 上，使用受控 `model-responses` 映射、合成消息、假凭据、`max_completion_tokens=4096`、`stream=false` 复现相同控制结构。

| 证据 | HTTP | Request／request_finished | Attempt／Usage | 独立受控上游接收 |
|---|---:|---:|---:|---:|
| [RED](assets/cpar-output-limit-20261011/public-red.json) | 400 | 1／1 | 0／0 | 0 |
| [最终 GREEN](assets/cpar-output-limit-20261011/public-reproduction-green.json) | 200 | 1／1 | 1／1 | 1 |

RED 的错误是 117 字节 `ClientRequestError`，响应摘要 `1ef3f6138442e24586fe0fa90bec1d7da616afa1b458fb34278c9adb9d39049e` 与失败 capture 相同。结合实际事件增量、接收数和代码链，拒绝位于 router 的协议准入、凭据选择之前。响应没有 x-request-id；这一单次受控复现使用串行窗口事件增量，不能冒充成功基础场景的精确 Request-ID/Usage 关联。GREEN 另校验实际上游请求收到 `max_output_tokens=4096`。

Chat codec 原本已经保留 `openai.chat.max_completion_tokens`。修复位于拥有保真投影语义的共享 `protocol_transform.rs`，沿用既有转换框架，没有删参数、改成旧参数或为单一渠道增加平行转换器。

2026-10-11 核对的官方 [Chat create](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create) 和 [token counting](https://developers.openai.com/api/docs/guides/token-counting) 说明现代 Chat `max_completion_tokens` 和 Responses `max_output_tokens` 都约束含 reasoning 的生成总量；旧 `max_tokens` 已弃用且存在模型限制。官方资料不提供双字段优先级。具体本地严格规则写入 [ADR-0104](../adr/ADR-0104-chat-output-limit-projection.md)、[BC-PROTOCOL-008](../contracts/BC-PROTOCOL-008-openai-chat-completions-codec.md)、[BC-ROUTER-004](../contracts/BC-ROUTER-004-protocol-transform-admission.md)：

| 输入／路径 | 本轮规则 |
|---|---|
| 单独现代 Chat 字段，原生 Chat | 保留字段名和值 |
| 单独现代 Chat 字段，Chat→Responses | 精确投影到 `max_output_tokens`，不丢字段或改变值 |
| Responses→Chat | 输出 `max_completion_tokens`，保持现代 total-output 语义 |
| 单独旧 Chat `max_tokens` | 原生保留旧拼写；保留项目既有兼容切片中的正整数映射，不新增“所有模型等价/都接受旧参数”的承诺 |
| 新旧 Chat 字段同时出现，同值或不同值，包括其中一个 null | 拒绝；不选择最大、最小或覆盖其中一个 |
| 外来输出上限 namespace／字段碰撞 | 拒绝，包括 `max_output_tokens`、`openai.responses.max_output_tokens`、`anthropic.messages.max_tokens` |
| OpenAI 字段缺失 | 保持缺失；Messages 原有必填规则不变 |
| 正整数 | 保留项目现有 `1..=u64::MAX` 表示域；实际测试 1、4096、u64 最大值。这不承诺 Provider/model 接受如此大的上限 |
| null、0、负数、小数、`4096.0`、字符串、布尔、数组、对象、超过 u64 | HTTP400、执行前拒绝；null 不默认成缺失 |

原生、跨协议和 exact passthrough 都经过既有严格准入。保留正式验收请求的 `max_completion_tokens=4096`；SSE 保留 `stream_options.include_usage=true`。

同链预演还暴露 generic Pin 在“源协议标识＋目标协议字段”之间的不一致：[Pin RED](assets/cpar-output-limit-20261011/generic-pin-red.json) 是 Attempt0／upstream_sent=false。修复让 generic probe 按其源协议构造上限，再交 router 投影；原生 Build 和既有 Codex probe 策略不变。Pin 仍然不能证明后续公共请求的参数或历史。

## 3. 本地过程、基础矩阵与原生覆盖

复用实际网关进程、SQLite 控制面、原有 loopback TLS 上游和正式 `cpar-batch-c-live.py --new-build run --execute`／`audit`。仅补本地 basic collector 接入及私有原字节 capture；使用既有证据 collector，保留正式断言、3/2/4 历史和工具结果。每轮执行前后核对实际进程身份；local collector 不允许作为 LIVE_NEW_BUILD／PRODUCTION 资格。

单账号、单 candidate、max_attempts=1、后台刷新与目录任务禁用；macOS sandbox 将网关出口限制为 loopback。独立非 loopback 探针得到 OS 拒绝。Provider 在鉴权和解析之前计数，所以拒绝样本的接收零值不是从 Attempt 推断。[最终公共回执](assets/cpar-output-limit-20261011/public-green-final.json)共 **190/190 PASS**、独立受控接收 **91**。

[参数边界](assets/cpar-output-limit-20261011/parameter-boundaries-final.json)共 **132** 个公共 HTTP 案例，覆盖 Chat→Chat／Chat→Responses 和 JSON／SSE：**28** 个合法案例精确保留实际上游控制，**104** 个拒绝案例全部 HTTP400、Attempt0、Usage0、上游接收0。另有 Responses→Chat JSON/SSE 逆向公共验证，实际收到现代字段 4096 且没有旧字段。

[最终矩阵](assets/cpar-output-limit-20261011/matrix-final.json)：

| 组合 | 文本多轮 | 单工具调用及结果 | 连续工具调用及结果 | 新本地 Pin | 基础结果 |
|---|---:|---:|---:|---:|---|
| Chat JSON | 3/3 | 2/2 | 4/4 | 1 | PASS 9/9 |
| Chat SSE | 3/3 | 2/2 | 4/4 | 1 | PASS 9/9 |
| Messages JSON | 3/3 | 2/2 | 4/4 | 1 | PASS 9/9 |
| Messages SSE | 3/3 | 2/2 | 4/4 | 1 | PASS 9/9 |
| Responses JSON | 3/3 | 2/2 | 4/4 | 1 | PASS 9/9 |
| Responses SSE | 3/3 | 2/2 | 4/4 | 1 | PASS 9/9 |

上述六组合均在通用 Responses fixture 上经过实际网络、公共协议转换和持久事件链，保留终态、SSE 结束、精确 Usage／ledger 关联及正式模型/历史审计。合计54个基础请求＋6个新Pin。各组合另经正式 audit；basic PASS，同时整体 formal **NOT_RUN／exit 2**，因为本轮基础矩阵没有补齐所有声明扩展和故障验收。

原生 Build 单独覆盖：新增 `build_chat_completion_limit_reaches_native_transport_and_collisions_do_not` 使用实际 Actix 公共应用装配、Build provider dispatch 和注入的原生 transport，JSON/SSE 各4轮共8个成功请求；非法／碰撞请求不增加原生 peer 接收或 Attempt。它在最终产品 source 的 Full 中通过。该证据是**原生装配的进程内 transport 测试**，不是通用 fixture 的真实进程网络证据，也不是与真实 Build 服务通信。原有跨协议 native factory 用例同步校验现代输出字段。

共享 fixture 回归另行运行：[354/354 PASS](assets/cpar-output-limit-20261011/shared-public-final.json)，受控合成接收52；[扩展回执](assets/cpar-output-limit-20261011/shared-extension-final.json)保留各类 PASS、预期 FAIL/BLOCKED 的原断言。它不增加真实扩展覆盖。最终定向 Python 回归 **54/54 PASS**。[定向日志索引](assets/cpar-output-limit-20261011/targeted-validation.json)保留中间 FAIL：初始 collector mock 回归、旧字段预期、generic Pin 及硬编码旧 source 的 `wrong_pin_runtime`。

新源码触发旧 fixture 的固定 `1b6023f5` 身份保护后，最小适配改为要求二进制内唯一完整40位源码 SHA；outer receipt 分列 `source_sha` 和 `checkout_head`。没有放宽 Pin/runtime 身份断言。[修正前原始回执](assets/cpar-output-limit-20261011/public-before-identity-fix.json)保留原先外层错标 checkout 的结果；最终回执及六个 bundle 均实际绑定 `abd45431…`。原始合成 bundle／upstream capture 留在本批0700/0600私有目录，仅 [摘要索引](assets/cpar-output-limit-20261011/private-capture-index.json)进入提交和公开结果。

## 4. 产品门槛与独立审查

`CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=4 bash scripts/verify-clean-checkout.sh abd454313f6a3707d6af5a5d653b731e2e54b08b full …` 在独立干净 clone、独立 target、release 环境变量 unset 下完成。[45项报告](assets/cpar-output-limit-20261011/full-check.md)、[source/日志回执](assets/cpar-output-limit-20261011/full-execution.json)记录退出0、534.81秒。包含格式、Clippy、workspace Rust tests、SPA、serve、多轮与流式回归、协议契约、secret scan、依赖策略和 RustSec 检查。该 Full 属于 `abd45431…`；其后 Python 工具身份适配单独由54回归、190／354实际进程检查及增量审查验证，不能改称 Full 在 `fe73140d…` 上运行。

两轴遵循 code-review：初始 `af7480b2…42a8ed3a` 全部23路径；最终 `fe73140d` 四路径增量＋19路径一致性复用。[冻结](assets/cpar-output-limit-20261011/review-freeze-final.json)、[两轴原意分列](assets/cpar-output-limit-20261011/review-final.md)、[结构化回执](assets/cpar-output-limit-20261011/review-final.json)可审查。

| 轴 | 最终结论 | 独立边界 |
|---|---|---|
| Standards | complete PASS；硬问题0，继承非阻断 heuristic1 | 参数保真、ADR/契约、范围和最终文件哈希；测试探针重复不扩展为无关重构 |
| Spec | complete PASS；未闭合问题0 | 参数语义、公共路径、零发送、正式基础流程和六组合 source/binary/capture 摘要 |

两份补充探子曾因自身辅助检查错误返回 BLOCKED（未跟踪规则的历史 Git blob 不存在；digest 脚本语法错误）；历史回执原样保留，新的独立一次性探子分别完成任务起始规则哈希、6/6 bundle 摘要、6/6 upstream 摘要核验后才签发 complete PASS。审查不替代 Full、候选或真实服务证据。

旧验收工具 `bb30f048`／`dbfb977a` 的审查只按文件哈希和范围复用。[108路径比对](assets/cpar-output-limit-20261011/historical-tool-review-reuse.json)中103路径未变，5条旧路径已变，另有本批新增内容；变化部分由本轮审查覆盖，不能笼统宣称旧工具全部哈希未变。

## 5. Linux ARM64 候选准备与具体缺口

[候选准备回执](assets/cpar-output-limit-20261011/candidate-preparation.json)固定产品 source `abd45431…`、Rust1.97.1、`aarch64-unknown-linux-gnu`、干净 `git archive` SHA、[配方](assets/cpar-output-limit-20261011/candidate.Dockerfile) SHA和既有镜像 digest。命令使用既有 `colima-cpar-acceptance-20261010`、`--pull=false --network=none --platform linux/arm64 --target builder`；源码全部来自新归档，旧镜像仅作依赖/build缓存来源。

第一次因归档没有 Git 根、release 脚本定位仓库失败；配方仅增加结构性 `git init`，真实 revision 仍显式绑定归档 SHA。第二次离线 `npm ci` 和 `cargo build --release --locked --workspace` 完成，随后 `scripts/normalize-macho-uuid.rb` 找不到 Ruby，release 脚本退出127、Docker build失败。既有带标签工具镜像也没有 Ruby；未安装包、未引入新服务或修改生产构建脚本。一次辅助镜像盘点误用了未带标签名称，自动拉取 latest 失败；无镜像安装，后续探针强制 `--pull=never --network=none`，两个正式构建均禁用构建网络。

新 Linux binary SHA／manifest 文件 SHA／manifest payload SHA 均为 **null／未生成**；不拿已编译的中间步骤或旧二进制补位。本地 macOS 验证二进制独立保存在 `output/cpar-output-limit-20261011/local-macos/gateway`，不能当作 Linux 候选。

可审查的后续构建准备位于 `output/cpar-output-limit-20261011/source-abd45431.tar`、同名解包源码、固定 Dockerfile 及保存的完整命令。缺口是**为既有 Linux ARM64 builder 提供可运行 Ruby（含脚本所用标准库）**，然后重新完整执行 release 流程、提取 ELF、验证 embedded source/target/toolchain 并生成两文件 candidate manifest；完整 P12 发布包仍是独立门槛。本轮不安装该缺失环境，也不借用远端。临时启动的原有 Colima profile 已恢复任务起始 Stopped 状态。

没有新候选上传、远端实例启动、真实账号操作、push、发布或部署；远端 CI/运行验收 NOT_RUN。

## 6. 历史真实证据与下一批可审查范围

历史产品 `1b6023f52f4c00ea6d8296524d306c141e4edae7` 的 Responses JSON 9/9 和 Responses SSE 9/9 继续保留，历史合计2/6组合、6/18场景。Chat JSON Pin成功后首个公共请求HTTP400及停批、其余三个组合NOT_RUN也全部保留。旧真实证据只属于原 source/binary/manifest；新产品真实基础覆盖目前 **0/6**，真实扩展仍未完成。

后继源码改变共享投影与 generic Pin，并产生不同二进制身份，故 Responses 两组合也需要新版本复验。对 Grok Build 的原生 Responses→Responses 直通，本轮没有宣称发现新的行为回归；要求复验来自新交付身份和共享变更后的端到端证据不能转移。

| 下批组合 | 理由 | 新 Pin 上限 | 基础请求上限 |
|---|---|---:|---:|
| Chat JSON | 原失败组合，保留现代4096参数复验修复 | 1 | 9 |
| Chat SSE | 原未执行组合 | 1 | 9 |
| Messages JSON | 原未执行组合 | 1 | 9 |
| Messages SSE | 原未执行组合 | 1 | 9 |
| Responses JSON | 新产品/二进制身份，重新建立 source-bound 证据 | 1 | 9 |
| Responses SSE | 同上，并保留历史 SSE FAIL 来源 | 1 | 9 |
| 合计 | 六个独立新组合身份；每组3＋2＋4 | **6** | **54** |

下一批可审查的推理发送**预算意图上限60**（原剩余四组合40＋Responses新版本复验20），不是本轮真实调用授权，也不是预计实际费用。应在候选成功准备后，另行固定 source/binary/manifest、账号连续性/lease、route/model/出口、有效期、能力投影及每组新Pin；按实际运行重新记账。保留失败/未知/证据矛盾立即停批规则，不自动重试，不释放失败占用，不延续旧余量38，不包括扩展、其他渠道或目录/配额查询。

| 计量口径 | 历史真实累计 | 本轮真实新增 | 下批准备口径 |
|---|---:|---:|---|
| 预算意图 | 33 | 0 | 提议上限60，尚未执行 |
| 网关／Pin确认发送 | 32 | 0 | 待独立回执记录，不能预填60 |
| 独立 Provider 接收 | UNKNOWN | 真实请求0；本地143次接收另计 | 未观察前为UNKNOWN |
| 实际费用 | UNKNOWN | 未发生真实调用；没有服务账单查询 | 不从预算/Usage成本null推断 |

本地143次是最终190检查的91次＋共享354检查的52次，属于两个合成执行实例，不汇入任何真实批次账本；开发过程中的早期本地 RED/GREEN 也不进入真实计数。

## 7. GitHub 同步

本轮仅向既有 [#47进度评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57进度评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9进度评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)追加脱敏结果，保留旧正文作为完整前缀，不改 issue body／AC／title／labels／assignees／依赖／OPEN 状态。写后使用独立 GET 逐项比对，回执单独归档。代码和报告只在本地提交，未 push。
