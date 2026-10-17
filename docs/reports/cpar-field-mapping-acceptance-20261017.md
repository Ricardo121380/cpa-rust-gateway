# CPAR native-v1 field acceptance matrix — Proposed

配套 [ADR-0106](../adr/ADR-0106-native-fidelity-profile.md)。本矩阵定义用户已确认方向之后的实施验收，
不把未实现字段记为 PASS。当前 profile 全部 NOT_RUN/待实施；已确认现状复用原回执，
不重新捕获 Provider 响应。

## Source-bound facts and seams

基线 `312584184e7680594878460499c39af0a244d440`；依赖提交不修改这些 Rust/Python 文件。

| 组 | implementation / oracle seam | 本批证据及状态 |
|---|---|---|
| R | `crates/provider-grok/src/build_responses.rs::handle_reasoning_delta/append_part`（1784/2142）；`protocol-openai-responses/src/native_output.rs`（110）；`gateway-router/src/response_transform.rs::project_native_identity`（219/277） | 现有 summary/content 身份、parts<64、items≤256 已静态核对；rich桥接严格拒绝，旧公共矩阵保留 |
| U | `provider-grok/src/build_responses.rs::parse_usage`（2301）；`gateway-core/src/gateway_event.rs::UsageSummary/from_usage`（492/552） | raw仅在未映射信息出现时保留；source-event故意不携raw，完整耐久raw仍待实施 |
| M | `apps/gateway/src/runtime.rs`（5175）；`provider-grok/src/inference.rs`（380）；Build root created（1294）；三codec metadata | actual selected upstream、公开号和原生root分别静态定位；native root model/时间目前未完整传到公共封套 |
| O | `assets/cpar-serving-declaration-20261015/tools/remote-chat.py::verify_success_fidelity`（44/46） | 旧正式oracle确有 native/public 都字面等于grok-4.5的assert；未运行、未修改；通用`cpar_new_build_acceptance.py`另验公开模型，不能替代该assert |

安全合成输入继续使用 [minimal-native](assets/cpar-native-chat-offline-20261017/minimal-native.json)、
[usage-extensions-native](assets/cpar-native-chat-offline-20261017/usage-extensions-native.json)。
原样本只有 [structure](assets/cpar-native-chat-offline-20261017/structure.json) /
[identity-and-shape](assets/cpar-native-chat-offline-20261017/identity-and-shape.json) 的白名单形状用于决策。
所有私有原始响应始终排除。本批未重复原样本离线拒绝点诊断。

## Field and mode matrix

每项默认同时验三协议 JSON/SSE；S 为成功、N 为拒绝，预期都必须从实际公共 compiler/factory/handler
和独立 controlled receive count 观察。已知请求冲突应接收数0；响应字段冲突接收数1且无新Attempt。
下面的 NOT_RUN 专指新profile验收；旧行为证据不被改写。

