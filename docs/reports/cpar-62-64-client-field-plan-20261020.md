# #62／#64 实际客户端字段方案与下一实施切片

日期：2026-10-20（Asia/Shanghai）。状态：**REVIEW_DRAFT；未批准新字段准入或首版缩围。**

推荐继续 #62 的 **原生 namespace/function 身份全链路**，先解决实际 Codex 的首个解析门槛。
原生表示与确定的关联检查属于软件工作；固定 Build 服务是否支持该表示是另一项资格。
当前不能声称存在已经满足全部前置、可以直接放行原样客户端的生产修复。
本文给出可开发的准确表示、仍需的最小资格和一项完整后续任务，不以新测试框架或恢复矩阵替代它。

## 1. 当前事实与状态校正

工作树为 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar65-evidence-process-recovery-20261020`。
实际分支 `feature/65-evidence-process-recovery`，本地 HEAD 和 `git ls-remote` 的同名远端均为
`bbe3a53f9163fc55f6eb699a12983fee0c350210`；包含 #65 代码 `5f3c2b4035dca3aa20a74bb086afab1295b19c7d`。
主仓库仍为 #62 的 `098c92a51ac549a38c8f15fd265e0fb379de2c77`，不能将它当最新交付源码。
起始暂存区为空，9 个 tracked 改动、7 个 untracked 文件已逐项冻结内容 hash、mode、symlink；全部保留。
本轮只新增本文和校正 BC-PROTOCOL-009 的历史状态，没有修改生产行为、Accepted ADR 或历史报告。

已读取实际 AGENTS、CONTEXT、domain/issue 规则、cross-boundary 尾部、ADR-0100/0101/0106–0114、
BC-ROUTER-004、BC-PROVIDER-003、BC-PROTOCOL-009，以及指定 #61/#62/#64/#65 交付材料。
主代理亲读基础设计；四个一次性只读探子压缩代码、实际请求及一手语义，承重位置另行点验。

| 当前证据 | 本轮采用的状态；不继承旧文档的过时判断 |
| --- | --- |
| GitHub 当前正文、相关最新评论及原生 blocked_by | #58/#61–67 均 OPEN；#61→#59/#60（两者 closed），#62/#63→#61，#64/#65→#63，#66→#62/#64/#65，#67→#66。没有解除依赖。 |
| #62 最新交付 | [监听器报告](cpar-62-listener-cancel-20261019.md)及[最新远端评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/62#issuecomment-6262886267)：原版 Pi 顺序显式历史工具闭环与受控取消链 PASS；旧约15秒/一秒 FAIL 属旧测试 listener，不能再列当前阻塞。失败用量显示与成功完整 companion 缺口仍在。 |
| #63/#64 | ADR-0114 的 Messages summary 受控实现已存在；[实际 Claude 报告](cpar-64-claude-messages-20261020.md)和[最新评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/64#issuecomment-6282264588)仍仅为观察/准确预拒绝。实际工具、summary 读回、活客户端取消没有通过。 |
| #65 | [进程恢复报告](cpar-65-process-recovery-20261020.md)及[最新评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/65#issuecomment-6284130045)：受控进程恢复切片已交付；整票 PARTIAL/BLOCKED。复用，不重做。 |
| #61 | [定向证据](cpar-61-targeted-evidence-20261018.md)及[最新评论](https://github.com/Ricardo121380/cpa-rust-gateway/issues/61#issuecomment-6251598716)：`cost_in_usd_ticks` 继续 NOT_ADMITTED。没有新增 cost 研究或再次提请批准旧草案。 |

[#58 正文](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58)中的“新留存均未实现”等表述是规格发布时快照；
按上述后续交付理解，不覆写历史。#66 的[候选资格](https://github.com/Ricardo121380/cpa-rust-gateway/issues/66)
和 #67 的真实验收均不能据局部 PASS 推进。标准客户端优先、三客户端均用 Grok Build、native-v1 独立的目标不变。

## 2. 证据定位与解释规则

- **C**：[原样 Codex ingress](assets/cpar-62-independent-20261019/liveness-fixed/actual-cli/codex-standard/ingress.json)，`[0].body`；最新版[交付归档](assets/cpar-62-codex-responses-compat-20261019/verified-evidence.tar.gz)中 `codex-required/codex-standard/ingress.json` 保持相同字段形状。历史实际身份为 Codex CLI 0.161.0，不声称本轮重验当前安装。
- **M**：[Claude 最终实际回执](assets/cpar-64-claude-20261020/actual-cli.json)及[原始归档](assets/cpar-64-claude-20261020/verification-logs.tar.gz)成员 `actual/final/standard/request-0.bin`、`request-1.bin`、`ingress.json`；历史实际身份为 Claude Code 2.1.302。
- **S**：[冻结一手源清单](assets/cpar-62-codex-responses-compat-20261019/public-source-manifest.json)：Codex 源码 `979011409de0a60b52f179721948e65531d26144`，Build 公开源码 `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`，production profile 仍为 **1.0.50**，release/source 对应 UNPROVEN。

“首拒绝”以下是原请求加当前源码顺序的**静态归因**；原 HTTP 响应仅有通用400，没有动态字段栈。
逐字段公共 JSON/SSE 探针另见[Codex controls](../../apps/gateway/src/runtime/batch_c/grok/standard_responses/codex_controls.rs#L81)
和[Claude controls](../../apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/messages/claude/controls.rs#L1)。
它们是合成独立输入，不能写成原样 CLI 删除某字段后的兼容结果。

本轮重新取得 [OpenAI create schema](https://developers.openai.com/api/reference/resources/responses/methods/create.md)，
506646 bytes，SHA256 `98dbc86be77dc477b1b33b862b5edfd334f6d30870e39616bd6c5bc54a20a664`，与 S 的旧快照相同。
下面引用标准 schema 只确定表示，不授予固定 Build 服务资格。

## 3. Codex 字段决策表

| 项目／实际形状与拒绝位置 | 可观察语义与已知上游 | 具体处理方案、归类和契约责任 |
| --- | --- | --- |
| **C1 namespace**：C 的 `tools[4]` 为 `namespace/multi_agent_v1`，含 name/description/tools；5个 children 都是 function：close_agent、resume_agent、send_input、spawn_agent、wait_agent，均 `strict:false`。没有 custom、嵌套 namespace 或实际同名冲突。[decode_tool:627](../../crates/protocol-openai-responses/src/lib.rs#L627) 的 function-only gate 是原样请求首拒绝。 | 客户端必须收到原 namespace 下正确的 function，不能将“网关处理 namespace”解释成网关执行多 agent。标准定义分组与其描述；Build 冻结客户端证明普通 function schema、name/call_id/history，未证明原生 namespace。 | **软件缺口＋固定服务资格缺口**。推荐完整保留有序 namespace 节点、描述及 children，不改名；执行身份为 `(namespace?, name)`，call_id 独立。同组重复拒绝；跨组同名靠完整身份区分。新增闭合 Responses tool-set 表示及执行视图；不能只开 codec allowlist。补 BC-PROTOCOL-009/BC-CORE-002/BC-CORE-003/BC-PROVIDER-003 与新后继 ADR；不改旧 Accepted ADR。原生保真本身无需新的产品取舍，正式 Build 放行仍需资格。 |
| **C2 selector／parallel**：实际 `tool_choice:"auto"`、`parallel_tool_calls:true`。带 namespace 的 forced selector、同名、serial/parallel 组合是派生负例；[控制检查:748](../../crates/protocol-openai-responses/src/lib.rs#L748)、[ToolSelection:12](../../crates/gateway-router/src/tool_controls.rs#L12)仅有 flat named selection。 | auto 允许零或多个已声明调用，parallel 仍须实际 ParallelTools 资格。当前标准 forced-function schema只有 type/name，未建立 namespaced forced selector 的无歧义拼写。 | **软件缺口＋特定 selector 的证据缺口**。auto/none/required 对完整集合实施既有约束；parallel 原值保留，serial 校验不放宽。flat named 只选择真正顶层工具；涉及 namespace 而无标准定位依据时预拒绝。不要发明 `namespace.function` 或新 selector 字段；这不阻碍实际 auto 请求的字段开发，也不宣称其他 selector 已支持。 |
| **C3 调用／结果／历史**：实际首请求仅 developer/user/user，没有工具历史。调用中的 namespace 被 codec 保留，Router [call:679](../../crates/gateway-router/src/protocol_transform.rs#L679)/[result:694](../../crates/gateway-router/src/protocol_transform.rs#L694)拒绝；Build [native whitelist:58](../../crates/protocol-openai-responses/src/native_output.rs#L58)在可执行 ToolCallStart 前拒绝 namespaced output。 | 标准 function_call 有 namespace/name/call_id；当前 create schema的 function_call_output **也可带 name/namespace**，两者可缺失或 null。不能断言实际 Codex 必定回传这些可选成员；尚无成功轮记录。Build 普通调用/结果有对称转换接缝。 | **软件缺口＋上游回传资格缺口**。建立 call_id→完整已声明身份关联；结果以 call_id 关联，若提供 name/namespace须核对并保留其 presence/null，缺失不补造。added/delta/done/terminal确认相同身份、索引和完整对象参数；未知或矛盾拒绝且无执行完成事件。历史按原顺序重编码；不按裸 name 匹配，不恢复或伪造未收到的原生字段。 |
| **C4 client_metadata**：7个 string 键：root_turn_id/session_id/thread_id/turn_id/x-codex-installation-id/x-codex-turn-metadata/x-codex-window-id；最后的 metadata 是 JSON **字符串**。codec保留为 `openai.responses.client_metadata`；Router [root:803](../../crates/gateway-router/src/protocol_transform.rs#L803)的准入集合为空。 | [Codex metadata源:224](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/core/src/responses_metadata.rs#L224)证明关联 blob；实际内含 sandbox/auto-review 等状态。不能由 string-map 形状推断所有值都无控制影响。Build 没有已证明的对应语义；其追踪 header不是映射依据。 | **新产品决定 D1，或原生上游资格缺口**。可由 CPAR明确承担本地诊断关联和完整加密留存；这改变接收方/追踪承诺，不能称 ADR-0107 已批准的响应元数据分离。若要求 Build 消费，则需目标端逐键依据并原样发送。两路均不改成标准 metadata、不解析再改写内部字符串、不授予 owner/路由/审批权限。 |
| **C5 web_search**：实际精确为 `{type:"web_search",external_web_access:false}`；true/缺失是派生探针。当前与 namespace 同一 function-only gate 拒绝，尚无独立 search 语义处理。 | [OpenAI cache-only定义](https://developers.openai.com/api/docs/guides/tools-web-search#live-internet-access)不是“关闭工具”。Build 公开源码确有 hosted web_search、search/open/find 类型与完成 payload，但未证明固定 profile 的 false 约束、完整引用与 Usage。 | **软件缺口＋上游证据缺口**。保留 provider-hosted 工具身份及 false；新增 search item/action/阶段/结果、sources/引用及历史的闭合表示，按实际源信息转换。不能删声明、把 false 改 true/default、改普通 function、造空结果。网关自办缓存索引需要新的搜索执行主体、数据来源和 Usage 契约；本地封网只限制网关，不能证明远端 Provider 未取实时内容。当前不推荐另建搜索服务。 |
| **C6 非assistant message id**：C 三条 input message均带 id；codec原样留在 message extensions，[Router:706](../../crates/gateway-router/src/protocol_transform.rs#L706)却仅允许 assistant 身份扩展。此门槛在原始首拒绝之后，未动态抵达。 | response input-items 列表的 message有id；但本轮重取的 create schema的 user/system/developer输入分支没有列id。**GET 返回字段不自动证明任意调用方 id 可用于 POST**。 | **表示缺口线索，资格尚不完整**。不得把它宣布为已有依据充分的“直接放行修复”；在 namespace 开发中保留原值并列为前置负例，补 Codex 当前写入语义及目标 profile 的 input-id依据，或明确新的 CPAR本地身份契约。不能删id或将它当普通无关metadata；不能由 assistant 的资格扩大到其他角色。 |

summary-only `reasoning:{summary:"auto"}` 已由 ADR-0113 实现，保持 effort 缺失，不再重做。
实际7个顶层 function与 namespace的5个 children在已观察请求中无同名；冲突能力仍必须由独立反例验证。

**上游证据已有而未被充分利用的部分：**冻结 Build [conversation ToolSpec/ToolCall](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation.rs#L474)
及[Responses转换](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation/responses.rs#L92)
可指导普通函数、call_id与显式历史的精确实现；[hosted选项](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/tool_overrides.rs#L154)
和[流解析](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/stream/responses.rs#L494)
证明 hosted search 的客户端路径。它们不是一份空白，也不构成1.0.50的全部服务保证。

**可逆名字编译的取舍：**用请求绑定双向表把 `(namespace,name)` 编译成唯一flat name，确实可由网关完成，
无需上游拥有 namespace 原语；但只证明身份可逆，未证明分组描述、模型选择或 deferred/custom语义等价。
它直接冲突 BC-PROTOCOL-009 的“不扁平化/不拼接名字”，不能在当前承诺内推荐为兼容通过方案。
即便未来另立明确有差异的产品范围，仍须解决名字约束、hosted保留名、碰撞、selector与历史，不能只换名字。

## 4. Claude Code 字段决策表

M 的两次请求均为 `POST /v1/messages?beta=true`、max_tokens32000、Read/Bash、stream=true；
首轮带 display:updates及其对应beta，第二轮由客户端自行移除二者，adaptive仍在。
原始 header已包含 `thinking-display-updates-2026-08-18`，故不能把首轮失败归咎于漏加该beta。
首拒绝静态定位在第二个 system text block的cache_control，之后的独立阻点不能忽略。

| 项目／实际形状与独立拒绝位置 | 客户端语义、上游边界 | 具体方案、差异与契约 |
| --- | --- | --- |
| **M1 adaptive/display**：`thinking:{type:adaptive,display:updates}`，第二次无display。codec将 adaptive放进Thinking.effort、display放扩展；[project_thinking:554](../../crates/gateway-router/src/protocol_transform.rs#L554)分别拒绝扩展/非enabled类型。 | [当前 thinking文档](https://platform.claude.com/docs/en/build-with-claude/thinking#progress-updates-between-tool-calls)区分 adaptive模式与 effort；updates隐藏 reasoning正文，但可呈现工具前进度，带原生signature并需完整历史。Build默认推理和summary不自动具有这些语义。 | **上游/跨协议语义缺口**。先将 adaptive作为模式单独建模，不能继续把它解释为 effort值。准确路径需要目标的自适应推理及进度/续轮机制；没有来源时预拒绝。ADR-0108/0114仅批准隐藏Grok summary，未批准伪造signature、用summary冒充progress、吞掉请求控制或把缺display默认为omitted。支持差异只能经 D2 独立选择。 |
| **M2 output_config.effort**：两次 `{effort:high}`，根扩展 `anthropic.messages.output_config`；[project_root_extensions:446](../../crates/gateway-router/src/protocol_transform.rs#L446)拒绝。 | [effort文档](https://platform.claude.com/docs/en/build-with-claude/effort#how-effort-works)定义对回答、工具及thinking的整体引导；[Claude Code说明](https://code.claude.com/docs/en/model-config#choose-an-effort-level)明确同名等级按模型校准。 | **跨协议语义缺口**。独立于thinking和max_tokens保留；不能映射成OpenAI/Grok reasoning.high或固定token预算。准确支持需要目标对同一整体控制的依据；目标仅有reasoning effort时不等价。若采用客户端受限配置，是 D2 的能力差异，不是软件同义字段重命名。 |
| **M3 context_management**：精确 `edits:[{type:clear_thinking_20251015,keep:all}]`，保留为根扩展后同上拒绝。实际历史仅一条user text，没有任何thinking/redacted_thinking或工具历史。 | [官方keep:all](https://platform.claude.com/docs/en/build-with-claude/context-editing#configuration-options-for-thinking-block-clearing)表示不清除受该策略管理的thinking；它不赋予外来signature/原生历史可读性。 | **可由网关准确承担的有界软件缺口**：只对这一个完整edit、无未知键/其他策略且无Reasoning/Opaque/thinking历史的输入，验证后保留全部历史，记录本地“未应用清除”；响应的context_management表示也须按标准核对。无需假造Build同名字段或新产品取舍，但需明确BC-ROUTER-004的等价恒等子集。任何后续不满足谓词的请求仍预拒绝；不能承诺实际Claude将来永远无thinking。它不会独自解除M1/M2/M5，故不选作下一主线充数。 |
| **M4 metadata.user_id**：150字符JSON字符串，内部 device_id/account_uuid/session_id；根扩展 `anthropic.messages.metadata`，同上拒绝。 | [Messages metadata定义](https://platform.claude.com/docs/en/api/messages/create)是外部用户关联标识，不是鉴权凭据。Build没有已证明的user_id对应。 | **D1本地消费或上游资格**。当不透明字符串处理，不用内部JSON生成owner/凭据/session header；按认证Request归属。若选本地关联，需明确不提供上游user关联/滥用检测输入，并完整私密留存；现有扩展仅能保存值，不等于已有持久化消费者。 |
| **M5 block cache_control**：system[1]、system[2]、messages[0].content[0]均 `{type:ephemeral}`，未带ttl。扩展仍在各TextContent，另汇总retention。[nested:422](../../crates/gateway-router/src/protocol_transform.rs#L422)先拒绝，随后还有[cache:400](../../crates/gateway-router/src/protocol_transform.rs#L400)。 | [prompt caching定义](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)虽不改变输出生成，却包含前缀breakpoint、默认5分钟TTL、缓存读写与费用/输入计数。Build全请求cache key或affinity不证明block等价。 | **上游缓存资格缺口；降级需D2**。准确路径须证明三个前缀位置、TTL和实际源cache计数；保留位置，不以global key替代。网关缓存整段回答会改变工具/执行/Usage，不能代替prompt缓存。无缓存profile只能公开承认没有该缓存承诺，不能吞标记后报“缓存兼容”或补零cache Usage。 |

已受控验证的max_tokens32000精确出站、named tool和serial控制继续保留；本轮没有撤销这些成果。
Messages隐藏summary的证据与请求thinking/display控制各自验收。没有真实工具轮前，不能预测Claude会回传何种历史。

## 5. 处理顺序与真正的新选择

1. **#62 原生namespace/function身份纵向切片**。它是当前实际首拒绝；只做声明解析会在调用/历史再次失真，必须一起收敛。C6同步保留为明确的下一预拒绝条件，不能顺手删id。
2. **request metadata接收责任（D1）＋hosted search契约**。D1可让网关承担明确的本地职责；search则优先取得false及动作/引用的目标依据，不扩大成所有字段/版本搜索。三项原样必需字段全部有资格后才进入实际Codex正向工具验收。
3. **#64共同边界再进入首拒绝cache与thinking/effort**。keep:all恒等子集可以准确实现，但不独立解阻。D2若不选择，按原控制语义继续设计，不把受限配置变成首版默认。
4. **合并实际客户端证据后再回到#66/#67**。#61 cost与#65恢复不重复；若有新的持久化数据，再补其受影响的有界恢复证明。

只有下面两项是新选择；均为**待选择候选**，本文没有替用户批准。标准保真、维持cost决定、保持依赖不再提问。

| 决策 | 具体方案与推荐 | 对承诺及代价的影响 |
| --- | --- | --- |
| **D1：CPAR是否成为已审查request关联元数据的最终接收方？** | A：CPAR本地诊断消费（推荐作为独立新契约）；B：必须交给Build消费，保持阻断直到目标语义成立。A只准C4的7键string map、M4的user_id字符串；保留完整原值/内部字符串、按Request及实际Attempt（若有）关联，未知键/形状拒绝。C4 blob内涉及执行/审批/历史的成员须逐项分类；不能仅按外层7键整体宣称无害。 | A明确不承诺Codex服务端追踪/历史摄入/审批处理或上游user关联；不将请求metadata当响应可分离metadata。需要新后继ADR、请求字段BC、加密确认/授权reader/TTL/上界；可取64KiB总界，超界拒绝不截断。无Attempt时只记Request归属，禁止伪造Attempt。写失败不得承诺“已保留”；不进入模型输入、普通日志或计费。B没有这项新本地存储成本，但保留服务证据依赖。 |
| **D2：是否另行选择有明确能力差异的Claude受限配置候选？** | A：继续原样客户端控制（本轮推荐，沿用现范围）；B：仅在新的隔离配置中评估官方支持的无cache、无adaptive/display/effort请求方式，仍用当前Claude和Grok Build。B先验证当前版本是否真能通过公开配置做到，不能改客户端/降级/静默删字段。 | B失去所列缓存、推理/进度和整体effort承诺；仅改配置并不保证发出的字段已消失，且不解决#61。若要计入首版，须用户明确修订#58/#64及有关控制/缓存BC，再做完整实际工具/summary/取消验收；否则只是单列候选，不能替代当前#64。本文没有批准B或提供未经实测的开关配方。 |

不另要求用户在“精确支持”与“伪造/近似通过”间选择。namespace名字编译、自办搜索/缓存、
把Anthropic high改Grok high均未获得当前等价证明，不能作为默认实施方案。

## 6. 唯一后续 implement 任务：#62 原生 namespaced function 全链路

**目标：**实现C1–C3的原生表示与两次相关工具续轮，让原样Codex能越过namespace首个解析门槛；
保留C4/C5/C6及#61的独立阻断。不是#62整票完成任务，不增网关多agent执行器。

**开工与启用分开判定：**标准schema足以指导准确的软件表示与本地受控开发，无需用户批准语义损失。
但[BC-PROTOCOL-009](../contracts/BC-PROTOCOL-009-codex-responses-controls.md)要求的固定Build资格尚未满足；
**当前状态为 LOCAL_IMPLEMENTATION_FEASIBLE / BUILD_ACTIVATION_BLOCKED**。
若下一轮任务要求的是立即放行生产Build或完成原样CLI正向闭环，则前置未齐，不能标READY。
不能以“表示已实现”为由删除该资格门槛；也不能把只移动解析失败点作为本任务完整PASS。

**最小缺失输入：**目标服务方可固定日期/版本、明确适用于固定proxy/profile1.0.50的原生namespace适用说明，
至少确认：function-only namespace声明（含strict:false）、auto/parallel控制，以及JSON/SSE中namespace/name/call_id、
完整参数、结果和显式历史的规则。正式适用声明可以直接提供，不要求重新调查所有release或调用收费Provider。
如选择源码证明，须可核验source/release对应；原生输入message id另须C6依据。
材料取得后先核对适用范围，再启用该字段；metadata/search不随之准入，cost决定不随之改变。

**涉及模块与最小责任：**

| 模块 | 本切片必须完成的行为 |
| --- | --- |
| `protocol-openai-responses::{decode_tools,decode_function_call,decode_function_call_output}`及`native_output` | 有界、闭合的有序namespace/function表示；保留group description、child schema/strict、所有身份presence；仅覆盖实际function子集，custom/deferred/嵌套等尚无完整资格时预拒绝。 |
| `gateway-core::{tool,message,canonical_request,canonical_event}`和现有RawExtensions边界 | 完整身份与call_id分离；建议采用协议拥有的`ResponsesToolSet`解析/校验视图，语义保留在已有扩展envelope，避免为一个控制改写所有Thinking/Tool持久化形状。旧flat函数序列化不变；若确须改变持久格式，先停下补版本/reader契约，不暗改payload v1。 |
| `gateway-router::{protocol_transform,tool_controls,response_transform}` | 对完整工具集合验证选择/parallel/历史；同组重复、悬空结果、错误namespace、未知返回名等预拒绝或单次上游协议失败。只有Responses同协议及取得字段资格的实际adapter/target/mode适用；不依User-Agent。 |
| `provider-grok::build_responses`及既有共享native-output校验 | 原样native节点出站；JSON/SSE所有阶段和完成快照确认身份，结果/history无重命名。普通function的现有profile/模型/lease/Usage边界不变。 |
| `apps/gateway/.../standard_responses/{codex_controls,tool_rounds,lifecycle}` | 复用生产compiler/factory/leased adapter/公共HTTP、独立receiver和writer，增加这一字段的真实端到端行为；不新建通用server、恢复框架或资格平台。 |

**可观察验收／停止条件：**

1. 保留现有公共namespace请求RED。公共JSON与1/7/31字节SSE经完整Build装配：完整声明原样到达独立receiver；两组同名child由namespace区分，调用/参数/call_id/结果/第二轮/后续问答保持；只有完整验证参数才交付可执行调用。受控上游资格只能标LOCAL_CONTROLLED，不授予真实profile资格。
2. auto/none/required与parallel原值按已有语义验证；没有标准依据的namespaced forced selector仍在发送前失败，receiver=0。重复、冲突、丢namespace、改call_id、结果身份不符、畸形/未完成参数、done/terminal改写分别不能成功、重发或制造工具完成事件。
3. 已知Usage和summary不减；未知Usage仍拒绝且走既有exact Attempt证据，不准入cost，不为取消补读。现有Pi工具/失败/取消、Responses原生summary与#63限定Messages策略做受影响回归，#65不重跑整套矩阵。
4. 在原有隔离驱动下重新冻结当前实际Codex，保留全部请求字段，证明首解析门槛是否已前移；版本漂移重新判断、不降级。C4/C5/C6未解决时仍FAIL/BLOCKED，不将400或receiver=0当兼容PASS。若固定profile资格缺失，生产发送继续停止；仅本地表示完成不得结束整任务为成功。
5. 原样Codex正向闭环必须等C1–C6适用条件共同满足。届时改**现有驱动的具体Codex场景**：使用其实际exec_command/write_stdin，实际连续提示/结果依赖与后续问答；替换成功分支的固定400/零接收断言，保留旧拒绝分支，不削弱成功条件。当前[oracles:144](../../apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/cli/oracles.rs#L144)、[cli:178](../../apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/cli.rs#L178)硬编码失败，peer脚本是Pi工具；`--require-codex-success`六项门禁本身尚非正向场景。实际仍须满足三完成turn、五增量response/独立接收与exact计量、两次真实工具结果及result.txt=42，不能由合成namespace循环代填。
6. 任一必须控制/字段资格未知、原样字段意外变化、错误身份、额外发送、私有数据外泄、取消后伪成功或持久化冲突立即停止该验收；先保留失败，不换Provider/账号/模型或删字段续发。未来实现按风险运行定向检查及项目要求的最终验证；本轮纯文档不执行它们。

## 7. 所有字段成立后的实际客户端验收

共同链路固定为**实际客户端→公共listener/鉴权→生产compiler/factory→实际leased Build adapter→独立受控上游→标准输出与授权证据读回**。
固定source/manifest/lock/client身份与隔离环境，独立接收数不能由缺少Attempt推断。分别记录：

| 范围 | 必须可观察的正反例 |
| --- | --- |
| C1–C3 | 原样Codex首轮不再预拒绝；真实声明与输出namespace身份、工具执行、结果回传、第二相关循环和后续问答；JSON/SSE协议对照与冲突/selector反例分列。 |
| C4/M4 | 若D1获准：完整私密读回、原字符串不改、认证owner隔离、无上游metadata/追踪header暗映射、写入失败不承诺保留；若选转发则核对独立receiver原字段及目标消费行为。未知键/控制型内容仍按契约拒绝。 |
| C5 | 受控上游确实返回search/open/find及sources/引用，核对offset/文本/动作/阶段/历史/适用Usage，不伪造未收到的搜索。另有固定服务false约束的权威证据；网关loopback隔离不能替代该证据。 |
| M1/M2/M3/M5 | 实际Claude原样控制被精确执行或按已明确选择的差异profile声明；两次真实工具及结果/后续历史、每Attempt summary授权读回与已知Usage。恒等context只在逐请求谓词成立时适用；cache按实际源计数，缺失不补零。 |
| 两客户端生命周期 | 实际增量→lease=1→用户取消→同PID存活至独立upstream close/lease=0→最后cleanup；唯一失败/取消终态、已验证Usage保留、无额外发送。#64目前零lease不能代替此链。 |

这些是后续验收条件，**本轮未执行**。受控客户端通过、真实Grok Build、远端CI、候选、发布部署分别报告；
任一缺口不能由helper绿灯、准确拒绝或旧Full覆盖。#58首版三客户端门槛及#61/#62/#63/#64/#65整票条件保持。

## 8. 本轮静态验证与交付边界

本文及BC-PROTOCOL-009状态校正仅做本地文档链接、引用/字段形状点验、工作区文件秘密规则、空白及差异/原内容保留检查。
没有生产代码/配置/schema更改，没有行为测试或Full，没有提交/推送、GitHub写入、真实Provider调用、发布或部署。

| 本轮实际检查 | 结果 |
| --- | --- |
| 文档链接 | PASS：1237份Markdown的本地目标存在性；不冒称远端链接可用性测试。 |
| 契约测试引用静态解析 | PASS：250个引用可定位；没有执行这些行为测试。 |
| 本轮两文件的工作区秘密扫描 | PASS：直接扫描工作区内容（含未跟踪的本文），沿用原secret regex与禁止路径规则，不用只扫描index冒充检查新文件。 |
| 两文件UTF-8、末尾换行、空白与owned diff | PASS。 |
| 原16文件及Git基线 | PASS：内容hash/mode/link均未变，HEAD/分支不变、暂存区仍为空。 |
