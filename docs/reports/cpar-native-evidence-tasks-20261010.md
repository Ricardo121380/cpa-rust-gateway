# Kiro／Grok Web 原生阻断取证任务（2026-10-10）

结论：**具体任务已准备；原生等价实现依据仍 NOT_PROVEN／BLOCKED**。未读取真实客户端、SSO、cookie、浏览器账号或 private account material，未真实抓包或推理。不重复搜索同一批旧 SDK。下列任务必须获得相应账号／客户端／网络／费用授权后才执行；当前公开资料准备已经完成。

## 已有证据的边界

复用 [冻结原生索引](assets/cpar-channel-delivery-20261010/primary-source-index.json) 和 [原生语义报告](cpar-channel-native-semantics-20261010.md)。AWS Toolkit IDE serializer/models 固定在 `e1b0f9bc573bac1830f0b9aa7ced25a6c69bb477`；旧 Q CLI input/README 固定在 `15cc8f3cd18c4272925ce1c7053268eedff1ea0a`。它们是一手生成定义，但没有证明当前 Kiro IDE／CLI cap；旧定义的检索零命中不能外推当前闭源客户端。

Grok Web 参考作者 grok2api 的 gateway/chat 固定在 `7c889a960e2638341b4dae9a5c81af0e0f38c87f`，它们是作者源码，**不是 xAI 官方协议承诺**。WS 的附着 conversation 与成功 `response.done` response.id，以及 REST user/model response 的候选 parent 来源分别记录；不能把 WS、REST、canonical ResponseId 混为同一个方言。新增 xAI Official Responses cap 资料也不能外推 Grok Web Messages cap。

原公共 handler + controlled transport 回执继续按原来源复用：Kiro 两端点 JSON/SSE 的 uncapped 工具多轮和 cap pre-send 拒绝；Web unrepresentable history/tools/cap 的 Attempt/DNS=0 拒绝。它们不证明真实 cap 或 Web 续接成功。

## Kiro：五个独立缺项

| ID／缺项 | 下一份具体材料 | 最小观测／记录 | 权限及停止条件 | 证据充分后的最小实现／公开验收 |
| --- | --- | --- | --- | --- |
| K01 原生 cap 字段／范围／单位 | 当前 IDE 与 CLI 分别提供版本、build、原生 input schema 或官方契约；若只能抓包，提供正常控制请求与只改变 cap 的请求 | 保存客户端版本／OS／模型／endpoint hash、cap JSON path、合法类型、min/max、默认、省略／null 行为、units；禁止从提示词反推 | 文档公开可读；真实客户端／材料／请求待授权。先查版本和可配置字段，无来源或不可设置则停止、请求 0；不向未知端点试猜字段 | 在既有 builder 明确映射、validate range；公共三协议 JSON/SSE 正确 native payload + receiver count；未知／非法值 Attempt/DNS=0 |
| K02 reasoning／可见输出计数 | 当前版本的 usage 定义／账单说明，或有完整 usage 的原生终态样本 | native input/output/reasoning/cache 字段、units、包含／独立关系、缺失语义；保留差值和上限覆盖范围 | 真实 usage／费用材料只读范围待授权；不得把 unknown 补成 0。K01 未满足不付费做 cap 实验 | canonical Usage 的 source accounting 精确映射；全部字段与 durable Usage／ledger 一致，无法等价仍拒绝 |
| K03 命中上限终态 | K01/K02 完备后，当前 IDE／CLI 每端点一份正常终态、一份确定 cap-hit 的 native EventStream（最少每端点 2 次，合计 4 次，另批批准） | 完整有序 event type／序号／payload allowlist、finish／truncation／exception、cap值、Usage、stop时间；正常与cap-hit对照 | 另批真实请求、费用及抓包授权；不能证明全计费上界则不发；未出现可信 cap-hit 不自动加请求 | 生产 decoder 精确区分成功／截断／失败；公共 handler JSON/SSE 不得把 cap-hit 当完整成功，partial Usage 规则可追溯 |
| K04 端点／凭据／地区 | 当前 native endpoint 契约及所选账号的脱敏 scope；IDE q.<region> 与 CLI runtime.<region>.kiro.dev 分开 | endpoint host/path、IDE/CLI build、credential kind、region来源、签名／scope 类型（不保留 token）、非秘密时间和对应 material evidence ID | 真实材料检查须限定账号。当前只有所选环境 proof，不能声称覆盖所有 region／credential；API Key 不冒充 OAuth | 既有 endpoint policy 最小扩展；有依据的正例及跨 region／kind 非法组合公共 pre-send 拒绝 |
| K05 profile ARN 边界 | 当前客户端对 profile 的官方规定或只读非秘密配置；明确哪类 credential 必需／禁止／可选 | ARN 脱敏 hash、region／partition结构、所属授权 scope、原生字段省略／携带规则；不输出实际私有 ARN | 未明确所有权／地区／必填规则则停在 K04/K05，不自动补 ARN、不跨账号调用 | builder 逐种凭据按已证明规则传递；public boundary mismatch 拒绝且 transport receive=0 |

K01/K02/K04/K05 是 K03 付费实验的先决条件。取得公开定义可以先完成不依赖真实材料的 RED／最小映射；若只有参考实现或未证明当前版本，继续 NOT_PROVEN。正常 cap-hit 实验的 4 次仅是最小观察建议，未授权且不含授权登录、目录、quota 或探测／重跑。

