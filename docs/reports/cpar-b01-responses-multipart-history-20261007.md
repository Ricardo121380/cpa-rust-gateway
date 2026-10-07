# B-01 Responses 多文本片段同协议历史回传修复

日期：2026-10-07，Asia/Shanghai。**B-01/P2 已 RESOLVED_LOCAL**：公开 JSON/SSE 多轮均 RED → GREEN，定向／静态检查、新源码 Full 和独立 Standards／Spec 全部 PASS。本轮要求的本地验证及评审已完成；项目发布保持 **BLOCKED**。

## 身份、基线与契约

固定起点为 `ee521c82dad311f8e82e15f0f63f1e4f5189b35f`，分支 `codex/prism-v4-delivery`。此前产品修复 `42bc473f2eba8c34484b54aaf09fff455aa13c9c` 仅处理 A-01；核对 HEAD 和后续日志后没有 B-01 修复。范围是 [2026-10-05 B-01/P2](assets/cpar-release-readiness-20261005/review-results.md)、[#27/CPAR-18](assets/cpar-b01-20261007/issue-27.md)、[#31/CPAR-22](assets/cpar-b01-20261007/issue-31.md)与[父规格 #9](assets/cpar-b01-20261007/issue-9.md)。[scope](assets/cpar-b01-20261007/scope.md)冻结测试边界和授权；四份 GitHub 快照均只读，工单保持 OPEN。

主代理亲读 AGENTS.md、issue-tracker、相关最新跨界日志、原复核与 A-01 报告、CONTEXT、适用执行门槛、协议准入及原生表示／保真 ADR 和契约；修改前完整阅读确切函数和受影响测试。原用户修改及无关文件保留。

最终修复源码与 Full 对象为 **`939028d63519126f16252c3e72d3e1d5ace7ca75`**，包含必要代码、测试、契约及开发 RED/GREEN 检查点资料。其后只提交评审、Full 和状态收口文档，产品／测试源码不变。开发运行当时的 HEAD 为起点加工作区改动；四份文件的 SHA256 与已提交源码逐一匹配，随后不可变源码 Full 重跑受影响测试，没有把旧候选的结果移到新提交。

## 缺陷触发与最小修复

实际路径：`POST /v1/responses` → `protocol_openai_responses::decode_request` → 有序 `CanonicalMessage.content` → route/factory `project_protocol_request` → `canonical_rejection` → `validate_target_message` → `valid_responses_content` → `valid_assistant_tool_history`。旧 helper 只允许索引零处的首个 Text；后续 Text 只对 Messages 放行。原生 JSON/SSE 输出能保留两片，但客户端把合法 output-message 作为同协议 history 带入第二轮后，第二片使内部准入返回 `ToolHistoryUnsupported`，公开结果为 400 ClientRequestError。

生产修复只在 Responses 专用 `valid_responses_content` 中准入纯 Text 数组。此前的空内容拒绝、逐消息／逐片闭集校验、角色和能力门槛仍先执行；reasoning-only 和已有工具历史仍经原路径。共享 `valid_assistant_tool_history` 保持原样，避免扩大混合 text/tool grammar。目标编码仍逐片 push 并以 content 数组输出，没有拼接、合并或移除元数据，没有新依赖／Canonical／Schema／能力承诺。这恢复 ADR-0100／0101 已规定的 native Responses 历史表达，契约只补回归证据。

## JSON/SSE RED → GREEN

[JSON RED](assets/cpar-b01-20261007/red-json.md) 与 [SSE RED](assets/cpar-b01-20261007/red-sse.md) 都在旧产品源码上执行新公开回归：第一轮 200、完整 output 结构通过，实际返回 items 加入下一轮 input 后失败为 **400**，上游 handoff 总数仍为 **1**。退出码均 101。[精确回执](assets/cpar-b01-20261007/red-receipts.json)记录起点、命令、时间、日志与源码哈希；[RED 测试 patch](assets/cpar-b01-20261007/red-tests.json)保留当时准确测试。初次 phase-only 输出失败不计作 B-01 RED，见[独立观察](assets/cpar-b01-20261007/additional-observation.md)。

两条主回归使用临时 SQLite、合成账号、生产 route compiler／Client Key 鉴权／leased executor／provider builder、实际公开 Actix handler 与真实 loopback HTTP 上游。下游由 Actix test service 分派，没有运行浏览器或部署进程。第一轮 input 为合法 user message；第二轮只复用实际返回 `output` 中可回传的 message/items，保留 id/status/role/content，不把 response-level model、usage、error 等外壳塞入 input。官方 [input union](https://github.com/openai/openai-python/blob/main/src/openai/types/responses/response_input_param.py)与 [output-message parameter](https://github.com/openai/openai-python/blob/main/src/openai/types/responses/response_output_message_param.py)允许此消息形式；null/empty logprobs 的当前原生兼容边界依据 ADR-0101，不新增非空 token metadata 支持。

GREEN 不仅断言 200：公开完整 output 等于独立字面量，含一条消息和两个不同有序 output_text，分别为 `first segment`、`second segment`；两个独立 synthetic URL citation 保持各自片段和索引，logprobs 分别为 null／空数组。loopback responder 在实际接收 HTTP body 时记录请求；第二轮上游实际 input 等于完整的 user → 双片 assistant → next-user 结构，消息 ID／status、内容、数量、顺序和每片元数据一致，选定 upstream model 与 stream 模式也一致。正常两轮恰有两次实际接收。

SSE 上游发送 created/in-progress、item/part added、每片两次 delta、annotation added、text/part/item done 和 completed，并带单调 sequence_number。客户端断言四次 delta 的 content_index／顺序和两个 completed part 的完整结构，唯一 response.completed 的完整 output 再进入下一轮；不以最终拼接文本相等代替此闭环。现有三种原生 Grok factory 的 JSON/SSE metadata 回归扩为两片，保留 phase、reasoning 层级及每片 citation/null/empty logprobs，同时维持不可保真 Chat/Messages 输出桥接拒绝。

## 定向验收与门槛

完整命令／输出见 [execution](assets/cpar-b01-20261007/execution.md)；[开发验证](assets/cpar-b01-20261007/development-verification.json)记录每条运行及四个 Rust 文件 SHA256，源码提交后核对绑定。下面均为本地受控执行，不是真实 Provider 验收。

| 检查 | 结果 | 证明范围 |
| --- | --- | --- |
| public factory 回归 `batch_c::` | 10/10 PASS | JSON/SSE 双片闭环、phase 原生回传、单片段、三协议四轮工具与不可保真拒绝 |
| 新严格反例 | 20/20 PASS | 10 种 × stream false/true；均 400 ClientRequestError，实际上游接收、handoff、Attempt 均为 0 |
| router protocol transform | 30/30 PASS | reasoning-only、工具 item ID／结果关联、字段／角色／能力／桥接规则 |
| core canonical request | 7/7 PASS | pending-tool 与 reasoning-only 不能越过未返回工具结果 |
| Chat reasoning-only 回传 | 1/1 PASS | 既有相邻语义 |
| 公共三协议孤立工具结果、Responses 非字符串 item type | 1/1、1/1 PASS | 请求阶段拒绝，既有零执行断言 |
| targeted locked Clippy、workspace fmt check | PASS | 受影响 gateway/router 全 target／feature 与格式 |
| source policy／crate boundaries／contract references／secret scanner／staged whitespace | PASS | 334 Rust 文件、21 packages、130 测试引用；没有忽略或放宽检查 |
| 不可变源码本地 Full | 45/45 PASS | `./scripts/check.sh full`；1461 Rust tests passed、0 failed、12 原有 ignored |
| 独立 Standards／Spec | PASS／PASS，各 COMPLETE 19/19 | 固定起点到最终修复提交；两轴均无可行动发现，未扩增为整个 B 批 |

严格反例涵盖非法 message role、未知 message 字段、不完整 item status、unsupported phase、第二片未知字段／非空 logprobs／倒置 citation 范围／不可表示 refusal、孤立工具结果，以及双片 assistant 越过 pending-tool。工具关联和逐字段验证未放宽；synthetic citation 不触发 URL/file 读取，没有伪造签名或真实证据。

新源码 Full 在干净 detached checkout 执行，2026-10-07 13:44:00–13:48:58 UTC，耗时 298.103 秒、exit 0；运行前后工作区均干净，四份 Rust 文件哈希匹配。只复用编译缓存，不使用主工作区未跟踪源码。[45 步记录](assets/cpar-b01-20261007/full-check.md)、[精确回执](assets/cpar-b01-20261007/full-receipt.json)和[原始输出](assets/cpar-b01-20261007/full-output.json)可复查。原始输出用 JSON text 字段保留所有字节及尾随空白，解码后的 SHA256 对应回执；1461／0／12 是 Rust tests 步骤全部 test-result 汇总，不把脚本检查数混入。依赖策略有 14 条 duplicate-version warning，policy 和 RustSec audit 均 exit 0，未调整忽略项。Full 中带 production/live 名称的脚本是本地 oracle／mock 检查，不是生产执行；真实网关多轮 harness 使用 owned loopback TLS mock，真实 Provider inference calls 为 0。

[两轴原文](assets/cpar-b01-20261007/review.md)和[结构化覆盖](assets/cpar-b01-20261007/review.json)分别记录 Standards 静态合规、Spec 静态行为符合性：各 COMPLETE 19/19、PASS、零可行动发现。两名探子只读、单轮、fork_turns=none，主代理等待全部终态后核定结论；审查者不执行测试，运行证据由主代理负责。后续仅文档收口由主代理静态复核，不扩大上述覆盖范围。

Task 指定的定向／review／fmt／secret 和公开边界 Full 门槛均完成；按项目规则，本地修复证据相当于 `LOCAL_PASS_PENDING_PHASE_GATE`，不改写历史 Phase 计划。远端正式 Phase Gate 属于后续批次冻结，本轮没有新 Phase tag、推送或 GitHub Gate，不能把本地结果称为整个 B 批／Phase DONE。最终文档 docs 七步、staged secret 与 whitespace 检查均 PASS，结果归档于本轮 verification。

## 本地交付与剩余边界

本地原子源码提交已完成，后续文档提交收口评审、Full 和状态；[本地 #57 补充](assets/cpar-b01-20261007/issue-57-evidence.md)仅将 B-01 改为 RESOLVED_LOCAL，保留原发现、失败回执和其他问题实际状态。A-01 既有 RESOLVED_LOCAL 保留。原 13 项 P2 中尚有 **11 项**：A-02–08、B-02–05；S-01 与 E-01–05 保留实际状态。C／#57／#9 未完成，#27／#31 全范围验收不由本轮关闭。新增 O-01 phase-only 普通 JSON 准入观察保持 OPEN_OUT_OF_SCOPE，单独记录，不替代或混入原 P2 统计。

[工作区保留核验](assets/cpar-b01-20261007/workspace-preservation.json)确认任务开始时已存在的 4277 个用户修改／未跟踪文件全部哈希不变、无丢失；包括 AGENTS.md。完整原清单仅保留本机，不提交用户无关文件。所有新增测试、账号、引用及网络上游均为合成材料。

原 source80 的签名、制品、Full 与迁移演练保留原绑定；本次没有生成或重签发布候选。浏览器不适用于此纯协议切片；真实 Provider、生产、部署、推送和 GitHub 写入均 NOT_RUN／0。本轮本地完成标准没有遗留项。下一项建议独立处理 **B-02：Responses SSE done 确认事件的闭集与一致性校验**；B-03 输出终态顺序、B-04 用量、B-05 密钥集合、E-05 规格冲突和 O-01 均不在本轮修改范围。