| ID | 输入 / 范围 | 必须可观察结果 | 新profile状态 |
|---|---|---|---|
| R01 | summary存在、body缺失；文本为空/非空 | S: summary单独保存、body仍缺；不生成thinking/body；JSON与SSE reducer相等 | NOT_RUN |
| R02 | body存在、summary空；multiple parts | S: separate lists/parts/IDs原序；兼容plain body只在已证明子集；不合并rich parts | NOT_RUN |
| R03 | 多item、interleaved summary/content/tool | S: output_index、part三元组、semantic ordinal与source sequence有无保持 | NOT_RUN |
| R04 | item换ID/重复ID/part错index、done改片段、封口后增量、terminal换序 | N: safe error、唯一失败终态、Usage flush，无DONE/completed/root | NOT_RUN |
| R05 | 未知part/item/annotation、foreign ciphertext、伪造signature | N: strict拒绝，不用opaque扩展逃过闭合校验 | NOT_RUN |
| R06 | 源sequence重复/倒序/超u64；无sequence | N:非法序号；S:缺失仍缺失，gateway ordinal不冒充source sequence | NOT_RUN |
| U01 | 完整known counters/未知root与details/0/null/[]/{} | S: raw片段逐字节保留、lossless值等价，标准计数与其一致，unknown不计数/不计费 | NOT_RUN |
| U02 | Usage root absent/null/{}/known null/details null | S: presence各自保持；已知counter unknown不填0；schema合法性独立 | NOT_RUN |
| U03 | total=known sum、缺operand、缺/null total | S:精确核对或保留raw未知；不制造operand/total | NOT_RUN |
| U04 | 负数/分数/非数字known计数、矛盾/overflow total、重复key | N:不能靠raw保留绕过known验证；保存先前合法证据，不读取后续 | NOT_RUN |
| U05 | 未知数字9007199254740993、更大词法、explicit 0/null/[] | S:raw_json完整；专用lossless消费者保持精确值；标准JS parse误差不算兼容PASS | NOT_RUN |
| U06 | 重复Usage、failure/cancel、被拒Usage事件本身 | S:1 source-event＋1 ledger，raw evidence同Attempt；冲突raw不覆盖；actual cost仍UNKNOWN | NOT_RUN |
| U07 | native price字段非0/catalog无价/estimated/unknown | S:raw证据保留；无未经证明真实charge；catalog计算与独立Provider账单分列 | NOT_RUN |
| M01 | public/upstream/native字面相同 | S:请求public、actual selected upstream、native、公开号逐层确认；不能用同名掩盖错channel/owner | NOT_RUN |
| M02 | public alias、native不同，有可信冻结证据 | S:三段oracle＋证据hash/范围/失效检查；旧oracle/FAIL保留 | NOT_RUN |
| M03 | 不同名无证据/过期/错误渠道/版本/权限 | N或验收BLOCKED：不放宽断言、不换模型或账号；native值保持UNKNOWN关系 | NOT_RUN |
| M04 | native model absent/null；public/native时间不同 | S仅表示presence和来源保留；不能得到模型等价或时间来源PASS | NOT_RUN |
| M05 | root metadata/control回显/未知root | S已准入原值，不变请求控制；N未知语义或敏感root，不静默删掉取得成功 | NOT_RUN |
| P01 | 无opt-in头；未支持版本/多值/未知mode/candidate | 当前严格规则保持；请求侧N在lease前、receive0；默认客户端不收到新profile | NOT_RUN |
| P02 | profile合法JSON/SSE＋split UTF-8/大转义片段 | S:同schema最终值；序号/count/length/hash确认后唯一标准成功终态 | NOT_RUN |
| P03 | profile事务丢片/重片/错hash/截断/error/cancel | N:没有fidelity complete/成功终态；cancel传播，partial输出不透明重放 | NOT_RUN |
| P04 | bytes/depth/items/parts/aggregate/frames边界±1 | S界内，N界外；JSON/SSE总边界一致，不截断；source更小界限优先 | NOT_RUN |
| S01 | stored encode失败/write失败、GET异主/过期 | 无成功completed-root；确切owner/TTL/AEAD；失败原文不进event/telemetry | NOT_RUN |
| S02 | 新/旧schema、process restart/key轮换、旧reader | 新封套exact读回；旧值Unknown；不支持版本拒绝但保留历史；binary-only rollback不默认安全 | NOT_RUN |
| S03 | 原生history回传/compact/WS、跨channel/owner/profile | 合法原身份原序；无伪造control/signature；各声明能力独立资格，不靠JSON/SSE通过升级 | NOT_RUN |

## Actual consumer evidence

复现：在独立临时目录把附件 `consumer-package.json`、`consumer-package-lock.json` 分别复制为
`package.json`、`package-lock.json`，复制 `consumer-probe.mjs`，执行
`npm ci --ignore-scripts --no-audit --no-fund` 后 `node consumer-probe.mjs`；
包锁及 integrity 见 [consumer-package-lock](assets/cpar-field-contract-dependency-20261017/consumer-package-lock.json)，
脚本见 [probe](assets/cpar-field-contract-dependency-20261017/consumer-probe.mjs)，
结果见 [results](assets/cpar-field-contract-dependency-20261017/consumer-results.json)。
全部 fetch 由内存 Response 替换，SDK未建立socket；7 mock fetch，外部Provider=0。

| 实际消费者场景 | 观察 | 允许结论 |
|---|---|---|
| 三协议JSON | root/message/usage未知字段保留；Responses helper另生成output_text | 这些固定版本JSON读取不删除所测字段；未证明展示/业务使用 |
| Chat SSE helper | 两片段a/b，final未知delta仅b；root/usage扩展保留 | 通用赋值不能替代按协议累计 |
| Messages SSE helper | raw已知delta带扩展；final content/Usage无新扩展；custom event 0 | 标准helper不是fidelity reducer |
| Responses SSE helper | 自定义事件触发 `Unhandled response stream event: unknown` | 新事件不能直接喂未经适配helper |
| Responses无自定义事件对照 | final output_text=ab；root/usage扩展保留 | 拒绝由新增事件触发，fixture正常路径可完成 |
| JS普通JSON大整数 | 9007199254740993解析成9007199254740992；raw string保留 | 专用消费者还需lossless数值规则 |

客户端产品（Prism、CLI或其他）、Python/Go/严格生成客户端、不同SDK版本及真实渠道均UNKNOWN/NOT_RUN。
以上是可复现的SDK行为证据；本profile事务/reducer、完整消费者和正式兼容仍未实现。

## Implementation gates after decision

按已确认选择固定 schema/field表和 #9 extended-vs-standard 验收台账，再补行为契约、
decoder原始片段/root保留、Canonical typed证据、projector与三codec、SSE reducer、
加密证据migration/reader与模型oracle。复用原fixture和12组公共matrix；增加本表正反例。
通过本地Task/Phase/Full和固定双轴审查之后，另行判断新候选资格及实际复测授权。
当前标准能力与rich profile分别记账；依赖门禁PASS不改变本表NOT_RUN或渠道BLOCKED。