## Grok Web：六个独立缺项

| ID／缺项 | 下一份具体材料 | 最小观测／记录 | 权限及停止条件 | 证据充分后的最小实现／公开验收 |
| --- | --- | --- | --- | --- |
| W01 当前 REST／WS 方言 | 当前 Web 客户端 build/asset hash、transport URL及原生 request/event schema，或官方可追溯承诺 | 固定 REST 或 WS 一条路径；请求字段／事件版本／UID来源／创建或附着动作。分别保留另一方言为参考，不能拼接 | 已冻结参考作者源码不足；真实客户端／账号／会话采集待授权。无法证明当前方言先停止、请求 0 | 在既有 web transport 接入单一已证明方言；公共 handler 断言原生 payload／actual sends；无错误回退到另一方言 |
| W02 conversation／parent 成功推进 | 同方言一份正常根及一份成功子轮的完整事件（建议 2 次推理，须单独批准） | conversation附着来源、parent可信response事件、root/child实际 Request/Attempt、原生terminal、owner证据ID、按序旧→新parent；标识值private、公开hash | 单一账号和隔离 conversation；only success可推进。标识缺失／冲突／失败根则停，不猜 parent、不加补探测 | 将 existing binding primitives 接公开 owner／成功终态提交；public三协议适用文本 JSON/SSE 精确续接、失败根／伪造parent零接收拒绝 |
| W03 并发／取消／失败／会话失效 | 当前方言的官方状态机，或明确另批批准的事件样本：两并发子轮2次、输出前取消1次、输出后取消1次、已知安全失败1次、失效会话1次，最多6次提交 | 复用W02成功根；request/cancel/terminal时序、每次actual send、CAS/lease／赢家、parent是否推进、资源释放、服务状态读回；单次不确认保持unknown | 不在真实共享会话中注入。每一失败／失效必须预先确定可安全复现的动作与单位；没有安全动作则对应项BLOCKED、请求0。不得额外制造根或探测／重跑以凑样本 | 单root单推进、取消／失败不形成成功child；公共 controlled oracle 验证2并发只有合法winner、零late send、unknown write先读回 |
| W04 owner／account／revision／egress | 当前会话身份映射／退出或重新授权事件；所选授权的非秘密账户谱系和出口证据 | owner Client Key hash、account hash、credential revision、session证据revision、expires、egress ID/revision/fingerprint；一一绑定，不新增未经定义的 sidecar 字段 | 材料和会话只读另授权；不同owner、手动替换、重新授权、出口变化的真实探测计数目前0；先受控fixture | 接已有准确绑定并保持 revision闭集；跨owner／账号／凭据／出口 rejection在public handler Attempt/DNS=0；Web不继承Build E-05例外 |
| W05 Messages native cap／计数 | 当前 Web方言 cap字段/范围/units、reasoning/tool覆盖、usage与cap-hit终态 | 与K01/K02/K03同样完整原始字段和计数；必要正常／cap-hit样本至少2次，需方言及费用证明先完备 | 未证明上限不做付费 cap探测；不得把Official API的visible-only cap移植到Web或删除Messages必填字段 | 严格等价才映射；public Messages JSON/SSE cap、非法输入、命中terminal及账本；不充分继续pre-send拒绝 |
| W06 真实材料取得／脱敏 | 仅用户指定的一个客户端／账号／隔离会话；明确采集时间、来源路径及保留范围 | raw在权限0700目录／文件0600；移除Authorization/Cookie/SSO/header secrets，文本使用合成提示，本地salted/opaque身份；保留event字段和值类型、hash、索引／时序 | 不扫描用户其他浏览器空间、账号、历史或共享流量；发现非目标材料立即停。公开GitHub只报告来源类型／版本／hash和结论 | 可重放的合成fixture由获准样本抽取并保留provenance；fixture先通过、再正式live separately授权 |

W02／W03／W05 建议上限分别 2／6／2，合计最多10次原生提交；取消前发送可能为0，仍按提交与接收分别记数。这些均未授权，不含登录／refresh／目录／quota。若失败方式、根复用或完整计费上界未证明，相应项在发送前BLOCKED；不能将不确定动作伪装成已就绪。实际 session 标识不公开；脱敏需保留配对一致性，不能用随意新造标识改变原始关系。

## 解除阻断的证据包和完成判定

每项交付共同包含 `source_kind`（官方契约／当前客户端观察／参考作者源码）、publisher、version/build、hash、UTC、endpoint/credential/region/profile适用集合、private raw path/hash、公开 allowlist 和脱敏映射 hash、请求与费用授权引用、actual send counts、native usage／终态及失败保留。未知字段保留 null，旧来源不修改归属。

只有 K01–K05 或 W01–W06 的必要契约完整，才在真实公共入口先建立 controlled RED，然后主代理实施最小 builder/decoder/state接线并回归。公共 local proof、真实 Provider 和远端 CI／部署分别报告。#39／#49／#50 的 AC／原生依赖尚未齐备，保持 OPEN；Grok Web 12 个工具基础组合仍 NOT_APPLICABLE_ACCEPTED，Messages 精确计数的既有限制保持原范围。无需重问或扩大 Grok Build E-05，也不重做 T12。
