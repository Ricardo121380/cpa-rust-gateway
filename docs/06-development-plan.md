# Rust AI Gateway 详细开发计划

## 0. 计划元数据

| 字段 | 值 |
|---|---|
| 计划版本 | `v1.149` |
| 生效日期 | `2026-08-03` |
| 状态 | `Locked for execution` |
| 当前阶段 | `P1` 至 `P6`、P9、P10 与 P11 已完成；P12 正在执行，P12-01 已验收。P7 Kiro OAuth 与 P8 Official API-key E2E 仍延后。 |
| 当前任务 | P12-09 `IN_PROGRESS_P1_FIX_BUILD`。Jakarta SSH 已在修正 Clash TUN 排除规则后恢复；Newapi 的官方 API 迁移、CPAR 全量切换、真实回滚与再次恢复均已执行。最终检查发现上游在不同 Chat SSE 请求间复用 response ID，导致 2 个 Required Usage 事件被持久化层隔离；按 P1 规则已立即把 Caddy 与 Newapi 恢复到旧 CPA。修复与回归测试本地通过，等待 exact-revision artifact、受控部署和重新验收。P12-10 未开始；CC Switch 仍不读取、不修改。 |
| Rust Workspace | 20-package（P0-03 建立 21 个；`CR-P12-06-001` 批次删除两个从未落码的保留 crate，`tests/differential` 成为工作区成员） |
| 生产部署 | 新主机（aarch64）已完成 P12-07：服务 active/disabled-at-boot、仅回环监听、测试域名 `cpar` 公网暴露且七项断言通过；最终切换为生产主机名全量指向 CPAR，旧 CPA 仅保留有限回滚窗口并在 P12-10 关闭 |
| 行为参考 | CPA `v7.2.80` 为生产基线，CLIProxyAPI `v7.2.101` 的 handler/translator/executor/auth/registry 源码及测试为 P12-08 起的首要移植参考；CPAR 用 Rust 新架构复现其已准入行为，并保留已冻结的 AxonHub/New API/Sub2API/grok2api/Kiro-RS 快照作为渠道专项补充 |
| 已批准变更 | `CR-P1-G1-001`：将 G1 的 Chunk 条件精确为 P1 范围内的 Tool 语义投影一致性；原始 bytes/EventStream 不变性仍由 Provider 阶段验证。 `CR-P3-G3-001`：P3-10/G3 的真实验证公开别名改为 test-only `p3-chatgpt-compat`，不把 ChatGPT-family 上游误称为 `minimax-m3`。 `CR-P3-G3-002`：test-only SSE 单帧有限上限改为 64 KiB。 `CR-P3-G3-003`：仅 P3-10 ignored live profile 的 SSE idle 上限改为 45 秒，其他 transport 边界不变。 `CR-EXEC-001`：缓存化 Full CI、docs-only Gate、单探针诊断 harness。 `CR-EXEC-002`：缓存可见交付引用、补充供应链 Gate 与缓存度量。 `CR-EXEC-003`：Task Card、集中补丁、去重验证、证据模板和时延度量。 `CR-EXEC-004` 至 `CR-EXEC-006`：按风险路由 Luna/默认/高级模型与最低足够思考强度。 `CR-EXEC-007`：P 级开发分支与单次远端正式 Delivery Gate，保留 Task 级本地 review/test，并为 CI/cache 等不可本地证明的变更保留提前远端例外。 `CR-P4-G4-001`：新增非 HTTP、只读的管理状态查询与 403 账户受控恢复，以闭合 G4；认证 HTTP/UI 仍属 P10。 `CR-P6-03-001`：将 P6-03 已授权真实验证改为有限、可审计的模型 × 模式矩阵；每个 harness 进程仍严格只发送一次，不重试相同元组。 `CR-P6-03-002`：在前一矩阵全部得到同一脱敏失败类别后，加入一项不记录值的响应分类诊断和一个显式登记的一次性复测。 `CR-P6-03-003`：在确认 2xx JSON 错误对象后，增加最终一次仅从标准错误元数据映射安全类别的诊断调用。 `CR-P6-03-004`：新增一个与固定直连验收隔离的、服务器本地 grok2api Build 路由代理参考探针。 `CR-P6-03-005`：采用服务器参考的当前 Build 请求轮廓，并只登记新的固定端点 T11 非流式与 T12 SSE 验证。 `CR-P6-03-006`：通过 grok2api 支持的管理 API 导入指定 OAuth 文件并做账号专属额度刷新诊断；不重放固定直连元组，也不把共享路由调用误归因到该账号。 `CR-P6-03-007`：仅以本机官方 Grok CLI 做一次交互式 OAuth 重新认证并记录安全状态投影；不发送 P6 请求或改变服务器/路由。 |
| 已批准变更（续） | `CR-P6-03-008`：以 CPA、grok2api 和 Sub2API 的 clean-room 行为参考扩展 Grok Build 的已知 OAuth 凭据来源；保留标准 JSON/Device Code/Refresh，新增 CPA xAI 文件和官方 Grok CLI indexed cache 的内存导入，不纳入 Cookie/SSO Web 转换。 `CR-P6-03-009`：仅修正 T13 零发送 wrapper 的一次替代 T15 验证；T15 的 4xx 已停止矩阵。 `CR-P6-03-010`：基于官方 CLI 静态证据更正 workspace User-Agent，仅登记新的 T16 非流式直连验证，并在 T16 完整成功时条件允许一次 T17 SSE。 `CR-P6-03-011`：T16 在无网络预检的本地标签门槛前停止后，以不同的合法短标签重新登记 T18 非流式直连；仅其完整 Canonical 成功时允许条件 T19 SSE。 `CR-P6-03-013`：用户批准完成 P6 全部要求，解除 P6-03 对后续本地安全/连续性实现的流程阻塞；不声称 T18 成功、不发送 T19 或重放任何闭合 tuple。 `CR-P6-03-014`：以当前官方 CLI 的成功模型/会话和新完成的可注入执行链路登记 T20/T21，不重放任何已关闭 tuple。 `CR-P6-03-015`：T20 的新的 2xx JSON 协议失败后，只输出固定的无值结构类别诊断 T22。 `CR-P6-03-016`：T22 发现投影在合法压缩前运行，登记一次解压后无值结构诊断 T23。 `CR-P6-03-017`：T23 仍失败后，仅投影第一个固定 decoder requirement gate 的 T24。 |
| 已批准变更（续 2） | `CR-P7-G7-001`：P7 因 Kiro 外部账号重新认证而阻塞时，允许 P8 按自身顺序进行本地实现与审查；其与 `CR-P7-DEFER-002` 冲突的 P8/G8/P9-P12 顺序约束已由后者替代。 `CR-P7-DEFER-002`：Kiro OAuth 延后；P8-G12 按自身非 Kiro 依赖推进，P8 可执行自身 Phase Gate 与 Delivery Gate；真实 xAI 验证仍仅按 P8 自身明确授权进行。 `CR-P8-DEFER-001`：无 Official API Key 时，P8-07/G8 与 P7-09 一并延后至最终外部认证验收包；P9-P12 Gate 依赖不变。 `CR-P11-04-001`：用户批准把纯 loopback 合成 Soak 的最低门槛由 24 小时改为 10 小时；已完成的 10h13m 用户停止 receipt 仍如实标为 `INCOMPLETE`，P12 的真实 Canary 72h 观察不变。 |
| 已批准变更（续 3） | `CR-P12-05-001`：为 P12-05 的生产数据面组成创建一次新的、精确 SHA 绑定的私有 GitHub OIDC/Sigstore 制品，并仅在独立 loopback Staging 上执行受控临时图写入与最小验收；既有 CPA、公开流量和后续 P12 Task 均不改变。 `CR-P12-05-002`：对 P12-05 已 review/Full-gate 修复的精确 SHA 续签同一私有 OIDC/Sigstore 制品；同一隔离 Staging、Credential、Provider 与公开边界不扩张。用户要求此类同范围续签直接批准，但每次仍须记录精确 SHA 并独立验证。 `CR-P12-05-003`：一次已删除的本机 `0600` Bearer 选择临时文件不计为 memory-only 证据；按用户的直接批准约定，保持同一 P12-05 范围继续，但须在任何图写入前用纯内存 helper 重新预检，后续不得再创建明文 Secret 文件。 `CR-P12-05-004`：为同一临时 Staging 图的 P12 Krill 请求补入已验证、非机密的 Codex-compatible `User-Agent`；仍须以该精确 SHA 重新生成并独立验签私有 artifact，且不扩展 Credential、Provider、公开监听或流量边界。 `CR-P12-05-005`：将 P12 的唯一 Anthropic `max_tokens` Canonical 扩展映射为 OpenAI Responses 输出上限，其他外来扩展仍拒绝；不刷新 CC Switch 凭证，并以新精确 SHA artifact 仅重跑未覆盖的隔离 Staging 验收。 `CR-P12-05-006`：在服务器侧 `/models` 已证明当前 endpoint/Bearer 可用后，允许一次不保留正文的同请求轮廓 `/responses` 结构分类；只用于决定是否需要 P12 decoder 兼容修复，不替代 Staging 验收。 `CR-P12-05-007`：CR-006 的本地结果收集失败，保守视为其唯一请求已消耗；允许一项由服务器 root-only 无值 receipt 先持久化的独立替代分类请求，不重试 CR-006。 `CR-P12-05-008`：在 replacement classifier 仅确认 Responses 结构子集后，允许用同一已验收 artifact 进行一次 receipt-enhanced isolated Staging 重跑，安全记录精确 HTTP/error-envelope 类别；不改二进制或公开边界。 `CR-P12-05-009`：Staging 的 502 表示首事件前协议截断；允许一次完整 decoder-contract 的无正文 `/responses` 分类，区分响应字段不兼容与请求构造前失败。 `CR-P12-05-010`：完整 classifier 通过后，允许同一 signed artifact 的一次最终 isolated Staging retry；成功才继续 Tool/Explain，重复 502 即停止而不猜测修复。 `CR-P12-05-011`：源代码复核发现前置 classifier 遗漏 builder 固定的 input message type；允许一次完整同形 `/responses` 分类，避免基于近似请求作结论。 |

| 已批准变更（续 4） | `CR-P12-05-012`：精确 P12 request classifier 已通过而 isolated Staging 仍重复 `502` 后，增加仅受保护 loopback 管理面可读、固定阶段枚举的有界 attempt 投影；不刷新 CC Switch、无新的外部请求，新的 Staging 诊断仍须在 exact-SHA artifact 独立验签后执行。 |
| 已批准变更（续 5） | `CR-P12-PORT-001`：后续实现以旧 CPA 固定版本源码、测试与实际行为为移植基线，先建立可追踪行为清单，再用 CPAR 的 Rust 分层端口；原生同协议优先保真透传，跨协议只接纳可证明的 Canonical 无损映射，旧 CPA 的已知缺陷、无界输入、Secret 暴露、热路径可变配置与隐式降级不得复制。P12-08D-G 拆为可独立 review 的端口批次，缺少 Kiro/Grok Official 账号只延期对应 live receipt，不阻止本地实现与默认禁用的生产组成。 `CR-P12-08G1-001`：首次 G1 在 Chat JSON Text PASS、Chat SSE Text 客户端生命周期合并分类失败后已完整回滚；拆分 DONE/finish/Usage 固定类别，只允许从无值 receipt 精确替代失败 tuple，成功后才执行未发送 tuple，绝不重发已 PASS tuple。 `CR-P12-08G1-002`：替代 tuple 精确为 Chat SSE finish 缺失后，只允许一次 OpenClaw-backed 直连结构分类；仅保留 key set、计数和 finish/DONE/error/Usage 类别，不保留值或正文，不计为 G1 PASS。 `CR-P12-08G1-003`：结构分类发现 choice.message、delta.reasoning_content 及 choices 同帧 Usage 后，仅追加一次同形单发送值类别分类；reasoning 非空时禁止兼容放宽。 `CR-P12-08G1-004`：旧 CPA 原生 Chat 仅透传、不能证明 message 可丢弃，故追加一次仅保留嵌套键/值类别/与既有 delta 相等关系的单发送分类；只准入可逐字段证明为重复的最终 message、空 reasoning 与单次同帧 Usage，其余继续 fail closed。 `CR-P12-08G1-005`：首个修复 artifact 仍产生 stream error 后，分类证明仅最终重复汇总帧改用 `chat.completion` 与另一 ID；只允许该帧在合法 finish、message 与既有 text/Tools 完全相等时复用原 Canonical ID，其它 object/ID 变化仍拒绝。 `CR-P12-08G1-006`：固定旧 CPA fixtures 证明后续 Chat delta 的 `role:null` 与终端 Tool delta 的 `tool_calls:null` 表示无增量；仅将这两个 null 视为 absent，非 null 类型与生命周期仍严格校验。 `CR-P12-08G1-007`：精确谓词分类排除其它门后，确认同一流在非终止 chunk 重复声明相同 assistant role；只把相同字符串声明视为幂等，其它 role/类型仍拒绝。 `CR-P12-08G1-008`：生产解码器内存管道与封闭变异矩阵把失败收敛到终端 summary 同时重复发送的完整 text delta；仅当既有 Canonical text、终端 delta text 与 summary message text 三者完全相同、非空且无 Tool 时抑制第二次 TextDelta，其它形状仍走原严格拒绝。 |

| 已批准变更（续 6） | `CR-P12-08G1-009`：逐帧无值序列确认真实终端 `chat.completion` summary 可含 Message/finish/Usage 而完全省略 delta；仅该终端形状可省略，普通 chunk 与显式 null/错误类型仍拒绝。 |
| 已批准变更（续 7） | `CR-P12-08G1-010`：Chat SSE Text 已通过后，Chat JSON Tool 在零 upstream Attempt 前本地 4xx，而同形 `tool_choice:required` 直连为 2xx；Chat/Responses 仅增加 string required 且必须存在 typed function Tool，未知/对象 choice 与无 Tool required 仍拒绝。 |
| 已批准变更（续 8） | `CR-P12-08G1-011`：c71351e artifact 将 Chat JSON Tool 从 decoder 4xx 推进为 Router canonical admission 的零 Attempt 5xx；仅准入同协议、目标命名空间且结构严格合法的 Chat/Responses/Messages Tool choice，required/any 必须存在 Tool，跨协议仍因无已审查的无损映射而 fail closed。 |
| 已批准变更（续 9） | `CR-P12-08G1-012`：f2689b2 artifact 的 Chat JSON Tool 已产生 upstream Attempt，但在 decoder 以 `StreamTruncated` 失败；只允许一次同形非流式 Tool 结构分类，CC Switch 仅只读且不得修改，仅保留封闭 key/type/count/finish/decoder-gate 类别，不保留任何值或正文。 |
| 已批准变更（续 10） | `CR-P12-08G1-013`：CR-012 将唯一差异收敛到非流式 Tool call 的额外 `index`，但未保留其顺序关系；仅追加一次同形分类，输出 index 是否为唯一无符号整数且严格等于零基数组位置，成立才可将其作为冗余 wire 元数据忽略。 |
| 已批准变更（续 11） | `CR-P12-08G1-014`：398a1a1 artifact 使 Chat JSON Tool 通过后，Chat SSE Tool 在一帧安全错误处停止；只允许一次同形 SSE Tool 结构分类，保留封闭 key/type/count/finish/DONE/Usage、index 位置关系及 summary 与 delta 相等布尔值，不保留任何响应值。 |
| 已批准变更（续 12） | `CR-P12-08G1-015`：CR-014 发现六个 Tool delta 均重复完整 identity/name 键，但未保留值类别和与首次声明的关系；仅追加一次同形分类，只有 absent/null/empty 或与首次声明完全相等的重复元数据才允许幂等忽略，冲突值继续拒绝。 |
| 已批准变更（续 13） | `CR-P12-08G1-016`：CR-015 证明 continuation metadata 全为空，summary 的 type/name/arguments 与流完全相等但 call ID 被重建；仅终端冗余 summary 可在位置及完整 Tool 语义相等且新 ID 自身合法时保留首次流式 ID，普通非空冲突仍拒绝。 |
| 已批准变更（续 14） | `CR-P12-08G1-017`：4d16c3a artifact 使 Chat SSE Tool PASS 后，Responses JSON Text 在一个受控 5xx 处停止；仅允许一次精确同形单发送，在服务重启前通过受保护 loopback 管理面读取该请求的有界 attempt outcome/stage，不保留请求或响应值。 |
| 已批准变更（续 15） | `CR-P12-08G1-018`：CR-017 复现唯一 failed/decoder Attempt；仅允许一次同形直连 Responses JSON Text 结构分类，CC Switch Krill 配置只读且不得修改，只保留封闭 root/output/content/usage key/type/count/status 类别及 decoder gate 布尔值。 |
| 已批准变更（续 16） | `CR-P12-08G1-019`：CR-018 将首个差异收敛到 6 个 root、3 个 message 及 1 个 Usage detail 扩展字段，但未证明其值类别；仅追加一次同形单发送，保留 null/empty/zero/nonzero、容器子键类别、phase 固定关系与完成时间顺序布尔值。 |
| 已批准变更（续 17） | `CR-P12-08G1-020`：CR-019 证明完成时间有序、moderation=null、phase=final_answer、cache_write_tokens=0，但 penalties 为浮点、Tool usage/turn metadata 为嵌套对象；仅追加一次同形分类，保留有限数零值、已知 cache retention、嵌套数值叶全零及两处 turn_id 合法相等布尔值。 |
| 已批准变更（续 18） | `CR-P12-08G1-021`：CR-020 证明 penalties=0、retention 属于固定类别、Tool usage 全数值叶为 0、turn metadata 合法相等；Responses decoder 仅准入这些精确零值/冗余形状，cache_write_tokens 仅准入 0，任一非零、未知或冲突继续拒绝。 |
| 已批准变更（续 19） | `CR-P12-08G1-022`：CR-021 exact-SHA artifact 仅续跑 Responses JSON Text 一次仍为 `http_5xx`，且精确二进制与回滚边界均验证；仅允许一次同形、无重试、无值直连结构分类，用于判断上游响应是否相对 CR-020 发生形状变体；CC Switch 仍只读。 |
| 已批准变更（续 20） | `CR-P12-08G1-023`：CR-022 当前无值结构与 CR-020 完全相同，但 Python 分类器会合并重复 JSON 名而 Rust decoder 会预解析拒绝；仅允许分类器记录重复对象/名出现次数及零值布尔量，然后进行一次同形直连确认；不保留重复名或值。 |
| 已批准变更（续 21） | `CR-P12-08G1-024`：CR-023 证明当前响应无重复 JSON 名；源码复核发现直连分类器缺少 CPAR 固定 Krill 兼容 User-Agent，因此此前“同形”不包含完整请求头等价；分类器补齐精确四头后仅允许一次无重试分类。 |
| 已批准变更（续 22） | `CR-P12-08G1-025`：CR-024 完整请求头分类仍与旧收据相同；随后只读比对证明失败的 G1 v1 图与当前 CC Switch Krill 的 endpoint/model 均不同，且本机无 provider 匹配旧图。旧图不再代表生产目标；建立 `p12-08g1-codex-v2` 当前 Krill 图并从 0 重新验证 12 tuple，首个失败即停。 |
| 已批准变更（续 23） | `CR-P12-08G1-026`：G1 v2 从 0 执行后前 6 tuple PASS，含 Responses JSON/SSE Text；第 7 个 Responses JSON Tool 为 `http_5xx` 并完整回滚。仅重试失败 tuple 一次，回滚前读取同进程 Attempt 数量、outcome 与封闭 stage；不重发前 6 tuple。 |
| 已批准变更（续 24） | `CR-P12-08G1-027`：CR-026 失败 tuple 仅发送一次并回滚，但诊断器误查 `p12-request-*`，而生产 metadata factory 源码证明实际为 `p1-request-*`，因此 Attempt 投影未被读取。仅允许用正确封闭 ID 空间再重试失败 tuple 一次并立即查询；仍不重发前 6 tuple。 |
| 已批准变更（续 25） | `CR-P12-08G1-028`：CR-027 使用正确前缀仍无投影；源码与持久日志证明 metadata factory 每次重启从序号 0 开始，但 event_id 全局唯一，导致重启后 Attempt 持久冲突且管理面 fail closed。为 factory 增加惰性 128-bit 随机进程命名空间加单调序号，随机源失败则请求失败，Debug 隐藏命名空间。 |

本文是后续开发的唯一执行基线。功能矩阵定义“做什么”，行为契约定义“必须怎样表现”，本文定义“按什么顺序、交付什么、怎样证明完成”。

文档优先级：

```text
用户明确的新指令
  > 本开发计划
  > 关键行为与兼容性契约
  > 目标架构与专项设计
  > 功能矩阵的初始建议
  > 参考项目当前实现
```

### 0.1 旧 CPA 行为移植原则（`CR-P12-PORT-001`）

后续功能不再默认从空白设计。对旧 CPA 已实现的能力，先固定参考版本、源文件、对应测试和可观察
行为，再将其端口到 CPAR 的 Rust 边界。旧 CPA 是行为与兼容性的主要来源，不是 CPAR 内部架构、
安全缺陷或实现语言的复制模板；用户指令、本文和已冻结行为契约仍具有更高优先级。

| 旧 CPA 参考边界 | CPAR 承载边界 | 移植要求 |
|---|---|---|
| HTTP handlers、stream bootstrap/error | `gateway-http-actix`、`gateway-stream` | 保持状态码、SSE 生命周期、`[DONE]`/终止顺序；保留正文、帧、idle/total 上限与取消传播 |
| translator registry 与 Chat/Responses/Claude translators | 三个 `protocol-*` crate、`gateway-router::protocol_transform` | 原生同协议优先只改模型/受控字段；跨协议经 Canonical；不可表达语义在出网前稳定拒绝 |
| Codex/Claude/OpenAI-compatible/xAI executors | `provider-*` crate、`apps/gateway` runtime | 端点、header、OAuth/API-key、请求修整、response decode 和错误分类逐项端口；网络统一经过 DNS-pinned egress |
| auth、refresh、credential selection | `gateway-auth`、Provider credential runtime、router scheduler | 保持账号能力与 refresh 行为；使用 SecretRef、CAS/singleflight、隔离 Quota/Health/Circuit，不复制明文或隐式 fallback |
| model registry、alias 与 provider availability | `gateway-catalog`、`gateway-control`、`gateway-router` | 保持模型可发现与 provider-aware 可用性；发布期组成 fail-closed，Route Explain 可说明选择/拒绝 |
| watcher/config/runtime reload | `gateway-store`、control-plane snapshot | 仅通过已验证不可变 snapshot 发布；流式热路径不读文件、不查 SQLite、不观察半更新状态 |
| usage、reasoning/tool/session tests | Canonical、protocol/provider tests、differential harness | 将旧测试意图移植为脱敏 fixture/property/differential 测试，不机械复制测试数据中的敏感值 |

每个移植 Slice 在写实现前必须提交或在报告中冻结一份 `Legacy Behavior Manifest`，至少包含：旧
CPA tag/commit、参考源文件、参考测试、输入/输出不变量、错误/stream 边界、CPAR 目标模块和有意差异。
实现后必须给出三类结论：`PARITY`（行为等价）、`INTENTIONAL_HARDENING`（CPAR 更严格）或
`UNSUPPORTED_FAIL_CLOSED`（当前不可无损表达）。任何未分类差异都阻止该 Slice 本地通过。

移植顺序固定为：行为清单与测试意图 → Rust 类型/边界 → 最小端口实现 → 定向 parity 测试 →
安全偏差 review → Slice 报告。不得先大规模复制代码再补契约，也不得为了追求旧 CPA 表面兼容而
降低已有正文限制、Secret、SSRF、重试/FSE、Quota/Circuit 或 snapshot 原子性门禁。

## 1. 严格执行规则

### 1.1 任务状态

计划中的任务只能处于以下状态：

| 状态 | 含义 |
|---|---|
| `PENDING` | 前置条件未满足或尚未开始 |
| `IN_PROGRESS` | 当前正在执行；全计划同时最多一个 |
| `LOCAL_PASS_PENDING_PHASE_GATE` | 正常 Task 的实现、review、指定本地测试、格式与 Secret 检查已通过，证据已提交；等待本 Phase 唯一的远端正式 Delivery Gate。不是 `DONE`，也不计为 `IN_PROGRESS`。 |
| `LOCAL_PASS_PENDING_CI` | 仅用于 CI/workflow/cache/required-status 等必须提前在 GitHub 验证的例外 Task；本地条件已通过，等待该一次明确记录的提前远端 Gate。不是 `DONE`，也不计为 `IN_PROGRESS`。 |
| `DONE` | 代码、测试、文档和证据均完成 |
| `BLOCKED` | 已明确记录阻塞条件，无法继续 |
| `DEFERRED` | 经用户批准移出当前发布范围 |

### 1.2 每个任务的执行循环

1. 读取本文，确认当前 Phase、Task 和前置依赖。
2. 将且仅将一个 Task 标记为 `IN_PROGRESS`。
3. 实现该 Task 的最小完整改动，不夹带下一 Task 功能。
4. 运行该 Task 指定的定向测试、review、格式与 Secret 检查；安全、Schema、迁移、重试、依赖、
   公开边界或 CI 变更额外运行本地完整门禁。整合全局快速/完整门禁属于 Phase preflight，不对普通
   Task 机械重复。
5. 保存可复查证据：测试输出、基准、Fixture、日志或报告，并将实现、测试与该 Task 的证据尽量合并为同一提交。
6. 同步代码文档、行为契约和必要的矩阵状态，并将正常 Task 标记为 `LOCAL_PASS_PENDING_PHASE_GATE`；不得为普通 Task 单独启动远端 Code/docs Gate。
7. 在保持全计划仅一个 `IN_PROGRESS` 代码 Task 的前提下，`LOCAL_PASS_PENDING_PHASE_GATE` Task 是同一 Phase 后续依赖的有效本地证据。下一 Task 只能在前置本地证据充分时开始；发现回归时冻结受影响的后续 Task，先修复最早受影响提交。
8. 只有 CI/workflow/cache/required-status、部署控制面，或其它明确写入计划且无法由本地证明的变更，才可使用 `LOCAL_PASS_PENDING_CI` 并启动一次提前远端 Gate；该例外必须写明原因、范围和不通过时的冻结边界。
9. Phase 内全部 Task 均为 `LOCAL_PASS_PENDING_PHASE_GATE` 或已完成例外 Gate 后，运行整合本地完整门禁、Phase review 和 Phase-specific 验收。随后以 Phase closeout target 创建注释 tag，触发该 Phase 唯一的远端正式 Fast + Full Delivery Gate；不得先对同一 target 运行 Code Gate 再重复运行 tag Gate。
10. 该远端 Delivery Gate 通过后，Phase 内正常 Task 满足 Definition of Done 并一并标记 `DONE`；失败时不得进入下一 Phase，须从最早受影响提交修复并重建同一 Phase closeout target。

### 1.3 禁止事项

- 不因顺手方便实现未进入当前 Phase 的功能。
- 不用真实 Provider 的特殊字段污染 `gateway-core`。
- 不跳过 Mock、Fixture 或差分测试直接接管生产流量。
- 不在流式热路径查询 SQLite、读取配置文件或执行网络模型发现。
- 不在任何日志、Fixture、错误信息或 Git 历史中保存真实 Secret。
- 不把“能编译”“手工请求成功”单独视为 Task 完成。
- 不在 Phase Gate 失败时用 Feature Flag 掩盖必需功能后继续下一阶段。

### 1.4 计划变更流程

以下变化必须创建 Change Request，并由用户明确批准：

- 调整 Phase 顺序或跳过 Gate。
- 新增或删除公开接口、Provider、协议或持久化实体。
- 修改 Canonical Request/Event、错误分类、重试边界或 Secret 方案。
- 把 `Later/Drop/PENDING` 功能提前加入当前发布。
- 放宽安全、性能、测试或部署门槛。
- 改变 Task 状态语义、GitHub required Gate 分类或 Phase Gate 触发条件。

Change Request 格式：

```text
CR-ID:
原因:
影响的 Task / Matrix ID / ADR:
兼容性与迁移影响:
测试与回滚变化:
用户批准:
计划版本变更:
```

仅修正文案、补充测试或不改变对外行为的内部重构，可以在原 Task 内完成，但仍需记录在完成证据中。

### 已批准 Change Request：CR-EXEC-001

```text
CR-ID: CR-EXEC-001
原因: P0-P3 回顾显示，P3 的 39 次 GitHub workflow 累计约 516 分钟；近期 Full gate 的
      cargo-deny/cargo-audit 固定安装约占单次 Full 运行时长的 74%。P3-10 的真实 Endpoint
      验收还暴露出固定四探针 harness 不适合定位单个 Target/Mode 传输问题。
影响的 Task / Matrix ID / ADR: 执行规则、CI、P4-00 与未来所有需要真实 Endpoint 验证的
      Provider Task；不改变任何功能矩阵、公开 API、Canonical 类型、Provider 协议、Schema、
      数据库、部署或既有 P0-P3 验收结果。
兼容性与迁移影响: 无客户端兼容性、数据或部署迁移。只新增受控交付状态、CI 分类和 test-only
      诊断能力；正式四探针验收 harness 与其既有隐私/调用上限保持独立。
测试与回滚变化: P4-00 必须证明 cache miss/hit 的版本校验、code/docs/phase-tag Gate 分类、
      docs-only Secret/链接检查、状态转换阻断规则和单探针零授权/一探针上限。回滚为禁用缓存、
      恢复所有提交 Full Gate、停止使用 LOCAL_PASS_PENDING_CI 并移除未使用诊断 harness；
      不影响已完成 Phase。
用户批准: APPROVED，2026-07-21（要求将全部提速方案写入开发计划）
计划版本变更: v1.2
```

### 1.5 交付提速纪律（CR-EXEC-001）

1. **CI 分层而不降门槛。** Rust、`Cargo.toml`/`Cargo.lock`、工具链、workflow、脚本、
   迁移、Fixture、契约或安全策略变更必须在所属 Phase 的远端正式 Delivery Gate 运行 GitHub
   Fast + Full supply-chain Gate；普通 Task 不逐个触发远端 Gate。纯报告、索引或计划状态变更在
   不属于 Phase closeout 时运行显式 `docs-only` Gate：Markdown/格式、文档链接、Secret scan 和
   计划一致性检查；它不得伪装为 Full 成功。每个 Phase closeout tag 始终强制运行 Fast + Full。
2. **缓存只加速受版本约束的工具，不替代验证。** CI 可缓存 `cargo-deny`、`cargo-audit` 的
   二进制及其 Cargo registry/git 下载；缓存 key 必须包含 runner OS、固定 Rust 版本和
   `tools/quality-tool-versions.env` 摘要。每次恢复后仍执行版本检查；缺失或不匹配时仍以
   `cargo install --locked` 重新安装。不得缓存 Credential、环境文件、真实测试配置或把 cache hit
   当作供应链通过证明。预装镜像只有在来源固定、版本可复查且另有供应链审查后才可替代缓存。
3. **状态流水线保持一个代码 Task。** 正常 Task 使用 `LOCAL_PASS_PENDING_PHASE_GATE`；它可作为
   同一 Phase 后续 Task 的前置证据，但不允许跨 Phase、合并、发布或提前宣称 `DONE`。
   `LOCAL_PASS_PENDING_CI` 仅表示不可本地证明的提前远端例外。任何 Fast/Full 失败都会冻结
   受影响的后续工作，先恢复最早失败 Task 的绿色状态；同一时刻仍只有一个 `IN_PROGRESS`。
4. **文档证据随 Task 写入，Phase 一次收口。** 每个 Task 的实现、测试、ADR/Contract 和报告
   骨架都在其提交中完成。Phase closeout target 在 tag 前聚合不可变本地证据与报告，不为每个
   Task 或 tag 结果再创建 docs-only 状态提交；GitHub run、tag 和 job summary 是远端最终证据。
5. **真实 Endpoint 先诊断、后验收。** 未来需要真实 Endpoint 的 Phase 必须使用与正式验收
   harness 分离的 ignored 单探针诊断路径。它必须要求单独的显式授权、Target、Mode 和精确
   `max_external_requests=1`，保持 `max_attempts=1`、无自动重试/failover、脱敏输出和有界
   timeout/read；普通 CI、缺失授权或错误配置必须零网络。它只能定位一个已授权 Target/Mode，
   不能替代正式多 Target 验收，也不能重用或扩大既有 P3-10 授权。
6. **先收集 readiness，再消费真实调用。** 在每个真实 Provider 验收前，先完成不触网的
   Base URL/egress/profile/model-alias/调用预算预检；用户批准的调用预算、网络 profile 和
   停止条件必须在第一次真实请求前写入私有 operator-controlled 配置与公开脱敏计划。
7. **度量目标。** P4-00 完成后，warm Full Gate 中“安装固定质量工具”硬性目标不高于 90 秒，
   运行目标不高于 10 秒；每轮 Phase Gate 记录 Fast、Full、docs-only 的次数与 step 时长。若
   连续两轮未达到目标，先调查 cache key、安装源和 runner，而不是削弱 Full Gate。

### 已批准 Change Request：CR-EXEC-002

```text
CR-ID: CR-EXEC-002
原因: P4-00/P4-01 的实测表明，质量工具缓存对 GitHub ref 可见：P4-01 在新分支首次 Code Gate
      仍发生约 495 秒冷安装，而同一 ref 的暖态测量将安装降为约 1 秒；同时现有 Full job 在独立
      runner 重复了 Fast 已执行的 Workspace 检查。P4-00/P4-01 的两次 docs 状态提交也产生了
      无新的功能证据的额外等待。
影响的 Task / Matrix ID / ADR: 执行规则、CI、P4-02 及后续顺序代码 Task；新增 ADR-0023 与
      BC-DELIVERY-002。它不改变功能矩阵、公开 API、Canonical 类型、Provider 协议、Schema、
      数据库、部署、P0-P3 结果或“全计划最多一个 IN_PROGRESS Task”规则。
兼容性与迁移影响: 无客户端、数据或部署迁移。顺序代码 Task 使用 cache-visible delivery ref；新
      ref 必须先有同 ref cache seed 或使用经批准的共享/default ref。GitHub Fast 仍是完整 Workspace
      快速检查，GitHub Full 改为依赖同 SHA Fast 的版本校验、cargo-deny 和 cargo-audit 补充检查；
      本地 ./scripts/check.sh full 保持完整检查。代码 Gate 后只做一次 docs-only 收口。
测试与回滚变化: P4-02 必须验证 supply-chain mode、workflow 结构、cache hit/miss summary、Fast
      + Full fail-closed required gate、local full 以及单次 docs closeout。缓存 miss 不是正确性失败，
      但必须在报告中记录并触发性能调查。回滚为将 GitHub Full 恢复为 ./scripts/check.sh full、停止
      使用 cache-visible ref 规则和单次收口；不影响已有 Task 产物。
用户批准: APPROVED，2026-07-21（要求按该方案优化开发计划并以 P4-02 测试效果）
计划版本变更: v1.3
```

### 1.6 缓存可见 Phase 交付与补充供应链 Gate（CR-EXEC-002、CR-EXEC-007）

1. **共享缓存优先于短生命周期分支缓存。** P4 的 `codex/p4-01-catalog-singleflight` 曾证明同 ref
   的暖态缓存有效，但最终 tag 不能读取该分支缓存而发生冷安装。自 `P5-00` 通过后，固定质量工具
   缓存必须以 runner OS、固定 Rust 版本和 `tools/quality-tool-versions.env` 摘要为 key，并先在
   `main`/default ref 创建可供 Phase tag 读取的受版本约束 seed。一个 Phase 只在末尾交付时，单靠
   Phase 分支并不能产生可复用缓存；缓存 miss 仍 fail-safe 地重新安装，不得跳过版本校验。
2. **Fast 完整、Full 补充。** GitHub Fast 是完整 Workspace fast check。GitHub Full 依赖同一
   workflow/SHA 的 Fast，仅执行固定质量工具版本检查、`cargo deny check` 与 `cargo audit`；Required
   Gate 对两个结果均 fail-closed。本地 `./scripts/check.sh full` 继续执行完整 Fast 加供应链检查，
   因此本地需要完整门禁的变更只运行一次 `full` 即覆盖 Fast，不机械地紧接着重复 `fast`；Phase
   Tag 仍逻辑要求 Fast + Full。
3. **Phase 一次收口。** 代码提交包含实现、测试、ADR、契约和报告骨架；Phase closeout target 在
   tag 前汇总本地证据。普通 Task 与 tag Gate 通过后均不得再为 run ID 创建 docs-only 状态提交；
   tag、GitHub job summary 和下一 Phase 开始时的计划状态共同构成外部收口。独立于任何 Phase 的
   纯计划/报告变更仍走一次 docs-only Gate。
4. **缓存可观测性与目标。** Full job 必须把 cache hit/miss 写入 GitHub job summary。miss 不会降低
   Gate 的正确性结论，但报告必须记录原因。暖态质量工具安装运行目标 `<=10s`、计划硬门槛 `<=90s`；
   暖态 Phase Delivery workflow 目标 `<=4min`（不含 GitHub queue），独立 docs-only workflow 目标
   `<=45s`。P5-00 必须以提前远端例外证明 default-ref seed、tag restore、cache hit/miss summary 和
   fail-closed fallback；之后不额外为普通 Task 手工暖态重跑。

### 已批准 Change Request：CR-EXEC-003

```text
CR-ID: CR-EXEC-003
原因: P4-02 的总 wall-clock 约 43 分钟，但已验证的 GitHub Code Gate 仅约 4 分 05 秒、docs-only
      Gate 仅约 40 秒。本地完整门禁约 43 秒；剩余时间主要来自重复范围读取、分散补丁、一次
      Clippy 返工、重叠验证、Gate 通过后才整理 closeout，以及手工证据汇总。需要压缩代理执行
      时间而不降低 review、Full Gate、Secret 扫描、文档或单 Task 纪律。
影响的 Task / Matrix ID / ADR: 后续所有顺序 Task 的执行循环、汇报与测量；不改变 Matrix、公开
      API、Canonical 类型、Provider 协议、Schema、数据库、部署、GitHub required Gate 或任何已
      完成 Task 的验收。无需新增 ADR，因为这是代理工作流而非系统架构决策。
兼容性与迁移影响: 无客户端、数据、部署或安全迁移。仍最多一个 IN_PROGRESS Task；仍使用既有
      Code Gate、LOCAL_PASS_PENDING_CI 和单次 docs-only 收口。
测试与回滚变化: 本次计划变更走 docs-only Gate。后续每个 Task 必须在报告中记录范围等级、局部
      执行、Code Gate、closeout 和总时长；若时间预算连续两次超标，先记录具体阻塞并调整执行
      方法。回滚只移除本节的代理效率纪律，不移除任何测试、review、Gate 或功能产物。
用户批准: APPROVED，2026-07-21（要求将代理执行提速方案写入开发计划）
计划版本变更: v1.4
```

### 1.7 代理执行提速纪律（CR-EXEC-003）

1. **先建 Task Card，再读代码。** 每个 Task 开始时，在本轮可见的执行说明中固定：范围等级
   `S/M/L`、依赖、最多需要修改的文件、必须成立的不变量、定向测试、最终 Gate 和禁止项。首次
   读取只限计划行、直接依赖、最近相似实现及对应行为契约；只有出现具体编译/测试/设计冲突时
   才扩展调查范围，不能为了“可能有用”重复浏览已确认文件。
2. **显式的本地执行预算。** `S`（单 Crate、无 Schema/公开边界）目标为从 Task Card 到代码提交
   `<=15min`；`M`（最多三 Crate 或一个契约边界）目标 `<=25min`；`L` 必须先给出细化子计划与
   预算，不能隐式按小 Task 推进。预算是过程度量而非降低质量的 deadline；超出时必须记录原因。
3. **集中补丁而非微补丁。** 范围已明确后，代码、定向测试、ADR/Contract/报告骨架和索引应按
   一个一致的改动集完成。格式化或由失败测试直接要求的修复可以追加；不为尚未验证的猜想拆分
   多轮文件编辑。写操作保持串行以避免冲突，互不依赖的只读检查并行执行。
4. **开发验证去重。** 开发中仅运行当前边界的定向 test/Clippy 以获得快速反馈。最终需要完整
   门禁时只运行一次 `./scripts/check.sh full`，它已覆盖 Fast 与 supply-chain；不得在其前后机械
   追加独立 `fast`、`supply-chain` 或相同范围 Clippy。staged Secret/whitespace review 仍必须在
   提交前执行，且不被 Full 替代。
5. **Phase Gate 等待与 closeout 重叠。** Phase tag 推送前完成 closeout target、报告表格和索引
   diff；远端 Gate 运行时只读取一次必要的 job 摘要。Gate 通过后以 tag/run/job summary 作为不可变
   证据，不创建额外 docs-only 收口；Gate 失败时修正最早受影响提交并重建 Phase closeout target，
   绝不借草稿进入下一 Phase。
6. **证据模板化而不删证据。** 后续 P4 Task 复用固定 ADR、Contract、报告、追踪行和 closeout
   模板，只填写本 Task 的决策、行为、测试和时延差异。没有新增架构决定或可观察行为时，不凭
   习惯额外创建文档；现有要求的证据、链接、Secret 约束仍完整保留。
7. **远端查询最小化。** workflow 运行时使用低频状态轮询；完成后先读取一次完整 job 摘要，仅在
   cache、失败或安全证据缺失时读取相关 job log。不得为复制同一状态或刷新无变化页面重复查询。
8. **统一时延报告。** 每个 Task 报告记录 `Task Card`、`代码提交`、`本地 review/测试通过`、范围
   等级、重复验证次数、返工次数和超预算原因；每个 Phase report 额外记录 `closeout target/tag`
   与 `Phase Delivery Gate 通过`。此数据用于下一 Task 纠正流程；连续两次同类超预算必须先优化
   执行方法，再考虑扩大并行度。

### 已批准 Change Request：CR-EXEC-004

```text
CR-ID: CR-EXEC-004
原因: 简单、确定且低风险的执行（例如已知命令的只读检查、固定格式的 docs-only 收口、明确状态
      查询）不需要默认占用高级模型与深度思考。一次短 GitHub TLS 验证曾与后续 P4 开发混入同一
      高强度执行回合，导致用户等待时间和实际探针耗时无法清楚区分。需要在不降低质量门禁的前提
      下，按风险和不确定性自动选择 Luna 与最低足够的思考强度，并保持短任务的独立交付边界。
影响的 Task / Matrix ID / ADR: 后续所有代理执行、Task Card、状态查询、docs-only 收口与报告。
      不改变 Matrix、公开 API、Canonical 类型、Provider 协议、Schema、数据库、部署、GitHub
      required Gate、真实 Provider 授权或既有功能验收。无需新增 ADR，因为模型路由是执行层策略。
兼容性与迁移影响: 无客户端、数据、部署或安全迁移。Luna 的可用性由执行平台决定，仓库不伪造
      已切换状态；平台不能提供 Luna 时，使用当前可用模型的等价低强度通道并明确记录 fallback。
测试与回滚变化: 本次计划变更走 docs-only Gate。后续抽样检查 Task Card/状态更新是否包含执行通道、
      思考强度和 fallback（如有）；发现简单任务被无理由提升时先纠正路由。回滚仅移除本节的模型
      路由规则，不移除 review、Secret scan、测试、Gate、单 Task 或真实调用授权约束。
用户批准: APPROVED，2026-07-21（要求简单任务自动切换 Luna 并按需选择思考强度）
计划版本变更: v1.5
```

### 1.8 模型与思考强度路由（CR-EXEC-004）

1. **先选执行通道，再开始工具调用。** 每个请求先按风险、不确定性、可逆性和影响面路由；既有
   `S/M/L` 仍只表示代码范围和时延预算，不能被误当成模型等级。用户显式指定的模型或思考强度
   始终优先于本节默认值。
2. **简单任务默认走 Luna-fast。** 具备模型切换能力时，下列任务必须优先切至 `luna`：已知命令
   的只读状态/版本/连通性检查、单目标且不改状态的重复采样、固定证据驱动的 docs-only 收口、
   已确认文件中的机械索引或格式修正、以及单一 GitHub/Git 状态查询。普通简单任务默认使用
   `low`；需要合并固定输出、执行多步检查、机械写入或有限语义判断时使用 `medium`。`minimal`
   仅限零判断的单一事实/命令查询，不能用于普通执行、诊断、写入或 Gate 判断。不得因为任务来自
   既有大 Task 就自动升级为高级模型或深度思考。
3. **用最低足够强度逐级升级。** 范围明确的代码改动、有限测试失败定位或需要语义判断的文档变更
   使用当前默认模型的 `medium`。只有架构设计、跨 Crate 公开边界、并发/内存安全、安全审查、未知
   根因排查、非可逆配置变更、真实 Provider/凭据操作，或用户明确要求深度分析时，才可使用高级
   模型和 `high`/`xhigh`。每次升级必须在 Task Card 或首个状态更新中写明触发条件；`high`/`xhigh`
   不是默认模式。
4. **Luna 不可用时 fail-transparent。** 执行平台未暴露 `luna`、切换失败或任务在执行中越过
   Luna-fast 边界时，不得悄悄以高级模型继续。应在下一条可见状态中记录 `fallback: luna unavailable`
   或升级原因，并使用当前可用模型的 `minimal`/`low`（或在风险升级后相应强度）。任何安全、真实
   调用或代码质量 Gate 都不能因 fallback 被跳过。
5. **短任务独立交付。** Luna-fast 请求应先给出其自身结果；除非用户在同一条请求明确要求继续，
   不把后续高强度开发、Gate 等待或无关排查折叠进该短任务。若外部命令超过 60 秒，先报告正在等候
   的具体对象和已知结果；不得用未归因的墙钟时间冒充简单检查成本。
6. **质量边界不因模型降级而变化。** 低强度通道可以减少推理和范围读取，不能省略已适用的 review、
   Secret scan、定向测试、Full/Code/docs Gate、授权检查或最终状态核对。任务一旦需要这些边界之外
   的判断，必须先升级通道而不是在 Luna-fast 中猜测。
7. **记录并复盘路由效果。** 每个代码 Task 的 Task Card 记录 `执行通道`、`思考强度`、选择理由和
   fallback（如有）；简单任务至少在首条状态更新中声明通道。每五个简单任务抽样比较“收到请求至
   首个结果”的墙钟时长；若两次因错误路由而超过 5 分钟，先修正规则或任务切分，再增加模型强度。

### 已批准 Change Request：CR-EXEC-005

```text
CR-ID: CR-EXEC-005
原因: 当前主会话未必能在原位切换至 Luna。为落实 CR-EXEC-004 的低成本执行通道，可以在确有
      独立、低风险和可复核工作的情况下，使用低成本子代理代替主会话持有高级模型/深度思考；但
      无上限或嵌套委派会重新引入上下文竞争、隐藏等待和多 Task 并行风险，因此必须设定硬上限。
影响的 Task / Matrix ID / ADR: 后续执行编排、Task Card 和状态汇报；不改变 Matrix、产品代码、
      Canonical 类型、Provider 协议、Schema、数据库、部署、GitHub required Gate、真实 Provider
      授权或单 Task 规则。无需新增 ADR，因为这是代理调度策略而非系统架构决策。
兼容性与迁移影响: 无客户端、数据、部署或安全迁移。若平台未暴露 Luna 或其他已批准低成本模型，
      不伪造模型选择；主会话继续使用当前模型的最低足够强度。子代理的结果始终是建议或受控执行
      结果，不能替代主会话的 review、提交、推送或验收。
测试与回滚变化: 本次计划变更走 docs-only Gate。每次委派必须可从状态更新复原模型通道、作用域、
      活跃数量、返回结果和主会话复核；发现越权、嵌套或并发超限时立即停止并由主会话收束。回滚
      仅禁用本节的子代理 fallback，不影响 CR-EXEC-004 的原位模型路由和任何质量门禁。
用户批准: APPROVED，2026-07-21（当前会话不能切换模型时允许受限低成本子代理，并限制数量）
计划版本变更: v1.6
```

### 1.9 受限低成本子代理 fallback（CR-EXEC-005）

1. **触发条件全部满足才可委派。** 主会话不能原位选择 `luna`（或用户批准的低成本模型）；工作
   与当前 Task 的主实现不存在未解决的语义依赖；作用域、文件或命令、预期返回格式和停止条件均能
   在一条 prompt 中写清。仅为“多想一点”、缩短主会话责任或绕开 review 不得委派。
2. **硬数量上限。** 每个主会话同一时刻最多一个活跃低成本子代理；禁止子代理再创建子代理。第二
   个低成本子代理、任何并行代码 Task 或超过此上限的并发，必须获得用户新的明确批准。该上限不
   改变“全计划最多一个功能 Task 为 `IN_PROGRESS`”的规则。
3. **允许的受控工作。** 子代理可做已知命令的只读检查、有限重复采样、指定测试/格式/链接检查、
   精确文件范围内的机械文档改动，或主会话预先界定且可回滚的简单执行步骤。涉及写入时，prompt
   必须声明唯一文件所有权；主会话随后必须审阅 diff、运行适用 Gate，并决定是否保留结果。
4. **禁止的工作。** 子代理不得决定架构或公开边界、进行安全/并发最终审查、读取或处理 Secret、
   发起真实 Provider 请求、修改代理/TUN/网络设置、创建/切换主分支、提交/推送 Git、标记 Task
   `DONE`、批准 Gate，或替代主会话对用户作最终交付。遇到这些需求必须立即返回主会话升级。
5. **模型与思考强度。** 可用时子代理默认使用 `luna` 的 `low`；需要汇总固定输出、执行多步
   检查或机械写入时使用 `medium`。`minimal` 仅限零判断的单一事实/命令查询，且不得用于写入、
   诊断或 Gate 判断。平台没有可调用的低成本模型时，不得把高级模型伪称为 Luna 子代理；主会话
   改用自身等价的 `low`/`medium` 通道完成，或向用户报告该 fallback 不可用。
6. **可见性与收束。** 委派前的状态更新必须写出 `delegated`、模型/强度、任务边界和“活跃子代理
   数=1”；返回后必须写出命令/改动摘要、结果、耗时和是否升级。主会话确认无其它活跃子代理后，
   才可开始下一项委派或高强度开发，避免后台工作被折叠进无关短任务。

### 已批准 Change Request：CR-EXEC-006

```text
CR-ID: CR-EXEC-006
原因: `minimal` 适合零判断的单一事实查询，但对普通简单执行过低；过度压低思考强度会把速度收益
      变成遗漏检查或返工。需要保留 Luna 的效率优势，同时把简单任务的默认判断下限设为 `low`，
      并让有限多步/机械写入工作使用 `medium`。
影响的 Task / Matrix ID / ADR: CR-EXEC-004 与 CR-EXEC-005 的模型/思考强度路由；不改变功能、
      安全边界、Task 并发上限、review、测试、Gate、Provider 授权或部署。
兼容性与迁移影响: 无客户端、数据或部署迁移。只调整执行器的默认思考强度；用户显式指定强度仍
      优先，Luna/低成本模型不可用时的透明 fallback 规则不变。
测试与回滚变化: 本次计划变更走 docs-only Gate。后续复盘检查简单执行是否至少使用 `low`，并对
      多步检查/机械写入是否使用 `medium`；若质量证据显示该下限仍不足，再显式提升而非默默返工。
      回滚仅恢复 CR-EXEC-004/005 先前的强度映射，不影响质量门禁。
用户批准: APPROVED，2026-07-21（不将普通简单任务默认降至过低思考强度）
计划版本变更: v1.7
```

### 1.10 简单执行的思考强度下限（CR-EXEC-006）

1. **默认下限。** Luna-fast 和受限低成本子代理的普通简单执行默认 `low`，不是 `minimal`。
2. **有限步骤下限。** 需要汇总固定输出、运行多个已知检查、编辑指定文档或进行有限判断时使用
   `medium`；这仍不是高级模型/深度思考的默认入口。
3. **minimal 的窄例外。** 仅限不写入、不诊断、不作 Gate 结论的单一事实或单命令查询；不能因
   “任务很小”自动选择它。

### 已批准 Change Request：CR-EXEC-007

```text
CR-ID: CR-EXEC-007
原因: 用户要求将远端上传与正式测试从“每个 Task 一次”改为“每个 Phase 一次”，并将开发分支
      从“每个 Task 一个”改为“每个 Phase 一个”。P4 的实测也表明，同一开发分支的质量工具缓存
      暖态有效，但最终 tag 因 GitHub ref 缓存作用域未能读取该分支缓存，重新安装工具约 8 分 10 秒。
      需要减少重复远端 Gate，同时保留每个 Task 的本地 review 和定向测试。
影响的 Task / Matrix ID / ADR: 执行状态、Task 循环、Git 规则、Definition of Done、CI/cache
      交付流程、P5-00 及所有尚未开始的 Phase；仅取代 CR-EXEC-001 至 CR-EXEC-003 中“普通 Task
      必须逐个远端 Gate/逐个分支/docs-only 收口”的部分。不改变 P0-P4 已完成的验收、功能矩阵、
      公开 API、Canonical 类型、Provider 协议、Schema、数据库、部署、Secret 规则或真实调用授权。
兼容性与迁移影响: 无客户端、数据或部署迁移。每个 Task 保持独立提交、review、定向自动化测试、
      格式与 Secret 检查；远端 Fast + Full 仍 fail-closed，只改为 Phase closeout 的唯一正式运行。
      CI/workflow/cache/required-status 等无法本地证明的改动保留一次提前远端例外。
测试与回滚变化: P5-00 必须证明 Phase branch/tag 只产生一次普通正式 Delivery Gate、default-ref
      质量工具 cache seed 可被 tag 恢复、缓存 miss 安全回退、Fast + Full required 状态与例外 Gate
      冻结规则。每个普通 Task 必须保留本地定向测试/review；Phase 必须保留整合本地 full、跨 Crate
      /契约/Phase-specific 验收和一次远端 Fast + Full。回滚为恢复每 Task 分支和远端 Gate；不回溯
      或重跑已完成 Phase。
用户批准: APPROVED，2026-07-21（要求按 P 级分支与 P 级远端验收方案修改开发计划）
计划版本变更: v1.9
```

### 1.11 P 级分支与单次远端正式验收（CR-EXEC-007）

1. **一个 Phase，一个开发分支。** 未开始的 Phase 使用 `codex/p<phase>-<short-name>` 分支，
   例如 `codex/p5-anthropic`。该分支只承载该 Phase 的顺序 Task 和其修复，不跨 Phase、不并行
   功能开发；每个 Task 仍使用自己的 `P5-01:` 等提交前缀，保持可 review、可 bisect 和可回滚。
2. **Task 本地验收不延后。** 每个普通 Task 必须完成范围内的定向测试/Clippy、`cargo fmt --check`、
   staged Secret/whitespace review、行为契约/文档更新和独立代码 review 后才可进入
   `LOCAL_PASS_PENDING_PHASE_GATE`。安全、Schema、迁移、重试、依赖或公开边界改动额外运行一次
   本地 `./scripts/check.sh full`；不得把所有测试积压到 Phase 末尾。
3. **一次普通远端交付。** 全部 Task 本地验收后，执行整合本地 full、跨 Crate/契约/Phase-specific
   测试和 Phase review，将结果写入 closeout target，创建注释 `phase-p<phase>-complete` tag，并以
   Phase 分支和 tag 的单次交付事件运行 Fast + Full。tag 通过前，不得开始下一 Phase、合并或发布。
4. **不重复状态提交。** 正常 Task 不逐个 push、不逐个跑 Code/docs Gate，也不在 tag 通过后为 run ID
   另建 docs-only 提交。需要记录的远端证据放在 tag、GitHub run/job summary 和下一 Phase 开始时的
   计划状态中；独立计划文档变更仍可走 docs-only Gate。
5. **提前远端例外是窄边界。** CI workflow、cache、required-status、分支/发布控制或其它只能在
   GitHub/目标环境证明的改动，必须在该 Task 后跑一次提前远端 Gate，并在 Task Card 和报告中写明。
   该 Gate 不是普通 Task 的模板；真实 Provider 请求仍只按单独授权和既有调用边界执行。
6. **缓存与单次交付配套。** 因为最后才 push 的 Phase 分支没有自己的暖态 cache，P5-00 必须先建立
   default-ref 的受版本约束 cache seed，并验证 tag 能恢复它。缓存仅缩短安装；每次仍做版本验证、
   `cargo deny` 和 `cargo audit`，cache miss 必须安全重新安装并记录为性能调查项。
7. **完成语义。** `LOCAL_PASS_PENDING_PHASE_GATE` 是本地通过而非 `DONE`；Phase 唯一正式 Gate
   成功才把其中 Task 一并转为 `DONE`。任何本地或远端失败都回到最早受影响提交修复，不能用后续
   Task、Feature Flag 或报告文字掩盖。

### 已批准 Change Request：CR-P6-03-001

```text
CR-ID: CR-P6-03-001
原因: 用户于 2026-07-22 明确授权 P6-03 对指定 CPA OAuth 测试账号进行不受既有请求数/预算限制的
      综合验证。为避免把该授权误解为无限重试或扩大操作面，需要把它收敛为有限、预先记录且可复核
      的模型 × 模式验证矩阵。
影响的 Task / Matrix ID / ADR: 仅 P6-03 的真实验证操作政策、报告和 BC-PROVIDER-003；覆盖 Matrix
      `C28` 的固定 Build Responses 非流式与 SSE 兼容性。不改变公开 API、固定端点、Canonical 类型、
      Provider 代码、OAuth 账户状态、P6-04+ 行为、服务器配置或代理/TUN 配置。
兼容性与迁移影响: 无客户端、数据、数据库、部署或账号迁移。只使用一个由用户指定的 CPA OAuth
      账号；凭据在内存中投影为严格 OAuth JSON，不写入磁盘、日志或 Git。
测试与回滚变化: 在发送前记录有限的不同 `(模型候选, 模式, 网络配置)` 元组。每次 ignored harness
      调用仍必须固定 `P6_03_MAX_EXTERNAL_REQUESTS=1`，只会调用一次 send；同一元组不得自动重试、
      failover、刷新或循环。每个候选/网络配置组合只测试 `non_streaming` 和 `sse`；遇到非 2xx、超时、协议/语义
      错误或凭据不可用时，只记录脱敏类别并继续下一个不同元组。回滚为停止未执行元组并恢复此前的
      单探针授权文案；不需要撤销 Provider、服务器或网络状态。
用户批准: APPROVED，2026-07-22（“你自己决定，不限制次数方案，可以都测试”）
计划版本变更: v1.10
```

### 1.12 P6-03 有限真实验证矩阵（CR-P6-03-001）

1. **有限性优先。** “不限制次数”只取消历史的预算计数，不允许无界探测。开始前必须在 P6-03 报告中
   列出候选模型、`non_streaming`/`sse` 两种模式、网络配置及停止条件；没有记录的元组不得发送。
2. **一进程一请求。** 现有 ignored harness 的 `P6_03_MAX_EXTERNAL_REQUESTS=1` 不得放宽。不同元组
   必须通过独立进程调用；同一 `(模型, 模式, 网络配置)` 元组最多一次，且不含自动重试、刷新、
   candidate 选择、failover 或续接。只有前一有限矩阵全部得到无法区分的安全类别、另有 Change
   Request 明确登记诊断目标、并且该诊断只输出状态/内容类型/无值结构分类时，才可增加同元组的
   非自动诊断调用；每项诊断仍一进程一 send，必须比前一项增加更窄的新安全事实。原矩阵之外最多
   两项此类诊断，且第二项后绝不继续同元组调用；它们不能作为成功重试或替代功能验收。
3. **单账号、固定边界。** 仅使用用户指定的一个 CPA OAuth 账号，且仅访问 P6-03 固定
   `https://cli-chat-proxy.grok.com/v1/responses` 端点。不得枚举其它认证文件、改变账号、服务器、
   代理规则或 TUN 排除地址。
4. **最小语义与安全记录。** 每请求固定 `Reply with exactly: ready`、32 token 上限和已验证的精确
   Egress policy；报告只可记录候选标签、模式、网络配置、墙钟时间和脱敏的成功/失败类别，绝不记录
   Token、OAuth JSON、私有模型映射、请求体或响应内容。
5. **通过与停止。** 至少一个候选模型的固定端点非流式与 SSE 都得到有效 Canonical
   `ResponseStart`、文本和 `ResponseEnd`（无 `StreamError`）才可让 P6-03 进入
   `LOCAL_PASS_PENDING_PHASE_GATE`。所有候选均失败时保持 `IN_PROGRESS`/`BLOCKED` 并提交证据；
   P6-04 在此之前不得开始。

### 已批准 Change Request：CR-P6-03-002

```text
CR-ID: CR-P6-03-002
原因: 已登记的 T1-T8 都独立发送一次，并都安全地停止为 `response_protocol_failed`。原 harness
      故意不输出 HTTP 状态、内容类型或 JSON 结构，因而无法判断是上游以 2xx 返回错误对象、协议
      漂移，还是解码器不兼容。需要一项不记录原始响应值的分类诊断，外加一个明确的一次性诊断调用。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored harness、报告和 `C28` 的 live evidence；不改变
      生产 Provider、固定端点、Canonical 行为、请求体、凭据、账号状态、P6-04+、服务器或代理/TUN。
兼容性与迁移影响: 无客户端、数据、部署或账号迁移。诊断只输出 HTTP 状态类、内容类型是否符合预期
      和无值 JSON body shape；不输出 Header 值、URL、模型值、Token、请求/响应文本、ID 或错误文本。
测试与回滚变化: 增加合成 redaction 测试和一项 T9-DIAG：仅重观察已失败的 `build-static-01`、
      `non_streaming`、`socks5` 组合一次。它每进程仍为一 send，不自动重试，不能使该组合变为通过。
      回滚移除安全诊断输出并停止 T9；T1-T8 的事实证据保留。
用户批准: APPROVED，2026-07-22（“你自己决定，不限制次数方案，可以都测试”）
计划版本变更: v1.11
```

### 已批准 Change Request：CR-P6-03-003

```text
CR-ID: CR-P6-03-003
原因: T9-DIAG 已证明指定 OAuth token 取得 `2xx`、期望 JSON 内容类型和 error-like object，排除了
      网络/内容类型层，但还不能区分模型、凭据、请求或额度类的应用层错误。需要最终一次只检查错误
      对象标准 `code`/`type`/`param` 元数据的白名单映射；不读取或输出 message 等自由文本。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored harness、报告和 `C28` live diagnosis；不改变生产
      Provider、请求、固定端点、Canonical 行为、账号状态、P6-04+、服务器或代理/TUN。
兼容性与迁移影响: 无客户端、数据、部署或账号迁移。输出只可能是 `model`、`credential`、`request`、
      `quota` 或 `unrecognized` 类别；任何未知 code/type/param 值均不输出原值。
测试与回滚变化: 增加合成白名单/未知值脱敏测试，并登记最终 T10-DIAG：只重观察 `build-static-01`、
      `non_streaming`、`socks5` 一次。它是第二且最后一次同元组诊断，仍为一 send、无重试、不能通过
      P6-03；若结果仍不足以支持修复，Task 保持阻塞而非继续探测。
用户批准: APPROVED，2026-07-22（“你自己决定，不限制次数方案，可以都测试”）
计划版本变更: v1.12
```

### 已批准 Change Request：CR-P6-03-004

```text
CR-ID: CR-P6-03-004
原因: 用户明确授权使用当前服务器已部署 grok2api 的现有 endpoint 与 API key，取得一条实际
      Build 路由代理请求的脱敏参考证据，以区别“固定直连 Build profile 不被接受”和“Build
      账号/路由本身不可用”两种情况。
影响的 Task / Matrix ID / ADR: 仅补充 P6-03 报告的诊断证据；不改变 C28 的固定端点、P6-03
      生产代码、行为契约、Canonical 语义、P6-04+、账号/路由配置、服务器、代理或 TUN。
兼容性与迁移影响: 无。grok2api 是独立的 OpenAI-compatible 代理边界；其成功绝不替代 P6-03
      固定 `cli-chat-proxy.grok.com` 直连端点的非流式与 SSE 验收。
测试与回滚变化: 服务器内存中仅选取一把已启用且仅绑定 Build 路由的既有 client key；恰好一次
      `GET /v1/models` 用于选择该 key 已许可的模型，随后恰好一次、非流式 `POST /v1/responses`
      （固定短提示、`max_output_tokens=32`）。无重试、无 key/model fallback、无 SSE，且不由
      操作员进行账号、路由或数据库写入；服务正常的 last-used/用量会计写入是该一次调用的固有
      副作用。只记录状态类、内容类型类、结构形状、耗时和紧随其后的安全日志投影；不记录或
      输出 endpoint 私有部分、API key、加密材料、模型映射、头、请求/响应正文或文本。
用户批准: APPROVED，2026-07-22（“grok2api有endpoint和apikey可以使用，你可以自己调用然后获取日志”）
计划版本变更: v1.14
```

### 已批准 Change Request：CR-P6-03-005

```text
CR-ID: CR-P6-03-005
原因: CR-P6-03-004 的受控服务器参考表明，本地 P6-03 所冻结的 Build 客户端身份和请求元数据已
      过时且不完整。用户已明确批准以该参考的当前、非秘密协议轮廓更新本地 Builder，并对固定
      `cli-chat-proxy.grok.com` 边界做新的双模式验收。
影响的 Task / Matrix ID / ADR: 仅 P6-03 的 Build 请求轮廓、合成测试、BC-PROVIDER-003、报告与
      C28 固定端点证据。更新静态客户端身份/版本、客户端标识与模式、模型覆盖和安全关联元数据，
      并在语义等价受测后允许恰好一个纯文本 user 输入采用标量编码。不改变固定 URL、OAuth
      账户、Canonical 请求/响应语义、P6-04+、服务器、代理或 TUN 配置。
兼容性与迁移影响: 无客户端、数据库、部署或账号迁移。关联值每次请求生成、仅存在于请求内存，
      `Debug`、报告、日志与 Git 不得输出其值；已知可用的 Build 模型值仅在忽略的操作员调用内存
      中获得和使用，不记录或提交。
测试与回滚变化: 先完成静态轮廓、标量输入语义等价、零泄露 Debug、精确 egress 和原有解码
      测试，再运行本地 full gate 与独立 review 并提交。之后仅由两个独立 harness 进程发送：
      `T11=(build-profile-02, non_streaming, direct)` 与
      `T12=(build-profile-02, sse, direct)`；每进程仍固定
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、固定短提示和 32 token 上限。T1-T10（含诊断）绝不
      重放，也不进行 retry、refresh、failover、候选选择或网络配置变更。任一非 2xx、超时、
      非预期内容类型、协议/语义错误或泄露防护失败均结束该元组；只有 T11 和 T12 均产生
      Canonical `ResponseStart`、文本及无 `StreamError` 的 `ResponseEnd` 才可使 P6-03 进入
      `LOCAL_PASS_PENDING_PHASE_GATE`，否则恢复 `BLOCKED`，P6-04 仍不得开始。
用户批准: APPROVED，2026-07-22（“批准”）
计划版本变更: v1.15
```

### 已批准 Change Request：CR-P6-03-006

```text
CR-ID: CR-P6-03-006
原因: 用户指出 T11/T12 的 4xx 可能与指定 OAuth 账号的额度或账号状态有关，并明确要求将该当前服务器
      CPA OAuth JSON 导入已部署的 grok2api 后调用测试。需要把该外部、持久化账号导入收敛为可归因的
      额度诊断，避免以共享账号池的生成结果错误推断该指定账号。
影响的 Task / Matrix ID / ADR: 仅补充 P6-03 报告的账号/额度诊断证据；不改变 C28 固定直连端点、
      T1-T12、P6-03 生产代码、Canonical 语义、BC-PROVIDER-003、P6-04+ 或任何 Phase 状态。
兼容性与迁移影响: 目标 OAuth 文件将通过 grok2api 受支持的管理员导入 API 持久化为外部服务账号；这是
      用户要求的唯一服务器写入。不得直接写 SQLite、导出凭据、修改路由、client key、优先级、账号
      enabled 状态、代理/TUN 或服务配置；不得在 Git、日志或报告中记录 OAuth 内容、JWT、内部 ID、
      API key、模型映射、请求/响应正文或原始 Header。
测试与回滚变化: 先仅在服务器内存中以 bootstrapAdmin 登录，上传恰好一个指定 OAuth 文件，随后以
      只读账号投影确定其内部关联，并恰好一次调用该账号的 `POST /accounts/{id}/refresh-quota`。该调用
      是唯一可证明绑定到导入账号的 Provider 测试。仅当现有服务无需新增 route/key、禁用其它账号或
      调整调度就能同样证明绑定时，才允许额外一次非流式生成；共享 `/v1/responses` 调用一律不作为
      此诊断证据。导入后的外部账号按用户意图保留；如需删除，必须有单独的显式服务器变更授权。无论
      结果如何都不重放 T1-T12，也不解除 P6-03 `BLOCKED` 或启动 P6-04。
用户批准: APPROVED，2026-07-22（“4xx可能是账号的额度的问题，你可以把这个账号…导入grok2api然后调用测试”）
计划版本变更: v1.16
```

### 已批准 Change Request：CR-P6-03-007

```text
CR-ID: CR-P6-03-007
原因: 本机已发现的 Grok Build OAuth 缓存均为过期副本，且与刚导入服务器诊断的账号不同；用户在得知
      需要重新认证后明确回复“可以”。需要以官方本机 CLI 重新获得可验证的本地 OAuth 会话，才可判断
      该会话是否能安全、正确地形成 P6 所需的标准 OAuth 输入。
影响的 Task / Matrix ID / ADR: 仅补充 P6-03 的本机认证诊断证据；不改变 C28、T1-T12、P6-03
      生产代码、固定端点、Canonical 语义、BC-PROVIDER-003、P6-04+、服务器、路由、代理或 TUN。
兼容性与迁移影响: 允许恰好一次本机官方 `grok login --oauth` 交互式认证。用户只在该正式认证页面中自行
      输入账号、密码、MFA/验证码；不得由代理读取、代填、输出、复制、提交或记录任何凭据、完整邮箱、
      JWT、Token、Refresh Token、Cookie 或会话标识。CLI 自身按其既有安全存储写入的本地会话是该
      用户操作的预期副作用；不得修改、重命名或手工转换缓存文件。
测试与回滚变化: 认证前后仅允许对已知本机 CLI 会话做安全投影：登录是否成功、缓存的字段命名类别、
      有效期类别，以及能否在不伪造字段的条件下取得 P6 严格导入所需的标准 OAuth 输入。不得调用模型、
      不得发送 `/v1/responses` 或任何 P6 harness、不得重放 T1-T12、不得刷新/测试服务器账号，也不得
      启动 P6-04。若认证未完成或安全投影仍不能形成正确输入，P6-03 保持 `BLOCKED`；任何新的直连验证
      均须另行登记 CR 和新 tuple。
用户批准: APPROVED，2026-07-22（“可以”）
计划版本变更: v1.17
```

### 已批准 Change Request：CR-P6-03-008

```text
CR-ID: CR-P6-03-008
原因: 用户要求 Grok Build 参考 CPA、grok2api、Sub2API 的源码做法，并把多个 OAuth 方案纳入本
      Provider。只读差分确认三者使用同一公开 xAI client ID 与 refresh grant；差异集中在合法 OAuth
      凭据的落盘/缓存形状：标准 token response、CPA xAI auth file、grok2api account credential 与官方
      Grok CLI indexed auth cache。
影响的 Task / Matrix ID / ADR: 重新打开 P6-03 为 `IN_PROGRESS`，仅扩展其凭据来源适配、合成测试、
      BC-PROVIDER-003 与 C28 的新版受控 live matrix。P6-01/02 的 Device Code、Refresh Singleflight、
      Revision/CAS 与 Secret 边界保留；不改变 Canonical Request/Event、固定 Responses URL、P6-04+、
      服务器、路由、代理或 TUN。
兼容性与迁移影响: 保留现有严格 `access_token` + `refresh_token` + `expires_in` JSON 导入；新增只接收
      明确版本化/已知形状的内存适配：(a) CPA/grok2api xAI OAuth JSON 的绝对 `expired` 或 `expires_at`；
      (b) 官方 Grok CLI 的精确 issuer+public-client indexed cache，其中 `key` 仅映射为当前 Bearer
      access token。所有绝对时间必须严格解析、未过期且安全转换为 P6 绝对毫秒；client ID 必须匹配
      P6 公开 client。Provider API 只接收 bytes、不得自行读取任意文件路径；忽略 harness 才可读取一份
      用户已登录的明确本机 cache 路径。不得写回/重命名/导出缓存，不得将 Token 放入命令行、环境变量、
      日志、报告、Fixture 或 Git。Sub2API 的 SSO/Cookie→Build 转换不是 OAuth 凭据来源，留在独立
      Grok Web 安全边界，未获本 CR 许可。
测试与回滚变化: 以纯合成 Fixture 覆盖四类来源（标准 JSON、CPA、grok2api、官方 CLI indexed cache）、
      RFC3339 绝对时间、client/issuer 不匹配、重复字段、多个匹配条目、过期/过长/未知形状和 Debug/
      persisted secret-redaction；不把真实 CPA 文件传至本机。完成定向测试、Clippy、格式、Secret scan、
      full local gate 和 review 后，才允许新版固定端点矩阵：`T13=(official-cli-cache-01, non_streaming,
      direct)` 与 `T14=(official-cli-cache-01, sse, direct)`，均使用已登录本机 cache、同一既有不记录值
      的 Build 候选、短提示、32 token、独立进程且每进程恰好一 send。不得 refresh、retry、failover、
      proxy/TUN 变更或重放 T1-T12；任一失败即停止，P6-03 恢复 `BLOCKED`，任何更多 tuple 须新 CR。
用户批准: APPROVED，2026-07-22（“针对grok build这个渠道，我建议你参考grok2api/cpa/sub2api这几个项目的源码是怎么操作的，当然可以有多个oauth方案，你可以都做进来；测试的账号我也给你了”）
计划版本变更: v1.18
```

### 已批准 Change Request：CR-P6-03-009

```text
CR-ID: CR-P6-03-009
原因: CR-P6-03-008 的 T13 唯一 harness 进程已在本地 wrapper 的环境变量展开错误处停止，且在
      `result=started`、credential import、DNS、HTTP 和 `send` 之前退出。用户明确批准登记一次
      修正 wrapper 后的替代验证；不能把零发送自动当作原 tuple 可重试。
影响的 Task / Matrix ID / ADR: 仅重新打开 P6-03 的 C28 live evidence、BC-PROVIDER-003、报告与
      traceability；不改变生产 Provider、固定 Responses URL、Canonical 类型、OAuth source adapter、
      P6-04+、服务器、路由、账号、代理或 TUN。T1-T13 及其历史结果保持关闭。
兼容性与迁移影响: 无。wrapper 只在单一子进程内以已赋值的本地 shell 变量向受控 `env -i` 传递
      非秘密的授权开关、cap、标签、模式、cache path 和既有不记录值的 model candidate；不把 Token、
      cache 内容、path、model 或响应值打印、写入 Git 或持久化。generic credential JSON、P6 SOCKS
      变量及环境 proxy 均不传入。
测试与回滚变化: 先恰好一次 ignored no-network preflight，以 T15 的同一配置读取最多 64 KiB cache 并
      只构造请求；它不得 DNS/HTTP/refresh/send，失败则停止且不发送 T15。预检通过后只登记
      `T15=(official-cli-cache-corrected-01, non_streaming, direct)`，独立进程、
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、固定短提示与 32 token，恰好一次 send。T15 的任一非
      Canonical success、超时、内容类型、协议、语义或安全失败均停止，T14 不得发送并使 P6-03
      `BLOCKED`。仅 T15 出现 Canonical `ResponseStart`、文本、无 `StreamError` 的 `ResponseEnd`
      时，才重新授权原本未发送的 `T14=(official-cli-cache-01, sse, direct)` 一次；T14 失败同样
      `BLOCKED`。不得 refresh、retry、failover、candidate selection、proxy/TUN change 或新增 tuple。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.19
```

### 已批准 Change Request：CR-P6-03-010

```text
CR-ID: CR-P6-03-010
原因: T15 的安全 `4xx` 不能归因，但其后的只读官方 CLI 静态审计已证明本地 Builder 当时硬编码的
      Linux shell User-Agent 与本机 `0.2.106` 官方 CLI 的 workspace 请求轮廓不一致。该常量已在
      已审查的本地提交中更正为版本化 workspace 值；需要一个不同的、有限的直连元组验证该修正，
      不能重放 T15 或把未发送的 T14 自动视为可发送。
影响的 Task / Matrix ID / ADR: 仅重新打开 P6-03 的 C28 live evidence、BC-PROVIDER-003、报告与
      traceability；不改变固定 URL、Canonical 类型、OAuth source adapter、Provider 生产接口、
      P6-04+、服务器、路由、账号、代理或 TUN。T1-T15 的历史结果保持关闭。
兼容性与迁移影响: 无。只使用已登录本机官方 CLI cache 和既有不记录值的 Build candidate；wrapper
      仅向受控 `env -i` 子进程传递非秘密授权、cap、标签、模式、cache path 和既有 candidate，
      不打印、写入 Git 或持久化 cache 内容、Token、模型、响应、请求体或原始头。generic credential
      JSON、P6 SOCKS5 和环境 proxy 变量均不得传入。
测试与回滚变化: 先恰好一次 ignored no-network preflight，以 T16 的相同配置读取最多 64 KiB cache
      并只构造请求；它不得 DNS/HTTP/refresh/send，失败则停止且不发送 T16。预检通过后仅登记
      `T16=(official-cli-cache-workspace-ua-01, non_streaming, direct)`：独立进程、
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、固定短提示、32 token、恰好一次 send。任一非 Canonical
      success、超时、内容类型、协议、语义或安全失败均停止并使 P6-03 `BLOCKED`，T17 不得发送。
      仅 T16 出现 Canonical `ResponseStart`、文本、无 `StreamError` 的 `ResponseEnd` 时，才条件
      允许 `T17=(official-cli-cache-workspace-ua-01, sse, direct)` 一次；T17 亦为独立进程且恰好
      一 send，任一失败同样 `BLOCKED`。不得 refresh、retry、failover、candidate selection、
      proxy/TUN change、T1-T15 replay 或新增 tuple。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.20
```

执行结果（2026-07-23）: 唯一 T16 无网络预检以 `InvalidTargetLabel` 停止。登记的标签超过既有
      harness 32 字符不透明标签上限，因此 `from_values` 在 credential/cache 导入、DNS、HTTP、refresh
      或 `send` 之前失败。不得修正标签后重跑该预检；T16 和条件 T17 均未发送，P6-03 恢复 `BLOCKED`。

### 已批准 Change Request：CR-P6-03-011

```text
CR-ID: CR-P6-03-011
原因: CR-P6-03-010 的唯一预检只在本地不透明标签长度门槛停止；该停止发生在 credential/cache
      导入、DNS、HTTP、refresh 和 send 之前，未消耗其登记的直连 tuple。用户明确批准以一个不同、
      合法的短标签完成受控验证，并在成功后继续 P6 的后续任务。
影响的 Task / Matrix ID / ADR: 仅重新打开 P6-03 的 C28 live evidence、BC-PROVIDER-003、报告与
      traceability；不改变 Provider 生产接口、固定 URL、Canonical 类型、OAuth source adapter、
      P6-04+、服务器、路由、账号、代理或 TUN。T1-T16 的历史结果保持关闭。
兼容性与迁移影响: 无。仅使用已登录本机官方 CLI cache 和既有不记录值的 Build candidate；wrapper
      只向受控 env -i 子进程传递非秘密授权、cap、短标签、模式、cache path 和 candidate，且不输出、
      写入 Git 或持久化 cache 内容、Token、模型、响应、请求体或原始头。generic credential JSON、
      P6 SOCKS5 和环境 proxy 变量均不得传入。
测试与回滚变化: 先恰好一次 ignored no-network preflight，以 T18 的相同配置、合法短标签
      `cli-cache-ua-01` 读取最多 64 KiB cache 并只构造请求；它不得 DNS/HTTP/refresh/send，失败则
      停止且不发送 T18。预检通过后仅登记
      `T18=(cli-cache-ua-01, non_streaming, direct)`：独立进程、
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、固定短提示、32 token、恰好一次 send。任一非 Canonical
      success、超时、内容类型、协议、语义或安全失败均停止并使 P6-03 `BLOCKED`，T19 不得发送。
      仅 T18 出现 Canonical `ResponseStart`、文本、无 `StreamError` 的 `ResponseEnd` 时，才条件允许
      `T19=(cli-cache-ua-01, sse, direct)` 一次；T19 同为独立进程且恰好一 send，任一失败同样
      `BLOCKED`。只有 T18/T19 均成功，才可按依赖顺序开始 P6-04 至 P6-08。不得 refresh、retry、
      failover、candidate selection、proxy/TUN change、T1-T16 replay 或新增 tuple。
用户批准: APPROVED，2026-07-23（“测试完成后如果没有问题就继续完成 P6 剩下的内容”）
计划版本变更: v1.21
```

执行结果（2026-07-23）: 唯一 T18 无网络预检通过；它读取受限官方 CLI cache 并构造固定请求，未做
      DNS、HTTP、refresh 或 send。随后唯一 T18 独立直连进程在 2.66 秒到达固定端点并安全停止为
      `4xx / error_like_object / unrecognized`，没有 Canonical `ResponseStart`、文本和无
      `StreamError` 的 `ResponseEnd` 生命周期。T19 未发送；不得重试 T18、发送 T19 或开始 P6-04，
      P6-03 恢复 `BLOCKED`。

### 已批准 Change Request：CR-P6-03-012

```text
CR-ID: CR-P6-03-012
原因: T18 已到达固定端点，但脱敏 `4xx / error_like_object / unrecognized` 不能单独归因到 OAuth
      权限、账号权益/额度、模型可用性或请求轮廓。用户批准先做有限的只读诊断，再决定是否需要新的
      修复或验证 CR。
影响的 Task / Matrix ID / ADR: 仅重新打开 P6-03 的故障归因证据、BC-PROVIDER-003、报告与
      traceability；不改变生产 Provider、固定 URL、Canonical 类型、OAuth source adapter、
      P6-04+、服务器、路由、账号、代理或 TUN。T1-T18 保持关闭，T19 仍不具备发送条件。
兼容性与迁移影响: 无。只读检查已有的本机官方 CLI 状态/日志和已有服务器日志或其安全状态投影；
      不打开新的 debug 日志、不读取或输出 Token、cache path、模型、请求/响应体或原始 headers，
      不写入账户、数据库、日志、Git、服务器或网络配置。
测试与回滚变化: 最多检查一个本机官方 CLI 的既有状态来源与一个既有服务器日志/状态来源；只输出
      可审计的无值类别。禁止 DNS、HTTP、Provider send、OAuth refresh、官方 CLI 交互、server action、
      retry、failover、candidate selection、proxy/TUN change、T18 replay 或 T19。若没有足够的既有
      证据，结论必须为 `unattributed` 并使 P6-03 恢复 `BLOCKED`；不得以诊断为由新增请求。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.23
```

执行结果（2026-07-23）: 本机已有的官方 CLI 状态证据仅证明 indexed cache 通过严格导入并且未过期；
      它不能证明上游已接受该次请求。经配置的字面 IP SSH 做只读容器状态检查，grok2api 名称匹配
      不是唯一实例，因此没有读取日志、也没有可与本机直连 T18 关联的服务器证据。未执行 DNS、HTTP、
      Provider send、OAuth refresh、CLI 交互、服务/账号/代理变更。结论为 `unattributed`；P6-03
      恢复 `BLOCKED`，T19 及 P6-04 仍不得开始。

### 已批准 Change Request：CR-P6-03-013

```text
CR-ID: CR-P6-03-013
原因: 用户明确批准“完成 P6 所有要求”，并确认此前 P6-03 的单 tuple 停止规则不再阻塞后续
      本地实现、验证和 clean-room 差分工作。T18 已有的安全 4xx 仍不能被伪称为直接成功。
影响的 Task / Matrix ID / ADR: 解锁 P6-04 至 P6-08；新增 P6 runtime-state migration、
      ADR-0045、BC-PROVIDER-004 和差分证据。P6-03 的 fixed URL、Canonical 行为、OAuth 凭据
      边界、T1-T18 历史和 T19 发送条件均不改变。
兼容性与迁移影响: 新增 schema version 7 的 Provider-private Billing/catalog/quota、affinity、
      ownership 与 AEAD replay 表；Build cache key 改为版本化 tenant HMAC identity，原始客户端
      key 不再可直接发送上游。无服务器、账号、代理/TUN、路由或管理 HTTP 变更。
测试与回滚变化: P6-04 至 P6-07 的合成隔离、单调性、加密和错误矩阵必须通过；P6-08 只允许
      结构化无值服务器证据。G6 仍需本地 Full gate、独立 review 和一次 Phase Delivery Gate。
      回滚移除 version 7 与 P6 Provider-private modules，不重放任一真实 tuple。
用户批准: APPROVED，2026-07-23（“批准”；继续完成 P6 所有要求）
计划版本变更: v1.25
```

### 已批准 Change Request：CR-P6-03-014

```text
CR-ID: CR-P6-03-014
原因: 重新审计发现 P6-03 原有的 Builder/Decoder 与运行时状态没有实现可由网关执行的
      `InferenceAdapter`，且 T18 未满足 C28 的真实双模式通过条件。随后，本机官方 Grok CLI
      `0.2.106` 使用当前 OAuth 会话实际完成了一个最小请求；其安全会话投影表明调用模型为
      `grok-4.5`，完成模型为 `grok-4.5-build`。需要补齐可注入执行链路，并以此当前、已验证
      的模型选择登记新的验收 tuple；这不是对 T18 的重放。
影响的 Task / Matrix ID / ADR: 重新打开 P6-03 的 `C28`；新增 Provider-private
      `InferenceAdapter`、DNS-pinned transport 注入和 Router mode bridge，以及 loopback/fixture
      E2E。固定 URL、OAuth 来源、Canonical 类型、P6-04 至 P6-08 的私有状态模型不变。
兼容性与迁移影响: 无公开 API、数据库、服务器、账号、代理或 TUN 变更。应用入口不读取 OAuth
      文件；credential、transport、egress policy 和 resolver 都由运行时显式注入。不会记录 token、
      cache path、模型映射、请求/响应正文或 header 值。
测试与回滚变化: 在真实发送前必须通过 adapter 非流式/SSE 分块、Router 选择和失败分类 E2E，
      然后仅登记 `T20=(official-cli-current-01, non_streaming, direct)`；它由独立进程、
      `P6_03_MAX_EXTERNAL_REQUESTS=1`、本机官方 cache、`grok-4.5`、固定短提示和 32 token
      构成，恰好一 send、无 refresh/retry/failover。只有 T20 得到 Canonical `ResponseStart`、
      文本和无 `StreamError` 的 `ResponseEnd` 时，才允许 `T21=(official-cli-current-01, sse,
      direct)` 一次；T21 使用相同限制。任一失败则 P6-03 保持 `BLOCKED`，并且不得启动 P7。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.26
```

### 已批准 Change Request：CR-P6-03-015

```text
CR-ID: CR-P6-03-015
原因: T20 已确认当前官方 CLI OAuth cache 与 `grok-4.5` 模型选择到达固定端点并取得 `2xx` 和
      预期 JSON Content-Type，但严格 Responses 解码在任何 Canonical 事件前停止。保留的安全输出
      没有 2xx JSON 的对象类别，无法区分成功对象格式漂移、包装对象或 2xx 错误对象。
影响的 Task / Matrix ID / ADR: 仅 P6-03 C28 诊断 harness 和报告；不改变生产 Builder、Adapter、
      URL、OAuth、Canonical 类型、状态表、服务器、账号、代理/TUN 或 T1-T20 历史。
兼容性与迁移影响: 无。新增投影只可能输出 `responses_object`、`wrapped_response_object`、
      `error_like_object`、`chat_choices_object`、`other_object`、`non_object` 或 `invalid_json`；它不
      输出 key 全集、值、请求/响应文本、ID、token、模型映射或 headers。
测试与回滚变化: 先以合成私有值证明投影无值；随后仅登记
      `T22=(official-cli-current-shape-01, non_streaming, direct)`，仍为独立进程、32 token、
      一 send、无 refresh/retry/failover。T22 只缩小归因，不能通过 C28，且 T21 仍不得发送。
      若无法形成明确修复，P6-03 为 `BLOCKED`；回滚仅移除该 ignored-harness 投影。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.27
```

### 已批准 Change Request：CR-P6-03-016

```text
CR-ID: CR-P6-03-016
原因: T22 的 `invalid_json` 是在应用当前已允许的 non-streaming Content-Encoding 之前对原始
      bytes 做的安全投影；它不能区分 gzip 压缩的成功 JSON 与实际无效响应。需要在与生产 decoder
      相同的 1 MiB 有界 gzip/identity 步骤之后，才投影固定对象类别。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored diagnostic harness 和报告；不改变 Builder、
      Adapter、固定 URL、OAuth、Canonical、状态、服务器、账号、代理/TUN 或历史 tuple。
兼容性与迁移影响: 无。投影仅增加 `content_encoding` 的 `identity`/`gzip`/`other_or_missing` 类别，
      以及 `decode_failed`；不输出 header 值、压缩/解压 bytes、JSON key/value、文本、token 或模型。
测试与回滚变化: 合成测试覆盖 private JSON 与安全无值类别；仅登记
      `T23=(official-cli-current-decoded-shape-01, non_streaming, direct)`，独立进程、一 send、
      无 refresh/retry/failover。它不能通过 C28，T21 仍不得发送；若诊断不足，P6-03 `BLOCKED`。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.28
```

### 已批准 Change Request：CR-P6-03-017

```text
CR-ID: CR-P6-03-017
原因: T23 已确认响应在 gzip 解压后是 `responses_object`，但严格 decoder 仍在产生任何
      Canonical Event 前停止。需要一次镜像 decoder 核心固定前置条件的无值 projection，避免将
      未知 output 类型、未完成状态或无效 item 误判为 OAuth/transport 故障。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored diagnostic harness；不变更生产 Builder、
      Adapter、URL、OAuth、Canonical、状态、服务器、账号、代理/TUN 或任何关闭的 tuple。
兼容性与迁移影响: 无。结果仅为预定义 requirement label，不输出 response/item ID、文本、字段值、
      token、model、arguments、header 或 JSON key 集合。
测试与回滚变化: 合成 fixture/私有值测试后，只登记
      `T24=(official-cli-current-core-shape-01, non_streaming, direct)`，独立进程、一 send、
      无 refresh/retry/failover。它不能通过 C28，T21 仍不得发送；无法修复即 P6-03 `BLOCKED`。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.29
```

### 已批准 Change Request：CR-P6-03-018

```text
CR-ID: CR-P6-03-018
原因: T24 将 current Build 的 2xx Responses JSON 失败缩小为 `reasoning_content_invalid`。
      需要只观察 reasoning item 的固定 content 类别，确定是否应安全忽略空 reasoning，或映射一个
      已知的非敏感 text 变体；不读取 reasoning 内容本身。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored diagnostic harness/报告；生产 API、URL、OAuth、
      Canonical、状态、服务器、账号、代理/TUN 和关闭 tuple 均不变。
兼容性与迁移影响: 无。唯一输出是 `missing_content`、`empty_content`、`reasoning_text`、
      `summary_text`、`text`、`missing_text` 或 `other_or_missing` 等固定类别。
测试与回滚变化: 合成投影检查后，仅 T25 一次直连 non-streaming send；它不能通过 C28，T21 未授权。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.30
```

### 已批准 Change Request：CR-P6-03-019

```text
CR-ID: CR-P6-03-019
原因: T25 将差异精确定位为 completed reasoning item 可合法缺少 `content`。该 item 没有可投影的
      reasoning 文本；将其映射为零 `ReasoningDelta` 保留 response lifecycle，且不伪造内容。
影响的 Task / Matrix ID / ADR: P6-03 decoder 兼容性、合成 fixture、C28 live evidence；不变更 URL、
      OAuth、请求 profile、Canonical 公共类型、运行时状态、服务器、账号、代理/TUN 或历史 tuple。
兼容性与迁移影响: 无。缺失 `content` 仅对 `reasoning` completed item 接受；message 和 Tool 仍 fail-closed。
测试与回滚变化: 必须通过零 Delta fixture、既有 arbitrary-chunk/non-streaming equivalence、adapter
      E2E 和 Clippy；随后仅 T26 non-streaming 一 send。只有它得到完整 Canonical 成功时才允许 T27 SSE。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.31
```

### 已批准 Change Request：CR-P6-03-020

```text
CR-ID: CR-P6-03-020
原因: T27 的固定端点 SSE 取得 2xx/预期 Content-Type，但严格 decoder 在任何可验收生命周期完成前
      停止。需要只输出最后一个完整 SSE record 的预定义事件类别，判断是未知事件还是已知事件 payload
      差异；不读取 data 值。
影响的 Task / Matrix ID / ADR: 仅 P6-03 ignored SSE diagnostic harness/报告；生产接口、URL、OAuth、
      Canonical、状态、服务器、账号、代理/TUN 和历史 tuple 均不变。
兼容性与迁移影响: 无。投影仅输出预定义 record 类别，未知值合并为 `unknown_or_malformed`。
测试与回滚变化: 仅 T28 SSE 一 send；不通过 C28，未有具体修复前不再发送新的成功验收 SSE tuple。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.32
```

### 已批准 Change Request：CR-P6-03-021

```text
CR-ID: CR-P6-03-021
原因: T28 的 `unknown_or_malformed` 类别不足以区分已知标准 Responses 的 content-part、output-text
      completion 和 reasoning-summary event families。仅扩展固定白名单类别后，才可决定精确兼容修复。
影响的 Task / Matrix ID / ADR: 仅 ignored SSE diagnostic harness/报告；其它行为和历史 tuple 不变。
兼容性与迁移影响: 无。未知名称继续合并，无原始 event/data 输出。
测试与回滚变化: 仅 T29 SSE 一 send；不能通过 C28，具体修复前不再发送验收 tuple。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.33
```

### 已批准 Change Request：CR-P6-03-022

```text
CR-ID: CR-P6-03-022
原因: T29 识别了 standard `response.reasoning_summary_text.delta`，其为现有 `ReasoningDelta` 的
      明确语义同类。配套 summary-part 只携带结构边界，受限验证后不产生合成文本。
影响的 Task / Matrix ID / ADR: P6-03 SSE decoder/fixture 和 C28；不改变 OAuth、请求、URL、状态、
      服务器、账号、代理/TUN 或关闭的 tuple。
兼容性与迁移影响: summary text delta 映射为 Canonical ReasoningDelta；summary part 仅验证
      reasoning item identity 和 object 结构。其它未知 SSE event 仍 fail-closed。
测试与回滚变化: 定向 decoder/adapter tests 后，T30 为唯一修复后 SSE send；通过才闭合 C28。
用户批准: APPROVED，2026-07-23（“批准”；此前已授权完成 P6 的真实测试）
计划版本变更: v1.34
```

### 已批准 Change Request：CR-P6-03-023

```text
CR-ID: CR-P6-03-023
原因: T30 在已验证的 summary 兼容修复后仍以 SSE protocol failure 停止；原 diagnostic observer 在把
      一整个上游 transport chunk 交给 decoder 后才记录 record，因此其“最后事件”可能晚于真正失败的
      record，不能据此修改 strict decoder。需要把观察和 decoder 都按单字节推进，并只投影该精确
      完成 record 的固定 event 类别及 output-item 安全形状。
影响的 Task / Matrix ID / ADR: 仅 P6-03 C28 的 ignored SSE diagnostic harness、合成观察测试和
      报告；不改变生产 Builder/Adapter/decoder、OAuth、请求 profile、URL、Canonical、状态、服务器、
      账号、代理/TUN 或关闭的 T1-T30。
兼容性与迁移影响: 无。输出仅能为既有有限 SSE event 类别和 `message_valid_id`、
      `reasoning_valid_id`、`function_call_valid_id`、`other_or_invalid`、`not_output_item`；不得输出
      record/body/header、ID、字段值、文本、token、模型或 cache path。observer 的逐字节合成测试
      证明 transport 分块不影响此诊断归因。
测试与回滚变化: 定向 harness test/Clippy/docs check 通过后，仅登记
      `T31=(official-cli-current-sse-byte-shape-01, sse, direct)`；独立进程、官方 CLI cache、
      `grok-4.5`、32 token、一 send、无 refresh/retry/failover。T31 仅定位，不能通过 C28；若它
      显示明确兼容差异，必须先写 strict synthetic fixture/test，再以新的 CR 登记替代 SSE 验收。
      无明确差异则 P6-03 保持 IN_PROGRESS/BLOCKED，不启动 P7。回滚只移除该 ignored observer。
用户批准: APPROVED，2026-07-23（“批准”）
计划版本变更: v1.35
```

### 已批准 Change Request：CR-P6-03-024

```text
CR-ID: CR-P6-03-024
原因: T31 以逐字节归因确认 strict decoder 停在 standard `response.reasoning_summary_text.done`，而非
      output-item 形状。clean-room 本地 Responses 行为参考及其合成 fixture 表明这是 summary text 的
      终态确认事件：它携带与已累积 delta 相同的完整 text。此前 decoder 只接受 delta，因而错误地将
      该已知终态当作未知事件。
影响的 Task / Matrix ID / ADR: P6-03 C28 的 SSE decoder/fixture 和替代验收 tuple；不改变 OAuth、
      请求、URL、Adapter/Router、Canonical 公共类型、状态、服务器、账号、代理/TUN 或关闭 T1-T31。
兼容性与迁移影响: 只对已登记 reasoning item 接受此 event，要求 string `text`。若已有 delta，
      `text` 必须与按顺序累积值完全一致且不重复 Canonical delta；若上游只给该终态 text，则仅映射
      一次 `ReasoningDelta`。错 item、缺失 text、冲突 text 仍 fail-closed。message/tool 终态不放宽。
测试与回滚变化: 合成成功 fixture 覆盖 matching delta/done 且断言不重复事件；mismatch fixture 必须
      是 UpstreamProtocolError；既有 arbitrary-chunk、adapter/Router 与 Clippy 必须通过。随后仅登记
      `T32=(official-cli-current-sse-summary-done-01, sse, direct)`，独立进程、官方 CLI cache、
      `grok-4.5`、32 token、一 send、无 refresh/retry/failover。仅 T32 的 ResponseStart/text/
      无 StreamError ResponseEnd 可闭合 C28；失败时 P6-03 保持 IN_PROGRESS/BLOCKED，不启动 P7。
      回滚只移除该 event 分支和 fixtures。
用户批准: APPROVED，2026-07-23（此前无次数上限的真实测试授权及本轮“批准”）
计划版本变更: v1.36
```

### 已批准 Change Request：CR-P6-03-025

```text
CR-ID: CR-P6-03-025
原因: T32 在 summary terminal 兼容后继续到 `response.content_part.added` 才停止，证明上次修复已生效。
      当前 decoder 只消费 text delta/最终 output item，遗漏标准 Responses 的 text content-part 边界和
      adjacent `output_text.done`/`reasoning.done` 完整文本确认。已从 clean-room 本地行为参考和合成
      byte-chunk fixture 确认该序列的严格结构。
影响的 Task / Matrix ID / ADR: 仅 P6-03 C28 SSE decoder/fixtures 和替代验收 tuple；不改变 OAuth、
      请求、URL、Adapter/Router、Canonical 公共类型、状态、服务器、账号、代理/TUN 或关闭 T1-T32。
兼容性与迁移影响: 只接受已登记 message item 的 `output_text` part；added 必须为空 text，done 必须
      匹配已累积 text（无 delta 时才映射一次 TextDelta），且 part 未关闭时 output item 不得完成。
      `output_text.done` 使用相同完整文本确认；`reasoning.done` 复用已验证的 reasoning terminal rule。
      refusal/未知 part、错误身份、非空 added、重复/未关闭 part、缺失/冲突 text 全部 fail-closed。
测试与回滚变化: byte-by-byte 完整 lifecycle fixture 必须证明一次 TextDelta/ResponseEnd；非空 part
      start fixture 必须是 UpstreamProtocolError；summary matching/mismatch、adapter/Router、Clippy 和
      docs check 必须通过。随后仅登记 `T33=(official-cli-current-sse-content-part-01, sse, direct)`，
      独立进程、官方 CLI cache、`grok-4.5`、32 token、一 send、无 refresh/retry/failover。仅 T33 的
      ResponseStart/text/无 StreamError ResponseEnd 可闭合 C28；失败则 P6-03 保持 IN_PROGRESS/BLOCKED，
      不启动 P7。回滚只移除该标准 text-content event 处理和 fixture。
用户批准: APPROVED，2026-07-23（此前无次数上限的真实测试授权及本轮“批准”）
计划版本变更: v1.37
```

### 已批准 Change Request：CR-P4-G4-001

```text
CR-ID: CR-P4-G4-001
原因: G4 复核发现 P4 已有 Health、Quota、Circuit、Route Explain 与事件能力，但没有可供管理
      进程只读查询的统一状态投影；`CredentialForbidden` 也不会转换为可恢复的精确账户状态。
      因而不能证明 G4 的“403 账号状态、429、Quota、Circuit 和恢复可见”条件。
影响的 Task / Matrix ID / ADR: 新增 P4-10；Matrix G20、G21、G26、H19、H20；新增 ADR-0032
      与 BC-MGMT-001。P4-04/P4-05/P4-06 的既有运行时状态与 Explain 被只读组合，不改其
      已验收的目标范围。
兼容性与迁移影响: 新增 `gateway-router` 的 in-process 只读管理查询和绑定级 403 账户状态/受控
      恢复 API；无 HTTP 路由、认证机制、Provider 请求、SQLite schema、Canonical Event、数据或
      部署迁移。P10 仍独占认证 HTTP 管理 API、Web UI 与持久化管理读模型。
测试与回滚变化: P4-10 必须证明 403 精确绑定隔离、恢复票据、429/Quota source-confidence、
      Circuit/Recovery、Route Explain 可见性、读取无副作用、调度 fail-closed 与 Secret-safe Debug。
      回滚移除新状态查询与账户恢复状态；既有 Health/Quota/Explain/SQLite/Event 行为不变。
用户批准: APPROVED，2026-07-21（用户选择“新增一个只读、非 HTTP 的管理状态查询边界”）
计划版本变更: v1.8
```

### 1.13 G4 只读管理状态边界（CR-P4-G4-001）

1. `P4-10` 的管理 API 是供受控管理进程调用的 in-process Rust 查询边界，不注册 HTTP route，
   不处理认证、不会读/写 SQLite、不会发送 Provider 请求，也不进入 HTTP/Router 响应热路径。
2. 查询固定在调用方提供的观察时间，返回精确 Endpoint/Credential（可选 model）上的结构化
   Health、403 账户、Quota/429 source-confidence、Circuit 与受控恢复状态；错误必须是无目标、
   无 URL、无 Header、无 Body、无 Secret 的 fail-closed 类别。
3. 已验证的 Provider/driver 只有在给出既有 `CredentialForbidden` 安全错误时才记录 403 账户状态；
   它不透明重试。恢复必须经非克隆票据完成，普通流量在恢复完成前保持不可调度。
4. P10 才能将该边界暴露为认证 HTTP 管理接口、UI、持久化查询或远程控制面；P4-10 不提前实现
   其中任何一项。

### 已批准 Change Request：CR-P7-G7-001

```text
CR-ID: CR-P7-G7-001
原因: P7-01 至 P7-09 的本地实现、定向测试、完整本地门禁和原生 Adapter 垂直链路已经通过；
      唯一未闭合的 P7 工作是外部 Kiro 账号重新认证后才能进行的真实 CLI/IDE 验证，当前安全投影
      为 `auth_failed`。用户批准在该外部阻塞期间先开启 P8，以避免本地开发停滞。
影响的 Task / Matrix ID / ADR: 仅调整 P7→P8 的执行顺序：P8-01 至 P8-06 可在同一 `codex/p8-official`
      分支上按自身依赖进行本地实现、review 与测试。P7/G7、P8/G8、既有 Matrix、ADR、公开 API、
      Canonical 类型、Provider 协议、Schema、数据库、部署、Secret 规则及真实请求授权均不改变。
兼容性与迁移影响: 无客户端、数据或部署迁移；不切换任何生产流量。P7 继续是
      `BLOCKED_AUTH_REAUTH_REQUIRED`，不因 P8 的本地进度而成为完成状态。
测试与回滚变化: 每个 P8 Task 仍须按既定本地定向测试、review、格式和 Secret 检查执行；真实
      Official E2E 仍需测试账号与明确授权。P8 不得执行 Phase closeout、创建正式 Delivery tag、
      推送 Phase Delivery Gate、合并、发布或宣称 `DONE`，直到 G7 真实通过且 P7 的正式 Delivery
      Gate 成功。若 P7 认证复核出现本地实现回归，立即冻结 P8，从最早受影响的 P7 提交修复；
      回滚为将 P8-01 至 P8-06 恢复 `PENDING` 并停止 P8 分支工作。
用户批准: APPROVED，2026-07-23（“P7就只剩kiro账号的oauth吗？如果是的话可以先开启P8”）
计划版本变更: v1.38
```

### 1.14 P7 阻塞期间的 P8 本地开发边界（CR-P7-G7-001）

> 历史记录：本节中将 P8/G8 与 G7 绑定的顺序约束，已由 §1.15 的
> `CR-P7-DEFER-002` 完整替代；当前执行仅以 §1.15 为准。

1. 全计划仍然最多一个 `IN_PROGRESS` 代码 Task；P7-09 是 `BLOCKED`，P8-01 至 P8-06 的本地
   序列已完成，因此当前没有 `IN_PROGRESS` 代码 Task。不得由此开启 P9 或跳过 P7/G7。
2. P7 的 G7 和其 Phase Delivery Gate 保持 fail-closed。P8 的每项本地证据仅为后续 P8 本地 Task
   的前置条件，不能跨越 G7 作为下一 Phase、合并、发布或完成的证据。
3. P8 的真实 Official E2E 与 P8 closeout 均不得早于 G7；本 CR 不新增 Endpoint、Credential、
   Probe 或额度授权。外部账号、真实流量与现有服务器均不得因先行开发而修改。
4. Kiro 账号完成重新认证后，优先完成 P7-09、G7 和 P7 的唯一正式 Delivery Gate；通过后再以
   已保存的 P8 本地证据继续常规 P8 验收。

### 已批准 Change Request：CR-P7-DEFER-002

```text
CR-ID: CR-P7-DEFER-002
原因: 用户明确确认 P8 是独立的 Grok Official API-key Provider，而非 Kiro OAuth 工作；要求先跳过
      Kiro 账号认证，完成其余开发后再回补 P7。原先 CR-P7-G7-001 将 P8 收口与 G7 绑定只是流程
      顺序，不是技术或认证依赖，现予以解除。
影响的 Task / Matrix ID / ADR: P7-09/G7 保持 BLOCKED；P8-01 至 P8-06、G8、P8 Delivery Gate
      以及后续 P9-P12 的非 Kiro 依赖路径可继续。P8 的前序 Gate 改为 G6；P9 仍依赖 G8，P10-P12
      仍各自依赖前一非 Kiro Phase Gate。既有 P8 ADR/Contract/Task 的功能矩阵、公开 API、Canonical
      类型、Provider 协议、Schema、数据库和 Secret 规则均不改变。
兼容性与迁移影响: 不启用、删除或修改 Kiro Provider、账号、路由、Credential、服务器或生产流量。
      Kiro 保持不可调度，直到独立完成 P7-09/G7。P8 与后续 Phase 不得把本地 Fixture 当作真实 xAI
      行为证明；任何 Official live E2E 仍必须使用 P8 自身测试 Credential 和单独明确授权。
测试与回滚变化: P8 必须完成其 G8 本地验收、Phase review 和一次正式远端 Delivery Gate；P9-P12
      各自保持原有 Gate。若非 Kiro Phase 发现其与 P7 有实际代码依赖，冻结该 Phase 并先修复最早
      受影响项。回滚为恢复 CR-P7-G7-001 的顺序约束，不影响 P7 已有 BLOCKED 证据。
用户批准: APPROVED，2026-07-23（“P8不是grok的认证吗，应该跟kiro无关吧，还是先跳过kiro的认证，等后续其他都开发完了之后再回来完成认证”）
计划版本变更: v1.39
```

### 1.15 P7 延后与独立 Phase 收口边界（CR-P7-DEFER-002）

1. P7-09、G7 和 P7 Delivery Gate 保持 `BLOCKED_AUTH_REAUTH_REQUIRED`；它们不再是 P8-G12 的
   流程前序条件。Kiro Provider 继续不得进入测试/生产路由或宣称完成。
2. P8 在完成 G8 本地验收与自己的唯一 Phase Delivery Gate 后，可按常规 Definition of Done 进入
   `DONE`；P9-P12 各自仍必须等前一**非 Kiro** Phase Gate 成功。
3. 本 CR 只解除 Kiro 对顺序的阻塞，不授予 xAI 请求、API-key、配额、服务器、路由或发布以外的
   权限。Official live E2E 的证据若被 P8 Gate 要求，仍必须先以独立授权登记。

### 已批准 Change Request：CR-P8-DEFER-001

```text
CR-ID: CR-P8-DEFER-001
原因: 用户当前没有 xAI Official API Key，明确要求先跳过 P8 Official 的真实 E2E，并在其余开发
      完成后与 P7 Kiro OAuth 认证一并回补。P8-07 的本地安全 harness 已完成，但不得以 Fixture
      或未持有的 Credential 代替 Provider Gate 的真实证据。
影响的 Task / Matrix ID / ADR: 仅将 P8-07 与 G8 从 BLOCKED 改为 DEFERRED 外部认证工作；
      关联 ADR-0060、BC-E2E-004、§20.1 的真实 E2E 要求和已提交的零网络 harness 均不改变。
      P7-09 同样保持 DEFERRED。P9-P12 的既有 Gate 依赖、公开 API、Provider 协议、Schema、
      数据库、服务器、路由与生产流量均不改变。
兼容性与迁移影响: 无。不会搜索、导入、生成或替代 Official API Key，不发 xAI 请求，也不改变
      Kiro/OAuth、代理/TUN、服务器、账号或部署。
测试与回滚变化: 已有本地默认零网络测试和 Full gate 仍有效。恢复时先将 P8-07 复位为 BLOCKED，
      再登记 Official key/model/单请求授权；不得因延期而开始 P9 或声称 P8/G8 完成。回滚只恢复
      P8-07/G8 的 BLOCKED 等待状态，不执行外部请求。
用户批准: APPROVED，2026-07-23（“我也没有这个official的api key，可以先跳过，等后面跟kiro一起再测试”）
计划版本变更: v1.41
```

### 1.16 P8 Official E2E 延后边界（CR-P8-DEFER-001）

1. P8-07 与 G8 是 `DEFERRED`，不是 `DONE`、本地通过替代品或对 xAI 行为的断言。P8-01 至
   P8-06 的本地证据保持有效，但不能越过 §20.1。
2. P8-07 与 P7-09 在其他本地 Phase 全部完成后进入同一外部认证验收包；两者仍使用各自的
   Credential、授权与验收标准，不互相替代。
3. 本项原 P9 顺序约束已由 §1.17 的 `CR-P9-LOCAL-001` 替代；P10-P12 仍依赖各自的前序非 Kiro Gate。
4. P12/G12 之后、任何包含 Kiro 的发布前，必须回到 P7-09，完成 OAuth、G7 及 P7 Delivery Gate。
   不得以延后状态永久遗漏 Kiro Provider 验收。

### 已批准 Change Request：CR-P9-LOCAL-001

```text
CR-ID: CR-P9-LOCAL-001
原因: 用户要求继续按照最新开发计划完成 P9，而 P8-07/G8 的唯一缺口是没有 Official API Key 的
      外部 E2E，已明确延后。P9 的独立 Web Provider 本地实现不读取、不改变、也不依赖该 Credential。
影响的 Task / Matrix ID / ADR: 允许 P9-01 至 P9-08 按自身依赖顺序进行实现、review、定向测试和
      本地 Full gate；P9-09、G9、P9 Delivery Gate、合并、发布及 `DONE` 仍需 P9 自身的真实 Web
      账号/Canary 授权。P10-P12 的 Gate 依赖不变。既有 P8/P7 状态、公开 API、Canonical 类型、
      Provider 协议、Schema、数据库、服务器、路由、Secret 规则和生产流量均不改变。
兼容性与迁移影响: 无。P9 开发不搜索 Cookie/SSO/浏览器 Profile/代理/TUN/服务器文件，不发送 Web
      请求，不改变账号、出口、Feature Flag 或生产配置。
测试与回滚变化: 每个 P9 本地 Task 仍执行适用的格式、Clippy、定向测试、Secret scan 和 review；
      P9-09 保持 DEFERRED，只有显式测试账号/Canary 授权才可恢复。若发现对 P8 或 P7 的实际代码
      依赖，冻结 P9 并修复最早受影响项。回滚为恢复 P9-01 至 P9-08 为 PENDING，不发送外部请求。
用户批准: APPROVED，2026-07-23（“继续按照最新的开发计划完成P9”）
计划版本变更: v1.42
```

### 1.17 P9 本地开发边界（CR-P9-LOCAL-001）

1. P9-01 至 P9-08 可以以 P8 的已验证本地隔离证据为前置，不能将 P8/G8 的延期状态误写成通过。
2. P9-09 是 `DEFERRED`：无真实 Web 测试账号、明确 Canary、单独流量授权时，不进行 Cookie/SSO
   导入、浏览器自动化、Web API/Statsig/gRPC-Web 请求或出口变更。
3. P9 的任何本地 fixture、差分或安全 harness 不证明远程 Web 协议、额度、WAF、Cookie 或账户状态。
4. P9-01 至 P9-08 通过后只可进入 `LOCAL_PASS_PENDING_PHASE_GATE`；不得创建 P9 Delivery tag、
   推送远端 Gate、合并、发布或宣称 P9/G9 `DONE`，直到 P9-09 与 G9 的真实证据补齐。

### 已批准 Change Request：CR-P9-CANARY-001

```text
CR-ID: CR-P9-CANARY-001
原因: 用户指定“账号去grok2api里面拿web账号就行”，以当前服务器 grok2api 的 active `grok_web/sso`
      账号补齐 P9-09 所需的独立 Web 测试来源。只读元数据和受控导出已确认其为 Web SSO，而不是
      Build OAuth、Official API Key 或 Kiro 凭据。
影响的 Task / Matrix ID / ADR: 仅恢复 P9-09 与 G9 的 Canary 实施；覆盖 P9 Matrix
      `C29-C34`、`D28-D30`、`E27-E29`、`F17`、`G24-G28` 及 ADR-0061 至 ADR-0068。
      不解除 P7-09/G7、P8-07/G8 的外部认证延期，也不提前开始 P10。
Canary 边界: 至多三次真实 `POST`，固定为一个临时 grok2api `grok_web/sso` 导出的独立 SSO
      生命周期、固定 `https://grok.com` Web Chat 目标、短文本、无附件、默认关闭 Tool emulation。
      第一次只发现受限协议形状；后续仅在前次结果支持时用于本仓库的非流式/SSE Canonical
      验证。熔断/403/协议漂移演练优先复用本地 P9-06/P9-07 确定性状态，不为了制造失败额外发送
      请求。任何凭据、Cookie、Authorization、email、账号 ID、完整 URL Query、上游 Body 和原始
      响应均不得输出、持久化或提交。
兼容性与迁移影响: 导出只存在于受控临时内存/进程生命周期；不导入数据库、不创建路由、Client Key、
      Server 配置、浏览器 Profile、代理/TUN 变更或生产开关。真实观察只能更新 P9-09 的脱敏报告，
      不能改变 Build/Official/Kiro 的状态。
测试与回滚变化: 新增默认 ignored、显式授权、严格最多三次的 P9-09 harness；每次请求前必须通过
      精确目标、HTTPS/DNS-pinned Egress、固定超时、零值日志和一条请求计数审查。失败时停止，不重试
      同一元组，并将分类限定到 P9 egress/account/protocol。回滚为恢复 P9-09/G9 的 `DEFERRED`，
      清除临时进程环境并不发送后续请求。
用户批准: APPROVED，2026-07-23（“账号去grok2api里面拿web账号就行”）
计划版本变更: v1.43
```

### 已批准 Change Request：CR-P9-CANARY-002

```text
CR-ID: CR-P9-CANARY-002
原因: 第一针以本机直连且未携带当前 Statsig 签名得到 WAF 类 403，不能归因到 grok2api Web
      账号。用户明确授权“你想怎么测试都可以”，允许以同一服务器现有 Web 运行轮廓进行受控复测。
影响的 Task / Matrix ID / ADR: 仅扩展 P9-09/G9 的受控 Canary；不改变 P7/P8 延期、P10 前置、
      公开 API、Provider 路由、Credential 存储或生产 Feature Flag。
Canary 边界: 保留 CR-P9-CANARY-001 的最多三次固定 Conversation `POST`，已消耗一次；每个后续
      固定目标 POST 最多配套一次服务器直连的 `GET /index` 与一次已知 Statsig 签名端点 POST，以取得
      同一 `POST /rest/app-chat/conversations/new` 的当期签名。允许本机仅绑定 loopback 的临时 SSH
      SOCKS5 转发到同一服务器出口；不修改服务器路由、代理/TUN、账号、配置、数据库或生产开关。
      所有辅助请求拒绝重定向、固定超时与响应上限；签名只在单进程内存中存在。
兼容性与迁移影响: 受控复测必须仍由本仓库的 DNS-pinned Rust transport 发送固定 Conversation
      POST；服务器只提供已存在的 Web 账号、同源出口和当期签名材料，不能把 grok2api 的代理响应当作
      本仓库 E2E 成功。
测试与回滚变化: 对新增的 Statsig Header 与 loopback SOCKS5 输入补充零网络合成测试；每次真实
      Conversation POST 前执行任务级 review。若结果仍非接受的 Conversation frame，停止该 tuple，
      仅记录脱敏类别并保持 G9 fail-closed。回滚为关闭临时 SSH 转发、清除进程环境与恢复
      `DEFERRED_EXTERNAL_CANARY`。
用户批准: APPROVED，2026-07-23（“允许，你想怎么测试都可以”）
计划版本变更: v1.44
```

## 2. Release 1 范围

### 2.1 必须交付

- Rust + Actix Web 单节点高性能网关。
- `GET /healthz`。
- `GET /v1/models`。
- `POST /v1/responses`，非流式与 SSE。
- `POST /v1/chat/completions`，非流式与 SSE。
- `POST /v1/messages`，非流式与 SSE。
- `POST /v1/messages/count_tokens`，仅在存在可证明准确的 Provider/本地能力时返回结果。
- 自有 Client API Key 和 Access Group。
- Upstream、单协议 Endpoint、Endpoint-Credential Binding。
- Public Model、Alias、Model Route、Route Candidate。
- Priority + Smooth Weighted Round-robin。
- Candidate 与 Credential 两阶段调度。
- 首语义事件前 Failover；首语义事件后禁止透明重放。
- 每 Endpoint+Credential 模型发现和最后成功快照。
- 结构化 Request、Attempt、Usage、Health、Quota 和 Route Explain。
- OpenAI-compatible Chat/Responses 与 Anthropic-compatible Messages；三种入站协议经 Canonical
  模型在可证明无损的范围内路由到任一已声明兼容的上游 Endpoint，不能表达时 fail closed。
- Grok Build、Kiro、Grok Official、Grok Web 四个专项切片（代码完成并通过本地验证；生产路由启用延后至各自外部认证收口，见 `CR-P12-ROLLOUT-001`）。
- 管理 API、最小可用管理 Web UI、备份与恢复。
- systemd 和固定版本 Docker 产物。
- 与现有服务器链路的差分、灰度和回滚。

### 2.2 Release 1 明确不包含

- 图片、音频、视频、Gemini 与 Interactions 入站协议。
- 动态二进制插件、插件商店和在线安装。
- PostgreSQL、S3、Git Store、Redis 协议复用和多节点控制面。
- 商业计费、支付、余额充值和按美元扣费。
- New API/AxonHub 持续同步；只允许后续的一次性 Import Adapter。
- Grok Web Tool Emulation 默认启用。
- 未经显式 Route Policy 的跨 Provider 或跨协议 Failover。

## 3. 已冻结的技术基线

这些决定在计划 `v1.0` 中视为已确认，开发期间不得重新隐式选择。

| ID | 决定 |
|---|---|
| BL-01 | Rust stable + Edition 2024，HTTP 服务使用 Actix Web；核心不依赖 Actix 类型。 |
| BL-02 | Release 1 公开推理入口同时包含 OpenAI Chat Completions、OpenAI Responses 和 Anthropic Messages；三者共享 Canonical 核心但保留各自严格 HTTP/SSE 语义。 |
| BL-03 | 所有请求先进入 `CanonicalRequest`，所有输出转换为 `CanonicalEvent`。 |
| BL-04 | Tool、Reasoning、Text、Usage、Error 使用显式流状态机；网络 Chunk 边界不影响语义。 |
| BL-05 | `FirstSemanticEvent` 是透明重试边界；SSE Keepalive 不算语义事件。 |
| BL-06 | 一个 Upstream Endpoint 只绑定一种 API Format；共享 URL 也拆成不同 Endpoint。 |
| BL-07 | 先选择 Route Candidate，再在 Endpoint 内租用 Credential；两层权重独立。 |
| BL-08 | 路由使用不可变 `RouteSnapshot` + `ArcSwap`；一个请求固定使用开始时版本。 |
| BL-09 | SQLite 保存控制面、版本、长期状态和异步事件；请求热路径不查询 SQLite。 |
| BL-10 | Request/Attempt/Usage 明细进入有界异步队列；Body 默认关闭，队列满时只允许丢低优先级诊断。 |
| BL-11 | 上游 Secret 使用 AEAD；Client Key 保存 Prefix + HMAC 摘要；主密钥与数据库分离。 |
| BL-12 | CacheAffinity、ResponseOwnership、ReasoningReplay、WebConversationState 使用不同存储命名空间。 |
| BL-13 | Session/Cache Identity 至少隔离 `client_key + provider + upstream_model`。 |
| BL-14 | Grok Official、Build、Web 使用独立 Provider ID、凭据池、Quota、错误和连续性状态。 |
| BL-15 | Kiro IDE/CLI 是同一 Kiro Provider 下的 Endpoint Policy，不与通用中转站 Endpoint 混淆。 |
| BL-16 | 未知 403 默认归类为短期 `EgressRejected`；只有账号级证据才设为 Credential Forbidden。 |
| BL-17 | Unauthorized 退出调度直到重新授权；Quota 到 Reset 后受控探测；长期状态重启后恢复。 |
| BL-18 | Catalog 默认建议：Fresh 6h、Stale 24h、Expired 72h；移除需连续 3 次成功缺失且不少于 24h。 |
| BL-19 | 管理 API 先于 UI；UI 使用独立 TypeScript SPA，不能进入推理热路径。 |
| BL-20 | Grok Web 使用 Feature Flag，Tool Emulation 默认关闭；通过独立 Gate 后才允许生产启用。 |
| BL-21 | 现有 CPA/grok2api/Kiro-RS/New API 仅做一次性迁移或临时兼容上游，不成为新网关运行时数据库。 |
| BL-22 | Release 1 Client Key 只实现权限、到期、RPM、并发和可选 Token 上限，不做美元计费。 |

## 4. 代码库与交付物约定

### 4.1 目标目录

```text
Cargo.toml
rust-toolchain.toml
deny.toml

apps/
  gateway/

crates/
  gateway-core/
  gateway-protocol/
  protocol-openai-responses/
  protocol-anthropic/
  gateway-provider/
  gateway-upstream/
  gateway-catalog/
  gateway-router/
  gateway-auth/
  gateway-stream/
  gateway-observability/
  gateway-store/
  gateway-control/
  gateway-http-actix/
  provider-openai-compatible/
  provider-anthropic-compatible/
  provider-grok/
  provider-kiro/

docs/
  adr/
  contracts/
  reports/

migrations/
tests/
  fixtures/
  integration/
  differential/
  e2e/
benchmarks/
deploy/
  systemd/
  docker/
```

### 4.2 Git 规则

- `cpa-rust-gateway` 使用独立 Git 仓库，不依赖父目录的未跟踪状态。
- 主分支：`main`。
- 开发分支：`codex/p<phase>-<short-name>`。
- 一个分支只承载一个未完成 Phase 的顺序 Task 与其测试修复；每个 Task 保持独立提交，不因共用分支
  而合并范围、跳过 review 或跨 Phase 混入功能。
- Commit 标题以 Task ID 开头，例如 `P1-03: add canonical event state machine`。
- 普通 Phase 只在 closeout 时上传 Phase 分支并创建带说明的阶段 Tag，例如 `phase-p3-complete`；该
  单次交付触发 Fast + Full。提前远端例外只能按 `CR-EXEC-007` 记录并运行。
- Release 使用 SemVer，首个服务器候选版本从 `v0.1.0-alpha.1` 开始。
- 不提交 `.env`、真实数据库、Token、Cookie、OAuth JSON 或生产日志。

### 4.3 Definition of Done

一个 Task 只有同时满足以下条件才能标记为 `DONE`：

- 实现满足本 Task 和对应行为契约。
- 正常、边界、错误和取消路径有自动化测试。
- `cargo fmt --check` 通过。
- 受影响 Crate 的 Clippy 与测试通过；整合 Phase preflight 再运行
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` 与 `cargo test --workspace`。
- 没有新增未解释的 `TODO/FIXME`、明文 Secret 或宽泛 `unwrap/expect`。
- 对外行为、配置或 Schema 变化已更新文档和迁移说明。
- 旧 CPA 已有对应能力的移植 Task 已冻结 `Legacy Behavior Manifest`，且旧参考、CPAR 测试与有意
  差异可以双向追踪；不得只写“参考旧 CPA”而没有源文件/测试/行为映射。
- 移植差异已全部归类为 `PARITY`、`INTENTIONAL_HARDENING` 或 `UNSUPPORTED_FAIL_CLOSED`；
  未分类差异、静默丢字段或靠 live 请求代替离线 parity 测试均不满足完成条件。
- 保存完成证据，并更新本文状态。
- 普通代码 Task 已达到本地验收条件，并由所属 Phase 唯一的远端 Fast + Full Delivery Gate 覆盖；
  CI/workflow/cache 等例外 Task 另有明确的提前远端 Gate。纯文档/状态变更为显式 docs-only；
  Phase Tag 为 Fast + Full。`LOCAL_PASS_PENDING_PHASE_GATE` 与 `LOCAL_PASS_PENDING_CI` 都不满足
  本条件。

## 5. Phase 总览

| Phase | 目标 | 进入条件 | 退出 Gate | 状态 |
|---|---|---|---|---|
| P0 | 仓库、工具链、ADR、CI 基线 | 本计划锁定 | G0 | DONE |
| P1 | Canonical Core + Mock 垂直链路 | G0 | G1 | DONE |
| P2 | 聚合控制面、Secret、RouteSnapshot | G1 | G2 | DONE |
| P3 | OpenAI Responses 聚合 MVP | G2 | G3 | DONE |
| P4 | Catalog、Health、Quota、Explain、观测 | G3 | G4 | DONE |
| P5 | Anthropic/Claude Code 兼容 | G4 | G5 | DONE |
| P6 | Grok Build | G5 | G6 | DONE |
| P7 | Kiro IDE/CLI | G6 | G7 | DEFERRED_EXTERNAL_AUTH |
| P8 | Grok Official | G6（`CR-P7-DEFER-002`） | G8 | DEFERRED_EXTERNAL_E2E |
| P9 | Grok Web | P8 local isolation evidence（`CR-P9-LOCAL-001`） | G9 | DONE |
| P10 | 完整管理 API、Web UI、备份恢复 | G9 | G10 | DONE |
| P11 | 差分、性能、安全与发布加固 | G10 | G11 | DONE |
| P12 | 服务器部署、灰度、切换与回滚 | G11 | G12 | IN_PROGRESS |
| P13 | Release 1.1 候选功能 | G12 + 新 CR | 独立计划 | DEFERRED |

## 6. P0 - 仓库与工程基线

目标：得到可重复构建、可审计、无业务功能的 Rust Workspace。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P0-01 | 为目录建立独立 Git 仓库、忽略规则和 Secret 扫描规则 | 计划 v1.0 | `git status` 干净；测试 Secret 被阻止提交 | DONE |
| P0-02 | 创建 `docs/adr`、`contracts`、`reports` 和需求追踪索引 | P0-01 | 文档链接检查通过 | DONE |
| P0-03 | 固定 Rust stable/Edition 2024、Workspace、基础 Crate 骨架 | P0-01 | `cargo metadata`、`cargo check --workspace` | DONE |
| P0-04 | 配置 fmt、Clippy、测试、license/advisory 检查 | P0-03 | fmt/clippy/test/deny/audit 全通过 | DONE |
| P0-05 | 配置本地统一命令和 CI 快速/完整两条流水线 | P0-04 | 干净环境 CI 成功日志 | DONE |
| P0-06 | 记录本地 Mac 与 Jakarta VPS 的硬件、Rust、内核和基准环境 | P0-05 | `docs/reports/environment-baseline.md` | DONE |

### G0 门禁

- Workspace 在全新目录可重复构建。
- 所有 Crate 依赖方向符合目标架构。
- `#![deny(unsafe_code)]` 默认启用；例外必须新建 ADR。
- License Allowlist 不允许无意引入 AGPL 代码。
- CI 和本地命令得到相同结果。

## 7. P1 - Canonical Core 与 Mock 垂直链路

目标：不依赖真实上游，跑通从 Actix 入站到 Canonical、Mock Provider、再到目标协议输出的完整路径。

主要矩阵：`A01 A03 A07 B01-B17 B23-B30 K01 K02 K05 K09 K10`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P1-01 | 定义稳定 ID、RequestContext、GatewayError 和错误作用域 | G0 | 单元测试 + 错误编码快照 | DONE |
| P1-02 | 定义 `CanonicalRequest`、消息、内容、Tool、Thinking 和 Raw Extension | P1-01 | JSON/结构 Round-trip 测试 | DONE |
| P1-03 | 定义 `CanonicalEvent` 与 Response/Text/Reasoning/Tool/Usage 状态机 | P1-02 | 状态转换和非法序列测试 | DONE |
| P1-04 | 实现有界流、背压、取消传播和 FirstSemanticEvent Tracker | P1-03 | 慢消费者、取消和容量测试 | DONE |
| P1-05 | 实现 OpenAI Responses 入站/非流式/SSE Adapter | P1-02,P1-03 | 官方形态 Fixture + 事件快照 | DONE |
| P1-06 | 定义小能力 Provider Trait，并实现 Deterministic Mock Provider | P1-02,P1-03 | Mock 文本、Tool、错误、延迟 Fixture | DONE |
| P1-07 | 实现 Actix `/healthz` 与 `/v1/responses` 最小 Handler | P1-04,P1-05,P1-06 | HTTP E2E 测试 | DONE |
| P1-08 | 实现内存 Client Key Auth Port，为 P2 持久实现保留接口 | P1-07 | 有效、无效、禁用 Key 测试 | DONE |
| P1-09 | 建立 Chunk 随机切片、并行 Tool、空参数 Tool 的属性测试 | P1-03,P1-05 | 固定 Seed 与随机 Seed 报告 | DONE |

### G1 门禁

- `/v1/responses` 非流式和 SSE 均通过 Mock E2E。
- 对保留每个 Tool 本地片段顺序的任意合法、已解码 UTF-8 参数片段切分与交错，得到相同 Tool 语义投影：`call_id`、`name`、最终 `RawJson`、Responses SSE 和非流式 Function Call 输出一致；`ToolCallArgumentsDelta` 的片段边界可不同。原始网络 bytes、UTF-8 标量内切分及 EventStream 帧不变性仍由后续 Provider 阶段验证。
- `EnterPlanMode`、`ExitPlanMode` 和普通无参数 Tool 输出 `{}`。
- 客户端取消后没有遗留上游任务或无限缓冲。
- FirstSemanticEvent 前后重试状态可被测试明确区分。

## 8. P2 - 聚合控制面、安全与 RouteSnapshot

目标：建立可版本化配置、加密 Secret、自有 Key 和无数据库热路径的路由快照。

主要矩阵：`D01-D31 E01-E29 H01-H13 J01-J03 J08 J09 J15 J18-J20 L01-L05 L17-L36`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P2-01 | 设计并迁移 Config Version、Upstream、Endpoint、Credential 与 Binding 表 | G1 | Migration up/down + FK 测试 | DONE |
| P2-02 | 设计并迁移 PublicModel、Alias、Route、Candidate、AccessGroup、ClientKey 表 | P2-01 | Schema 约束和唯一性测试 | DONE |
| P2-03 | 实现 AEAD Secret Store、Key Version、Nonce 和主密钥加载 | P2-01 | 加解密、错误密钥、轮换测试 | DONE |
| P2-04 | 实现 Client Key 生成、Prefix、HMAC 摘要和常量时间验证 | P2-02,P2-03 | 创建一次可见、验证和撤销测试 | DONE |
| P2-05 | 实现 Repository/Service 事务，禁止控制面实体泄露到 Provider | P2-01,P2-02 | Repository 集成测试 | DONE |
| P2-06 | 实现 Route Compiler：Alias、引用、能力、Catalog 和冲突校验 | P2-05 | 冲突矩阵与错误快照 | DONE |
| P2-07 | 实现 `RouteSnapshot`、ArcSwap、版本固定与回滚 | P2-06 | 并发读/发布/回滚测试 | DONE |
| P2-08 | 将 P1 内存 Auth 替换为 Snapshot ClientKeyView | P2-04,P2-07 | 热更新、禁用、过期测试 | DONE |
| P2-09 | 实现 EgressPolicy：Scheme、Host、Port、CIDR、DNS、Redirect 校验 | P2-01 | SSRF、DNS Rebinding、私网 Allowlist 测试 | DONE |
| P2-10 | 提供最小管理 API/CLI：创建配置、验证、发布、回滚 | P2-05,P2-07 | 原子发布 E2E + 审计事件 | DONE |

### G2 门禁

- 无效 Alias、悬空 Candidate、重复 Endpoint Format 整版拒绝发布。
- 100 个并发请求跨 Snapshot 发布仍固定使用各自起始版本。
- 数据库和备份中不存在明文上游 Secret 或完整 Client Key。
- 本机上游只有显式 Egress Allowlist 才能访问。
- 推理热路径通过测试证明不调用 Repository。

## 9. P3 - OpenAI Responses 聚合 MVP

目标：用两个独立 Upstream 实现自有 Key、统一模型名、平滑轮询和首事件前 Failover。

主要矩阵：`C16 D10-D25 E15-E24 G05 G12 G13 G15 G21 K03-K06 L06 L20-L26 L28-L32 L40`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P3-01 | 实现 OpenAI-compatible Responses Endpoint URL/Header/Body 组装 | G2 | URL 组合和 Header 脱敏测试 | DONE |
| P3-02 | 实现共享上游 Client Pool、connect/TTFB/idle/total timeout 和代理隔离 | P3-01 | 连接复用、超时、代理测试 | DONE |
| P3-03 | 实现 Priority Tier + 预编译 Smooth Weighted Schedule + 原子 Cursor | P2-07 | 权重分布和并发公平性测试 | DONE |
| P3-04 | 实现 Endpoint Credential Pool、并发租约、站内权重和释放保证 | P3-03 | 泄漏、取消、饱和和双层公平测试 | DONE |
| P3-05 | 实现 Runtime Health/Cooldown/Circuit 基础状态与分片存储 | P3-04 | 状态隔离和恢复测试 | DONE |
| P3-06 | 实现 Attempt Orchestrator、排除集合、Retry Budget 和 FirstSemanticEvent Gate | P3-02,P3-05 | 连接/429/5xx/已开流故障矩阵 | DONE |
| P3-07 | 实现从 RouteSnapshot 生成的 `/v1/models` 与响应模型名回写 | P2-07,P3-03 | AccessGroup、hard-eligible、回写测试 | DONE |
| P3-08 | 发出 Request/Attempt/Usage 结构化事件，不阻塞响应 | P3-06 | 事件关联和队列背压测试 | DONE |
| P3-09 | 建立两个可控 Mock HTTP Upstream 的聚合 E2E 套件 | P3-01-P3-08 | 轮询、Failover、取消完整报告 | DONE |
| P3-10 | 使用两个真实测试中转 Endpoint 做最小非流式与 SSE 验证 | P3-09 | 脱敏请求/响应和 Trace 证据 | DONE |

### 已批准 Change Request：CR-P3-G3-001

`2026-07-20` 用户批准。P3-10 的真实测试此前把 `minimax-m3` 同时用作公开路由名和
验收文字；而两个已核对的私有 Candidate 映射属于 ChatGPT-family 上游。这会混淆“稳定的
客户端别名”与“实际提供方模型身份”。

- P3-10/G3 的真实验证公开模型改为 test-only `p3-chatgpt-compat`，其请求别名为
  `p3-chatgpt-compat-alias`。它仅表示对两条 ChatGPT-family 中转的 Responses 兼容性验证，
  不是任何具体上游模型版本或产品承诺。
- A、B 继续分别使用 operator-controlled 私有配置中明确给出的上游模型；P3-10 不发现、
  不展示、也不将它们写入 Git。
- 此变更不追溯修改 P3-09 的纯 Mock fixture，也不决定 P10 以后的产品 PublicModel。此前
  两条停止记录保留为安全/传输事实，但不构成修订后公开别名的兼容性证据。
- 任何修订别名下的真实请求都需要新的明确外部调用与预算授权；先前未使用的调用额度不能
  自动迁移到这个不同的测试边界。

### G3 门禁

- test-only `p3-chatgpt-compat` 通过本项目 Base URL/Key 可调用两个独立的 ChatGPT-family Candidate；它不声明具体上游模型身份。
- 等权 1000 次选择分布偏差不超过 10%；加权分布符合预期区间。
- 一个站配置多个 Key 不改变站间目标流量比例。
- 首事件前故障可切站，首事件后故障绝不透明重放。
- 429、5xx、连接错误和凭据饱和都有独立排除原因。
- 热路径无 SQLite、全局 Mutex 和无界 Channel。

## 10. P4 - Catalog、Health、Quota 与可观测性

目标：让模型目录、可用性和每次路由决策可查询、可解释、可恢复。

主要矩阵：`C26 C27 D20 D24 E18-E22 F13-F15 G01-G28 H12 H13 H19 H20 L09-L16 L30-L33`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P4-00 | 落实 `CR-EXEC-001`：受版本校验的质量工具缓存、code/docs/tag Gate 分类、`LOCAL_PASS_PENDING_CI` 状态守卫和独立单探针诊断 harness | G3 | cache miss/hit、Gate 分类、状态阻断、零授权/单请求/脱敏诊断测试；不发送未经单独授权的真实流量 | DONE |
| P4-01 | 实现每 Endpoint+Credential 的 ModelCatalogSource 调度与 Singleflight | P4-00 | 并发同步和凭据差异测试 | DONE |
| P4-02 | 实现 CatalogSnapshot、Fresh/Stale/Expired 与最后成功回退 | P4-01 | 时间推进和失败保留测试 | DONE |
| P4-03 | 实现 added/suspected_removed/removed Diff、Preview/Apply 和移除隔离 | P4-02 | 3 次缺失 + 24h Fixture | DONE |
| P4-04 | 实现 Endpoint/Model/Credential Probe、EWMA 和 Circuit 恢复 | P3-05 | 健康时间线和半开测试 | DONE |
| P4-05 | 实现 QuotaSnapshot、来源/置信度、Reset 与受控恢复探测 | P4-04 | 429/Quota/Reset Fixture | DONE |
| P4-06 | 实现 Route Explain 和 Candidate 排除原因查询 | P3-06,P4-04,P4-05 | 固定输入决策快照 | DONE |
| P4-07 | 实现 SQLite 异步 Request/Attempt/Usage/Health Event Writer | P3-08 | 队列、批写、崩溃恢复、quick_check | DONE |
| P4-08 | 实现 tracing JSON、Prometheus 和 OpenTelemetry 导出 | P4-07 | 指标/Trace 关联测试 | DONE |
| P4-09 | 实现日志脱敏、Body 采样开关和 Secret 泄漏测试 | P4-07,P4-08 | 自动 Secret 扫描报告 | DONE |
| P4-10 | 落实 `CR-P4-G4-001`：只读管理状态查询、403 账户状态与受控恢复 | P4-04,P4-05,P4-06 | 精确状态/恢复、调度隔离、无副作用与 Secret-safe 查询测试 | DONE |

`P4-00` 是 P4 的工程效率与验证前置项，不交付 Catalog、Health、Quota 或公开 API；它完成前，
不得将任一 P4-01 至 P4-09 标记为 `IN_PROGRESS`。该 Task 的单探针路径只在未来获得新的明确
operator 授权后才可发送一条真实请求，不能回溯或重跑 P3-10。

### G4 门禁

- 某 Credential 模型同步失败不影响其它 Credential 和最后成功快照。
- Catalog 临时故障不会导致 `/v1/models` 抖动。
- Route Explain 能说明每个 Candidate 的保留、排除和最终选择原因。
- 403 账号状态、429、Quota、Circuit 和恢复在管理 API 中可见。
- Event Queue 满时不阻塞推理；关键失败事件不得静默丢失。
- SQLite `quick_check`、重启恢复和事件关联通过。

`CR-P4-G4-001` 中“管理 API”仅指 P4-10 的只读 in-process 查询边界；认证 HTTP 管理面仍在 P10。

## 11. P5 - Anthropic 与 Claude Code 兼容

目标：完成 `/v1/messages`、Claude Code Tool 流和同一 Upstream 多协议 Endpoint 的安全能力门禁。

主要矩阵：`A07 A08 B04 B09-B16 B22 B24-B28 F01-F04 F09-F11 L08 L21 L22 L40`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P5-00 | 落实 `CR-EXEC-007`：P 级 delivery trigger/default-ref cache seed/tag restore 与提前远端例外验证 | G4 | workflow/计划守卫、本地 full、一次提前 GitHub Gate、tag cache hit/miss 与 fail-closed 证据 | DONE |
| P5-01 | 实现 Anthropic Messages 入站、非流式和 SSE 出站 Adapter | P5-00 | Anthropic Fixture 与事件快照 | DONE |
| P5-02 | 实现 `count_tokens` Canonical 路由和 Provider Capability；无准确能力时明确拒绝 | P5-01 | 准确路径和 Unsupported 测试 | DONE |
| P5-03 | 完成 Tool 增量 JSON、并行 Tool、空参数、必填参数和 ID 映射状态机 | P5-01 | 1-byte Chunk 属性测试 | DONE |
| P5-04 | 实现 Pass-through/Canonical/Lossless Bridge 能力分析器 | P5-01,P3-01 | 字段/Tool/Reasoning 不可损转换矩阵 | DONE |
| P5-05 | 支持同一 Upstream 的 Responses 与 Anthropic 独立 Endpoint/健康/熔断 | P5-04 | 单协议故障隔离 E2E | DONE |
| P5-06 | 实现 Thinking、Stop Reason、Usage、Cache 字段和响应模型回写 | P5-01 | 协议对照 Fixture | DONE |
| P5-07 | 建立 Claude Code `--bare` 最小 E2E 和 Plan Mode 回归 | P5-03-P5-06 | 真实客户端脱敏日志 | DONE |
| P5-08 | 加入未知字段、畸形流、截断 Tool 和取消 Fuzz/Property Test | P5-03 | 固定 Corpus 和无 Panic 报告 | DONE |

`P5-00` 是 P5 的交付流程前置项，不交付 Anthropic 功能，也不因本计划更新自动开始 P5。它是
`CR-EXEC-007` 的窄提前远端例外：必须先在 `codex/p5-anthropic` 上证明 GitHub 的实际 trigger、
required-status 与 tag cache 行为，之后 P5-01 至 P5-08 恢复“Task 本地验收、P5 一次正式远端验收”。

### G5 门禁

状态：`DONE`；P5-01 至 P5-08 的本地 review/验收和 `phase-p5-complete` 的唯一远端
Fast、Full supply-chain、Required Delivery Gate 均已通过。

- Claude Code 普通对话、普通 Tool、并行 Tool、Plan Mode 全部通过。
- 无参数 Tool 补 `{}`；非空未闭合 JSON 必须明确失败。
- Responses/Anthropic 跨协议只在能力分析通过时参与路由。
- 同一站一种协议故障不污染其它 Endpoint。
- SSE 终止事件、Usage 和错误语义符合目标协议。

## 12. P6 - Grok Build

目标：完成第一个专项 Provider，验证 OAuth、Quota、Cache 和 Response Continuity。

主要矩阵：`C02 C28 C32 C33 E06-E12 E25-E29 F07 F10-F18 G24-G28`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P6-01 | 实现 Grok Build Credential、OAuth JSON 导入和 Device Code | G5 | OAuth Mock + 脱敏导入测试 | DONE |
| P6-02 | 实现每 Credential Refresh Singleflight、Revision/CAS 和持久化 | P6-01 | 刷新风暴与旧 Token 覆盖测试 | DONE |
| P6-03 | 实现 Build Responses HTTP 请求、流和错误解析；兼容已知 OAuth 凭据来源 | P6-02 | `InferenceAdapter`/Router vertical link plus real T26 non-streaming and T33 SSE Canonical ResponseStart/text/clean ResponseEnd; T1-T33 are closed; remediated Delivery Gate passed | DONE |
| P6-04 | 实现模型、Billing、Quota Window 和 Reset 同步 | P6-03 | 来源/置信度和窗口测试；Credential-scope Catalog/Quota、version-7 migration up/down revalidation passed; remediated Delivery Gate passed | DONE |
| P6-05 | 实现租户隔离 Cache Identity 与 Cache Affinity | P6-03 | 稳定性、隔离和断裂事件测试；tenant isolation/rebind durable-break revalidation passed; remediated Delivery Gate passed | DONE |
| P6-06 | 实现 ResponseOwnership 与 ReasoningReplay | P6-03,P6-05 | previous_response 与多轮 Tool 测试；ownership/replay encrypted-clear revalidation passed; remediated Delivery Gate passed | DONE |
| P6-07 | 实现 Build 专用 401/403/429/Quota/Transient 分类 | P6-04 | 错误 Fixture 矩阵；failure action matrix revalidation passed; remediated Delivery Gate passed | DONE |
| P6-08 | 与 CPA/grok2api Build 行为做 clean-room 差分 | P6-03-P6-07 | 差分报告和 intentional diff 清单；clean-room source-boundary/report revalidation passed; remediated Delivery Gate passed | DONE |

### G6 门禁

状态：`DONE`。旧 `phase-p6-complete` Delivery Gate 只证明其旧 closeout target，不能替代 C28 的
真实双模式生命周期或可执行 Provider 垂直链路。修复后的 `phase-p6-remediated-complete` tag 已在
[GitHub Actions 29974725810](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29974725810)
完成：Classify、Fast、Full supply-chain 和 Required 均成功（Docs-only 正确跳过），所以 P6-03 至
P6-08 已按本计划的 Definition of Done 一并恢复为 `DONE`。

- 两个 Build Credential 的并发、轮询、刷新、Quota 和 Failover 通过。
- 固定直连 Build 的非流式与 SSE 都产生 Canonical `ResponseStart`、文本和无 `StreamError` 的
  `ResponseEnd`。
- Cache Identity 和 Affinity 均稳定，跨 Client Key 不串缓存。
- Response Ownership 不允许静默换账号续接。
- 旧请求不能覆盖新 Token 或错误封禁已刷新 Credential。

## 13. P7 - Kiro IDE/CLI

目标：原生实现 Kiro，不依赖 Kiro-RS 作为长期运行层，并保持 Claude Code 兼容。

主要矩阵：`C35-C47 E25 E26 G24 G28`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P7-01 | 实现 Social、IdC/Enterprise、`ksk_` 三类 Credential | G6 | 各类解析、加密和刷新 Fixture；singleflight/CAS/AEAD revalidation passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-02 | 实现 IDE/CLI Endpoint Policy、Region、Header、Origin 和 URL | P7-01 | 请求快照对照测试 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-03 | 实现 `profileArn` 查询、回退、注入、来源和审计 | P7-01,P7-02 | Builder/Enterprise 场景测试 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-04 | 实现 CanonicalRequest 到 Kiro Conversation Request | P7-02 | 多轮消息/Tool Fixture passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-05 | 实现 AWS EventStream 增量解析、CRC、边界和错误恢复 | P7-04 | 任意 Chunk + 损坏帧测试 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-06 | 实现每 Credential 动态模型与订阅能力、最后成功快照 | P7-01,P4-02 | 部分失败和 stale 测试 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-07 | 实现 Kiro Tool、AskUserQuestion、Plan Mode 和 Thinking 映射 | P7-04,P7-05 | Claude Code 回归套件 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-08 | 实现 Kiro 网络、账号、模型、额度和普通 429 分类 | P7-06 | 错误与恢复矩阵 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P7-09 | 与服务器定制 Kiro-RS 做差分、原生 `InferenceAdapter` 垂直链路和真实 `--bare` E2E | P7-03-P7-08 | 原生 IDE/CLI Adapter Fixture、差分报告、日志、模型列表；外部 OAuth 验证延后至最终认证验收包 | DEFERRED |

### G7 门禁

- CLI 和 IDE Endpoint 各自通过非流式、SSE、Tool 和 Thinking。
- 模型列表不出现重复 `-thinking` 模型。
- 单 Credential 模型查询失败不拖垮模型并集。
- EventStream 任意 Chunk 切分结果一致，CRC 错误不可静默忽略。
- 现有 Kiro-RS 生产路径可作为明确回滚方案。

## 14. P8 - Grok Official

目标：实现 xAI 官方 API Key 路径，并与 Build/Web 完全隔离状态。

主要矩阵：`C01 C03 C04 C31 C33 F07 G24-G27`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P8-01 | 实现 Official API Key、Endpoint、Header 和模型发现 | G6（`CR-P7-DEFER-002`） | 请求/目录 Fixture passed；见 P8-01 report | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-02 | 实现 Official Responses HTTP 非流式与 SSE | P8-01 | 本地 HTTP/SSE Fixture、严格分块/错误/Redaction 验收 passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-03 | 实现 Quota/Rate Header、Reset 和 Billing 元数据 | P8-02 | 严格 Header/Reset/Usage Fixture、Redaction 和本地 Full Gate passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-04 | 实现 Official Tool、Reasoning、Search 能力声明与转换 | P8-02 | Capability/Tool/Reasoning Fixture、Search 显式非能力与本地 Full Gate passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-05 | 验证 Official/Build 状态、Affinity、Quota 和故障完全隔离 | P8-02-P8-04 | Exact Header-to-quota、Build state/affinity、401/403/429/transient 隔离 Fixture 与本地 Full Gate passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-06 | 完成官方路径差分、负载和错误矩阵 | P8-05 | JSON/SSE Tool/Reasoning 差分、12 OS 线程 × 8 解码器的 96 实例隔离、精确错误/Quota 矩阵、本地 Full Gate 与 review passed | LOCAL_PASS_PENDING_PHASE_GATE |
| P8-07 | Official API Key 一次真实 E2E | P8-06、§20.1 Provider Gate E2E | 已忽略的零网络 harness、精确一 send、DNS-pinned direct egress、脱敏 lifecycle 结果；无 Official API Key，延后至与 P7-09 的最终外部认证验收包 | DEFERRED |

### G8 门禁

- Official 与 Build 同名 Public Model 只有显式 Route 才能共同候选。
- 一个来源的 401/403/429 不改变另一个来源状态。
- 官方 Tool/Reasoning 能力与公开元数据一致。
- 使用 P8 测试 API Key 的一次已授权真实 E2E，必须产生 `ResponseStart`、文本和无 `StreamError` 的 `ResponseEnd`；该授权与 P7 Kiro OAuth 无关。

当前状态：`DEFERRED_EXTERNAL_E2E`；仅缺少 Official API Key/授权，和 P7-09 一起在最终外部认证验收包回补。

## 15. P9 - Grok Web

目标：实现独立 Web/Console Provider，处理浏览器会话、出口指纹和网页协议漂移。

主要矩阵：`C29-C34 D28-D30 E27-E29 F17 G24-G28`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P9-01 | 实现 SSO/Cookie Credential、血缘和独立生命周期 | P8 local isolation evidence（`CR-P9-LOCAL-001`） | 导入、加密、失效测试 | DONE |
| P9-02 | 实现 BrowserEgressSession：Cookie、UA、TLS Profile、Proxy 绑定 | P9-01 | 指纹一致性和隔离测试 | DONE |
| P9-03 | 实现 Grok Web Chat 请求和流响应解析 | P9-02 | 脱敏网页 Fixture | DONE |
| P9-04 | 实现 WebConversationState 与账号/出口强绑定 | P9-03 | 多轮、过期和账号不可用测试 | DONE |
| P9-05 | 实现 Statsig 签名缓存、受限失效和 SSRF 防护 | P9-02 | 403、Redirect、域名测试 | DONE |
| P9-06 | 实现 REST/gRPC-Web Quota、Tier、Window、Source/Confidence | P9-03 | Quota Fixture | DONE |
| P9-07 | 实现 WAF/EgressRejected 与账号 Forbidden 分离 | P9-02,P9-03 | 403 分类矩阵 | DONE |
| P9-08 | 实现 Tool Emulation Feature Flag，默认关闭并标记 `emulated` | P9-03 | 开关与能力元数据测试 | DONE |
| P9-09 | 完成 Feature Flag 下真实账号 E2E、协议漂移和熔断演练 | P9-04-P9-08 | [Canary 报告](reports/p9-09-authorized-web-canary.md)：`CR-P9-CANARY-001/002` 下临时 grok2api Web SSO 账号的三次固定请求与 [G9 Delivery Gate](reports/g9-gate-report.md) | DONE |

### G9 门禁

- Web 账号、出口、Conversation 和 Cookie 不与 Build/Official 共享状态。
- WAF 403 默认只影响 Egress Session，除非存在账号级证据。
- Web 协议漂移只熔断 `grok.web`。
- Tool Emulation 关闭时不会对 Prompt 做隐式注入。
- 未经 Canary 报告批准，生产 Feature Flag 保持关闭。

## 16. P10 - 管理 API、Web UI、备份恢复

目标：形成可安全运营的完整控制面，但不影响数据面延迟。

主要矩阵：`H01-H22 J02 J08 J09 J11-J15 J18-J20`。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P10-01 | 完整管理 OpenAPI：Upstream、Endpoint、Credential、Catalog、Route、Group、Key | G9 | OpenAPI Contract Test、独立 review、本地 Full gate 与 P10 Delivery Gate passed | DONE |
| P10-02 | 实现管理鉴权、仅本机/私网策略、审计和 CSRF/CORS 边界 | P10-01 | 未授权和跨站测试、独立 review、本地 Full gate 与 P10 Delivery Gate passed | DONE |
| P10-03 | 建立 TypeScript SPA、生成 API Client 和静态资源构建 | P10-01,P10-02 | 65-operation generated client、可重复前端构建、独立 review、本地 Full gate 与 P10 Delivery Gate passed | DONE |
| P10-04 | 实现 Upstream/Endpoint/Credential 管理与测试工作流 | P10-03 | 浏览器 E2E、独立 review、本地 Full gate 与 P10 Delivery Gate passed | DONE |
| P10-05 | 实现 PublicModel/Route/Candidate/AccessGroup/ClientKey 工作流 | P10-03 | 创建两站 `minimax-m3` 聚合 E2E、一次性 Key redaction/reload、独立 review 与 P10 Delivery Gate passed | DONE |
| P10-06 | 实现 Catalog Diff、Health、Quota、403、Route Explain 和请求追踪页面 | P10-03,P4-06 | [Runtime 管理工作流报告](reports/p10-06-runtime-management.md)：loopback browser E2E、定向 HTTP/SPA checks、Secret/docs checks、独立 review 与 P10 Delivery Gate passed | DONE |
| P10-07 | 实现 Config Version、发布、回滚和操作审计页面 | P10-03,P2-10 | [Configuration 生命周期报告](reports/p10-07-configuration-lifecycle.md)：受保护 HTTP、loopback browser E2E、定向 checks、Secret/docs checks、独立 review 与 P10 Delivery Gate passed | DONE |
| P10-08 | 实现加密备份、恢复预检、Schema Version 和 Secret Key 说明 | P10-01 | [加密备份与空目标恢复报告](reports/p10-08-encrypted-backup-restore.md)：SQLite 空机恢复演练、受保护 HTTP、browser E2E、Secret/docs checks、独立 review 与 P10 Delivery Gate passed | DONE |
| P10-09 | 嵌入静态资源并验证 UI 不进入推理热路径 | P10-03-P10-08 | [嵌入管理 UI 与推理隔离报告](reports/p10-09-embedded-management-ui.md)：clean embedded build、精确静态 HTTP、资源隔离、独立 review 与 P10 Delivery Gate passed | DONE |

### G10 门禁

- 可以仅通过 UI/API 完成两站 `minimax-m3` 聚合配置。
- 所有 Secret 只显示一次或掩码，不可通过 API 回读明文。
- 配置发布失败时数据面继续使用上一 Snapshot。
- 空数据库可从备份恢复，并通过 SQLite `quick_check` 和真实请求。
- UI 开启/关闭对数据面基准无显著影响。

## 17. P11 - 发布加固

目标：证明兼容性、可靠性、性能和安全达到服务器灰度条件。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P11-01 | 建立 CPA v7.2.80、grok2api、Kiro-RS 的脱敏差分 Fixture Harness | G10 | [差异分类报告](reports/p11-01-differential-fixture-harness.md)：离线、脱敏、默认拒绝的六条 source-labelled Fixture 与 review/定向验证 passed | DONE |
| P11-02 | 完成网络、DNS、TLS、429、5xx、截断流、慢客户端和取消故障注入 | P11-01 | [Fault Matrix](reports/p11-02-fault-matrix.md)：loopback-only injection、Router ownership regressions、定向验证与 review passed | DONE |
| P11-03 | 建立 Mock Provider Criterion/HTTP 基准和回归阈值 | P11-01 | [基准报告](reports/p11-03-benchmark-baseline.md)：受控 offline Criterion、`baseline.json`、P99/吞吐/RSS fail-closed comparator、完整本地门禁与 review passed | DONE |
| P11-04 | 执行并发、长流、连接池、内存、背压和 ≥10h 本地 Soak | P11-02,P11-03 | [性能与 Soak 报告](reports/p11-04-load-soak.md)：`CR-P11-04-001` 下的 10h13m loopback receipt、回归、Full gate 与 review | DONE |
| P11-05 | 执行 SSRF、Secret、Auth、权限、依赖和供应链安全审计 | P11-01 | [Security Report + SBOM](reports/p11-05-security-audit.md)：SSRF/Secret/Auth/权限/依赖审计、路径脱敏 CycloneDX 与 Full gate/review passed | DONE |
| P11-06 | 验证优雅停机、流 Drain、崩溃重启、磁盘满和事件队列降级 | P11-02 | [Recovery Report](reports/p11-06-recovery-report.md)：loopback stream drain、crash/replay、deterministic `SQLITE_FULL` recovery、queue degradation、Full gate 与 review passed | DONE |
| P11-07 | 完成升级/降级 Migration、备份恢复和旧版本回滚演练 | P10-08 | [Upgrade/rollback report](reports/p11-07-upgrade-rollback.md)：in-place schema drill、loss-aware downgrade、empty-target backup recovery、Full gate 与 review passed | DONE |
| P11-08 | 生成 Release Candidate 清单、已知差异和生产默认配置 | P11-01-P11-07 | [`v0.1.0-alpha.1` candidate ledger](reports/p11-08-release-candidate.md)：inventory、safe defaults、known differences、P12 handoff 与 docs review passed；P11 最终 Delivery Gate 的 Fast、Full supply-chain 与 Required 已通过 | DONE |

### G11 门禁

- 所有差异均标记为 Intentional、Compatible 或已修复 Regression。
- 无未分类 Panic、数据竞争、流截断或 Secret 泄漏。
- 性能基准相对已批准 Baseline：吞吐下降不超过 10%，P99/RSS 恶化不超过 15%。
- Mock 上游网关附加延迟目标：本地 warm-path P99 不超过 5ms；服务器不超过 10ms。
- ≥10h loopback Soak 无内存持续增长、连接泄漏或 SQLite 损坏；真实部署后的 72h 观察仍由 P12-10 完成。
- 回滚包和恢复步骤已经实际演练，不是只写文档。

### 已批准 Change Request：CR-P11-04-001

```text
CR-ID: CR-P11-04-001
原因: 用户确认纯 loopback 合成 Soak 已运行足够久，可以视为完成；其重复有限流负载能证明
      本地任务/内存/连接稳定，但不能替代真实上游或服务器长期行为。
影响的 Task / Matrix ID / ADR: P11-04 与 G11 的本地 Soak 门槛；P12-10 的 72h Canary 保持不变。
兼容性与迁移影响: 无公开 API、Canonical、Provider、Schema、客户端或部署迁移。现有 receipt 保持
      `INCOMPLETE`，不将用户停止的 10h13m 运行伪造成 24h `COMPLETED`。
测试与回滚变化: 保留所有定向/Full/receipt 回归和 15% RSS fail-closed 条件；最低本地观察改为
      10h，真实长期运行由 P12-10 的 72h Canary 承担。回滚为恢复 P11 的 24h 本地门槛并新开 receipt，
      不重写本次证据。
用户批准: APPROVED，2026-07-24（“已经测试很久了，可以视作完成”）
计划版本变更: v1.45
```

## 18. P12 - 服务器部署与灰度

目标：在不破坏现有 CPA/AxonHub/New API/Kiro-RS 的前提下部署、验证、灰度和切换。

| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P12-01 | 构建固定版本二进制、Docker 镜像、SBOM、Checksum 和签名 | G11 | [可验证私有发布产物](reports/p12-01-release-artifact.md)；`CR-P12-01-002` 后按 x86_64 与 aarch64 双目标各自原生构建、独立签名与回执 | DONE |
| P12-02 | 编写 systemd Unit、只读 Secret、数据目录、日志和资源限制 | P12-01 | [deployment-envelope acceptance](reports/p12-02-deployment-envelope.md)：受支持条件、可执行 checker、负向回归、真实 Linux systemd 255 语法验证与本地 Full gate 均通过；未安装、启用或启动服务器 Unit | DONE |
| P12-03 | 备份当前服务器网关配置、数据库、版本和回滚命令 | P12-01 | [带时间戳、无值备份与回滚清单](reports/p12-03-server-backup-rollback.md)：现有 CPA 数据根静止快照、镜像身份、关联 unit 片段、权限、哈希和精确回滚步骤已独立复核；未安装或启动新服务 | DONE |
| P12-04 | 在独立端口和独立数据目录部署 Staging 实例 | P12-02,P12-03 | [Staging receipt](reports/p12-04-staging-receipt.md)：独立签名制品、精确 Unit、root-only 凭证、两个回环 listener、Health/管理面 admission、资源/日志/回滚均验收；服务 active 但 disabled-at-boot | DONE |
| P12-05 | 录入测试 Upstream/Key，验证 Responses、Messages、Tool、模型和 Explain | P12-04 | [CR-015 Tool/Explain 回执](reports/evidence/p12-05-cr-015-tool-explain-receipt-20260726.md)：一次新的无外部效应 Tool tuple 为 `2xx`/`valid`，唯一 protected attempt 为 `succeeded/decoder`，Explain 选中唯一 Candidate 且无新增 upstream attempt；完整回滚与独立 post-review 均通过 | DONE |
| P12-06 | 执行现有网关与新网关 Shadow/Differential 流量 | P12-05 | [OpenAI-compatible live differential](reports/p12-06-openai-differential.md)：临时 Krill 原生 `codex-api-key` 参考臂与新网关 candidate 均通过 10/10 SSE、非流式、Tool、Canonical/Usage 不变量与性能取证；修正过严的可选 Usage 明细比较后，无网络离线 review 为 9/9 PASS，完整回滚通过。Grok/Kiro 切片仍延期 | DONE |
| P12-07 | 配置独立 Cloudflare/Caddy 测试域名和最小暴露策略 | P12-04 | [暴露前验证回执](reports/p12-07-exposure-receipt.md)：新主机上从零完成产物验签、账户/目录/五凭据、unit 安装与启动（active、disabled-at-boot）；`cpar` 灰云 A 记录 + Let's Encrypt 证书；七项 fail-closed 暴露断言全通过（未认证与错误 key 均 401、管理面四路径均 404、公网直连 18180/18181 均不可达）；Caddy 变更前备份 preimage 并以 validate/adapt 证明其余五站点未变，reload 后 incumbent 仍正常。未录入任何真实上游凭据，`valid_key_accepted` 为 SKIP；该域名无限流（Caddy 标准版无模块）已记为缺口 | DONE |
| P12-08 | 完成 CPAR 全量替代准入、客户端 Key 迁移清单与生产切换准备 | P12-06,P12-07 | [`P12-08 readiness`](reports/p12-08-canary-readiness.md)、[`client migration inventory`](reports/p12-08-client-migration-inventory.md) 与 [G1 最终 review](reports/evidence/p12-08g1-final-review-20260803.md)：直接替代边界、客户端清单、三协议兼容和当前可用 Codex/Krill 12/12 真实矩阵完成；完整回滚；GitHub CI `30768254180` 的 Fast、Full supply-chain 与 Required delivery gate 通过 | DONE |
| P12-09 | 将生产主机名全量切到 CPAR，实际执行一次全量回滚并再次恢复 | P12-08 | [P12-09 execution plan](reports/p12-09-execution-plan.md)：Newapi 迁移、全量切换/回滚/恢复与三协议矩阵已执行；Required Usage 隔离触发 P1 安全回滚，等待 exact-revision 修复制品部署后重验 | IN_PROGRESS |
| P12-10 | CPAR 全量运行 72h，关闭旧 CPA，发布 Tag 和运维手册 | P12-09 | G12 报告；旧 CPA service/container 已关闭且不再承载生产流量 | PENDING |

#### P12-08 兼容性补全切片（`CR-P12-COMPAT-001`）

| Slice | 内容 | 完成证据 | 状态 |
|---|---|---|---|
| P12-08A | OpenAI Chat Completions 严格请求/响应/SSE Codec 与行为契约 | [BC-PROTOCOL-008](contracts/BC-PROTOCOL-008-openai-chat-completions-codec.md) 与 [P12-08A 报告](reports/p12-08a-openai-chat-codec.md)：非流式、事件/Tool 参数任意分片、Usage、终止与错误回归 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08B | Actix `/v1/chat/completions`、认证、正文上限、keepalive 与生命周期 | [BC-HTTP-002](contracts/BC-HTTP-002-actix-chat-completions-boundary.md) 与 [P12-08B 报告](reports/p12-08b-actix-chat-http.md)：JSON/SSE HTTP E2E、认证优先、4 MiB 上限、finish/Usage/`[DONE]` 顺序 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08C | `openai/chat-completions` Endpoint 格式与 OpenAI-compatible 出站 Adapter | [BC-PROVIDER-023](contracts/BC-PROVIDER-023-openai-compatible-chat-completions.md) 与 [P12-08C 报告](reports/p12-08c-openai-chat-adapter.md)：API Format 注册表、发布期校验、原生载荷、JSON/SSE decode、DNS-pinned transport | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08D0 | 冻结旧 CPA 三协议移植清单与差异分类 | [Legacy Behavior Manifest](reports/p12-08d0-legacy-behavior-manifest.md)：固定 v7.2.101 commit、八个显式 translator/一个 native fallback、197 个 translator tests、九协议 pair、Rust 目标模块与 parity/hardening/fail-closed 分类 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08D1 | 端口三协议请求侧转换 | [D1 request projection report](reports/p12-08d1-three-protocol-request-projection.md)：九协议 pair、三条原生载荷路径、脱敏 Tool fixture、Reasoning/输出上限 typed mapping、目标 builder 验证与属性测试通过；不可表达语义出网前拒绝 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08D2 | 端口三协议非流式与 SSE 响应转换 | [D2 response projection report](reports/p12-08d2-three-protocol-response-projection.md)：三类上游 JSON/SSE 解码、九种目标 JSON/SSE encoder 组合、任意 Chunk 最终语义投影、Tool/Usage/stop/error 闭合与全量有界限制通过 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08D3 | 接入转换注册表、runtime 与 Route Explain | [D3 runtime registry report](reports/p12-08d3-runtime-transform-registry.md)：九 pair 显式注册，native/Canonical/LosslessBridge 确定性执行；请求级转换与能力谓词在 Credential pool/lease/Attempt 前运行；Responses 生产解码与 D2 响应投影接线；Explain 给出无值原因且零 upstream attempt | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08D4 | 完成旧 CPA ↔ CPAR 离线差分与安全偏差复核 | [D4 legacy protocol differential](reports/p12-08d4-legacy-protocol-differential.md)：10 项脱敏 golden corpus 覆盖三协议 JSON/SSE/Tool/Reasoning/Usage；CPAR 侧驱动真实 codec/router 计算；6 `PARITY`、2 `INTENTIONAL_HARDENING`、2 `UNSUPPORTED_FAIL_CLOSED`，无未分类差异 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08E1 | 端口 Codex 与通用 OpenAI-compatible runtime | [E1 runtime report](reports/p12-08e1-openai-compatible-runtime.md)：Responses/Chat 共用严格 API-key/Codex OAuth 与刷新事务边界；有界 401/403/429/usage-limit/5xx 分类，Usage/Reasoning 生产解码及精确 Credential/Quota/Endpoint Health 隔离通过 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08E2 | 端口 Claude/Anthropic-compatible runtime | [E2 runtime report](reports/p12-08e2-anthropic-compatible-runtime.md)：API key/Claude OAuth 互斥授权、严格刷新事务、有界 Anthropic 错误分类、精确 Credential/Quota/Endpoint Health 隔离及 Messages/Chat/Responses Tool/Thinking/Usage/SSE vertical slice | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08E3 | 将既有 Grok Build/Official/Web Provider 接入统一 runtime | [E3 runtime report](reports/p12-08e3-grok-unified-runtime.md)：Build OAuth 与 Official API-key 接入固定目标 Canonical runtime；JSON/SSE、Tool/Reasoning/Usage、连续性与失败隔离回归通过；Web 仅登记合法 ID，因无通用生产 transport 而保持未绑定、默认禁用 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08E4 | 将既有 Kiro Provider 接入统一 runtime | [E4 runtime report](reports/p12-08e4-kiro-unified-runtime.md)：复用既有 native Adapter，统一接纳 raw API-key 与严格未过期 Social/Enterprise JSON；CLI/IDE profile、EventStream、Tool/Thinking 及精确 Credential/Quota/Endpoint Health 隔离通过 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08F1 | 组成多渠道生产图与模型/协议能力台账 | [F1 生产图与能力台账](reports/p12-08f1-multi-channel-production-graph.md)：不可变 adapter 能力表进入 Route Compiler；Alias/Access Group/Client Key/Endpoint/Credential/Candidate 无值台账、缺凭据渠道不入 active Version，Grok Web 无 runtime/不可选 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08F2 | 完成三协议 × 四类渠道的 loopback E2E | [F2 loopback 矩阵](reports/p12-08f2-three-protocol-four-channel-loopback.md)：7 个 `SUPPORTED` cell 完成 28 个 JSON/SSE × Text/Tool/Usage 请求；5 个 `UNSUPPORTED` cell 完成 10 个稳定拒绝且 Attempt=0；F1 Tool/JSON Schema 台账缺口已修正 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08F3 | 完成 Client Key、Alias 与客户端迁移 dry-run | [F3 客户端迁移演练](reports/p12-08f3-client-migration-dry-run.md)：OpenClaw `0600` 配置只读，独立临时配置/状态中完成 synthetic `rgw_` key、endpoint、active alias、协议保持与字节级回退验证，live source 未变；CC Switch 按 operator 指令 `DEFERRED_BY_OPERATOR`，未读取、复制、修改、重启或测试 | LOCAL_PASS_PENDING_PHASE_GATE |
| P12-08G1 | 对生产切换图中的可用渠道执行受控真实 E2E | [G1 最终 review](reports/evidence/p12-08g1-final-review-20260803.md)：`ec2fdf6` exact-SHA signed ARM64 artifact；当前可用 Codex/Krill 覆盖 Chat/Responses/Messages 的 JSON/SSE Text/Tool 十二 tuple 12/12 PASS；两项最终 Tool Attempt 均 succeeded；完整回滚 | DONE |
| P12-08G2 | Kiro 与 Grok Official 外部认证补验包 | 延续 P7-09/P8-07 的延期状态；获得账号后仅补对应 `SUPPORTED` tuple。账号缺失不阻止代码、本地 E2E 或不包含这些渠道的 P12 生产切换图，但禁止宣称其 live 可用 | DEFERRED |

P12-08D0-D4、E1-E4、F1-F3、G1 均是同一 P12 分支上的顺序 Task：每项独立 commit、定向 test 和
review，全部只在 P12 closeout 运行一次远端 Delivery Gate。旧 CPA fixture 只能保存脱敏后的最小
语义样本，不得把其凭据、账号、endpoint、请求/响应正文或运行日志复制进 CPAR 仓库。

### 已批准 Change Request：CR-P12-01-001

```text
CR-ID: CR-P12-01-001
原因: P12-01 的签名信任边界需要一个明确的 operator 决定；用户已选择 GitHub Actions OIDC 的
      keyless Sigstore 签名，并明确允许该 manifest 签名进入公开透明日志。
影响的 Task / Matrix ID / ADR: 仅 P12-01 的私有 CI 发布制品、manifest、signature bundle、
      独立 identity verification 与 receipt。P12-02 至 P12-10 的服务器、Staging、Canary、
      Cloudflare/Caddy 和发布权限不改变。
兼容性与迁移影响: 无公开 API、Canonical、Provider、Schema、客户端或生产服务改变。OCI 仅作为
      私有 GitHub artifact 上传；不得创建 GitHub Release/tag、推送 registry、登录服务器或部署。
测试与回滚变化: 新 workflow 必须固定 action/base image/toolchain，分别构建、SBOM 脱敏、manifest、
      `cosign sign-blob`、`cosign verify-blob` 和结构化 receipt；任一 identity/digest/SBOM/
      architecture/non-root 检查失败即冻结 P12。撤销为禁用该 manual workflow，不撤销已经写入
      Sigstore 公共透明日志的历史签名。
用户批准: APPROVED，2026-07-24（“可以”）
计划版本变更: v1.46
```

### 已批准 Change Request：CR-P12-01-002

```text
CR-ID: CR-P12-01-002
原因: P12-01 的发布流水线把 `x86_64-unknown-linux-gnu` 固化成唯一目标（workflow 的
      `RELEASE_TARGET`、产物名、`--platform linux/amd64`，以及 `p12-release-artifact.rb`
      的 `TARGET` 常量、ELF `e_machine` 断言、`architecture == "amd64"` 断言、
      `generate-p12-01-sbom.rb` 的目标白名单）。已确认实际生产主机是 `aarch64`
      （Ubuntu 24.04.4，4 vCPU/23 GB），既有签名产物在其上不可执行，因此 P12-08 的
      Canary 无法开始。GitHub 自 2026-01-29 起对私有仓提供标准 `ubuntu-24.04-arm`
      runner（计入计划免费额度，私有仓 2 vCPU），Arm 合作镜像内置 Docker/Buildx/Ruby/Node，
      因此可以原生构建而不引入 qemu 模拟。另经实测，`Dockerfile` 现钉的
      `debian:bookworm-slim@sha256:63a496b5...` 是单架构 amd64 manifest（其 config
      `architecture=amd64`），不是多架构 index，arm64 侧必须另钉自己的 digest。
影响的 Task / Matrix ID / ADR: P12-01 的产物矩阵与 §22 供应链纪律；P12-08 的前置条件。
      (1) 目标集合：发布流水线从单目标改为 `x86_64-unknown-linux-gnu` 与
      `aarch64-unknown-linux-gnu` 两个目标，各自在与目标架构相同的 GitHub 标准 runner 上
      原生构建（`ubuntu-24.04` / `ubuntu-24.04-arm`），不使用交叉编译或 qemu；因此
      "签名前在无网络、只读、非 root 下真实执行过该二进制"这条既有证据在两个目标上等价成立。
      (2) 产物形状：每个目标产出自己的一整套 payload（二进制、build metadata、SBOM、
      OCI 归档、signing identity），各自生成 manifest、各自 keyless 签名、各自 receipt；
      两套产物互不混合，manifest 不跨架构聚合，也不生成多架构 image index。
      (3) 校验器：`p12-release-artifact.rb` 的目标由常量改为封闭白名单，并按目标推导
      预期二进制名、ELF `e_machine`（x86_64=62 / aarch64=183）与 OCI `architecture`
      （amd64 / arm64）；未知目标一律 fail closed。SBOM 生成器同样改为封闭白名单。
      (4) 基础镜像：`Dockerfile` 的 `FROM` 改为按架构传入的 build arg，x86_64 沿用现有
      amd64 digest `sha256:63a496b5...`，aarch64 使用 arm64 digest `sha256:9b672946...`；
      两者都是 `debian:bookworm-slim` 同一 index（`sha256:7b140f37...`）的成员，仍逐架构
      钉死 digest，`base.name` 标签随之逐架构记录。因为基础镜像不再写死在 Dockerfile 里，
      "已钉死"这一性质改由产物侧重新建立：校验器把 `org.opencontainers.image.base.name`
      与目标表中的 digest 逐字比对，workflow 检查器再断言 matrix 的 `base_image` 与校验器
      表一致，两处任一漂移即 fail closed。该 `base.name` 比对不是冗余检查：本地实测在 arm64
      上用 amd64 digest 构建**不会**失败，而是产出一个 config 谎称 `architecture=arm64` 的
      损坏镜像，仅靠 architecture 断言无法识别，只有 `base.name` 比对能拒绝它。
      (5) 不变量：仍是 `workflow_dispatch` 手动触发、仍无任何 push/registry/deploy/tag
      步骤、仍为私有 CI artifact、仍 keyless OIDC 签名并独立 `cosign verify-blob`、
      OCI 仍非 root 且不暴露监听端口。
兼容性与迁移影响: 无公开 API、Canonical 协议、Provider、Schema 或数据迁移。既有 x86_64
      产物与 P12-01 至 P12-05 的历史回执不重写、不失效；`docs/reports/p12-01-release-artifact.md`
      记录的哈希继续对应其原始 x86_64 产物。服务器端 `scripts/p12-05-cr-013-staging-transaction.sh`
      的 `uname -m` 守卫按其所验证产物的架构判定，不放宽为"任意架构"。附带修复一处独立缺陷：
      两个 SBOM 生成器原以 `{apps,crates}/**` glob 清理 `cargo-cyclonedx` 原始输出，P12-06
      新增的 `tests/differential` 工作区成员因此会在 checkout 里留下含本地路径的未跟踪原始
      SBOM；改为从 `cargo metadata` 推导清理集合，覆盖全部现有与未来成员。P11-05 的冻结
      SBOM 证据文件不重写。
测试与回滚变化: `scripts/test-p12-release-artifact.rb` 扩展为对两个目标各跑一遍全部正向与
      负向路径，并新增跨架构混淆的负向用例（aarch64 目标配 x86_64 ELF、arm64 目标配 amd64
      OCI config 必须被拒），以及基础镜像 digest 的负向用例（另一架构的 digest 与未钉 digest
      的 `debian:bookworm-slim` 均必须被拒）。`scripts/check-release-artifact-workflow.rb`
      断言两个 job、两个 runner 标签、两个 platform、逐架构基础镜像 digest 与校验器目标表
      一致，拒绝任何 qemu/binfmt/多 platform 模拟构建，且继续断言无发布/部署命令；并静态
      校验 Dockerfile 中被 `FROM` 引用的 ARG 必须声明在第一个 `FROM` 之前（首次远端运行
      即因该作用域错误使两个 job 同时失败，本地无 buildx 时该错误不可见）。
      回滚为恢复本 CR 的文件 preimage（单目标 workflow 与常量），既有 x86_64 产物不受影响。
用户批准: APPROVED，2026-07-27（"1.做一个arm的"；"可以，开始A1"）
计划版本变更: v1.72
```

### 已批准 Change Request：CR-P12-02-001

```text
CR-ID: CR-P12-02-001
原因: P12-02 的 systemd Unit 必须指向可验证的长期运行入口，但 P12-01 的制品只有 transport-free
`gateway admin` CLI，没有 bind/listen 入口。用户确认在 P12-02 的部署装配范围内补齐最小 `gateway
serve`，而不是提交一个无法启动的 Unit。
影响的 Task / Matrix ID / ADR: 仅 P12-02 以及 P12-04 的未来 loopback Staging health 前置。新增两个
明确、不同的 loopback listener：数据 listener 仅 `HEAD /` 与 `GET /healthz`；管理 listener 仅装配
既有 P10 管理 API/UI、P10 admission state、SQLite lifecycle/resource state 与 backup facade。公开
Inference/Responses/Messages/Tools、RouteSnapshot、Client-Key data-plane auth、Provider transport、
模型路由、Caddy/Cloudflare、服务器配置和生产流量不在本 CR 范围；P12-05 仍须单独装配并验证真实
data-plane runtime。
兼容性与迁移影响: 现有 `gateway admin` 命令语义保持不变。新的 `serve` 仅接受显式的 loopback 地址、
systemd StateDirectory 和 LoadCredential 目录；不得读取环境中的 Secret、浏览器/Profile/代理、服务器
文件或隐式配置。管理网络仍由 P10 的 loopback-only policy 和独立 Management Key 保护。
测试与回滚变化: 为 `serve` 的参数、listener 隔离、凭据文件、P10 composition 以及 Unit 的
`systemd-analyze verify` 增加验证。macOS 本地保留相同的静态 invariant 校验，Linux P12 Delivery
Gate 强制执行 `systemd-analyze verify`。回滚删除 serve/deployment assets，不改变任何服务器或数据。
用户批准: APPROVED，2026-07-25（“确认”）
计划版本变更: v1.47
```

### 已批准 Change Request：CR-P12-04-001

```text
CR-ID: CR-P12-04-001
原因: 已验收的 P12-01 私有制品绑定 revision ecabe04e，早于 P12-02 新增的 `gateway serve`；该制品
      只能执行 transport-free `gateway admin`，不能诚实地满足 P12-04 的 systemd Staging 入口。用户
      明确批准为当前 P12 revision 重建私有 artifact 并新增一次 GitHub OIDC keyless Sigstore/Rekor 记录。
影响的 Task / Matrix ID / ADR: 仅 P12-04 的制品前置和一次 `LOCAL_PASS_PENDING_CI` 例外。先推送现有
      `codex/p12-deployment` 分支的已复核提交，再从该精确 GitHub SHA 运行既有 `release-artifact` workflow；
      生成的二进制、OCI、SBOM、manifest、签名 bundle 和 receipt 仍是私有 workflow artifact。P12-01 的
      既有验收历史不重写；P12-05-P12-10、P12 Delivery Gate、P7/P8 外部认证延期均不改变。
兼容性与迁移影响: 此例外本身不连接服务器、不安装/启动 Unit、不创建 PR/tag/GitHub Release、不推送 registry，
      也不改变 CPA/AxonHub/New API/Kiro-RS、Caddy、Cloudflare、DNS、Credential、Provider 或生产流量。
      新增的 Sigstore 签名写入公开 Rekor 透明日志；制品 payload 仍不公开。只有独立验证 identity、revision、
      digest、SBOM、架构、非 root OCI 和 `gateway serve --help` 全部通过后，才可将相同 digest 用于 P12-04
      的后续 loopback Staging；任一失败即冻结 P12-04 并不得写服务器。
测试与回滚变化: workflow 保持固定 action/base image/toolchain、manifest 签名和内部 verify；本地另行下载
      私有 artifact，运行 `p12-release-artifact.rb verify --require-signature --require-receipt`、OCI 结构检查
      及 `gateway serve --help`。远端例外失败时保留失败 run、停止任务；不得通过本地未签名二进制绕过。撤销为
      不下载/不部署新 artifact，不撤销已写入 Rekor 的历史透明记录。
用户批准: APPROVED，2026-07-25（“批准”）
计划版本变更: v1.48
```

### 已批准 Change Request：CR-P12-05-001

```text
CR-ID: CR-P12-05-001
原因: P12-05 的已审阅生产数据面组成改变了 Linux `gateway` 二进制；P12-04 已验收的
      Staging artifact 早于该来源，不能复用。用户批准从包含本 CR、运行时和证据的精确
      `codex/p12-deployment` SHA 重新生成私有 artifact，并在独立 Staging 执行受控验证。
影响的 Task / Matrix ID / ADR: 仅 P12-05 的一次 revision-bound artifact/deployment 例外。
      推送精确已审阅 SHA 后，手动运行既有 `release-artifact` workflow；二进制、OCI、SBOM、
      manifest、signature bundle 和 receipt 仍为私有 workflow artifact。独立验证签名 identity、
      revision、digest、SBOM、ELF、OCI 和 `gateway serve --help` 通过后，才可将同一 digest
      安装到既有 isolated loopback Staging。随后仅可备份其 `control.sqlite3`、通过 stdin 注入
      选定 Bearer、建立一个临时单例图，并执行 P12-05 的 Models/Responses/SSE/Messages/Tool/
      Explain 顺序验收。P12-06 至 P12-10、P12 Delivery Gate、P7/P8 延期均不改变。
兼容性与迁移影响: 不创建 PR、tag、GitHub Release 或 registry image；新增 Sigstore manifest
      签名会写入公开 Rekor 透明日志，artifact payload 保持私有。不得读取、复制或使用 CC Switch
      OAuth token；仅可通过受保护的 loopback 管理 API stdin 边界使用选定 Bearer。不得改动
      incumbent CPA/AxonHub/New API/Kiro-RS、Caddy、Cloudflare、DNS、任何公开 listener 或公开流量。
测试与回滚变化: workflow 必须通过其固定 build/SBOM/manifest/Cosign 验证；本地必须再次独立
      验证 artifact。任一 GitHub、provenance、listener、credential-envelope、protocol/lifecycle
      或 incumbent-continuity 检查失败即停止，不向后续 Task 推进。验收后默认在 Staging 停止时
      恢复其 P12-05 preimage，除非用户另行要求保留图供 P12-06 使用。
用户批准: APPROVED，2026-07-25（“批准”）
计划版本变更: v1.49
```

### 已批准 Change Request：CR-P12-05-002

```text
CR-ID: CR-P12-05-002
原因: CR-P12-05-001 的 `9d62339` artifact 在独立验签和 `serve --help` 后、切换 `current`
      前发现 P12 runtime 只接纳 `PriorityFailover`，而唯一受保护的 management API 只持久化
      `SmoothWeightedRoundRobin`。旧 artifact 从未启动，已从独立 release path 移除；修复后的
      精确 revision `104e72860f29805d0975dff03f3a771f40b0201d` 已通过 focused、package、
      management-contract 和 Full gate。
影响的 Task / Matrix ID / ADR: 仅 P12-05 的一次 replacement revision-bound artifact。推送
      该精确已审阅 SHA 后，手动运行既有 `release-artifact` workflow；仍只保留私有 binary、OCI、
      SBOM、manifest、signature bundle 和 receipt。独立验证 identity、revision、digest、SBOM、
      ELF、OCI 和 Linux `gateway serve --help` 后，才可进入既有 isolated loopback Staging 的
      备份、临时单例图与最小受控验证；P12-06 至 P12-10、P12 Delivery Gate、P7/P8 延期均不改变。
兼容性与迁移影响: 不创建 PR、tag、GitHub Release 或 registry image；新增 manifest 签名写入
      公开 Rekor 透明日志，payload 保持私有。不得读取、复制或使用 CC Switch OAuth token；只可
      经受保护 loopback 管理 API 的 stdin 边界使用选定 Bearer。不得改动 incumbent CPA/AxonHub/
      New API/Kiro-RS、Caddy、Cloudflare、DNS、公开 listener 或公开流量。
测试与回滚变化: workflow 和本地独立验签仍为硬门槛；任一 provenance、listener、credential-
      envelope、protocol/lifecycle 或 incumbent-continuity 异常即停止。用户要求后续仅当同一
      P12-05 范围、同一私有 artifact 形态、同一 isolated Staging、且不新增 Provider/Credential/
      公开暴露时，修复性 exact-SHA 续签可直接批准；仍必须登记 CR、重跑适用本地 gate 并逐次验签。
      超出这些边界仍须取得新的明确授权。
用户批准: APPROVED，2026-07-25（“批准，这种批准直接过就可以”）
计划版本变更: v1.50
```

### 已批准 Change Request：CR-P12-05-003

```text
CR-ID: CR-P12-05-003
原因: P12-05 post-artifact 本机配置复核的一次 helper 曾在用户临时目录中创建 `0600` 的
      Bearer 选择文件；该文件在同一 helper 内删除，absence check 通过，且没有 Value 输出、
      Git 写入、服务器传输、Staging SQLite 写入或服务器侧 Provider 请求。它仍不符合
      memory-only 证据标准，因此不可以支撑后续图写入。
影响的 Task / Matrix ID / ADR: 仅 P12-05。该偏差不扩展 Credential、Provider、artifact、
      Staging、监听器或公开流量范围；后续必须以纯内存 helper 重做一次同一无请求体直接
      `/models` 预检，然后才能执行既有 root-only SQLite preimage、临时图、最小验收与回滚。
兼容性与迁移影响: 不保留或复制临时文件，不读取/传输 OAuth access、refresh 或 ID token，
      不将 Provider URL、模型、Bearer 或完整请求/响应写入 repository、报告、命令行、环境或日志。
      后续唯一服务器 Secret 持久化仍是既有管理 API 创建的加密 Credential envelope。
测试与回滚变化: 该旧 helper invocation 明确排除在通过证据外。新的 helper 必须不创建任何
      plaintext Secret 文件；若再次发生，立即停止并重新评估。P12-05 验收后仍按原计划恢复
      SQLite preimage，除非用户另有明确方向。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定）
计划版本变更: v1.51
```

### 已批准 Change Request：CR-P12-05-004

```text
CR-ID: CR-P12-05-004
原因: 在 CR-P12-05-003 要求的纯内存、直连、无请求体 `/models` 预检中，选定 Krill/Codex
      endpoint 的标准三请求头轮廓返回 4xx；最小 delta 证明加入已验证、非机密的
      `User-Agent: codex_cli_rs/0.139.0` 即可获得 2xx JSON，且不需要 `OpenAI-Beta`。现有
      P12 runtime 仅构造前三个标准头，因此在创建临时图前必须修复这一确定的兼容性差异。
影响的 Task / Matrix ID / ADR: 仅 P12-05。该补丁只在 P12 的隔离 Krill 出站路径上增加该
      固定 User-Agent，并保留请求 URL 与已准入目标的精确绑定及其回归；通用
      OpenAI-compatible Provider 保持原有三头合约。相同 isolated loopback Staging、单例图、
      Credential、最小验证和回滚顺序保持不变。
兼容性与迁移影响: 不刷新或重新登录 CC Switch，不读取或传输 OAuth token，不新增 Provider、
      Credential、egress host、redirect、public listener、Caddy、Cloudflare、DNS 或公开流量。
      必须从包含该已 review 修复的精确 SHA 新建私有 GitHub OIDC/Sigstore artifact，并独立
      验证 identity、revision、digest、SBOM、ELF、OCI 与 Linux `gateway serve --help`，旧
      artifact 不得用于任何 P12-05 data-plane 写入或验证。
测试与回滚变化: focused regression、`gateway` package tests、Clippy、Full/docs/Secret gates
      和独立 review 均为硬门槛。任一 artifact、preimage、listener、credential-envelope、
      protocol/lifecycle 或 incumbent-continuity 异常都停止并恢复 P12-05 preimage；不得进入
      P12-06。沿用既有同范围、无新增 Credential/Provider/公开暴露的直接批准约定。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定；
      当前 CC Switch Krill key 与 base URL 可用、无需刷新）
计划版本变更: v1.52
```

### 已批准 Change Request：CR-P12-05-005

```text
CR-ID: CR-P12-05-005
原因: 使用 CR-P12-05-004 artifact 的受控 Staging 序列已通过 Models、OpenAI Responses
      非流式和 SSE，随后在 Anthropic Messages 的首次请求前本地转换失败并返回 gateway 5xx；
      harness 已停止，未执行 Tool/Explain，且已恢复 P12-05 preimage。根因是 Anthropic 必填
      `max_tokens` 经纯 codec 保留为 `anthropic.messages.max_tokens`，而 OpenAI Responses
      builder 正确拒绝非 `openai.responses.` 根扩展。当前 CC Switch Krill key 与 base URL
      经用户确认可用，无需刷新，因此不将该本地确定失败归因于 Credential 或 endpoint。
影响的 Task / Matrix ID / ADR: 仅 P12-05。P12 runtime 在打开 Credential 或构造出站请求前，
      仅将已验证为正整数的 `anthropic.messages.max_tokens` 移除并重写为
      `openai.responses.max_output_tokens`；同时存在目标扩展、非正/非整数源值、或任何其他
      foreign extension 均继续 fail-closed。通用 OpenAI-compatible Provider、Anthropic codec、
      egress、retry、Credential、监听器及公开边界不变。
兼容性与迁移影响: 不刷新、重新登录、复制或传输 CC Switch OAuth/Bearer 值；不新增 Provider、
      Credential、egress host、redirect、Caddy、Cloudflare、DNS、public listener 或公开流量。
      必须以包含已 review 修复的精确 SHA 新建私有 GitHub OIDC/Sigstore artifact 并独立验证
      identity、revision、digest、SBOM、ELF、OCI 和 Linux `gateway serve --help`；旧 artifact
      不得用于此修复后的 P12-05 验收。
测试与回滚变化: 新增已验证 Anthropic Canonical `max_tokens` extension → P12 translation → OpenAI
      Responses body 的 focused regression，断言输出上限和其余 foreign-extension rejection。focused/package/Clippy/
      Full/docs/Secret gates 与独立 review 均为硬门槛。新 artifact 后从 fresh temporary graph
      重跑 listener/readiness、Models、Messages、未覆盖 Tool、Explain；此前通过的 Responses
      非流式/SSE 路径由 translation-absent no-op regression 复用。任一异常立即停止并恢复 preimage，
      不得进入 P12-06。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定；
      当前 CC Switch Krill key 与 base URL 可用、无需刷新）
计划版本变更: v1.53
```

### 已批准 Change Request：CR-P12-05-006

```text
CR-ID: CR-P12-05-006
原因: CR-P12-05-005 的修复后二进制尚未写入新的 Staging 图，但上一轮 Messages 已在
      服务器侧返回 gateway 5xx。随后相同 selected Bearer/base URL 的 server-only `/models`
      诊断为 2xx JSON，排除了服务器直连、base URL 与选定 Bearer 的可用性。为避免在
      Staging 反复写入/回滚，需先用一次最小、无正文留存的 `/responses` 请求确定响应是否
      满足现有严格 JSON decoder 的结构门槛。
影响的 Task / Matrix ID / ADR: 仅 P12-05。允许恰好一次服务器本地、非流式、直连 HTTPS
      `POST /responses` 分类请求，使用当前 CC Switch Codex 配置的同一 selected Bearer、
      base URL、模型和已验证 User-Agent，短固定无副作用提示以及现有 P12 Responses request
      轮廓。响应只能在内存 pipe 中被分类为状态、content-type、JSON/严格 decoder gate 和
      安全白名单结构类别；不得写入或输出 endpoint、Bearer、OAuth、模型、ID、正文或 token
      fingerprint。该请求不是 Staging 验收、不能替代任何 P12-05 required request，且不允许
      Tool、Explain、P12-06 或公开暴露。
兼容性与迁移影响: 不刷新/重新登录、不复制或传输 OAuth 值，不新增 Provider、Credential、
      egress host、proxy、redirect、Caddy、Cloudflare、DNS、listener 或公开流量。不会写
      Staging SQLite、systemd unit、Client Key 或 incumbent CPA。若分类证明需要 source repair，
      必须先独立 review、适用 local gate、精确 SHA 私有 OIDC/Sigstore artifact 与独立验签，
      才可再次写入 Staging。
测试与回滚变化: 诊断进程最多发送一次，不重试；临时脚本与 header 文件在同一进程退出时删除。
      任一 transport/HTTP/JSON/structure 异常仅形成无值 receipt 并停止，之后根据分类决定
      repair 或已有 CR-005 Staging 序列，绝不直接进入 P12-06。沿用既有同范围、无新增
      Credential/Provider/公开暴露的直接批准约定。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定；
      当前 CC Switch Krill key 与 base URL 可用、无需刷新）
计划版本变更: v1.54
```

### 已批准 Change Request：CR-P12-05-007

```text
CR-ID: CR-P12-05-007
原因: CR-P12-05-006 的 selector 已通过，远程 ephemeral classifier 也已退出并自删除，但本机
      zsh 结果包装器使用了保留变量名，导致其已接收的无值分类输出未被打印或持久化。无法证明
      当次远程分类在输入校验前停止还是已经发送了请求，因此按最严格边界把 CR-006 视为其一次
      request 已消耗，绝不重放它。需要一项独立、可审计的替代请求，避免在又一次结果收集失败时
      丢失诊断证据。
影响的 Task / Matrix ID / ADR: 仅 P12-05。允许恰好一次新的 server-local、非流式、直连 HTTPS
      `/responses` 结构分类，输入/轮廓/无副作用提示与 CR-006 相同。新 classifier 必须在返回
      SSH 前先将仅含状态类别、content-type 类别、JSON/decoder gate 类别和 request count 的
      receipt 写入既有 root-only P12-05 receipt 根；不得写入任何 endpoint、Bearer、OAuth、
      模型、ID、正文、header 值或 digest。CR-006 不再允许任何重试。
兼容性与迁移影响: 不刷新/重新登录、不复制 OAuth/Bearer 值，不写 Staging SQLite、Client Key、
      systemd、listener、Provider/Credential/egress 配置、Caddy、Cloudflare、DNS、公开流量或
      incumbent CPA。该替代诊断仍不是 Staging 验收，不授权 Tool、Explain、P12-06 或任何公开暴露。
测试与回滚变化: 新的 local launcher 禁止使用 zsh 保留变量，直接流式输出远程 safe receipt；
      server classifier 在 stdout 前完成 root-only receipt 持久化。若 receipt persistence 或
      transport/shape 失败，停止并仅依据 receipt 处理；若需要 source repair，仍须 review、适用
      local gate、精确 SHA 私有 artifact 与独立验签后才可写 Staging。沿用同范围直接批准约定。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定；
      当前 CC Switch Krill key 与 base URL 可用、无需刷新）
计划版本变更: v1.55
```

### 已批准 Change Request：CR-P12-05-008

```text
CR-ID: CR-P12-05-008
原因: CR-P12-05-007 的 root-only receipt 证明当前服务器对同一 selected Bearer/base URL/model
      的短 Responses 请求得到 2xx JSON，并通过了安全的可见 decoder 结构子集。然而该分类
      有意不保留正文，不能替代 Rust 的完整 decode/Canonical 验证，也不能解释此前 Staging
      Anthropic Messages 的 5xx。原 Staging harness 仅记录 5xx 类别，遗漏了安全的精确 HTTP
      状态和 gateway Anthropic error envelope 类型，无法区分 local/internal、upstream-protocol
      或 transient path。
影响的 Task / Matrix ID / ADR: 仅 P12-05。允许在已经独立验收的 CR-005 exact-SHA artifact 上
      一次新的 root-only preimage、temporary singleton graph、loopback Models 和 Messages 重跑。
      临时 harness 可将 gateway 的精确 HTTP status 及白名单 Anthropic error envelope type
      （或成功 lifecycle）加入无值 receipt；不得保留 error message、request/response body、
      endpoint、Bearer、OAuth、模型、ID 或 header 值。若 Messages 通过，原定同一次临时图的
      no-side-effect Tool 与 Explain 顺序可继续；任一失败立刻回滚，不盲目重试。
兼容性与迁移影响: 不改 Rust source、artifact、Credential、Provider、egress、proxy、redirect、
      Staging listener/systemd、Caddy、Cloudflare、DNS、公开流量或 incumbent CPA。已验收的
      CR-005 artifact 是唯一允许使用的二进制；无需刷新或重新登录 CC Switch。
测试与回滚变化: harness 先以静态、白名单 parser 复核 error-envelope 分类，不将未知字符串
      输出；运行前仍验证 artifact SHA、incumbent、loopback 和 preimage，运行后强制 restore
      及 receipt/listener/incumbent 复核。根据 result 决定最小 source repair 或完成剩余
      P12-05，不得进入 P12-06。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定；
      当前 CC Switch Krill key 与 base URL 可用、无需刷新）
计划版本变更: v1.56
```

### 已批准 Change Request：CR-P12-05-009

```text
CR-ID: CR-P12-05-009
原因: CR-P12-05-008 Staging receipt 的精确结果为 HTTP `502` 和白名单
      `overloaded_error`。在该 one-candidate/one-attempt runtime 中，结合路由 failure 映射，
      这表示首个 Canonical 事件前出现了 protocol/truncation 级 failure，而不是 Credentials
      的 4xx/429。CR-007 的 classifier 仅验证可见结构子集，遗漏 `decode_json_events` 的
      空文本、Usage/Reasoning-token 与 Canonical lifecycle 细节，不能判断 failure 是上游响应
      字段不兼容还是发送前 P12 request conversion/admission。
影响的 Task / Matrix ID / ADR: 仅 P12-05。允许一次新的 server-local direct HTTPS
      non-streaming `/responses` classifier，复用相同 selected base URL/Bearer/model、固定
      prompt、User-Agent 和 request shape。它必须镜像现有 non-streaming Rust decoder 的每个
      可安全分类 gate，且先写 root-only 无值 receipt；只输出第一个固定 gate 名称、HTTP/
      content-type/JSON 类别和 request count。不得输出或保留正文、ID、文本、模型、URL、
      header 值、Credential、OAuth 或 digest。
兼容性与迁移影响: 不刷新/重新登录，不写 Staging 图、artifact、source、Provider/Credential、
      egress、listener、Caddy、Cloudflare、DNS、公开流量或 incumbent CPA。该 request 不是
      Staging acceptance，不授权 Tool/Explain/P12-06；CR-008 failed run 已完整 rollback。
测试与回滚变化: exactly one request/no retry；如果 full classifier 命中 gate，才按最小范围修复
      并走 source review/local gate/exact-SHA artifact；如果通过，则不假设 decoder 修复，改为
      审查 pre-send runtime branch。沿用同范围直接批准约定。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定；
      当前 CC Switch Krill key 与 base URL 可用、无需刷新）
计划版本变更: v1.57
```

### 已批准 Change Request：CR-P12-05-010

```text
CR-ID: CR-P12-05-010
原因: CR-P12-05-009 的 receipt-first full decoder classifier 对同一 direct Responses request
      返回 `2xx`/JSON，并通过 `accepted_exact_nonstreaming_contract`。这与 CR-008 的
      Staging `502/overloaded_error` 相冲突，但不构成 source repair 的充分证据：classifier
      不保留正文且不能证明 P12 request-time branch。此前 CR-004 的同一路径已通过 Responses
      non-streaming/SSE，CR-005 的窄 max-token translation 也有 local regression；因此一次
      完整、receipt-enhanced Staging retry 比推测性代码改动更能区分 transient/request-context
      差异与真实重复 runtime failure。
影响的 Task / Matrix ID / ADR: 仅 P12-05。允许使用当前已验收 CR-005 exact-SHA artifact 进行
      一次新的 isolated Staging preimage/singleton graph/restart/Models/Messages sequence，保留
      CR-008 的精确 HTTP/closed error-type receipt。如果 Messages 通过，同一次临时图可继续
      无副作用 Tool 和 Explain；任一错误立即 rollback。重复的 Messages 502 视为足够的
      source-attribution blocker，停止而不再用外部重试猜测。
兼容性与迁移影响: 不修改 Rust source、artifact、Credential/Provider/egress、proxy、listener、
      systemd、Caddy、Cloudflare、DNS、公开流量或 incumbent CPA；不刷新/重新登录。该 retry
      不是 P12-06，验收后继续恢复 Staging preimage。
测试与回滚变化: 运行前 SHA/loopback/incumbent/preimage checks 和运行后 receipt/root-only
      encrypted metadata/listener/incumbent/empty baseline checks均为硬门槛。成功后进入
      P12-05 review；失败后只进行本地 source diagnosis/review，再由新的 exact-SHA artifact
      路径处理。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定；
      当前 CC Switch Krill key 与 base URL 可用、无需刷新）
计划版本变更: v1.58
```

### 已批准 Change Request：CR-P12-05-011

```text
CR-ID: CR-P12-05-011
原因: CR-P12-05-010 的 Staging retry 重复了 `502/overloaded_error`。随后 source review 发现
      CR-007/009 direct classifier 手写的 `input` item 缺少
      `OpenAiResponsesRequestBuilder::flush_message_parts` 固定写入的 `"type":"message"`。
      因此先前的 2xx classifiers 是相近 request 的证据，不能证明真实 P12 outbound body；该
      形状差异可能令 Krill 选择不同的 compatibility path 或 response form。
影响的 Task / Matrix ID / ADR: 仅 P12-05。允许一次新的 server-local direct HTTPS non-streaming
      `/responses` full-decoder classifier，使用同一 selected configuration、max output limit、
      User-Agent 和固定 prompt，但 `input` item 必须包含 P12 builder 的 `type:"message"`、
      role 和 content 三字段。仍先写 root-only 无值 receipt，最多一次、无重试，只输出固定
      HTTP/content/JSON/decoder-gate 类别。
兼容性与迁移影响: 不刷新/重新登录；不写 Staging、artifact、source、Provider/Credential、
      egress、listener、Caddy、Cloudflare、DNS、公开流量或 incumbent CPA。该诊断不是 P12-05
      acceptance，不进入 Tool/Explain/P12-06。
测试与回滚变化: prior near-shape results保持事实但不得作为 exact-body decoder evidence。若新
      classifier 命中 gate，按该 gate 做最小 source repair；若通过，则 Staging retry 的
      repeat 502 需要 source-level attempt-stage instrumentation，不能再做外部推测性重试。
      沿用同范围直接批准约定。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定；
      当前 CC Switch Krill key 与 base URL 可用、无需刷新）
计划版本变更: v1.59
```

### 已批准 Change Request：CR-P12-05-012

```text
CR-ID: CR-P12-05-012
原因: CR-P12-05-011 的精确 P12 outbound-body classifier 已完成 `2xx` JSON 和完整
      non-streaming decoder-contract 验收，但 CR-P12-05-008/010 的 isolated Staging
      Messages 均在首个 Canonical 事件前返回相同 `502/overloaded_error` 并完整回滚。
      当前 P12 executor 使用 Noop event sink，且 runtime facade 对 request attempts
      总是返回空，`decode_json_response` 又把 body-read/decoder 异常折叠为同一
      `BootstrapTruncated`，因此无法以事实区分 request conversion、egress admission、
      transport、HTTP/content-type、body-read 与 decoder 阶段。
影响的 Task / Matrix ID / ADR: 仅 P12-05 的 Staging 可观测性修复。允许在 P12 runtime
      增加有界、进程内、非持久化的 attempt-stage 投影，并经既有受保护、loopback-only
      `/admin/requests/{request_id}/attempts` 读取。每项只能暴露既有 opaque request/
      attempt correlation、`succeeded|failed` 和固定 stage 枚举；不得暴露或保留 URL、
      Header、Body、Bearer/OAuth、模型、Endpoint、Credential、错误字符串、状态码、
      token、时序或 digest。该投影不是新公开数据面或新的管理 listener。
兼容性与迁移影响: 不刷新或重新登录 CC Switch，不改 selected Bearer/base URL、Provider、
      Credential、egress policy、proxy、redirect、Caddy、Cloudflare、DNS、listener、
      incumbent CPA 或公开流量。不得发送新的 direct classifier、重放已关闭的 request，
      或启动 P12-06。阶段记录不可用/饱和时必须 fail closed 为管理面 unavailable，且不得
      阻塞、重试或改变 data-plane request 结果。
测试与回滚变化: 此 source change 必须先完成 focused regressions、适用 local Full gate、
      secret scan 与独立 review；随后仅可对精确 SHA 生成私有 GitHub OIDC/Sigstore artifact
      并独立验签。只有该 artifact 在原有 isolated Staging 复核通过后，才允许一次有 receipt
      的 Messages 诊断；无论结果均立即恢复 preimage，并停止在 P12-05 处理结论。
用户批准: APPROVED，2026-07-25（既有“批准，这种批准直接过就可以”的同范围直接批准约定；
      当前 CC Switch Krill key 与 base URL 可用、无需刷新）
计划版本变更: v1.60
```

### 已批准 Change Request：CR-P12-05-013

```text
CR-ID: CR-P12-05-013
原因: CR-P12-05-012 的一次 receipt-enhanced isolated Staging Messages 诊断已通过
      Models，随后只发送了一次 Messages 请求并完整回滚。受保护 attempt 投影为
      `succeeded/decoder`，证明精确上游 Responses 响应已完成 P12 decoder；失败发生在其后的
      Anthropic 输出编码生命周期。源码复核确认非流式 decoder 将已报告 usage 仅作为
      MessageEnd 之后的 final delta 发出，并使用缺失 stop_reason 的 ResponseEnd，分别不满足
      Anthropic 的 message_start usage 和终止语义。无需再发送 direct classifier 或猜测
      Credential、endpoint、egress、proxy 或上游可用性。
影响的 Task / Matrix ID / ADR: 仅 P12-05。P12 non-streaming Responses decoder 在解析已报告
      usage 后，只有当真实 `input_tokens` 存在时才在 MessageStart 前发出 input-only 的
      non-final UsageDelta；绝不估算或伪造缺失 usage。MessageEnd 后 OpenAI Responses 仍保留完整
      final usage；Anthropic Messages 仅投影其可表示的总量与 cache-input 计数，并省略无对应字段的
      reasoning/cached 子计数。ResponseEnd 对文本 completion 映射为 `end_turn`、对 Function Call
      completion 映射为 `tool_use`。不改变泛用 OpenAI-compatible Provider、Anthropic decoder/encoder、
      路由、retry、数据模型、对外 listener 或任何 P12 以外的运行时。
兼容性与迁移影响: 不刷新、重新登录、输出、复制、持久化或扩大使用 selected Bearer；不改
      Provider、Credential、egress host、DNS、proxy、redirect、Caddy、Cloudflare、systemd、
      Staging listener、incumbent CPA 或公开流量。此前 CR-012 Staging 图已恢复，旧 artifact
      不得用于此修复后的验证。P12-06、Tool、Explain 和额外 direct probe 仍未获授权。
测试与回滚变化: 新增 decoded event 通过真实 Actix `/v1/messages` 与 `/v1/responses` 边界的
      offline regression，覆盖 input usage 顺序、Responses 完整 final usage、Messages 的可表示
      usage 投影、`end_turn` 与 `tool_use`。
      focused/package/Clippy/Full/docs/Secret gates 与独立 review 都是硬门槛。随后必须创建并
      独立验签精确 SHA 的私有 GitHub OIDC/Sigstore artifact；仅该 artifact 可在 fresh isolated
      Staging 图中重跑 readiness/listener、Models 和恰好一次 Messages，且无论结果都恢复
      preimage。任何异常即停止，不得进入 P12-06。
用户批准: APPROVED，2026-07-26（用户明确仅为完成本次 P12-05 受控验证，暂时继续使用该已暴露凭证；
      沿用既有同范围直接批准约定）
计划版本变更: v1.61
```

### CR-P12-05-013 执行结果

精确 SHA 私有 OIDC/Sigstore artifact 已在本机独立验签后写入隔离 Staging release 目录，并通过
Linux `gateway serve --help`。随后唯一允许的临时图事务完成 loopback Models 与恰好一次
Anthropic Messages：HTTP 为 `2xx`，内存 lifecycle 检查得到 `end_turn` 与数值 usage，受保护
attempt 投影为 `succeeded/decoder`。事务无失败并完整恢复数据库 preimage、原 current link、
Staging loopback 服务与 incumbent continuity；临时 harness 已删除，未执行 Tool、Explain、
direct probe、P12-06 或任何 public exposure。详见
[CR-013 Staging 回执](reports/evidence/p12-05-cr-013-staging-receipt-20260726.md)。

### 已批准 Change Request：CR-P12-05-014

```text
CR-ID: CR-P12-05-014
原因: CR-P12-05-013 已用独立验签的精确 artifact 成功完成 Models 与一次 Anthropic
      Messages 生命周期验证并完整回滚。P12-05 仅剩原计划中尚未发送的一次无副作用 Tool
      请求和本地受保护 Route Explain；用户已明确授权继续。
影响的 Task / Matrix ID / ADR: 仅 P12-05 的剩余验收。复用已独立验签的
      `49f8c0f3eb6326d3f2ed6cc612ec8ffd10915938` artifact，建立 fresh root-only
      preimage/temporary singleton graph。不得重发 Models、Responses、SSE 或 Messages。
      仅允许恰好一次非流式 OpenAI Responses Tool 请求；它只声明一个 test-only no-op
      Function，Gateway 不执行该 Function。只有该请求为 2xx、回传一个结构有效的
      `function_call` 表示、且 protected attempt projection 为唯一 terminal
      `succeeded/decoder` 时，才允许恰好一次受保护的 Route Explain。Explain 只能投影唯一
      Candidate，必须证明没有 upstream 请求。
兼容性与迁移影响: 不刷新、重新登录、输出、复制、持久化或扩大 selected Bearer/OAuth 的使用；
      不新增 Provider、Credential、endpoint、egress host、DNS、proxy、redirect、Caddy、
      Cloudflare、systemd、listener、公开流量或 artifact。现有 CPA/AxonHub/New API/Kiro-RS
      均不改变。P12-06 至 P12-10、Canary、公共暴露及任何 direct probe 不在本 CR 范围。
测试与回滚变化: root-only harness 必须先检查 artifact、disabled-at-boot、loopback-only
      listener、无 proxy、empty preimage、encrypted Credential envelope 和 incumbent continuity。
      Tool 的 request/response body、endpoint、Credential、模型、ID 或 token fingerprint
      均不得写入 receipt、报告、命令行或 Git；只记录封闭的状态/结构类别。Tool 失败、结构不符、
      attempt 不符、Explain 非 selected/no-upstream、listener 或 incumbent 异常均立即停止，
      不运行后续动作。无论结果都恢复数据库 preimage、原 current link、loopback Staging 和
      incumbent continuity；独立复核后才能更新 P12-05 状态。
用户批准: APPROVED，2026-07-26（“所有权限都通过”；本 CR 将授权限制为 P12-05 剩余 Tool/Explain，
      不隐含 P12-06 或公开暴露）
计划版本变更: v1.62
```

### CR-P12-05-014 执行结果

CR-014 的唯一 Tool 请求通过 loopback Staging 发送并收到 `2xx`，但内存中的结果没有通过
Function Call 表示的无值结构门槛。正文已在同一 root-only 进程中丢弃；因此该证据不能归因
为上游不支持、模型选择或 Gateway 解析错误。按照 fail-closed 顺序，受保护 attempt 投影和
Explain 都没有执行。事务已恢复数据库 preimage、原 current link、Staging loopback 服务和
incumbent continuity；独立复核确认空图、disabled-at-boot 和监听器边界均恢复。详见
[CR-014 Tool 回执](reports/evidence/p12-05-cr-014-tool-explain-receipt-20260726.md)。

### 已批准 Change Request：CR-P12-05-015

```text
CR-ID: CR-P12-05-015
原因: CR-P12-05-014 的唯一 Tool 请求已消耗：HTTP 为 `2xx`，但 response body 按规则未保留，
      只留下 `tool_representation=invalid`，故不能安全地推断它是文本完成、Function Call 名称/
      参数差异或其他已完成响应形态。其 request wording 中“do not execute”也可能被模型理解为
      不要声明调用。为避免盲重放同一 tuple，使用新的、明确说明“调用只是一项无外部效应的声明”
      的 test-only instruction，并把无值分类从二元结果细化为封闭类别。
影响的 Task / Matrix ID / ADR: 仅 P12-05 剩余验收。复用同一已独立验签 artifact，创建 fresh
      root-only preimage/temporary singleton graph。不得重发 Models、Messages、SSE、旧 CR-014
      Tool tuple 或任何 direct probe。仅允许恰好一次新的非流式 OpenAI Responses Tool tuple：
      同一 test-only no-op Function、不同的明确 instruction；它不执行 Tool，也不产生外部副作用。
      回执只可记录 `valid`、`no_function_call`、`multiple_function_calls`、`wrong_function_name`、
      `unexpected_*` 或其他封闭结构类别。只有 `valid` 和唯一 `succeeded/decoder` attempt 后，
      才允许恰好一次本地 Route Explain；Explain 必须选中唯一 Candidate 且不产生 upstream 请求。
兼容性与迁移影响: 不刷新、重新登录、输出、复制、持久化或扩大 selected Bearer/OAuth 的使用；
      不改 artifact、Provider、Credential、endpoint、egress host、DNS、proxy、redirect、Caddy、
      Cloudflare、systemd、listener、公开流量或 incumbent CPA。P12-06 至 P12-10、Canary、公共
      暴露以及任何新的直连诊断均不在本 CR 范围。
测试与回滚变化: 先复核协议 decoder、P12 Function Call lifecycle 与 Responses encoder 的现有本地
      回归；root-only harness 必须对新的 tuple 执行单发送/无 retry，任何 Tool transport/status/
      structure/attempt 或 Explain/边界异常均停止。正文、endpoint、Credential、模型、ID、URL、
      header 和 token fingerprint 不得进入 receipt、报告、命令行或 Git。无论结果都恢复 preimage、
      原 current link、disabled-at-boot loopback Staging 与 incumbent continuity。
用户批准: APPROVED，2026-07-26（“所有权限都通过”；本 CR 将该授权收窄为一项新的 P12-05
      Tool tuple 与条件性 Explain，不隐含 P12-06、直连诊断或公开暴露）
计划版本变更: v1.63
```

### CR-P12-05-015 执行结果与 P12-05 本地验收

CR-015 的一个新 Tool tuple 获得 `2xx`/`valid`，其 protected attempt 为唯一
`succeeded/decoder`。随后唯一允许的 Route Explain 选中唯一 Candidate；同一 attempt 投影保持
不变，证明 Explain 没有产生 upstream attempt。root-only receipt 与独立复核都确认数据库
preimage、empty graph、current link、disabled-at-boot loopback Staging、两个 loopback listeners
和 incumbent continuity 已恢复；临时 harness 已删除。详见
[CR-015 Tool/Explain 回执](reports/evidence/p12-05-cr-015-tool-explain-receipt-20260726.md)及
[CR-015 post-review](reports/evidence/p12-05-cr-015-post-review-20260726.md)。

至此 P12-05 的 Models、OpenAI Responses 非流式/SSE、Anthropic Messages、Tool 和 Explain
证据均已闭合。它成为 `LOCAL_PASS_PENDING_PHASE_GATE`，不是 P12 Phase 或 Release 的 `DONE`；
P12-06 Shadow/Differential 流量、P12-07 public test domain 以及任何公开暴露仍需各自的计划
边界与执行决定。

### 已批准 Change Request：CR-P12-05-016

```text
CR-ID: CR-P12-05-016
原因: 一次针对全仓的独立 review 在准备切换的 serve 二进制上确认了四项确定性缺陷，全部未被
      现有测试覆盖，且都会在 P12-06 至 P12-08 的真实流量下必然触发。
      (1) `/v1/messages` 的 `stream:true` 在 200 header 提交后 100% 失败：`OpenAiSseEventSource`
      在 `MessageStart` 前不发 `UsageDelta`，而 Anthropic 编码器硬性要求先有精确 input usage；
      `response.completed` 的 usage 未做投影，`ResponseEnd` 无 stop_reason。CR-013 的生命周期修复
      只打在非流式 JSON decoder，漏掉同文件的 SSE 源。Claude Code 默认 `stream:true`。
      (2) 流式 Tool 调用中断流：`tools` 无条件编入出站请求体，但 SSE 状态机拒绝 `function_call`
      item 与 `response.function_call_arguments.*`，且只允许一个 output item。
      (3) 三个数据面 handler 用 `web::Bytes`，继承 actix 默认 256 KiB 上限，长会话请求被
      提取器以非协议化纯文本 413 拒绝。
      (4) 全仓无 SSE keepalive（BL-05 只定义未实现），叠加 45s 绝对总超时与 64 KiB 正文/帧上限；
      P12-07 将本网关置于 Cloudflare/Caddy 之后，静默连接会被中间层回收。
影响的 Task / Matrix ID / ADR: 仅 P12-05 的 serve 组合与其协议边界。放宽一处锁定契约：
      `message_start` 不再要求已上报精确 input Usage。改为"起始放宽、终止收紧"——未上报时
      `message_start` 省略 `input_tokens`（不填 0、不估算），并在终止 `message_delta` 中
      强制要求精确 input 计数，未上报则 fail closed。据此更新 BC-PROTOCOL-002、BC-PROTOCOL-005
      与 ADR-0034；BC-HTTP-001 记录有界正文读取与 keepalive；BC-PROTOCOL-001 澄清 keepalive
      归 HTTP 边界所有而非编解码器帧。不改 Provider、Credential、egress、DNS、proxy、Caddy、
      Cloudflare、systemd、listener、公开流量或 incumbent CPA。P12-06 至 P12-10 范围不变。
兼容性与迁移影响: 客户端可见变化四项：Anthropic 流式 `message_start` 可能不含 `input_tokens`
      而由终止 `message_delta` 携带；超过 8 MiB 的请求体返回 `413` 与该路由自身的协议化错误
      信封；空闲 15 秒的流式响应写入 SSE 注释 `: keepalive`（非语义、不提交 FirstSemanticEvent、
      不关闭透明重试）；流式与非流式 transport 边界拆分为 8s/30s/120s/1h 与 8s/600s/600s/600s，
      两处 64 KiB 上限提高到 8 MiB。既有精确 usage 上游的帧字节完全不变，全部冻结 fixture
      与 snapshot 保持一致。无 Schema、迁移或管理面变化。
测试与回滚变化: 新增 27 项本地测试：Anthropic usage 延后交付与"从不上报即 fail closed"、
      流式 Tool 生命周期/并行 Tool/文本后接 Tool/空参数归一化/失败关闭矩阵、BL-04 任意分块
      不变性、两个协议边界的 E2E 编码、keepalive 的间隔与 BL-05 不提交性与双协议共享、
      有界正文的接受/拒绝/声明长度/未认证优先四类、以及 transport profile 的选择与上限。
      本地 `check.sh fast` 的 fmt、clippy `-D warnings`、全量测试、source policy、crate
      boundaries 与 secret 扫描均通过。回滚为 revert 本 CR 的提交；无服务器、制品或数据变更。
用户批准: APPROVED，2026-07-26（先批准放宽方案 A"发 0 或省略字段"，实现按 ADR-0034 已否决
      "发 0 会谎称已测量值"取省略；再明确"批准 CR 然后按照要求 commit"）
计划版本变更: v1.65
```

### 已批准 Change Request：CR-P12-05-017

```text
CR-ID: CR-P12-05-017
原因: CR-P12-05-016 的独立 review 留下三项已确认的遗留缺陷，均会在 P12-06 至 P12-08 的真实
      流量下触发。(1) 非流式 600s 传输上限不可达：orchestrator 把整个 driver.start() 包在
      路由 bootstrap deadline（准入上限 15s）内，而缓冲型上游生成完才回 header，整段生成
      等待都在 start() 里，超过 ~15s 的非流式回答必然被截断。(2) 卡死上游可占用唯一凭据租约
      直到 1 小时绝对上限：传输 byte-idle 只测字节静默，`response.in_progress` 等 no-op 帧
      会无限重置它，而本网关自身的 15s 客户端 keepalive 使客户端无从察觉。(3) `take_frame`
      每 chunk 从 0 全缓冲区双重扫描，帧上限提至 8 MiB 后为 O(n²)，单 worker 数据面可被拖停；
      流式工具标识符仅受帧上限约束，最坏 ~256 MiB 驻留状态。
影响的 Task / Matrix ID / ADR: P12-05 的 serve 组合与 gateway-router 的 Attempt 编排。
      gateway-router 新增带默认实现的 `AttemptDriver::start_timeout(remaining_bootstrap)`
      trait 端口：orchestrator 仅用它约束单次 in-flight start()，`RetryBudget` 与"何时允许
      开始下一次 attempt"完全不变，既有三个 driver 实现字节不变；BC-ROUTER-003 增补该条款。
      P12 驱动仅对非流式返回 bootstrap 余额加传输总额，流式保持原边界。P12 运行时新增双层
      语义活性：解码器内 4096 连续无进展帧预算（按帧计数，与分块无关），传输层 15 分钟
      进展预算——仅累计真正等待上游 next_chunk 的时间，下游客户端停读产生的背压不消耗
      预算，避免把健康完成误判为卡死；到期为终止性 StreamError，唯一凭据租约随之释放，
      最坏占用窗口由 1 小时降至约 17 分钟。解码器改为 consumed/scanned 双游标加摊销压缩，
      每字节至多扫描一次，缓冲上限 2 倍帧上限、活跃残留 1 倍；工具 item/call 标识符超过
      256 字节 fail closed。不改 Provider、Credential、egress、公开流量或 incumbent CPA。
兼容性与迁移影响: 客户端可见变化两项：超过 ~15s 的非流式回答不再被截断（上限为传输 600s）；
      连续 15 分钟无生成进展证据（或连续 4096 个 keepalive 帧）的流式响应以 StreamError
      终止而非空耗至 1 小时。推理模型思考停顿不受影响：reasoning-summary/reasoning-text、
      content-part、refusal 等帧即使被丢弃也计为进展。无 Schema、契约帧格式或管理面变化。
测试与回滚变化: 新增 16 项本地测试：orchestrator 的扩展上限/默认上限/首字节前重试保持/安全
      截断错误四项；活性的纯 keepalive 切断、注释帧同预算、进展重置、真实回环对端的切断与
      思考存活、下游停读不消耗预算六项；游标的混合分隔符跨块不变性、仅计活跃残留的帧预算、
      压缩后 1 MiB 帧、标识符边界通过/超界拒绝五项；以及常量关系断言。cursor 修改另经
      4300+ 例随机差分模糊对照旧实现验证输出与接受/拒绝完全一致。回滚为 revert 本 CR 的
      提交；无服务器、制品或数据变更。
用户批准: APPROVED，2026-07-26（"继续修这三个遗留项吧"）
计划版本变更: v1.66
```

### 已批准 Change Request：CR-P12-ROLLOUT-001

```text
CR-ID: CR-P12-ROLLOUT-001
原因: P12-06 至 P12-10 启动前有两个未决范围问题。(1) §2.1 把 Grok Build、Kiro、Grok Official、
      Grok Web 四个专项切片列为必须交付，而 P7/P8 的真实外部认证验证按 CR-P7-DEFER-002 与
      CR-P8-DEFER-001 延后，G12 的"100% 流量"因此含义不明。用户确认 Kiro 与 Grok 当前没有
      可用渠道、暂不测试：不存在此类生产流量，切换范围即全部实际流量。(2) P12-08 的
      10%→25%→50%→100% 分流机制未指定；Canary 规则要求"任一条件触发立即回滚"，而
      DNS/TTL 类切换在定义上不满足。现有生产链路为 Cloudflare（DNS/边缘）→ 服务器本地
      Caddy（TLS 终止/反代，见功能矩阵 A17）→ CPA 容器（见 P12-03 回执 Incumbent boundary）。
影响的 Task / Matrix ID / ADR: P12-06 至 P12-10 的范围与机制定义；§2.1 与 G12 的解释。
      (1) 范围：Release 1 的四个专项切片按"代码完成并通过本地验证"交付，生产路由不启用；
      启用延后至各自外部认证收口（P7-09/P8-07）后另行 CR。G12 的"100% 流量"指全部实际
      生产流量（当前均为 OpenAI-compatible 类）。(2) 分流层：Canary 百分比在服务器本地
      Caddy 上以加权/按 Client Key 匹配的反代规则执行，生产主机名不变；Cloudflare 配置
      保持现状不动，P12-07 的独立测试域名仅用于暴露前验证（DNS/TLS/Auth、管理监听器公网
      不可达、限流）。回滚手段为预置旧配置的 `caddy reload`；P12-09 演练须实测该 reload
      的生效时延并记为 RTO 证据。P12-07 上服务器核对现行 Caddy 配置时，须确认其读/空闲
      超时高于数据面 15 秒 SSE keepalive 间隔。
兼容性与迁移影响: 本 CR 为范围与机制决定，不含代码变更。公开 API、Canonical、Provider、
      Schema 不变。既有 CPA 在 Canary 期间继续承载未分流部分；Kiro/Grok 切片的既有本地
      验证证据与延后边界不变。
测试与回滚变化: P12-06 差分与 P12-08 各阶段证据按本范围采集；P12-08 的分流配置进入服务器
      前须在 Staging 复核语法；回滚为撤销本 CR 的文档变更，不触及服务器。
用户批准: APPROVED，2026-07-26（"kiro和grok还是没有渠道，先不测试"；"确认 Caddy 方案，写 CR"）
计划版本变更: v1.67
```

### 已批准 Change Request：CR-P12-05-018

```text
CR-ID: CR-P12-05-018
原因: 独立 review 确认 429/403 的 fail-closed 状态在生产路径不可恢复：runtime_quota 的
      begin/complete_recovery_probe 与 runtime_health 的 begin/complete_account_recovery
      在 workspace 内零生产调用者，P12 管理 facade 对配额恢复恒返回 Rejected。单一凭据
      部署下一次 429/403 即永久退出调度直至进程重启，违反 BL-17 的"Reset 后受控探测"。
影响的 Task / Matrix ID / ADR: gateway-router 与 P12 serve facade。live 选择路径在普通
      选择失败后，可把一个到期 `RecoveryRequired` binding 作为单张 CAS ticket、lease-first
      的受控探测 Attempt：成功以 Estimated 空窗口快照完成恢复；再次 429 以新快照取代
      ticket 回到 cooldown；Health 谓词先行；403 账户永不被当作配额探测（BL-16 要求账户级
      证据）。P12 管理 facade 注入与请求路径相同的 Health/Quota 运行时句柄：403 账户凭
      操作者证据本地 begin+complete(Allowed)；到期配额可操作者覆盖、Reset 前拒绝；均不
      发送 Provider 流量。BC-CRED-002、BC-ROUTER-003、BC-MGMT-001、ADR-0028、ADR-0032
      同步增补。
兼容性与迁移影响: 公开 API 不变。行为变化一项：一次 429 不再永久移除凭据——Reset 到期后
      的下一次选路自动发起恰一次受控探测（并发选路至多一张 ticket，无雷群）。
测试与回滚变化: 7 项新本地测试（单次受控探测自恢复、并发至多一张 ticket、失败探测回
      cooldown 不抖动、403 永不作为配额探测、仅到期 binding 可选、操作者配额覆盖、操作者
      403 恢复）。回滚为 revert 对应提交。
用户批准: APPROVED，2026-07-26（"完成前瞻三件事"）
计划版本变更: v1.68
```

### 已批准 Change Request：CR-P12-05-019

```text
CR-ID: CR-P12-05-019
原因: P4 的有界事件队列、append-only SQLite 事件 writer 与单消费者 telemetry fan-out
      （BC-OBS-001/002/003）此前从未被 serve 二进制实际组合：P12 数据面 sink 只喂内存
      stage ledger，durable 事件日志无生产写入者，Prometheus 计数器无生产暴露面，管理面
      Attempt listing 依赖 8 槽内存 ledger 并在其失效时整体 fail closed，隐藏了 durable
      证据本可回答的查询。
影响的 Task / Matrix ID / ADR: P12 serve 组合与管理监听器。(1) 数据面 sink 改为 fanout：
      先记 stage ledger（饱和队列不能隐藏 stage），再进有界队列，队列准入结果为权威
      （BL-10：Required 丢失显式为 RequiredQueueFull，仅低优先诊断可丢）。(2) serve 组合
      构建 BoundedEventQueue + AsyncSqliteEventWriter（挂 TelemetryPipeline：共享
      PrometheusMetrics、TracingJsonExporter 经进程级 tracing JSON subscriber、Noop OTel），
      部署 envelope 在两个监听器 bind 后 spawn writer，停机以 5s 有界等待 join，超时为显式
      EventLogFlushIncomplete 失败而非伪造 flush。(3) 管理监听器新增受保护
      GET /admin/observability/metrics（text/plain; version=0.0.4），仅渲染冻结有界计数器并
      在 scrape 时镜像队列准入计数，不读 durable 日志、不阻塞于 SQLite。(4) 管理 facade 持有
      SqliteEventStore 只读连接，Attempt listing 改为 durable 时间线（含事件已有的非秘密
      endpoint_id/credential_id），stage ledger 降级为仅对无歧义单 Attempt 时间线的 stage
      enrichment，任何 ledger 损失退化为无 stage 列表而非隐藏 durable 证据。BC-MGMT-001、
      BC-MGMT-002（OpenAPI 增补该只读暴露）、BC-OBS-002/003（组合引用）、ADR-0027/0030/0032
      相应增补。gateway_event_log 由迁移 0005 触发器保持 append-only，serve 期无保留策略，
      有界增长风险显式接受（每完成请求 3 行 Required），修剪需新迁移加 ADR-0027 修订。
兼容性与迁移影响: 公开数据面 API、Canonical、Provider、Schema 不变。管理面 Attempt 行为
      变化：listing 由内存投影改为 durable 日志支撑，新增 endpoint_id/credential_id 可选
      字段（契约 Schema 已声明），ledger 失效不再使 listing 整体不可用。新增管理只读
      metrics 路由在既有 X-Management-Key 准入之后，无请求级标签。
测试与回滚变化: 新增 5 项本地测试：serve 组合端到端持久化（HTTP 200 → writer flush 3 行
      Required → Prometheus 快照 → durable 读回 → facade listing 含 stage/endpoint/credential）；
      fanout 溢出保持 Required 损失显式且 stage 投影完好；ledger 耗尽后 durable listing 存活；
      管理 metrics 暴露的认证/内容类型/无值泄漏回归；组合 drop 后 writer 自行 flush 退出。
      3 项既有 ledger 测试改写为新读端口语义。回滚为 revert 本 CR 提交；无服务器或数据变更。
用户批准: APPROVED，2026-07-26（"完成前瞻三件事"）
计划版本变更: v1.69
```

### 已批准 Change Request：CR-P12-06-001

```text
CR-ID: CR-P12-06-001
原因: P12-06 影子流量与 P12-08 Canary 需要生产图形状（多 Upstream/Endpoint/Credential/Candidate、
      预首字节重试 max_attempts>1、别名、多公开模型与多 Client Key），而 P12-05 的 serve 组合
      仅放行经评审的 singleton 图（单 Endpoint/Credential/Key，max_attempts==1）。
影响的 Task / Matrix ID / ADR: apps/gateway serve 组合的准入与组装（runtime.rs）；
      ADR-0029 固定输入 Route Explain 的 P12 投影随之从单 Candidate 扩为全 Candidate 序。
      范围仍按 CR-P12-ROLLOUT-001 钉死 OpenAI-compatible：任一 Endpoint 的 api_format 非
      `openai/responses` 或 Candidate 非 Canonical 即整版拒绝（fail-closed，不静默跳过）。
      新增边界：max_attempts 1..=5（全部预首字节，BL-05）；绑定总并发 <=16（守住有界驻留
      内存）；egress 仍 HTTPS-only/Deny 重定向/无 CIDR 例外；bootstrap 上限 15s 不变。
      RouteCompiler 能力档案改为从控制库全部版本的 Endpoint 并集派生（能力集仍为空，
      Candidate 仍需 allow_unlisted_model）；新增全新 Endpoint 的草稿需一次隔离重启后才能
      validate，沿用既有 restart-after-publication 生命周期。
兼容性与迁移影响: 既有 singleton 图仍可组装（向后兼容测试保留）。公开 API、Canonical、
      Schema 不变；BL-05/07/08/09/10/17 基线不变。
测试与回滚变化: 新增组装/准入/失败切换（loopback 预首字节 failover）/越界拒绝测试；回滚为
      还原本 CR 涉及的 apps/gateway 变更。
用户批准: APPROVED，2026-07-26（"完成前瞻三件事"；范围与分流机制承接 CR-P12-ROLLOUT-001）
计划版本变更: v1.70
```

### 已批准 Change Request：CR-P12-06-002

```text
CR-ID: CR-P12-06-002
原因: §2.1 把聚合控制面列为必须交付，docs/05 §M0-D 要求 Anthropic-compatible Messages
      Endpoint 作为上游端点类型，docs/05:540 记录 Kiro-RS 迁移期正走该路径。但独立 review
      以证据确证三处后端缺口：(1) protocol-anthropic 只有入站方向，Canonical→Anthropic 请求
      与 Anthropic 响应/SSE→Canonical 全仓不存在；(2) provider-anthropic-compatible 是 6 行
      空壳；(3) BL-06 只做了数据建模——ProtocolFormat::from_api_format 已认得两种格式，但
      endpoint_api_format() 零生产消费者，没有任何代码按格式选择适配器，且 api_format 在
      store 中是自由 String，route_compiler 只查唯一性、从不校验取值可服务。
影响的 Task / Matrix ID / ADR: protocol-anthropic 新增出站方向（encode_upstream_request、
      decode_upstream_response、AnthropicMessagesSseDecoder），以入站解码器的同一
      CacheControlCollector 证明 prompt_cache_retention 可反导，并由自有 CanonicalEventState
      逐事件校验，使解码器结构上无法产出非法 canonical 序列。provider-anthropic-compatible
      填充为该格式的 provider 边界（x-api-key + anthropic-version 四固定 Header），并再导出
      响应方向，使装配根只经这一个边界进出该格式。gateway-protocol 空壳填充为封闭 ApiFormat
      词表 + ApiFormatAdapterRegistry，并成为 adapter_id↔api_format 配对的唯一真源：
      route_compiler 在 validate/publish 即以 unsupported_endpoint_api_format 拒绝不可服务的
      格式与错配的 adapter_id，装配期再校验工厂产出的适配器格式与声明一致。P12 执行器改用
      BC-ROUTER-005 的协议过滤入口 start_with_event_sink_for_protocol，使多格式图下的候选选择
      按客户端协议进行。Kiro/Grok 切片的延后边界不变（CR-P12-ROLLOUT-001）。
兼容性与迁移影响: 既有单格式配置图行为不变，OpenAI 路径的解码器、活性边界与传输 profile
      逐字节未改。新增客户端可见能力：可录入 anthropic/messages Endpoint 并由该格式的适配器
      服务。新增拒绝：未收录的 api_format、或 adapter_id 与 api_format 不配对的 Endpoint，在
      发布前整版拒绝而非运行时才失败。Usage 投影改为按客户端协议双向收窄——Anthropic 上游
      恒报的 cache-input 计数不再泄漏给 Responses 客户端编码器。
测试与回滚变化: 新增 22 项本地测试（出站编码器往返一致性含 cache_control 再导、SSE 解码器
      1/3/29/单发分块不变性、工具与思考流、fail-closed 矩阵、格式与 adapter_id 准入拒绝、
      工厂绑定校验、Anthropic SSE 活性与 bootstrap 拒绝）。回滚为 revert 本 CR 的提交。
用户批准: APPROVED，2026-07-27（"按照后端开发方案完成整个后端开发，批准你所有权限"）
计划版本变更: v1.71
```

### 已批准 Change Request：CR-P12-06-003

```text
CR-ID: CR-P12-06-003
原因: P12-06 的任务卡是"执行现有网关与新网关 Shadow/Differential 流量"，但其执行范围一直未
      获授权，且用户提供的新信息改变了该任务的性质。(1) incumbent CPA **没有 Kiro 渠道**，
      因此 Kiro 不存在"新旧两侧"可比，它是新网关独有的新能力；对它只能做功能验证与性能
      基线，不能称作差分。真正可差分的是 OpenAI-compatible 那条（CPA 有）。(2) 用户提供了
      线上 kiro-rs 的凭据用于测试。经只读盘点：四个凭据中三个 `authMethod=api_key`、
      前缀 `ksk_`、36 字符、`endpoint=cli`，与 cpa-rust `KiroApiKeyCredential` 的
      `starts_with("ksk_")` 校验吻合；当前仅 id=5（KIRO PRO MAX）启用。api_key 是静态凭据
      且代码明确"never participates in OAuth refresh"，因此复制给 cpa-rust 使用**不会**与
      线上 kiro-rs 互相失效 access_token（此前担心的共用 refresh_token 风险不存在）。
      (3) A3/A4 已查明服务端无延迟分位数（7 个计数器、零 histogram），因此性能证据必须走
      客户端侧测量，不能假称服务端提供。
影响的 Task / Matrix ID / ADR: P12-06 的执行范围与证据定义；不改代码、公开 API、Canonical、
      Provider、Schema，不改 incumbent 配置或 kiro-rs。
      (1) 范围与性质分离：P12-06 产出两类互不混淆的证据。
        (a) **差分**（有两侧可比）：仅 OpenAI-compatible 路径，逐字段比对 canonical 投影——
            事件序列、Tool call 结构、Usage 各字段（input/output/reasoning/cache_read/
            cache_creation）、错误分类、终止原因。
        (b) **功能验证 + 性能基线**（只有一侧）：Kiro 渠道，用 api_key 凭据。报告必须显式
            标注这一类**不是**差分，因为 incumbent 无此渠道。
      (2) 流量来源：固定合成请求，不做真实生产流量影子复制。理由：P12-06 要发现的是语义与
      协议层面的差异，合成请求可刻意打到边界条件（超长上下文、特定 tool schema、罕见参数
      组合），而影子复制会真实消耗上游配额、在上游账号留调用记录并产生费用，且 Caddy 只能
      路由不能镜像。真实流量的价值在 P12-08 的 Canary 自然获得。
      (3) 凭据边界：Kiro 用从 kiro-rs 复制的 api_key（静态、无刷新、不影响 kiro-rs）。
      OpenAI-compatible 侧的凭据边界不在本 CR 内，另行决定。凭据值不得进入对话或仓库，由
      用户写入服务器文件后按路径读取。
      (4) 性能指标（用户明确要求"至少包括首字耗时、总耗时和 token 输出速率"）：
        - **首字耗时**：按 `FirstSemanticEvent`（BL-05 透明重试边界）测量，即首个**语义**
          事件到达时刻，而非首个字节；这才对应"用户看到第一个字"。
        - **TTFB**：首个响应字节（含响应头）。并记的理由是它与首字耗时之差正好暴露上游思考
          停顿时长，对诊断有用。
        - **总耗时**：到流终止。
        - **token 输出速率**：输出 token 数 / (总耗时 - 首字耗时)，即首字之后的稳态速率；
          用总耗时作分母会把思考停顿摊进速率，掩盖真实吞吐。
        - 附带记录 token 间延迟 P50/P95/P99，用于暴露流式过程中的抖动。
        每项按 Kiro 与 OpenAI-compatible 分别汇总，且必须记录样本量——单次测量不作结论。
      (5) 边界不变：本 CR 不解除 `CR-P7-DEFER-002` 与 `CR-P8-DEFER-001` 的外部认证延期；
      Kiro 的 P7-09/G7 仍是 DEFERRED，本 CR 的证据**不**计为 Kiro 的 Provider Gate 通过。
      也不改变 `CR-P12-ROLLOUT-001` 的切换范围：Kiro 生产路由仍不启用，本 CR 仅授权在
      隔离测试域名与本地回环上验证该渠道。不授权任何生产主机名分流。
兼容性与迁移影响: 无代码、公开 API、Canonical、Schema、部署或安全迁移。线上 kiro-rs 不被
      修改、不被停止、其凭据文件只读不写。incumbent CPA 与用户的 CPA/CPAMP 迁移不受影响。
测试与回滚变化: 新增可执行的性能测量与差分采集脚本，其输出为无值证据（不含 prompt 内容、
      不含凭据、不含响应正文）。回滚为删除本 CR 新增的脚本与报告，并从 cpa-rust 配置图中
      移除测试用 Kiro Upstream/Endpoint/Credential。
用户批准: APPROVED，2026-07-31（"现在有kiro的凭据，可以拿线上kiro-rs里的凭据拿来测试"；
      "性能对比尽量详细"；"至少包括首字耗时，总耗时和token输出速率"；"先做 P12-06"）
计划版本变更: v1.75

待用户提供后方可执行的前置项（不阻塞本 CR 批准）：
  - kiro-rs 的 idc/OAuth（buildId 登录）缺陷修复需要源码；服务器 /opt/example-kiro-adapter 只有
    二进制与备份，无源码、无 .git。用户已同意该修复作为独立任务后置，不阻塞 P12-06。
```

### 已批准 Change Request：CR-P12-06-004

```text
CR-ID: CR-P12-06-004
原因: 用户要求在 cpa-rust 上测试 Kiro 渠道并明确 `CR-P12-ROLLOUT-001` 的"不启用"是"暂时
      不使用，现在需要使用"。核查发现 Kiro 渠道的开发确已完成（P7-01 至 P7-08 均
      `LOCAL_PASS_PENDING_PHASE_GATE`，`KiroInferenceAdapter` 完整实现 `InferenceAdapter`：
      请求构建、AWS EventStream 解码、失败分类、profileArn 注入齐备），但它**从未接入生产
      组合根**：`apps/gateway/Cargo.toml` 不依赖 `provider-kiro`，`p12_api_format_adapter_registry()`
      只注册 OpenAI Responses 与 Anthropic Messages 两个工厂。原因不是缺实现，而是 P12 运行时
      走的是"编解码器 + 请求构建器"组合方式而非 `InferenceAdapter` trait（对照：
      `provider-openai-compatible` 反而没有 `InferenceAdapter` 实现），而 Kiro 的垂直接线
      被归入 P7-09，该 Task 是 `DEFERRED`。
      关于 `ApiFormat` 的形状，按用户指示参考 kiro-rs 的实现逻辑得出结论：**不新增 ApiFormat
      词表值**。kiro-rs 中 `endpoint`（`ide`/`cli`）是**凭据级字段**，`KiroProvider` 持 endpoint
      注册表按凭据的 `endpoint` 选实现，而对外只暴露标准 Anthropic Messages（`/v1/messages`）；
      Kiro 没有自己的线格式，其特殊性全在上游侧（凭据类型、endpoint host、header、profileArn）。
      cpa-rust 的分层与之一致：`api_format` 是上游线格式，endpoint kind 已由
      `KiroEndpointKind` 表达在凭据/Endpoint 侧。
影响的 Task / Matrix ID / ADR: `BC-PROTOCOL-007` 契约放宽；`CR-P12-ROLLOUT-001` 的切换范围
      部分解禁；P12-06 的 Kiro 取证得以执行。
      (1) ApiFormat 不变：Kiro 复用 `anthropic/messages`，`ApiFormat::ALL` 仍为两个值，
      `api_format` 的存储字符串不变，已发布 Config Version 与 P12-01 至 P12-05 的既有证据
      不受影响、不重写。
      (2) BC-PROTOCOL-007 放宽：把"一个 api_format 恰好绑定一个 adapter"改为"一个 api_format
      可绑定多个 adapter，由 Endpoint 的 `adapter_id` 显式选择其一"。`ApiFormat::adapter_id()`
      （返回唯一 adapter_id 的方法）改为 `ApiFormat::adapter_ids()` 返回该格式的合法适配器
      集合；发布期校验从"`adapter_id == format.adapter_id()`"改为"`adapter_id` ∈
      `format.adapter_ids()`"，词表外一律 `unsupported_endpoint_api_format` fail closed。
      `ApiFormatAdapterRegistry` 的按格式固定槽位改为按 `adapter_id` 索引，重复绑定仍在
      build 期报错。契约不变量 1、3、4、5、6 全部保留：仍是单一字符串表、仍整版 fail closed、
      仍一次性绑定不跨代、仍在任何 Secret/URL/socket 之前做 per-attempt 守卫、诊断仍只印
      presence flag。
      (3) 新增 `kiro.messages` 适配器：`apps/gateway` 新增 `provider-kiro` 依赖，
      `p12_api_format_adapter_registry()` 注册第三个工厂。Kiro 的 endpoint kind（`ide`/`cli`）、
      API Region、profileArn、机器 ID 按 P7-02/P7-03 既有语义从 Endpoint/Credential 配置读取，
      不新增线格式、不改 Canonical。
      (4) `CR-P12-ROLLOUT-001` 部分解禁：该 CR 原文"四个专项切片按代码完成交付、生产路由不
      启用、启用延后至各自外部认证收口后另行 CR"中，**仅 Kiro 切片**由本 CR 解禁为可接入
      P12 运行时并可承载配置图中显式建立的路由。Grok 的三个切片（Build/Official/Web）保持
      原状不动，用户未要求且其外部认证仍缺。
      (5) 明确不解除的边界：P7-09/G7 仍是 `DEFERRED`。本 CR 解禁的是"使用 Kiro 渠道"，
      **不等于** Kiro 的 Provider Gate 通过——后者需要 P7-09 自身的 Kiro-RS 差分、原生
      Adapter 垂直链路与真实 `--bare` E2E。P12-06 由 Kiro 产生的功能验证与性能基线证据
      不得计入 P7-09/G7，也不得据此宣称 Kiro 已完成 Provider Gate。
兼容性与迁移影响: `api_format` 存储值不变，因此无 Config Version 数据迁移。公开 API 与
      Canonical 协议不变。`ApiFormat::adapter_id()` → `adapter_ids()` 是 crate 内公开边界
      变更，需同步 `route_compiler` 的发布期校验与 `apps/gateway` 的装配；
      `docs/crate-boundaries.md` 与 `check-crate-boundaries.rb` 的 EXACT 依赖集合需为
      `apps/gateway` 增加 `provider-kiro` 边。线上 kiro-rs 不被修改。
测试与回滚变化: 新增测试覆盖：同一 `api_format` 下多适配器的注册与解析、Endpoint 显式选择、
      `adapter_id` 不属于该格式时发布期整版拒绝、重复 adapter_id 在 build 期报错、Kiro
      Endpoint 的 ide/cli 分流。既有 `conflict_matrix_returns_stable_codes` 等依赖
      "格式↔唯一适配器"的测试需按新契约改写。回滚为恢复本 CR 的文件 preimage（含
      `apps/gateway` 移除 `provider-kiro` 依赖与契约文档）。
用户批准: APPROVED，2026-07-31（"CR-P12的意思应该是暂时不使用，现在需要使用了；针对
      Apiformat这个表可以参考kiro-rs的实现逻辑即可"；"确认"）
计划版本变更: v1.76
```

### 已批准 Change Request：CR-P12-06-005

```text
CR-ID: CR-P12-06-005
原因: CR-P12-06-004 接线完成后，在本地隔离网关上执行 P12-06 的第一次真实取证时发现：Kiro
      渠道拒绝 **100%** 的请求，错误为 `ClientRequestError`/`invalid_request_error`，且失败
      发生在任何网络调用之前。两侧组件各自都符合契约，是它们的组合有缺陷：
      (1) Anthropic Messages **要求** `max_tokens`（`validate_max_tokens` 强制正整数），而
          `ROOT_FIELDS` 不含该字段，故入站解码器按设计把它保留为根扩展
          `anthropic.messages.max_tokens`——Canonical 核心没有共享的输出上限字段，发明一个
          反而会污染核心。
      (2) Kiro 的 `conversationState` 没有任何输出上限字段：只有 `content`、`modelId`、
          `origin`、`envState`、可选 `tools` 与可选 `outputConfig.effort`。因此
          `BC-PROVIDER-007` 的"Canonical 根扩展一律拒绝"对一个不得静默丢弃客户端语义的
          转换器来说是正确的。
      两条正确的规则相乘，结果是任何合规 Anthropic 客户端都无法使用该渠道。这不是接线遗漏，
      而是 CR-P12-06-004 未预见的组合缺陷，只有真实请求才暴露得出来。
      参考实现口径：kiro-rs 在其 Anthropic 兼容面接受 `max_tokens` 并**不转发**（其上游请求
      体中不存在该字段），因为 Kiro 无处安放它。
影响的 Task / Matrix ID / ADR: P12-06 的 Kiro 执行路径；不改 `BC-PROVIDER-007` 本身、不改
      任何 Provider crate、不改 Canonical、公开 API、Schema、存储或部署包络。
      (1) 在组合根新增 `p12_kiro_request_projection`：仅当请求带根扩展时，丢弃恰好
      `P12_ANTHROPIC_MAX_TOKENS_EXTENSION` 一项，其余扩展全部保留后交给转换器。
      (2) 丢弃的语义边界是刻意选择的：`max_tokens` 是输出**上限**，上游协议无字段可表达；
      丢弃它不会破坏响应正确性——客户端要求"至多 N 个 token"，收到的是一个可能更短或更长的
      完整回答。而任何**其他**扩展仍被保留，因此客户端真正依赖的语义仍在转换器内 fail
      closed，并保留转换器自己的错误分类，不会被本投影静默吞掉。
      (3) 作用域仅 Kiro 一条 arm。OpenAI-compatible arm 继续用既有
      `p12_openai_compatible_request` 把该扩展**翻译**为 `max_output_tokens`（该协议有对应
      字段），Anthropic-compatible arm 继续原样透传由编解码器自行读取。三条 arm 的处理方式
      因上游协议能力不同而不同，这是正确的，不是不一致。
兼容性与迁移影响: 无。不改存储值、已发布 Config Version、公开 API 或凭据。incumbent CPA、
      用户的 CPA/CPAMP 迁移与线上 kiro-rs 均不受影响。
测试与回滚变化: 新增 `p12_kiro_drops_only_the_inexpressible_output_ceiling_and_keeps_every_other_extension`：
      无扩展请求原样透传、带 `max_tokens` 时仅该项被删且其余 canonical 字段不变、外来扩展
      被保留（从而仍由转换器拒绝）。本地隔离验证：修复前错误为 `ClientRequestError`（转换期
      失败），修复后为 `ProviderPermanent`（已成功转换、已出网到
      `runtime.us-east-1.kiro.dev`、被上游按假凭据拒绝）——证明整条路径连通。回滚为恢复
      `apps/gateway/src/runtime.rs` 的本 CR preimage。
用户批准: APPROVED，2026-07-31（沿用 CR-P12-06-003/004 的 P12-06 执行授权；同范围、无新增
      Credential/Provider/公开暴露、不改契约文本的缺陷修复按既有直接批准约定）
计划版本变更: v1.77
```

### 已批准 Change Request：CR-P12-06-006

```text
CR-ID: CR-P12-06-006
原因: CR-005 修复转换层后，请求首次真正到达 Kiro，但被拒。逐项隔离后确认根因是
      **`conversationState.chatTriggerType` 缺失**：其余字节完全相同时，
        不含 chatTriggerType -> 400 ValidationException / reason=REQUEST_BODY_INVALID
        含 chatTriggerType   -> 200 且 content-type: application/vnd.amazon.eventstream
      P7-04 的转换器从未发送该字段，因此每一个 Kiro 请求都被上游判为正文非法。
      诊断过程中的一次错误结论必须如实记录，因为它改变了本 CR 的内容：中途曾以为根因是 CLI 的
      Content-Type（`application/x-amz-json-1.0`），依据是"改用 `application/json` 后 HTTP 变为
      200"。该判断错误——那个 200 的**正文**是
      `com.amazon.coral.service#UnknownOperationException`，即服务根本没识别该操作。只看状态码
      不看正文是错误的验证方法。`application/x-amz-json-1.0` 自始就是正确值：它在 400
      ValidationException 中返回的是 Kiro 自己的服务命名空间
      `com.amazon.kiro.runtimeservice`，证明请求已抵达正确的操作、只是正文不合规。据此已
      `git revert` 该错误提交，`BC-PROVIDER-005`、P7-02 实现、契约测试与 P11-01 差分探针的
      Content-Type 全部恢复原值，`docs/04-channel-reference-analysis.md` §6.2 亦恢复。
      同时修正另一处早前的错误观察：曾测得 `chatTriggerType` "可省"，同样是因为只看了状态码。
影响的 Task / Matrix ID / ADR: P7-04 的 `KiroConversationRequestBuilder`；`BC-PROVIDER-007`
      的 Conversation shape 行。不改 Content-Type、URL、origin、`x-amz-target`、`tokentype`、
      凭据边界、Canonical、公开 API、Schema 或部署包络。
      (1) 每个 conversation 一律发送 `chatTriggerType: "MANUAL"`。`MANUAL` 是网关唯一正确的
      取值：本产品服务的每个请求都源自客户端的显式调用，不存在编辑器自动触发。
      (2) 该字段是常量而非可配置项：它描述的是"请求如何产生"这一事实，而网关对此没有选择权。
测试与回滚变化: 新增 `every_conversation_declares_the_required_manual_chat_trigger`，对 IDE 与
      CLI 两种 kind 单独断言该字段，独立于较大的正文快照，从而在快照被重排时仍能捕获遗漏；
      已用变异验证（删除该 insert 后该测试失败）。P7-04 与 P7-07 的既有正文快照同步。
      回滚为恢复 `conversation_request.rs` 与两份测试的 preimage。
用户批准: APPROVED，2026-08-01（沿用 CR-P12-06-003/004 的 P12-06 执行授权；同范围、无新增
      Credential/Provider/公开暴露的缺陷修复按既有直接批准约定。功能验证使用 kiro-rs 当前
      启用的 KIRO PRO MAX 凭据 id=5，单次调用计费 0.0155 credit）
计划版本变更: v1.78
```

### 已批准 Change Request：CR-P12-06-007

```text
CR-ID: CR-P12-06-007
原因: P12-06 已验证可用的 Kiro Pro Max API-key 凭据额度耗尽。用户明确要求从服务器
      kiro-rs 凭据库中选择其他 Free 凭据替换并开始验证，因此需要在不泄露 Secret、
      不影响线上 kiro-rs、也不扩大到 OAuth 刷新的前提下，为每个替代凭据创建不可变的
      successor Config Version 并做最小功能分类。
影响的 Task / Matrix ID / ADR: P12-06 的 Kiro 功能验证和性能基线；不改变 P7-09/G7 的
      DEFERRED 状态，不改变 OpenAI-compatible differential，不启用 Kiro 生产路由，不改
      incumbent CPA、Caddy、Cloudflare、DNS、公开监听或生产主机名流量。
执行边界:
      (1) 仅测试 kiro-rs 中除已超额账号外的 headless `ksk_` Free API-key 凭据；凭据只在
          服务器内存中进入 root-only 临时图，不输出值、不写入仓库。
      (2) active Config Version 不可变，因此 id=4 与 id=2 分别进入 `p12-06-kiro-v3` 和
          `p12-06-kiro-v4` successor；每个版本只有一个 Candidate、`max_attempts=1`、
          concurrency=1 和独立 version-scoped Client Key。
      (3) 每个候选只发送一次 gateway 功能请求；失败后各允许一次同形直连分类，不保存正文。
          两个早期分类命令因 shell quoting 失败而发送空正文并各得 `400`，明确列为无效诊断；
          修正为先在内存中完整构造并校验 JSON 后，两个候选均由 Kiro 上游返回 `403`。
      (4) 功能成功数为零时不得启动 warm-up 或性能采样，避免从失败渠道制造性能结论。
      (5) 凭据库剩余的 Free 账号是已过期的 IdC OAuth；刷新 token、组合 OAuth 运行时或扩大
          P7-09 范围均不属于本 CR，必须单独批准后才可执行。
结果: 两个替代 headless Free 凭据均不可用；网关对同一失败返回 `502`，与安全边界把未归类
      上游 `403` fail closed 为 `EgressRejected` 一致。Kiro 性能基线未执行，P12-06 保持
      `IN_PROGRESS`；服务器测试域名图停在最后一个可审计 successor，服务仍 loopback-only、
      disabled-at-boot，incumbent 与生产流量未变。
测试与回滚变化: 修复录图 helper 的 successor parent 记录和不可覆盖 `0600` ledger；`--finish`
      现在加载既有 ledger，fresh enter 拒绝复用旧路径。回滚可恢复修复前 helper；服务器 SQLite
      preimage 已保留，测试图不承载生产流量。
用户批准: APPROVED，2026-08-02（“之前说的kiro pro max账号已经超额了无法使用，随便在
      kiro-rs里面取几个其他的free凭证替换吧；开始”）
计划版本变更: v1.79
```

### 已批准 Change Request：CR-P12-06-008

```text
CR-ID: CR-P12-06-008
原因: CR-007 已穷尽 kiro-rs 中可直接使用的替代 headless Free API-key：二者均被 Kiro
      上游 `403` 拒绝，原 Pro Max 已超额，唯一剩余 IdC OAuth 已过期。继续等待或反复测试
      不可用凭据只会阻塞与 Kiro 无依赖的 OpenAI-compatible differential。用户明确决定
      “先不管 kiro”。
影响的 Task / Matrix ID / ADR: P12-06 的完成边界与 Kiro 切片状态。P7-09/G7 继续
      `DEFERRED`；P12-06 的 Kiro 功能/性能切片变为 `DEFERRED_EXTERNAL_CREDENTIAL`，不再是
      P12-06 完成的前置。Kiro 生产路由、P12-08 Canary 范围及 P7 Provider Gate 均不解禁。
执行边界:
      (1) P12-06 继续且只继续 OpenAI-compatible live differential；不读取、刷新或重试任何
          Kiro 凭据，不再为 Kiro 创建 Config Version 或发送请求。
      (2) incumbent CPA 与新网关 Krill 侧不共享同一账号池，且生成本身非确定；因此“逐字段”
          仅适用于跨账号池仍可判定的 canonical 投影：事件种类/顺序约束、Tool call 结构与 JSON
          完整性、Usage 字段存在性/非负性/守恒关系、错误所有权类别和终止语义。
      (3) 随机生成正文、响应 ID、上游模型标签和 Usage 具体数值不进入报告，也不得要求两侧相等；
          否则会把不同采样结果误报为网关回归。性能数据分侧报告并携带样本量，不宣称同账号 A/B。
      (4) 固定合成请求、loopback 数据面、Secret 不落库外明文、incumbent 不改配置以及生产主机名
          不分流的既有边界不变。
恢复条件: Kiro 仅在出现新的可用 `ksk_`，或另行批准并完成 IdC/OAuth 刷新与运行时组合后恢复；
      恢复时须新增 CR 和独立证据，不回写本次 OpenAI differential 的结论。
用户批准: APPROVED，2026-08-02（“可以，那就先不管kiro”）
计划版本变更: v1.80
```

### P12-06 OpenAI-compatible 执行结果

2026-08-02 按 `CR-P12-06-003/008` 的既有边界建立了 root-only successor 图和固定合成差分
harness。新网关 Krill candidate 的单次 loopback 非流式预检为 `2xx`，Canonical message、
`end_turn` 与 Usage 结构均有效。incumbent CPA 的普通 `/v1/models` 是 generic OpenAI inventory，
不能单独证明某模型可走 Responses；停止一个没有持久化精确计数的串行 inventory 诊断后，改用
其内建 Responses/Codex catalog family 与 typed-message 请求轮廓，结果仍为 `5xx`，安全投影仅能
归类为 `server_error/internal_server_error`，不能归因到账号、额度、OAuth、模型或网络。

因此未执行十轮成对流式采样、非流式/Tool 对照和性能结论；否则会把 incumbent 可用性故障误写成
兼容性差分。candidate 图仍为 active successor，但只监听 loopback、服务 disabled-at-boot，生产
主机名没有分流；incumbent 服务仍 active、配置未改，SQLite quick-check 与 root-only preimage
复核通过。详见 [P12-06 OpenAI-compatible live differential](reports/p12-06-openai-differential.md)。
P12-06 转为 `BLOCKED`；只有单独批准修复 incumbent Responses/account-pool 路径，或指定另一条
已工作且不可变的参考臂后才能恢复。P12-08 保持 `PENDING`。计划版本更新为 v1.81。

### 已批准 Change Request：CR-P12-06-009

```text
CR-ID: CR-P12-06-009
原因: P12-06 candidate 已通过固定非流式预检，但 incumbent CPA 的 Responses 参考路径返回
      `server_error/internal_server_error` 5xx，无法形成有效 paired differential。用户批准开始
      修复该 incumbent Responses/account-pool 路径。
影响的 Task / Matrix ID / ADR: 仅 P12-06 的 incumbent OpenAI-compatible 参考臂与其可用性；
      不改变 Canonical、Provider、公开 API、Schema、P12-08 Canary、Kiro 延期状态或新网关图。
执行边界:
      (1) 先只读定位具体失败层，再为 incumbent 配置、认证存储和 service 定义建立服务器本地、
          root-only、带完整性校验的 preimage；不得把 Secret 或配置正文写入仓库。
      (2) 只允许最小 account-pool/Responses 配置修复。不得改变 incumbent Client Key、公开
          Caddy/DNS、生产主机名分流、端口、TLS 或其它站点；不得升级 incumbent 二进制。
      (3) 每次验证使用固定合成、无副作用 Responses 请求；不保存请求/响应正文、模型、endpoint、
          OAuth 或 token 值。先通过单次非流式结构预检，才可复用 CR-003/008 的有界 harness
          执行 paired stream/non-stream/Tool/性能 corpus。
      (4) 修复或预检失败、服务不健康、公开监听变化、现有非 Responses 健康检查退化时，立即恢复
          preimage 并保持 P12-06 BLOCKED；不得启动 P12-08。
兼容性与迁移影响: incumbent 可能发生一次受控配置 reload/restart，但公开入口、Client Key 与其余
      路由保持不变；成功结论仅表示 P12-06 参考臂可用于差分，不表示生产切换。
测试与回滚变化: 修复前后核对 incumbent service/监听、固定 Responses 结构、普通 models/health
      连续性与新网关隔离状态；回滚恢复配置、认证存储和 service preimage 后复核哈希与服务状态。
用户批准: APPROVED，2026-08-02（“可以 开始吧”）
计划版本变更: v1.82
```

### 已批准 Change Request：CR-P12-06-010

```text
CR-ID: CR-P12-06-010
原因: CR-P12-06-009 的只读排查确认 incumbent 现有 Grok/xAI OAuth 参考池不可用：
      已启用账号的 access token 均已过期，受控 refresh 样本被上游以
      `invalid_grant` 拒绝，本机官方 CLI 的交互登录也未落盘可用凭据。用户
      判断 Free 账号可能已不可用，明确要求先跳过 Grok 路线。
影响的 Task / Matrix ID / ADR: P12-06 的 Grok/xAI-backed incumbent 参考臂与
      OpenAI-compatible paired corpus。不改变已通过的 candidate 预检证据，不将未运行的
      differential 冒充为通过。
执行边界:
      (1) 停止 Grok/xAI OAuth 登录、refresh、账号轮询和真实探针；不再扫描其余
          凭据，不导入替代 Free 账号。
      (2) CR-009 已建立的 root-only 服务器 preimage 保留作为审计/回滚证据；因未
          写入 incumbent 配置或认证存储，无需执行数据回滚。
      (3) P12-06 转为 `DEFERRED`，Kiro/Grok 生产路由继续禁用。P12-08 依赖不自动
          解禁；须另行 CR 指定已工作且不可变的非 Grok 参考臂，或明确修改
          P12-06→P12-08 的发布门禁。
兼容性与迁移影响: 无代码、公开 API、Canonical、Schema、Caddy/DNS、Client Key、
      新网关图或生产流量变更。
测试与回滚变化: 本 CR 仅收口证据与任务状态；保留 candidate 预检、incumbent
      失败分类和 root-only backup。回滚为在新的有效参考凭据/参考臂到位后，以
      新 CR 恢复 P12-06，不重写当前失败历史。
用户批准: APPROVED，2026-08-02（“可能是现在free账号都不行了，先跳过grok这条线吧”）
计划版本变更: v1.83
```

### 已批准 Change Request：CR-P12-06-011

```text
CR-ID: CR-P12-06-011
原因: Grok/xAI 参考臂按 CR-010 延期后，用户明确选择当前 CC Switch 中已可用的
      Krill Codex 渠道作为 P12-06 的非 Grok 参考臂。incumbent CLIProxyAPI v7.2.101
      支持独立 `openai-compatibility` Provider，可以用唯一前缀避免旧 xAI 账号池参与选择。
影响的 Task / Matrix ID / ADR: 仅 P12-06 的 incumbent 参考臂、固定预检与 paired
      stream/non-stream/Tool/性能 corpus；不解禁 Grok/Kiro 专项切片或生产路由。
执行边界:
      (1) 复用 CR-009 的 root-only、完整性校验 preimage，并在写入前再建立当前
          incumbent config 的独立哈希备份。凭据仅从本机 CC Switch 当前 Krill Codex
          Provider 内存提取，通过 stdin 传入服务器 root-only 临时交易；不输出、不写入仓库。
      (2) incumbent 仅新增一个唯一 name/prefix、单 API key、单 model mapping 的
          `openai-compatibility` 条目；不改 incumbent Client Key、端口、其他 Provider、二进制、
          systemd unit、公开 Caddy/DNS 或新网关 candidate 图。
      (3) reload/restart 后先复核 incumbent active、唯一 loopback listener、models/health 连续性和
          新网关隔离状态；只允许前缀模型的固定无副作用非流式 Responses 预检。
      (4) 仅当两臂预检均为 2xx 且 Canonical/Usage 结构有效，才运行 CR-003/008 的
          有界 paired harness。任何失败立即恢复 incumbent config preimage 并重启复核；
          不得开始 P12-08。
兼容性与迁移影响: incumbent 可发生一次受控 reload/restart，但新 Provider 只能由测试
      前缀模型命中；生产主机名流量、Client Key 和原有模型选择不变。
测试与回滚变化: 预检后的 paired harness 仍只保存无值结构、终止语义、Usage
      不变量和延迟分布；不保存 endpoint、key、model、请求/响应正文或 token 指纹。
      回滚为原子恢复 config preimage、restart、哈希/监听/连续性复核。
用户批准: APPROVED，2026-08-02（“使用krill的吧”）
计划版本变更: v1.84
```

### 已批准 Change Request：CR-P12-06-012

```text
CR-ID: CR-P12-06-012
原因: CR-011 的两臂非流式预检均通过，但 paired harness 中 incumbent 的 11 次
      普通 SSE 和 1 次 Tool SSE 均为 `stream did not complete`；candidate 对应项全通过。
      当前无法区分是 CLIProxyAPI v7.2.101 的 Chat→Responses stream translator 兼容缺口，
      还是 differential harness 遗漏了合法的 Responses 事件轮廓。
影响的 Task / Matrix ID / ADR: 仅 P12-06 Krill incumbent 参考臂的一次 SSE 类别诊断。
执行边界:
      (1) 以 CR-011 相同的本机内存凭据与独立 Provider 形状重建参考臂；写入前
          重用 root-only preimage，请求结束或异常时无条件恢复。
      (2) 只发送 1 次固定、无副作用、普通文本 SSE Responses 请求；不发 Tool、不重跑
          warmup/性能 corpus、不重试相同 tuple。
      (3) 持久证据只能包含 HTTP 类别、Content-Type、SSE data 行数、去重的 `type`、
          `object`、是否存在 choices/delta 与 `[DONE]`；不保存 delta、正文、ID、Usage 值、
          endpoint、key、model 或 token 指纹。
      (4) 诊断后先回滚并复核 incumbent/candidate 服务与 loopback 监听，再决定是修正
          harness、更换参考转换路径，还是保持 P12-06 未通过。
兼容性与迁移影响: 无持久配置、代码、公开 API、Caddy/DNS、Client Key、新网关图
      或生产流量变更。
测试与回滚变化: 新增一份 root-only 无值诊断 receipt；回滚与 CR-011 相同，并须在诊断
      命令的 EXIT/INT/TERM trap 中执行。
用户批准: APPROVED，2026-08-02（承接“使用krill的吧”的同一 P12-06 参考臂目标；
      不新增 Provider、Credential 或公开边界）
计划版本变更: v1.85
```

### 已批准 Change Request：CR-P12-06-013

```text
CR-ID: CR-P12-06-013
原因: CR-012 的唯一诊断请求返回 2xx `text/event-stream`，且含
      `response.created`/`in_progress`/`output_item.added`/`content_part.added`/`output_text.delta`，
      但流结束时没有 `output_item.done`、`response.completed` 或 `[DONE]`。这排除了 harness
      漏识别合法终止事件，定位为 CLIProxyAPI v7.2.101 通用 OpenAI Chat→Responses
      stream translator 与当前 Krill Chat SSE 的兼容缺口。同一 incumbent 支持原生
      `codex-api-key` Responses 执行器，可避开该 Chat 转换。
影响的 Task / Matrix ID / ADR: 仅 P12-06 Krill incumbent 参考臂的 executor 选择、三项预检
      与条件性 paired corpus；不改新网关 candidate、Grok/Kiro 切片或 P12-08。
执行边界:
      (1) 凭据、endpoint、model、前缀、单凭据/单模型隔离和 root-only 传输与 CR-011
          相同；唯一变化是 incumbent config 顶层条目由 `openai-compatibility` 改为
          原生 `codex-api-key`，并保留已验证的非机密 User-Agent。
      (2) restart 后须先通过 active/loopback/models/candidate-isolation 连续性，再对 incumbent
          各发 1 次非流式、普通 SSE 和 Tool SSE。三项均须满足 Canonical 投影、
          终止语义、完整 Tool arguments 与 Usage 守恒。
      (3) 仅当三项 incumbent 预检全过，才对两臂重跑现有有界 paired harness；
          不改它的 samples/warmup/超时/正文禁止规则。
      (4) config 写入、预检、harness 和证据收集全部置于 EXIT/INT/TERM 回滚 trap 内；
          无论成功或失败都恢复 incumbent preimage，restart 并复核唯一 loopback listener。
兼容性与迁移影响: 无持久服务器配置、代码、公开 API、Caddy/DNS、Client Key、
      新网关图或生产流量变更。
测试与回滚变化: 若预检通过，新建一份 root-only、value-free paired receipt；若失败，
      只保留无值失败分类。回滚为原子恢复 config/model-selector preimage 和服务/监听复核。
用户批准: APPROVED，2026-08-02（承接“使用krill的吧”的同一参考臂目标；仅替换
      incumbent 内部 executor，不新增凭据或外部边界）
计划版本变更: v1.86
```

### 已批准 Change Request：CR-P12-06-014

```text
CR-ID: CR-P12-06-014
原因: CR-013 的完整 receipt 证明两臂均为 10/10 SSE、0 失败，非流式、Tool、
      Canonical 生命周期、终止语义、Usage 存在性/非负性/守恒与性能指标均通过。
      唯一差异是 incumbent 的 `input_tokens_details.cached_tokens` 为整数而 candidate 省略该可选
      明细。现有 harness 却将整个 Usage “可选字段存在形状”要求两臂完全相等，
      这超出 `CR-P12-06-008` 批准的可判定不变量，造成 false negative。
影响的 Task / Matrix ID / ADR: P12-06 differential classifier 与无网络回归；不改请求、
      解码、Canonical、Usage 取值、性能采集或服务器图。
执行边界:
      (1) 保留每臂的可选 cache/reasoning 明细存在性供观测，但跨臂 PASS 只要求每臂
          Usage 存在、核心 token 为非负整数、`total=input+output`；任一臂违反仍 fail closed。
      (2) 将 differential 评估提取为纯函数，补“可选 cached_tokens 存在性不同仍通过”
          和“守恒破坏仍失败”的无网络回归；receipt schema 升为 2。
      (3) 只对 CR-013 已有的 root-only、value-free schema-1 receipt 离线重算新判词，
          产生独立 review receipt；不改写原 receipt，不发任何新网络请求。
兼容性与迁移影响: 无公开 API、Canonical、Provider、Schema、Caddy/DNS、Client Key、
      服务器配置或生产流量变更。
测试与回滚变化: 增加 differential 纯评估回归，运行定向测试、docs/fast Gate、
      tracked Secret scan 与 whitespace review。回滚为恢复旧比较器，但那会恢复已证明的
      false negative，不得改写 CR-013 原始 receipt。
用户批准: APPROVED，2026-08-02（承接“使用krill的吧”的 P12-06 完成目标；
      纠正已批准判据的实现偏差，不扩张网络或生产边界）
计划版本变更: v1.87
```

### P12-06 Krill 参考臂最终结果

`CR-P12-06-011` 的通用 OpenAI-compatible Chat 执行器非流式通过，但流式转换遗失
终止事件；`CR-P12-06-012` 用一次无值诊断确认该失败，并完整回滚。
`CR-P12-06-013` 改用 incumbent v7.2.101 原生 `codex-api-key` Responses 执行器后，非流式、
普通 SSE 和 Tool SSE 三项预检均通过；成对 corpus 的两臂均完成 10/10 SSE、0 失败，
Canonical 投影、终止语义、完整 Tool arguments 与 Usage 存在/非负/守恒均通过。

原 schema-1 receipt 仅因 incumbent 保留合法 `cached_tokens` 可选明细而 candidate 省略它，
被旧比较器误报为三个 Usage shape 不等。`CR-P12-06-014` 将比较收窄到
`CR-P12-06-008` 已批准的核心不变量，新增正/负回归；对原 receipt 的无网络离线
重算为 9/9 PASS，原 receipt 未改写。新网关在 10 个交错样本中的平均 TTFB、首语义事件和
总时长分别比 incumbent 低约 41%、3% 和 7%，输出速率接近；样本较小，只作基线。

incumbent 的临时 Provider 与 model selector 已从 root-only preimage 原子恢复；incumbent 和
candidate 均 active，分别仅有原定 1 个与 2 个 loopback listener。Caddy/DNS、Client Key、
新网关图和生产流量未改。P12-06 转为 `LOCAL_PASS_PENDING_PHASE_GATE`；P12-08 仍 `PENDING`。
计划版本更新为 v1.88。

### 已批准 Change Request：CR-P12-07-001

```text
CR-ID: CR-P12-07-001
原因: CR-P12-ROLLOUT-001 把 Canary 百分比分流定在服务器本地 Caddy，并规定"进入服务器前
      须在 Staging 复核语法"、"P12-09 演练须实测 caddy reload 的生效时延并记为 RTO 证据"、
      "须确认读/空闲超时高于数据面 15 秒 SSE keepalive 间隔"。但仓库里没有任何 Caddyfile
      模板、没有回滚 preimage、也没有 RTO 测量脚本，这三条要求都无可执行载体。现场核对
      服务器现行 /etc/caddy/Caddyfile 后确认：现有站点块完全没有设置任何超时，而 Caddy 的
      服务器级超时默认为"无超时"，因此当前默认值恰好不会切断 SSE，但一旦有人按常规运维
      直觉加上 read/idle 超时就会静默切流，且没有任何检查会发现。
影响的 Task / Matrix ID / ADR: P12-07 的分流层配置产出、P12-09 的 RTO 证据定义。
      不改代码、不改公开 API、Canonical、Provider 或 Schema，不改服务器现行配置。
      (1) 新增 deploy/caddy/canary.Caddyfile：生产主机名不变，按非机密固定字面前缀
      `rgw_` 在 Authorization 与 X-Api-Key 两个头上分流到 127.0.0.1:18180，其余流量继续
      到 incumbent CPA 127.0.0.1:8317；不含任何 key 值；刻意不含到 18181 的任何路由；
      刻意不加 encode（压缩会在事件流前重新引入缓冲）；**刻意不含全局 `servers` 超时块**。
      Caddy 的 `servers` 按监听地址生效而非按站点，服务器五个站点共用同一 `:443` 监听器，
      加全局块会把超时施加到 cpam/grok/kiro/sub（已用 `caddy adapt` 对真实 live 配置实测
      确认）；要按站点隔离只能换端口，会改变公开面。省略是安全的：现行配置编译为
      `timeouts: NONE`，而"无超时"对长连接 SSE 正是所需，也天然满足 CR-P12-ROLLOUT-001 的
      ">15s" 要求。风险方向与运维直觉相反——危险是有人**加**超时而非缺超时。
      (2) 新增 deploy/caddy/rollback.Caddyfile：把同一主机名全量指回 incumbent，作为
      P12-09 的回滚 preimage；回滚是一次文件交换加一次 reload，不动 DNS/TLS/其它站点。
      (3) 新增 scripts/check-p12-caddy-split.rb：拒绝全局 `servers` 块；并在真有超时被
      引入时，从 Rust 源码读取 SSE_KEEPALIVE_INTERVAL、INFERENCE_REQUEST_BODY_TIMEOUT、
      P12_STREAMING_TOTAL_TIMEOUT、P12_STREAMING_PROGRESS_TIMEOUT 与之逐项比对，两侧任一
      漂移即 fail closed。同时断言管理面未暴露、无压缩、前缀匹配未退化、配置内不含 key 值、回滚确实
      移除网关且主机名一致。接入 check.sh fast 与 docs。
      (4) 新增 scripts/p12-09-measure-caddy-rto.sh：先 caddy validate 再交换并 reload，
      随后轮询探针直到实际观测到的后端改变，分别记录 reload 返回耗时与生效耗时。因为
      `caddy reload` 返回零只表示配置被接受，不表示下一个请求已走新路由，所以 RTO 必须
      按观测到的路由变化计时。脚本不打印 key、头值或响应正文，可选 0600 无值回执。
      (5) 新增 scripts/test-p12-caddy-split.sh：对 11 条回归路径逐一实测拒绝。
兼容性与迁移影响: 无代码、公开 API、Canonical、Schema 或安全迁移。本 CR 只新增仓库内的
      模板与检查脚本，不安装、不 reload、不改服务器现行 Caddy 配置；服务器仍只运行既有
      站点。用户正在进行的 CPA/CPAMP 迁移不受影响。分流启用仍属 P12-07/P12-08 的独立授权。
测试与回滚变化: 分流 fragment 已就地合并进服务器真实 live 配置的副本并用 Caddy v2.11.4
      `caddy validate` 通过；`caddy adapt` 核对编译结果为 cpa 站点两条 rgw_ 匹配指向 18180、
      缺省指向 8317，其余四个站点上游与 `timeouts: NONE` 均未改变。核对全程只读，随后删除临时
      目录，现行配置与 caddy 服务状态未变。回滚为删除本 CR 新增的四个文件与 check.sh 的两处接线。
用户批准: APPROVED，2026-07-30（"开始A5"）
计划版本变更: v1.74
```

### 已批准 Change Request：CR-P12-08-001

```text
CR-ID: CR-P12-08-001
原因: P12-08/P12-09/G12 的推进与回滚判据当前不可执行，有三个独立缺陷。
      (1) 样本量与触发阈值不匹配：§18 要求每阶段"至少 100 个成功请求"，而回滚触发是
      "相对旧网关新增错误率超过 1%"。实测该组合无意义：n=100 且零错误时，95% 单侧上界
      仍不能排除 2.95% 的真实错误率；在 0.5% 基线上以 alpha=0.05/power=0.80 检出 +1pp
      需每臂 1222 个请求；n=100 实际只能检出 >=6.64pp 的劣化，比触发阈值大六倍以上。
      即单次偶发失败即触发回滚，而真实的 1pp 劣化检不出来。
      (2) G12 的核心条件"无 P0/P1 故障"在全计划无任何定义，72h 观察结束时"算不算 P1"
      将变成事后争论。
      (3) 判据引用了不存在的信号。Prometheus 暴露面只有 7 个指标
      （attempts_total、events_total、usage_tokens_total、exports_total、
      queue_admission_total、durable_events_total、durable_pending_required），
      全部为计数器，零 histogram。因此 §18 要求逐阶段检查的 TTFT 与 P95/P99 在运行时
      不可观测；attempts_total 只有 succeeded/failed 两个标签值，没有 HTTP 状态码分布，
      也没有错误分类维度；Tool 与 Route 分布没有任何指标。ManagementRequestAttempt 的
      文档明确写着"without model, route, timing"。
影响的 Task / Matrix ID / ADR: §18 的 Canary 推进与回滚规则、G12 门禁；P12-08 与 P12-09
      的证据定义。不改代码、不改公开 API、Canonical、Provider 或 Schema。
      (1) 样本量：每阶段最低成功请求数由 100 改为 1250，并同时要求该阶段与对照的旧网关
      在同一时间窗内各自达到该量级；合成补足请求必须与真实流量分开计数，且其失败同样
      计入分子（否则合成请求会稀释错误率）。低于最低量时该阶段不得推进，只能延长窗口。
      50% 阶段最短观察时长由 2h 提升为 24h，用以补回 CR-P11-04-001 把本地 Soak 从 24h
      降到 10h 所让掉的长时覆盖。10%/25%/100% 阶段仍为 2h 起，且都受 1250 门槛约束。
      (2) 严重度：新增四级封闭分类（P0/P1/P2/P3），P0/P1 逐条列举且与 §18 既有回滚触发
      条件一一对应，使"无 P0/P1 故障"成为可判定谓词而非主观判断。
      (3) 可观测性缺口：明确把 TTFT 与实时 P95/P99 标为当前不可观测，并规定 P12-08 的
      延迟证据改由客户端侧采集（分流层前的合成探针记录端到端时长与首字节时长），
      Attempt 级时长由事件日志 started_at_ms/ended_at_ms 离线导出统计。错误率分子按
      Attempt 的 GatewayError 分类与客户端观测状态码共同判定，不假称存在按状态码的
      服务端指标。Tool 与 Route 分布按 Runbook 台账逐样本核对而非按指标。这些缺口写入
      Runbook §6 已知缺口表，不在本 CR 内以新增指标解决；是否补 histogram 由 P13 另议。
兼容性与迁移影响: 无代码、公开 API、Canonical、Schema、部署或安全迁移。既有 P12-01 至
      P12-05 的验收证据与 P7/P8 外部认证延期边界不变。CR-P12-ROLLOUT-001 界定的分流机制
      （服务器本地 Caddy 加权/按 Key）与切换范围不变。
测试与回滚变化: 本 CR 为判据定义，不新增自动化测试。样本量结论由
      scripts/check-p12-08-canary-thresholds.rb 以可执行形式固定：它重算零错误上界与
      双比例样本量，断言计划记载的 1250 门槛足以在 0.5% 基线上检出 1pp，并断言 100
      不足；同时断言计划与 Runbook 中的严重度分级与阶段时长未被静默改动。该脚本接入
      check.sh 的 fast 与 docs 模式。回滚为恢复本 CR 的文档 preimage 与删除该脚本。
用户批准: APPROVED，2026-07-30（"先做A4/A3"）
计划版本变更: v1.73
```

### 已批准 Change Request：CR-P12-08-002

```text
CR-ID: CR-P12-08-002
原因: 用户批准开始 P12-08。P12-06 Krill differential 与 P12-07 暴露前验证均已完成本地
      证据，但生产 Canary 仍必须经过独立的无值准入检查，避免把 loopback Staging 图、测试域名
      或未配置的 Client Key 误当作生产切片。
影响的 Task / Matrix ID / ADR: P12-08 准入检查及首个 10% Canary 阶段；不改 P12-06/P12-07
      既有证据，不解禁 Grok/Kiro，不改变 Cloudflare/DNS 或 incumbent 非 rgw_ 流量。
范围: 只读核对当前 active Config Version、Endpoint/Credential/Route/Access Group 图、独立
      rgw_ Client Key、Caddy 生产 preimage、回滚片段、管理/数据面监听和客户端侧观测链路；准入
      通过后才允许在生产主机名建立双接受窗口并切入 10%。本 CR 不自动推进 25%/50%/100%。
安全边界: 不输出或持久化 Secret、endpoint、模型、正文或 token 指纹；管理面保持 loopback；
      任何新增配置先备份并可单步回滚；P0/P1 立即回滚。10% 阶段仍须同窗每臂至少 1250 个
      成功请求、观察至少 2h，延迟由客户端侧取证。
测试与回滚变化: 先执行只读 readiness receipt；缺少任一条件即停止且不 reload Caddy。若进入
      10%，先保存 Caddy/CPA preimage，验证合法 rgw_ key 的新旧双接受，再以 rollback fragment
      和一次 caddy reload 恢复 incumbent；新网关配置发布后按 Runbook 重启。
用户批准: APPROVED，2026-08-02（“开始吧”）
计划版本变更: v1.89
```

### 已批准 Change Request：CR-P12-ROLLOUT-002

```text
CR-ID: CR-P12-ROLLOUT-002
原因: 用户澄清 CPAR 的目标是完成后全量替代 CPA；最终服务器只保留一个 CPAR，旧 CPA 关闭。
      先前按 `rgw_` Client Key 执行 10%→25%→50%→100% 分流、并要求 CPA/CPAR 双接受同一
      key，属于错误的部署拓扑。
覆盖关系: 本 CR 覆盖 CR-P12-ROLLOUT-001 的百分比分流机制、CR-P12-08-001 的多阶段样本门槛
      以及 CR-P12-08-002 的 CPA 双接受前置。既有 P12-06 differential、P12-07 独立域名暴露、
      P0/P1 分级、客户端侧延迟证据和 72h 观察要求继续有效。
目标拓扑: P12-08 完成 CPAR 生产图、独立 Client Key、客户端迁移/回退清单和 Caddy preimage；
      P12-09 将生产主机名一次性全量切到 CPAR，实际全量回滚至 CPA并再次恢复，用于证明 RTO；
      P12-10 在 CPAR 全量运行 72h 且 G12 通过后停止并禁用旧 CPA service/container。任何时刻
      不按 Key、百分比或请求做双网关生产分流。
客户端 Key: `rgw_` 只属于 CPAR。旧 CPA 无需接受它；刚才 CPA 对该 key 的 401 不再是阻塞。
      因 CPAR 当前不能导入旧 CPA key，P12-08 必须在切换前形成逐客户端的新 key 交付与回退
      清单。若要求客户端无感回滚，则须另行实现并审查受控 legacy-key import，不能把 CPA
      双接受当作替代品。
验证与回滚: 切换前冻结并记录旧 CPA 成功率/延迟基线，完成 CPAR 的 models、非流式、SSE、Tool、
      Usage、Explain 与观测预检。切换后全部生产请求走 CPAR，至少观察 72h 且成功请求不少于
      1250；P95 相对冻结基线持续恶化 >20%、新增错误率 >1% 或任一 P0/P1 立即全量回滚。
      回滚为恢复 Caddy preimage并按清单把客户端认证恢复为旧 CPA key；不依赖 CPA 接受 rgw_。
用户批准: APPROVED，2026-08-02（“不是分流而是替代；最终只保留一个 CPAR，原本 CPA 会关闭”）
计划版本变更: v1.91
```

### 已批准 Change Request：CR-P12-COMPAT-001

```text
CR-ID: CR-P12-COMPAT-001
原因: 用户明确 CPAR 是完整反向代理而非仅 Responses 网关：必须具备 Kiro、Grok、Codex、Claude
      等渠道，并向客户端兼容 Chat Completions、Responses、Messages 三种常见协议。当前 Release 1
      只有 Responses/Messages 入站，ApiFormat 只有两个值，P12 runtime 未接入 provider-grok，
      且活动服务器图仍是单渠道 differential 图，无法替代 CPA。
影响的 Task / Matrix ID / ADR: 将原 P13-01/P13-02 前移到 P12-08A-D；P12-08E-G 补齐四类渠道
      runtime、生产图、能力矩阵和真实 E2E。更新 Release 1 范围、BL-02、A09/L07、目标架构、
      BC-PROTOCOL/BC-ROUTER/BC-HTTP 及 G12。P12-09/P12-10 在这些切片完成前保持 PENDING。
兼容边界: “兼容三协议”不等于静默伪造语义。Text、ordered history、Tool、Reasoning、Usage、
      stop reason 与流终止只有在源协议→Canonical→目标协议可证明表达时才准入；目标协议或渠道
      无法表达的请求在出网前返回稳定 ClientRequestError。原生渠道能力可以大于公共交集，但
      Route Explain 必须说明该候选为何可选/被拒。
渠道边界: Kiro 使用 `kiro.messages`；Claude 使用 `anthropic-compatible.messages`；Codex 使用
      OpenAI-compatible Responses/Chat；Grok 的 Build/Official/Web 使用其已实现的专用边界并接入
      P12 runtime。Credential、Quota、Health、Circuit、Catalog 与连续性按 Endpoint+Credential
      隔离，任何渠道失败不得污染其它渠道。
测试与回滚变化: 新增 Chat Codec 属性/Chunk 测试、Actix HTTP E2E、第三 ApiFormat 发布期拒绝、
      三协议转换矩阵、每渠道 vertical slice 和协议×渠道能力矩阵。真实 E2E 仍使用受控无值回执；
      缺少账号时只能阻塞对应渠道的 live Gate，不能把未测试渠道写成完成。回滚为撤销本 CR 的
      新协议/注册表/runtime 边并恢复 P12-08 阻塞，不影响已完成 P0-P12-07 证据。
用户批准: APPROVED，2026-08-02（“CPAR作为一个反代项目，需要有kiro，grok，codex，Claude等
      渠道，且反代出来的需要兼容chat，response和message这三种常见协议”）
计划版本变更: v1.93
```

### 已批准 Change Request：CR-P12-PORT-001

```text
CR-ID: CR-P12-PORT-001
原因: 用户明确整个 CPAR 项目的源码与实现逻辑均可参考旧 CPA，目标是用 Rust 新框架移植已验证
      行为，而不是对旧 CPA 已有能力重新设计。现有 P12-08D-G 粒度过粗，没有要求冻结参考文件、
      测试意图、行为差异和安全偏差，容易导致重复研究、超大提交和兼容性遗漏。
影响的 Task / Matrix ID / ADR: 新增 §0.1；把 P12-08D-G 细化为 D0-D4、E1-E4、F1-F3、G1-G2；
      不改变 Chat/Responses/Messages、Kiro/Grok/Codex/Claude 的既定 Release 1 范围，不改已完成
      P0-P12-08C 的实现或证据，也不开始 P12-09/生产切换。
兼容性与迁移影响: CLIProxyAPI v7.2.101 的 handler/translator/executor/auth/registry 源码与测试
      成为首要移植参考；CPA v7.2.80 生产行为继续作为实际基线。原生同协议优先保真透传，跨协议
      只接纳可证明的 Canonical 无损映射。旧 CPA 的已知缺陷、无界输入、Secret 暴露、隐式 fallback、
      热路径可变配置或与 CPAR 已批准架构冲突的实现不得复制，须记录为 intentional hardening。
测试与回滚变化: 每个移植 Slice 先产出 Legacy Behavior Manifest，再端口对应 Rust 边界，并以
      脱敏 fixture/property/differential 测试将差异归类为 PARITY、INTENTIONAL_HARDENING 或
      UNSUPPORTED_FAIL_CLOSED。Kiro/Grok Official 缺账号只延期对应 live receipt；本地实现、
      loopback E2E 和默认禁用组成继续。P12-09 只能切换已通过本地与真实准入且实际进入生产图的
      渠道。回滚为恢复 v1.96 的粗粒度 D-G 编排，不回滚已经通过的安全边界或功能实现。
用户批准: APPROVED，2026-08-02（“整个项目的源码及实现逻辑都可以参考旧的CPA的源码，只是用
      新框架的开发语言实现而已”）
计划版本变更: v1.97
```

### 全量替代与回滚规则

- 不执行 10%→25%→50%→100% 百分比或按 Key 分流。P12-09 只有两个生产路由状态：生产主机名
  全量到旧 CPA，或全量到 CPAR。
- 切换前冻结旧 CPA 的成功率、TTFT、P95/P99、模型与语义基线，完成 CPAR 的独立数据面、认证、
  Tool、Usage、Explain、Credential 与观测预检；并为每个实际客户端登记新 `rgw_` key 的交付
  状态及旧 key 回退动作。任何客户端未准备好时不得切换。
- 切换后 CPAR 全量观察至少 72h，成功请求不少于 1250。低流量时可用固定合成请求补足，但合成
  请求须与真实流量分开计数，失败同样计入错误率分子。TTFT 与 P95/P99 当前服务端不可观测，
  须由客户端侧证据提供；旧 CPA 的冻结基线只用于对比，不承载并行生产请求。
- P12-09 必须实际执行一次全量回滚并再次恢复，记录 Caddy 路由生效 RTO、客户端认证回退时间和
  请求一致性。P12-10/G12 通过后停止并禁用旧 CPA，不保留双网关生产拓扑。
- 任一条件触发立即回滚：
  - 相对切换前冻结基线新增错误率超过 1%（按下述分子/分母定义）。
  - P95 延迟持续增加超过 20%。
  - 出现 Tool/Reasoning/Usage 语义回归。
  - 出现 Secret 泄漏、数据库损坏、流重复或错误跨账号连续性。
  - 无法用 Route Explain 解释实际选路。

#### 判据信号的实际来源

判据不得引用不存在的指标。当前 Prometheus 暴露面只有 7 个计数器
（`attempts_total`、`events_total`、`usage_tokens_total`、`exports_total`、
`queue_admission_total`、`durable_events_total`、`durable_pending_required`），没有任何
histogram；`attempts_total` 仅有 `succeeded`/`failed` 两个标签值，不含 HTTP 状态码或错误
分类维度；`ManagementRequestAttempt` 按设计不含 model、route 与 timing。

| 判据 | 实际来源 | 状态 |
|---|---|---|
| 错误率分子 | 客户端观测状态码 + Attempt 的 `GatewayError` 分类（事件日志载荷） | 可得，非单一指标 |
| 错误率分母 | CPAR 全量观察窗口的真实请求数（合成请求单独计数并同样计入分子） | 可得 |
| TTFT | 客户端侧合成探针记录首字节时长 | 服务端不可观测 |
| P95/P99 | 同上；Attempt 级时长由事件日志 `started_at_ms`/`ended_at_ms` 离线导出统计 | 服务端无实时 histogram |
| 缓存 | `usage_tokens_total{kind=cache_read}` 与 `{kind=cache_creation}` | 可得（累计） |
| Usage | `usage_tokens_total{kind=...}` | 可得（累计） |
| Credential 状态 | `GET /admin/runtime/availability` | 可得 |
| Tool / Route 分布 | 无指标；按 Runbook 台账逐样本核对 | 需人工证据 |

是否补充延迟 histogram 与按状态码/错误类别的指标维度，由 P13 另行 CR 决定；Release 1 内
按上表的客户端侧与离线路径取证，不得假称存在服务端实时分位数。

### 故障严重度分级

G12 的"无 P0/P1 故障"需要可判定的谓词。下表为封闭分类，P0/P1 与 §18 的回滚触发条件
一一对应；未列入 P0/P1 的即为 P2/P3，不阻塞 G12 但须记录。

| 级别 | 定义 | 判定信号 | 对 G12 的影响 |
|---|---|---|---|
| P0 | 数据或隔离边界破坏：Secret 泄漏、数据库损坏、流内容跨请求重复、错误的跨账号连续性、凭据被错误账号使用 | `PRAGMA quick_check` 非 `ok`；secret 扫描命中；同一流内容出现在不同 `request_id`；Attempt 的 `credential_id` 与其 Endpoint 绑定不符 | 立即回滚；G12 不通过 |
| P1 | 全量或大范围服务降级、语义回归、可观测性失效：相对切换前冻结基线新增错误率 >1%、P95 持续劣化 >20%、Tool/Reasoning/Usage 语义回归、无法用 Route Explain 解释实际选路、必需事件被隔离或写失败（`durable_events_total{outcome=required_quarantined}` 或 `{outcome=write_failed}` 增长）、必需队列满或 sink 关闭（`queue_admission_total{outcome=required_queue_full}` 或 `{outcome=sink_closed}` 增长） | 上表判据来源 | 立即回滚；G12 不通过 |
| P2 | 单凭据或单 Endpoint 范围的故障，且自动降级/重试已吸收，客户端未观测到错误 | `availability` 出现 `RecoveryRequired`，同时该阶段错误率未越线 | 不阻塞 G12；须在报告中列明与根因 |
| P3 | 仅诊断层面的问题：低优先级诊断事件丢弃（BL-10 允许）、日志噪声、非必需导出被拒 | `exports_total{outcome=rejected}`、诊断队列丢弃计数 | 不阻塞 G12；记录即可 |

### G12 门禁

- CPAR 承载全部实际生产流量运行 72h，无 P0/P1 故障（按上表分级判定）；不得存在百分比或按 Key 分流。
- 该 72h 窗口内成功请求数不少于 1250，且延迟证据按上表的客户端侧路径采集。
- 现有生产路径仍保留固定版本回滚包。
- G12 通过后停止并禁用旧 CPA service/container，服务器生产入口只保留 CPAR。
- 备份、恢复、升级、降级、Secret 轮换和故障排查手册齐全。
- Release 1 完成后才允许为 P13 创建新计划版本。

## 19. P13 - Release 1.1 候选范围

本阶段其余候选仍为 `DEFERRED`，不能在 Release 1 开发中顺手实现。P13-01/P13-02 已由用户通过
`CR-P12-COMPAT-001` 明确前移到 P12-08，不再属于 Release 1.1 候选；其余项须在 G12 后另行选择：

| ID | 候选功能 | 当前状态 |
|---|---|---|
| P13-01 | OpenAI Chat Completions 入站与 compatible Chat Endpoint（已前移到 P12-08，不再由 P13 执行） | DEFERRED |
| P13-02 | Chat/Responses/Messages 受限无损桥接（已前移到 P12-08，不再由 P13 执行） | DEFERRED |
| P13-03 | New API/AxonHub 一次性 Import Adapter | DEFERRED |
| P13-04 | Cost-aware、Fill-first、Least-loaded 路由 | DEFERRED |
| P13-05 | 管理调试用单请求 Channel Pin | DEFERRED |
| P13-06 | 其它矩阵 `Later` 项重新筛选 | DEFERRED |

## 20. 测试体系

### 20.1 测试层级

| 层级 | 目标 | 运行时机 |
|---|---|---|
| Unit | 纯函数、状态机、解析器、错误分类 | 每个 Task |
| Property/Fuzz | Chunk、JSON、EventStream、Alias、Route 冲突 | 相关 Task + Nightly CI |
| Integration | SQLite、HTTP Client、RouteSnapshot、Credential Lease | 每个 Phase |
| Contract | OpenAI/Anthropic API 形态和错误语义 | P1/P3/P5 后持续运行 |
| Differential | CPA/grok2api/Kiro-RS 行为差异 | Provider Gate 与 P11 |
| E2E | 真实客户端和测试账号 | 每个 Provider Gate |
| Load/Soak | 延迟、吞吐、RSS、连接、背压 | P3 基线、P11 完整 |
| Security | Secret、SSRF、Auth、权限、依赖 | P2、P10、P11 |

### 20.2 提交分类与快速门禁

每个代码、工具链、workflow、脚本、迁移、Fixture、契约或安全策略 Task 都必须在本地运行适用的
定向格式/Clippy/测试、Secret scan 与 changed-doc link check；整合本地 Phase preflight 和该 Phase 的
唯一远端 Delivery Gate 至少各运行一次以下完整集合：

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
secret scan
changed-doc link check
```

普通代码 Task 不逐个触发 GitHub Gate；CI/workflow/cache/required-status 的窄例外按 `CR-EXEC-007`
提前验证。纯报告、索引或计划状态变更使用 `docs-only` Gate：Markdown/格式、文档链接、Secret
scan 和计划一致性检查。它不能以未执行的 Rust/供应链检查冒充代码 Full Gate；具体 workflow 分类、
required-status 与 P 级 trigger/cache 行为由 P4-00 的既有基础和 P5-00 共同验证。

### 20.3 Phase 完整门禁

```text
all-feature tests
integration tests
contract snapshots
relevant property/fuzz corpus
cargo deny / cargo audit
coverage report
phase-specific E2E
phase report
```

关键状态机、路由编译器、Secret 和重试逻辑要求分支覆盖完整；全 Workspace 行覆盖目标不低于 80%，但不得用无意义测试追求数字。

## 21. 性能与资源纪律

- 所有流使用 `Bytes` 和有界 Channel。
- 上游 Client 按 Endpoint/EgressPolicy 复用连接池，不按请求创建。
- RouteSnapshot 读取无全局锁、无磁盘访问。
- Credential Runtime State 按 Provider/Endpoint/Credential 分片。
- Token Refresh 按 Credential Singleflight。
- Body 日志默认关闭；结构化事件批量异步写入。
- 每次 Phase Gate 记录 CPU、RSS、分配、连接复用、P50/P95/P99 和吞吐。
- 基准变化超过 G11 阈值必须创建性能说明或修复，不得直接更新 Baseline 掩盖回归。
- 本地构建关闭增量编译（`.cargo/config.toml` 的 `build.incremental = false`），与两个 CI
  workflow 的 `CARGO_INCREMENTAL: "0"` 对齐。原因：Cargo 从不回收 `target/`，增量会话目录
  只增不减——2026-07-30 本仓库曾积累 2711 个会话目录共 39 GiB，叠加按指纹保留的历史产物
  （单个 crate 的 `.rlib` 已达 49 份不同哈希），`cargo clean` 一次回收 110 GiB / 88 万文件。
  该仓库全量构建不足一分钟，因此丢失增量状态的代价小于其磁盘代价；同时消除"只在有增量
  状态时才可复现"的本地/CI 差异。
- `target/` 仍会随指纹变化增长且没有内置淘汰机制，需定期 `cargo clean`；Cargo 无 LRU 选项。
  清理时注意 `web/admin-ui/node_modules` 是 `gateway-http-actix` build script 的构建输入，
  删除后必须先 `npm ci --ignore-scripts` 才能重新构建。

## 22. 安全纪律

- 日志默认拒绝 Authorization、Cookie、OAuth、SSO、API Key 和原始 Cache Key。
- Fixture 只允许脱敏或人工生成数据。
- 自定义 URL 在连接前和 Redirect 后都重新执行 EgressPolicy。
- Client Key、Management Key 和上游 Credential 使用不同命名空间和权限。
- 管理端默认只监听本机/私网；公网暴露必须经过 Caddy/Cloudflare 和额外鉴权。
- Backup 不包含主密钥；恢复流程必须明确数据库与主密钥的组合要求。
- 依赖升级单独执行测试和差分，不使用无审核的自动大版本更新。

## 23. 需求追踪

| 功能矩阵模块 | 主要执行阶段 |
|---|---|
| A 接口与服务器 | P1、P3、P5、P12 |
| B 协议与流 | P1、P5、P6-P9 |
| C Provider | P3、P5-P9 |
| D 模型与路由 | P2-P4 |
| E 凭据与错误 | P2-P9 |
| F Thinking/缓存 | P5-P9 |
| G 可观测性 | P3、P4、P10、P11 |
| H Management | P2、P4、P10 |
| I 插件 | Release 1 Drop/Deferred |
| J 配置与部署 | P0、P2、P10-P12 |
| K 性能 | P1、P3、P11 |
| L 上游聚合 | P2-P5，延后项进入 P13 |

每个 Task 实现前需在代码或测试说明中引用对应 Matrix ID/行为契约；若无法映射，必须先创建 Change Request。

## 24. 后续每轮汇报格式

后续开发回合统一使用：

```text
Plan version:
Current phase / gate:
Current task:
Completed in this turn:
Verification evidence:
Files changed:
Execution timing (Task Card / code commit / local review-test pass / Phase closeout tag / Phase Delivery Gate):
Scope level and execution budget:
Repeated validations / rework count:
Risks or deviations:
Next task:
```

最终回复必须明确区分：已完成、仅设计、未验证、受阻和 Deferred，不能用“基本完成”替代状态。

## 25. 计划变更记录

| 版本 | 日期 | 变化 | 批准状态 |
|---|---|---|---|
| v1.0 | 2026-07-18 | 建立 Release 1 全阶段、任务、Gate、测试、安全、Git、灰度和变更控制基线 | 当前执行基线 |
| v1.1 | 2026-07-19 至 2026-07-21 | 记录 P1/G1 的 Tool 语义投影澄清，以及 P3/G3 的公开别名、有限 SSE 帧与 idle 边界 Change Request | 已批准的历史执行基线 |
| v1.2 | 2026-07-21 | `CR-EXEC-001`：缓存化受版本约束的质量工具、code/docs/tag Gate 分类、`LOCAL_PASS_PENDING_CI` 流水线、文档证据批处理、单探针诊断与真实 Provider readiness 纪律；新增 P4-00 前置项 | APPROVED；当前执行基线 |
| v1.3 | 2026-07-21 | `CR-EXEC-002`：缓存可见 delivery ref、Fast 后补充供应链 Full、单次 docs-only 收口、cache summary 与暖态时延目标；P4-02 开始 | APPROVED；当前执行基线 |
| v1.4 | 2026-07-21 | `CR-EXEC-003`：Task Card、S/M/L 执行预算、集中补丁、去重验证、Gate 等待重叠、证据模板与全程时延报告 | APPROVED；当前执行基线 |
| v1.5 | 2026-07-21 | `CR-EXEC-004`：按任务风险与不确定性路由 Luna/默认/高级模型及最低足够思考强度；简单任务独立交付并透明记录 fallback | APPROVED；当前执行基线 |
| v1.6 | 2026-07-21 | `CR-EXEC-005`：原位无法切换 Luna 时的受限低成本子代理 fallback；最多一个活跃代理、禁止嵌套，主会话保留 review 与验收 | APPROVED；当前执行基线 |
| v1.7 | 2026-07-21 | `CR-EXEC-006`：普通简单执行默认 Luna `low`，多步检查或机械写入使用 `medium`；`minimal` 收窄至零判断查询 | APPROVED；当前执行基线 |
| v1.8 | 2026-07-21 | `CR-P4-G4-001`：新增 P4-10 的只读管理状态查询、403 账户状态与受控恢复，闭合 G4 而不提前实现 P10 HTTP/UI | APPROVED；当前执行基线 |
| v1.9 | 2026-07-21 | `CR-EXEC-007`：未开始 Phase 改用一条 P 级分支、每 Task 本地 review/test、每 Phase 一次正式远端 Fast + Full；新增 P5-00 以验证 GitHub trigger/default-ref cache/tag restore，并保留 CI/cache 等提前远端例外 | APPROVED；当前执行基线 |
| v1.38 | 2026-07-23 | `CR-P7-G7-001`：P7 的外部 Kiro 账号认证阻塞期间，允许 P8 仅进行顺序本地开发；保留 G7、两阶段 Delivery Gate、完成语义、真实 E2E 与发布的 fail-closed 边界 | 已由 v1.39 的冲突顺序部分替代 |
| v1.39 | 2026-07-23 | `CR-P7-DEFER-002`：P7 Kiro OAuth 延后；P8-G12 按自身非 Kiro 依赖推进，P8 可进行自身 Gate/Delivery，Kiro 仍必须在 P12/G12 后、含 Kiro 发布前回补 | APPROVED；当前执行基线 |
| v1.40 | 2026-07-23 | 新增 P8-07：为既有 §20.1 的每 Provider Gate 真实 E2E 要求登记一个默认零网络、单目标/单模式/单发送的 Official API-key harness；不改变公开 API 或 Kiro 的延后状态 | APPROVED；当前执行基线 |
| v1.41 | 2026-07-23 | `CR-P8-DEFER-001`：因无 Official API Key，将 P8-07/G8 延后至与 P7-09 的最终外部认证验收包；不改变 P9-P12 的 Gate 依赖或提前开发权限 | APPROVED；当前执行基线 |
| v1.42 | 2026-07-23 | `CR-P9-LOCAL-001`：允许 P9-01 至 P9-08 在 P8/G8 外部 E2E 延后时完成本地实现；P9-09/G9/Delivery Gate 仍需 P9 自身真实 Web Canary 证据 | APPROVED；当前执行基线 |
| v1.43 | 2026-07-23 | `CR-P9-CANARY-001`：用户指定当前 grok2api `grok_web/sso` 账号作为 P9-09 的独立测试来源；恢复单目标、最多三次、无值日志的 Web Canary，P7/P8 延期与 P10 前置不变 | APPROVED；当前执行基线 |
| v1.44 | 2026-07-23 | `CR-P9-CANARY-002`：在用户扩大测试授权后，允许 P9-09 按现有服务器 Web 运行轮廓使用受限 Statsig 辅助请求与临时 loopback SSH SOCKS5 出口复测；固定目标 POST 预算与 G9 fail-closed 不变 | APPROVED；当前执行基线 |
| v1.45 | 2026-07-24 | `CR-P11-04-001`：将 P11 loopback Soak 的最低本地观察固定为 10 小时；P12-10 的真实 72 小时 Canary 保持不变 | APPROVED；历史执行基线 |
| v1.46 | 2026-07-24 | `CR-P12-01-001`：批准 GitHub OIDC keyless Sigstore manifest 签名和透明日志；OCI 仅为私有 CI artifact，不得推送/部署 | APPROVED；当前执行基线 |
| v1.49 | 2026-07-25 | `CR-P12-05-001`：批准一份精确 SHA 绑定的私有 OIDC/Sigstore artifact，并在验证后仅对 isolated loopback Staging 执行临时图与最小验收 | APPROVED；当前执行基线 |
| v1.50 | 2026-07-25 | `CR-P12-05-002`：P12-05 policy-alignment 修复的同范围私有 OIDC/Sigstore artifact 续签；同类不扩张范围的续签可直接批准，但仍逐次记录、gate 和验签 | APPROVED；当前执行基线 |
| v1.51 | 2026-07-25 | `CR-P12-05-003`：已删除的本机 `0600` Bearer 选择临时文件不计为 memory-only 证据；同范围直接批准下必须以纯内存 helper 重做预检后才能继续 | APPROVED；当前执行基线 |
| v1.52 | 2026-07-25 | `CR-P12-05-004`：P12-only Krill/Codex compatibility User-Agent 修复，重新生成并验签同范围私有 artifact；不刷新凭证、不扩展 Staging 或公开边界 | APPROVED；当前执行基线 |
| v1.53 | 2026-07-25 | `CR-P12-05-005`：P12-only Anthropic `max_tokens` 到 Responses 输出上限的窄映射，保留其余 foreign-extension 拒绝；新 artifact 后只重跑未覆盖的隔离 Staging 验收 | APPROVED；当前执行基线 |
| v1.54 | 2026-07-25 | `CR-P12-05-006`：服务器端 `/models` 预检通过后，允许一次无正文留存的 `/responses` 严格结构分类，以定位 decoder 兼容性边界 | APPROVED；当前执行基线 |
| v1.55 | 2026-07-25 | `CR-P12-05-007`：CR-006 结果捕获失败后，保守关闭其 request，并以 root-only 无值 receipt 允许一次独立替代分类 | APPROVED；当前执行基线 |
| v1.56 | 2026-07-25 | `CR-P12-05-008`：Responses 子集分类通过后，以同一已验收 artifact 允许一次错误类别增强的 isolated Staging 重跑 | APPROVED；当前执行基线 |
| v1.57 | 2026-07-25 | `CR-P12-05-009`：Staging 502 首事件前失败后，允许一次完整 decoder-contract 的无正文 Responses 分类 | APPROVED；当前执行基线 |
| v1.58 | 2026-07-25 | `CR-P12-05-010`：完整 decoder classifier 通过后，以同一 signed artifact 允许一次最终 isolated Staging retry | APPROVED；当前执行基线 |
| v1.59 | 2026-07-25 | `CR-P12-05-011`：发现 prior classifier 缺少 builder 固定 input message type 后，允许一次精确同形无正文分类 | APPROVED；当前执行基线 |
| v1.60 | 2026-07-25 | `CR-P12-05-012`：精确同形 classifier 通过而 Staging 仍 502 后，增加仅 loopback 管理面可读的固定阶段 attempt 投影；无新外部请求 | APPROVED；当前执行基线 |
| v1.61 | 2026-07-26 | `CR-P12-05-013`：CR-012 证明 decoder 成功后，修复 P12-only 非流式 Responses 至 Anthropic Messages 的 usage 时序与显式终止语义；需新私有签名制品和一次隔离重验 | APPROVED；已完成一次 Messages 受控复验并回滚，P12-05 其余 Tool/Explain 仍待范围决定 |
| v1.62 | 2026-07-26 | `CR-P12-05-014`：复用已验签 CR-013 artifact，仅完成一次无副作用 Tool 与条件性、无 upstream 的 Route Explain；每种异常都完整回滚 | APPROVED；已发送一次 Tool，`2xx` 未通过无值 Function Call 门槛，Explain 未运行并已回滚 |
| v1.63 | 2026-07-26 | `CR-P12-05-015`：CR-014 的 Tool `2xx` 未通过无值 Function Call 门槛后，以不同、明确的无外部效应声明执行一次新 tuple，并把回执细化为封闭结构类别 | APPROVED；Tool `2xx`/`valid`、条件性 Explain 与完整回滚均通过 |
| v1.64 | 2026-07-26 | 记录 CR-015 成功 Tool/Explain 回执、独立回滚复核及 P12-05 的本地验收收口 | P12-05 为 LOCAL_PASS_PENDING_PHASE_GATE；P12-06 仍 PENDING |
| v1.65 | 2026-07-26 | `CR-P12-05-016`：修复 serve 二进制上的 Anthropic 流式 usage 时序、流式 Tool 生命周期、256 KiB 入站正文上限与缺失的 SSE keepalive/45s 绝对超时；放宽 `message_start` 的精确 input Usage 要求为终止 `message_delta` 强制交付 | APPROVED；27 项新本地测试与 `check.sh fast` 全通过，未改服务器或公开边界 |
| v1.66 | 2026-07-26 | `CR-P12-05-017`：以 driver 声明式 in-flight 上限使非流式 600s 传输上限可达；双层语义活性把卡死上游的租约占用从 1 小时降至约 17 分钟且不误杀思考停顿与停读客户端；解码器双游标消除 O(n²) 扫描并约束工具标识符 | APPROVED；16 项新本地测试、4300+ 例差分模糊与全工作区门禁通过 |
| v1.67 | 2026-07-26 | `CR-P12-ROLLOUT-001`：切换范围界定为全部实际生产流量（Kiro/Grok 无渠道暂不启用，延后至外部认证收口）；Canary 分流在服务器本地 Caddy 以加权/按 Key 反代执行，Cloudflare 不动，测试域名仅作暴露前验证 | APPROVED；docs-only，无代码或服务器变更 |
| v1.68 | 2026-07-26 | `CR-P12-05-018`：交付受控配额恢复探测与管理面受控恢复：live 选择路径在普通选择失败后可将一个到期 `RecoveryRequired` binding 作为单张 CAS ticket、lease-first 的受控探测 Attempt（成功以 Estimated 空窗口快照完成，再次 429 由新快照取代 ticket，Health 谓词先行，403 账户永不被当作配额探测）；P12 管理 facade 注入与请求路径相同的 Health/Quota 运行时句柄，403 账户凭操作者证据本地 begin+complete(Allowed)，到期（Reset 后）配额可操作者覆盖，Reset 前拒绝，不发送 Provider 流量 | APPROVED；7 项新本地测试与受影响包 `cargo test`/`clippy` 全通过 |
| v1.69 | 2026-07-26 | `CR-P12-05-019`：serve 组合接通 P4 可观测性：数据面 sink 改为 stage-ledger-first 的 fanout 进有界队列（BL-10 损失显式）；生产 writer + telemetry fan-out 在监听器 bind 后 spawn、停机 5s 有界 flush join；管理监听器新增受保护只读 Prometheus 暴露；管理 Attempt listing 改为 durable 日志支撑（含 endpoint/credential 身份），stage ledger 降级为 enrichment | APPROVED；5 项新本地测试、3 项 ledger 测试改写，受影响包 `cargo test` 全通过 |
| v1.70 | 2026-07-26 | `CR-P12-06-001`：P12 serve 准入从 singleton 图扩至生产图形状（多 Endpoint/Credential/别名/多 Key，max_attempts 1..=5，总并发 <=16，OpenAI-compatible fail-closed）；随附生产配置图录入与客户端 Key 迁移 Runbook（docs/p12-rollout-runbook.md，双接受窗口 + Caddy `rgw_` 前缀分流） | APPROVED；6 项新本地测试与受影响包 `cargo test`/`clippy` 全通过 |
| v1.71 | 2026-07-27 | `CR-P12-06-002`：交付 Anthropic 出站编解码器与 provider 边界、填充 gateway-protocol 为封闭 ApiFormat 词表与适配器注册表、api_format 与 adapter_id 在发布期即校验、执行器改用 BC-ROUTER-005 协议过滤选择候选 | APPROVED；22 项新本地测试与全量门禁通过 |
| v1.72 | 2026-07-30 | `CR-P12-01-002`：发布流水线从单一 `x86_64-unknown-linux-gnu` 扩为 x86_64 与 `aarch64-unknown-linux-gnu` 双目标，各自在同架构标准 runner 上原生构建、独立 SBOM/manifest/keyless 签名/receipt；校验器改为封闭目标白名单并按目标推导 ELF `e_machine`、OCI architecture 与钉死的基础镜像 digest；Dockerfile 基础镜像逐架构传入并由产物侧 `base.name` 重新建立钉死性质。生产主机为 aarch64，此前产物在其上不可执行 | APPROVED；[run 30533211028](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30533211028) 两个 job 均 SUCCESS，两份私有产物均已本地 `--require-signature --require-receipt` 复验；aarch64 产物为真实 `ARM aarch64` ELF 且在容器内实际执行 |
| v1.73 | 2026-07-30 | `CR-P12-08-001`：Canary 阶段最低成功请求数由 100 改为 1250（并给出随基线上升的样本量表），合成补足请求单独计数且其失败计入分子，50% 阶段最短观察由 2h 提升为 24h 以补回本地 Soak 让掉的长时覆盖；新增四级故障严重度分级使 G12 的“无 P0/P1 故障”成为可判定谓词；明确 TTFT 与实时 P95/P99 服务端不可观测并改由客户端侧与离线路径取证 | APPROVED；`scripts/check-p12-08-canary-thresholds.rb` 以可执行形式固定该统计结论与分级，接入 check.sh fast/docs |
| v1.74 | 2026-07-30 | `CR-P12-07-001`：新增 Canary 分流与回滚两个 Caddyfile 模板（生产主机名不变、按非机密 `rgw_` 前缀在两个头上分流、刻意不含 18181 路由与压缩），超时按网关自身上限推导；新增校验器从 Rust 常量读取 keepalive/正文/流式上限并与 Caddyfile 逐项比对，漂移即 fail closed；新增 P12-09 的 RTO 测量脚本，按观测到的路由变化计时而非按 reload 返回 | APPROVED；两 fragment 在服务器真实 Caddy v2.11.4 上 validate 通过并以 adapt 核对编译后路由，13 条回归路径实测拒绝，服务器现行配置未变 |
| v1.75 | 2026-07-31 | `CR-P12-06-003`：界定 P12-06 范围——incumbent 无 Kiro 渠道故 Kiro 只做功能验证与性能基线（显式标注不是差分），可差分的仅 OpenAI-compatible 路径；流量用固定合成请求而非影子复制（避免真实配额消耗，且 Caddy 只能路由不能镜像）；Kiro 用 kiro-rs 的静态 `ksk_` api_key（不参与 OAuth 刷新，不影响线上 kiro-rs）；性能指标定为首字耗时（按 FirstSemanticEvent）、TTFB、总耗时、token 输出速率（分母用首字之后的稳态区间）与 token 间延迟分位数 | APPROVED；不解除 P7-09 外部认证延期，不改 CR-P12-ROLLOUT-001 的切换范围，Kiro 生产路由仍不启用 |
| v1.76 | 2026-07-31 | `CR-P12-06-004`：把已完成但从未接线的 Kiro 渠道接入 P12 运行时。按 kiro-rs 逻辑确认 endpoint(ide/cli) 是凭据级字段而非线格式，故 `ApiFormat` 词表与存储值不变、Kiro 复用 `anthropic/messages`；放宽 BC-PROTOCOL-007 为「一格式多适配器、Endpoint 显式选择」，`adapter_id()` 改 `adapter_ids()`，发布期校验改为集合成员判定；新增 `kiro.messages` 适配器与 `apps/gateway` 的 provider-kiro 边。仅解禁 Kiro 切片，Grok 三切片不动 | APPROVED；不解除 P7-09/G7 的 DEFERRED，本 CR 证据不计入 Kiro 的 Provider Gate |
| v1.77 | 2026-08-01 | `CR-P12-06-005`：修复 CR-004 接线后暴露的组合缺陷——Kiro 渠道曾拒绝 100% 请求。Anthropic Messages 强制 `max_tokens` 而入站解码器按设计将其保留为根扩展，Kiro 的 `conversationState` 无任何输出上限字段故 `BC-PROVIDER-007` 拒绝一切根扩展；两条正确规则相乘使任何合规客户端都无法使用该渠道。在组合根新增仅丢弃该一项扩展的投影（与 kiro-rs 口径一致：接受但不转发），其余扩展全部保留以继续在转换器内 fail closed | APPROVED；修复前 `ClientRequestError`（转换期失败）、修复后 `ProviderPermanent`（已出网并被上游按假凭据拒绝），本地隔离网关全链路验证通过 |
| v1.78 | 2026-08-01 | `CR-P12-06-006`：修复 Kiro 请求缺失必需字段 `conversationState.chatTriggerType`。其余字节相同时，不含该字段→400 `ValidationException`/`REQUEST_BODY_INVALID`，含该字段→200 且返回真实 `application/vnd.amazon.eventstream`。一律发送 `MANUAL`（网关的每个请求都源自客户端显式调用）。本 CR 同时如实记录并撤销了一次错误诊断：曾误判根因为 CLI 的 Content-Type，因为只看 HTTP 状态码而未看正文——`application/json` 的 200 正文实为 `UnknownOperationException`。`application/x-amz-json-1.0` 自始正确，已 revert | APPROVED；新增跨 kind 的独立断言并经变异验证；功能验证以 id=5 单次真实调用取得完整响应（`{"content":"OK"}`、`stopReason: END_TURN`、计费 0.0155 credit） |
| v1.79 | 2026-08-02 | `CR-P12-06-007`：Pro Max 凭据超额后，受控测试 kiro-rs 其余两个 headless Free API-key 凭据；使用不可变 successor Config Version、单候选/单 attempt、root-only ledger 和不保存正文的直连分类。修复录图 helper 的 parent lineage 与 ledger 覆盖缺陷 | APPROVED；两个候选的 gateway 功能请求均为 502、修正后的同形直连均由上游返回 403，故未启动性能采样；剩余 IdC OAuth 已过期且不在本 CR，P12-06 保持 IN_PROGRESS |
| v1.80 | 2026-08-02 | `CR-P12-06-008`：Kiro 切片因无可用凭据转为 `DEFERRED_EXTERNAL_CREDENTIAL` 且不再阻塞 P12-06；后续只执行 OpenAI-compatible live differential。两侧不共享账号池，因此差分只比较可判定的 canonical 结构、不变量、错误所有权与终止语义，正文、ID 和 Usage 具体数值不作相等比较 | APPROVED；Kiro 请求停止，P7-09/G7 与 Kiro 生产路由仍延期/禁用 |
| v1.81 | 2026-08-02 | 记录 P12-06 OpenAI-compatible live preflight、差分 harness 与停止判据：candidate 通过，incumbent Responses 返回无细分归因的内部 5xx，完整 paired corpus 未启动 | P12-06 `BLOCKED`；须单独批准 incumbent 修复或指定另一条已工作参考臂，P12-08 保持 `PENDING` |
| v1.82 | 2026-08-02 | `CR-P12-06-009`：批准对 incumbent CPA Responses/account-pool 参考路径执行先备份、最小且可回滚的修复；固定非流式预检成功后才恢复 paired corpus | APPROVED；P12-06 恢复 `IN_PROGRESS`，公开 Caddy/DNS、Client Key、新网关图、Kiro 与 P12-08 边界不变 |
| v1.83 | 2026-08-02 | `CR-P12-06-010`：incumbent Grok/xAI OAuth 池的 access token 已过期，受控 refresh 样本均为 `invalid_grant`，交互登录未产生可用凭据；按用户要求停止该路线 | APPROVED；P12-06 `DEFERRED`，未运行的 paired corpus 不计为通过，P12-08 仍 `PENDING` |
| v1.84 | 2026-08-02 | `CR-P12-06-011`：使用 CC Switch 当前 Krill Codex 渠道建立独立前缀、单凭据、单模型的 incumbent OpenAI-compatible 参考臂；预检通过后才运行 paired corpus | APPROVED；P12-06 恢复 `IN_PROGRESS`，Grok/Kiro 仍延期，P12-08 未解禁 |
| v1.85 | 2026-08-02 | `CR-P12-06-012`：Krill paired corpus 中 candidate 的 SSE/非流式/Tool 均通过，incumbent 非流式通过但所有 SSE 均未形成完整生命周期；只允许一次无值 SSE 类别诊断 | APPROVED；诊断后无条件回滚，不重跑整批，P12-08 未解禁 |
| v1.86 | 2026-08-02 | `CR-P12-06-013`：CR-012 证明通用 Chat→Responses translator 遗失 SSE 终止事件；改用 incumbent 原生 `codex-api-key` Responses 执行器承载同一 Krill 隔离参考臂，三项预检全过后才重跑 paired corpus | APPROVED；成功或失败均回滚，P12-08 未解禁 |
| v1.87 | 2026-08-02 | `CR-P12-06-014`：修正 differential harness 对 Usage 可选明细存在性的过严等值比较；跨臂仅比较已批准的存在/非负/守恒不变量，补正反回归并离线重算 CR-013 receipt | APPROVED；不新发网络请求，P12-08 待 P12-06 closeout review |
| v1.88 | 2026-08-02 | 收口 P12-06 Krill 参考臂：两臂 10/10 SSE、0 失败，非流式/Tool/Canonical/Usage 不变量全通过，9/9 离线复核通过，临时 incumbent 配置完整回滚 | P12-06 `LOCAL_PASS_PENDING_PHASE_GATE`；P12-08 仍 `PENDING`，Grok/Kiro 仍延期 |
| v1.89 | 2026-08-02 | `CR-P12-08-002`：用户批准开始 P12-08；先执行生产 Canary 准入只读检查，只有 active 图、独立 `rgw_` Client Key、凭据、Caddy rollback preimage 与观测链路齐备后才允许首个 10% 双接受/分流，不自动推进后续阶段 | P12-08 `IN_PROGRESS`；生产流量尚未切换 |
| v1.90 | 2026-08-02 | P12-08 准入最小双接受事务：新网关 key 本地 200、incumbent CPA 401；已恢复 CPA 配置 preimage 并确认服务 active，未 reload Caddy 或切生产流量 | P12-08 `BLOCKED`；待确定 CPA 支持的 key 注册路径/格式后再开新 CR |
| v1.91 | 2026-08-02 | `CR-P12-ROLLOUT-002`：用户纠正最终拓扑为 CPAR 全量替代 CPA；撤销百分比/按 Key 分流及 CPA 双接受前置，改为客户端 Key 迁移清单、全量 cutover、一次全量回滚/恢复、72h 全量观察后关闭旧 CPA | APPROVED；P12-08 恢复 `IN_PROGRESS`，生产尚未切换 |
| v1.92 | 2026-08-02 | P12-08 无值客户端清点：旧 CPA 三个历史 key 身份；近 1h 无推理流量；本机发现 OpenClaw CPA Provider 与 CC Switch Claude CPA 记录；当前 CPAR 仍为单模型 differential 图，历史 Chat 兼容须显式处置 | P12-08 `IN_PROGRESS`；未改任何客户端或生产路由 |
| v1.93 | 2026-08-02 | `CR-P12-COMPAT-001`：将 Chat Completions 与三协议无损桥接从 P13 前移至 P12-08，并新增 Kiro/Grok/Codex/Claude runtime、生产图、能力矩阵与真实 E2E 切片 | APPROVED；P12-08A `IN_PROGRESS`，P12-09/P12-10 保持 PENDING |
| v1.94 | 2026-08-02 | 完成 P12-08A OpenAI Chat Completions 严格纯 Codec、行为契约、Tool 参数分片不变性、Usage 溢出与 SSE 终止顺序回归 | LOCAL_PASS_PENDING_PHASE_GATE；P12-08B 为下一切片，P12-08/P12-09/P12-10 边界不变 |
| v1.95 | 2026-08-02 | 完成 P12-08B：在共享 Actix 数据面加入认证且有界的 `/v1/chat/completions` JSON/SSE 边界，复用 Canonical transport、keepalive、取消与 FSE 交付，并新增独立 Chat 请求观测协议 | LOCAL_PASS_PENDING_PHASE_GATE；P12-08C 为下一切片，尚无 OpenAI Chat 出站 Endpoint |
| v1.96 | 2026-08-02 | 以 CLIProxyAPI v7.2.101 native OpenAI translator 为行为参考完成 P12-08C：第三 ApiFormat、发布/组成注册表、显式入站协议与原生载荷、Chat JSON/SSE 上游解码及 DNS-pinned 交接；既有 CPAR 安全门禁不降级 | LOCAL_PASS_PENDING_PHASE_GATE；P12-08D 为下一切片，生产图与流量未改 |
| v1.97 | 2026-08-02 | `CR-P12-PORT-001`：确立旧 CPA 行为移植优先原则和 Legacy Behavior Manifest，把 P12-08D-G 拆为协议请求、响应/SSE、注册表、差分、分渠道 runtime、生产图/本地 E2E/迁移 dry-run 与 live receipt 小批次；保留 CPAR 安全 hardening，账号缺失渠道默认禁用并延期 live 补验 | APPROVED；下一切片 P12-08D0，仅优化计划，生产图与流量未改 |
| v1.98 | 2026-08-02 | 完成 P12-08D0：固定 CLIProxyAPI v7.2.101 精确 commit、八个显式 translator 加一个 Messages native fallback、197 个 translator tests 与 CPAR Rust 边界；请求/响应差异预分类为 parity、intentional hardening 或 unsupported fail-closed | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08D1，请求侧 typed port；无代码、服务器或流量变化 |
| v1.99 | 2026-08-02 | 完成 P12-08D1：九种 Chat/Responses/Messages 请求 pair 均有显式 native/typed projection；输出上限、Tool history 与固定 Reasoning 档位按目标协议映射，真实三类 request builder、脱敏 fixture 与属性测试通过；未知/不兼容语义稳定拒绝 | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08D2 响应/SSE typed port；无服务器、凭据、Config Version 或流量变化 |
| v1.100 | 2026-08-02 | 完成 P12-08D2：新增有界 Responses JSON/SSE 上游解码与事务式目标投影；九种 decoded source/target 均通过真实非流式及 SSE encoder，任意 Chunk 最终语义、Tool/Usage/stop/error/终止闭合；Reasoning→Chat 与未知语义 fail closed | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08D3 runtime/registry/Explain 接线；无服务器、凭据、Config Version 或流量变化 |
| v1.101 | 2026-08-02 | 完成 P12-08D3：九 pair 注册表接入 runtime 与 Route Explain，请求局部转换/能力检查在 Credential pool、lease 和 Attempt 前 fail closed；native、Canonical、LosslessBridge 路径确定，Responses 生产解码与 D2 响应投影接线，无值拒绝原因与零 Attempt 证据通过 | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08D4 离线差分与安全偏差复核；无服务器、凭据、Config Version 或流量变化 |
| v1.102 | 2026-08-02 | 完成 P12-08D4：新增固定旧 CPA 参考的 10 项 clean-room 三协议差分语料，覆盖 Chat/Responses/Messages JSON/SSE、Tool、Reasoning、Usage 和终止；CPAR 侧由真实 codec/router 计算，6 parity、2 hardening、2 unsupported/fail-closed，缺项/漂移/误分类/空心覆盖均拒绝 | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08E1 Codex/OpenAI-compatible runtime；无服务器、凭据、Config Version 或流量变化 |
| v1.103 | 2026-08-02 | 完成 P12-08E1：Chat/Responses runtime 统一使用严格 API-key/Codex OAuth 凭据与刷新事务；OpenAI-compatible 非 2xx 有界分类接入，401 精确隔离当前 Credential、usage-limit/429 精确隔离 Quota、5xx 仅冷却 Endpoint，既有 Usage/Reasoning vertical slice 与真实 loopback 回归通过 | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08E2 Claude/Anthropic-compatible runtime；无服务器、真实凭据、Config Version 或流量变化 |
| v1.104 | 2026-08-02 | 完成 P12-08E2：Anthropic Messages runtime 接入严格 API-key/Claude OAuth 互斥授权与刷新事务；有界 401/auth、429/rate-limit、529/overloaded 分类接入，复用精确 Credential/Quota/Endpoint Health 隔离及既有三协议 Tool/Thinking/Usage/SSE vertical slice | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08E3 Grok unified runtime；无服务器、真实凭据、Config Version 或流量变化 |
| v1.105 | 2026-08-02 | 完成 P12-08E3：Grok Build OAuth 与 xAI Official API-key 接入固定目标 Canonical runtime；耐久绝对过期凭据导入、JSON/SSE、Tool/Reasoning/Usage、连续性及精确失败隔离通过；Grok Web 因尚无经验证的通用生产 transport 仅登记合法 ID 并保持组合未绑定、默认禁用 | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08E4 Kiro unified runtime；无服务器、网络请求、真实凭据、Config Version 或流量变化 |
| v1.106 | 2026-08-02 | 完成 P12-08E4：既有 Kiro native Adapter 在统一 runtime 中同时接纳 backward-compatible raw API-key 与严格未过期 Social/Enterprise JSON；CLI/IDE 固定 endpoint/profile、AWS EventStream、Tool/Thinking 与精确 Credential/Quota/Endpoint Health handoff 通过 | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08F1 多渠道生产图与能力台账；无服务器、外部 OAuth、真实凭据、Config Version 或流量变化 |
| v1.107 | 2026-08-02 | 完成 P12-08F1：不可变 adapter 能力台账取代空 Endpoint profile，登记已本地通过的 Tools/Parallel Tools/Reasoning/JSON Schema/Streaming；未知 adapter、跨 Version Endpoint 能力冲突与无 runtime 的 Grok Web 均 fail closed，缺凭据渠道不进入 active 生产图 | LOCAL_PASS_PENDING_PHASE_GATE；F2 review 补齐 Tool 必需的 JSON Schema 能力；下一切片 P12-08F2 三协议 × 四类渠道 loopback E2E；无服务器、网络请求、真实凭据、Config Version 或流量变化 |
| v1.108 | 2026-08-02 | 完成 P12-08F2：三协议 × 四渠道本地矩阵中 7 个支持 cell 完成 28 个 JSON/SSE × Text/Tool/Usage 请求，5 个不支持 cell 完成 10 个稳定安全拒绝且 Attempt=0；Chat 不降级 Reasoning，Grok/Kiro provider-specific runtime 保持 Canonical-only | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08F3 Client Key/Alias/客户端迁移 dry-run；无服务器、外部网络、真实凭据、Config Version 或流量变化 |
| v1.109 | 2026-08-02 | 完成 P12-08F3：OpenClaw 在独立临时配置与状态目录中完成 synthetic `rgw_` key、endpoint、active alias、协议保持和字节级回退演练；live 配置未变；CC Switch 按 operator 指令明确延期且完全未触碰 | LOCAL_PASS_PENDING_PHASE_GATE；下一切片 P12-08G1 生产切换图受控真实 E2E；无服务重启、外部请求、真实凭据、Config Version、生产主机名或流量变化 |
| v1.110 | 2026-08-02 | 启动 P12-08G1：绑定 exact-SHA dual-target Sigstore run；Codex 固定十二个 Chat/Responses/Messages JSON/SSE Text/Tool 单发送 tuple；Claude 在 CC Switch 外无可导出有效凭据时明确 SKIP；首失败立即回滚 | IN_PROGRESS；不改 CPA、Caddy、DNS、CC Switch、生产主机名或公开流量 |
| v1.111 | 2026-08-02 | `CR-P12-08G1-001..004`：两次 G1 Chat SSE 失败均完整回滚；逐级无值分类证明上游终止有效，最终 message 是既有 delta 的重复投影、reasoning 为空且 Usage 与 finish 同帧；Rust 解码器仅接纳可验证重复并保留严格拒绝 | IN_PROGRESS；定向测试通过，待新 exact-SHA 签名 artifact 后只替代失败 tuple，再继续未发送 tuple |
| v1.112 | 2026-08-02 | `CR-P12-08G1-005`：d738b83 artifact 的替代 tuple 仍以 error frame 停止并回滚；最终无值分类确认只有终端重复汇总帧改用 `chat.completion` 与不同 ID，新增只在重复语义、finish 和 Usage 闭合时接纳并保留初始 Canonical ID 的严格规则 | IN_PROGRESS；定向测试通过，待新 exact-SHA artifact 后再次只替代失败 tuple |
| v1.113 | 2026-08-02 | `CR-P12-08G1-006`：1470a84 artifact 仍在同一替代 tuple 停止并回滚；按固定旧 CPA SSE fixtures 将后续 delta 的 `role:null` 与 `tool_calls:null` 严格解释为无增量，错误类型和重复 role 继续拒绝 | IN_PROGRESS；无需新诊断请求，待新 exact-SHA artifact 后再次只替代失败 tuple |
| v1.114 | 2026-08-02 | `CR-P12-08G1-007`：e45f1a1 artifact 仍在同一替代 tuple 停止并回滚；精确谓词和 count-only 分类确认唯一剩余拒绝门是重复的相同 assistant role，将其作为幂等声明且保留其它严格验证 | IN_PROGRESS；待新 exact-SHA artifact 后再次只替代失败 tuple |
| v1.115 | 2026-08-03 | `CR-P12-08G1-008`：6c011cb artifact 仍在同一替代 tuple 停止并完整回滚；无值 SSE 外层分类与生产 Rust 解码器内存变异矩阵把失败收敛到终端 summary 重复完整 text delta，只在与既有 Canonical text 及 summary message 三方完全相等时幂等抑制 | IN_PROGRESS；定向测试通过，待新 exact-SHA artifact 后再次只替代失败 tuple |
| v1.116 | 2026-08-03 | `CR-P12-08G1-009`：85f0d2a artifact 仍在同一替代 tuple 停止并完整回滚；逐帧无值序列纠正先前假设，确认第 3 帧是含 Message/finish/Usage 但完全省略 delta 的终端 `chat.completion` summary，仅准入该严格终端形状 | IN_PROGRESS；定向测试通过，待新 exact-SHA artifact 后再次只替代失败 tuple |
| v1.117 | 2026-08-03 | `CR-P12-08G1-010`：e297fa1 artifact 使 Chat SSE Text 真实通过，随后 Chat JSON Tool 在零 upstream Attempt 前本地 4xx；同形 required Tool 直连 2xx，新增 Chat/Responses required Tool choice 的严格保留与无 Tool 拒绝 | IN_PROGRESS；2/12 tuple PASS，待新 exact-SHA artifact 从 Chat JSON Tool 续跑 |
| v1.118 | 2026-08-03 | `CR-P12-08G1-011`：c71351e artifact 将 Chat JSON Tool 从 decoder 4xx 推进为 Router canonical admission 的零 Attempt 5xx；严格准入三个目标协议各自同协议 Tool choice，并以 provider builder 测试证明 wire 保留，跨协议继续拒绝 | IN_PROGRESS；2/12 tuple PASS，待新 exact-SHA artifact 从 Chat JSON Tool 续跑 |
| v1.119 | 2026-08-03 | `CR-P12-08G1-012`：f2689b2 artifact 已使 Chat JSON Tool 进入 upstream Attempt，但 decoder 以 `StreamTruncated` 失败并完整回滚；登记一次只读 CC Switch 凭据、同形且不保留值的非流式 Tool 结构分类 | IN_PROGRESS；2/12 tuple PASS，分类不计为验收 tuple，首个封闭 decoder gate 决定后续修复 |
| v1.120 | 2026-08-03 | `CR-P12-08G1-013`：CR-012 的 2xx JSON 通过其余 decoder gate，唯一差异为非流式 Tool call 多出 `index`；关系分类证明其为 unsigned、unique 且严格等于零基数组位置，非流式 decoder 仅准入缺失或位置完全相等的 index | IN_PROGRESS；2/12 tuple PASS，定向测试与 Full gate 后生成新 exact-SHA artifact，从 Chat JSON Tool 续跑 |
| v1.121 | 2026-08-03 | `CR-P12-08G1-014`：398a1a1 artifact 使 Chat JSON Tool PASS，随后 Chat SSE Tool 在安全 stream error frame 停止并完整回滚；登记一次只保留封闭结构和相等关系的同形 SSE Tool 分类 | IN_PROGRESS；3/12 tuple PASS，Chat JSON Tool 不重发，分类定位首个 decoder mismatch |
| v1.122 | 2026-08-03 | `CR-P12-08G1-015`：CR-014 证明六个 Tool delta 重复完整 identity/name 键且参数累计等于 summary，但未证明重复值是否空或相等；登记一次只保留值类别与首次声明相等关系的最终分类 | IN_PROGRESS；3/12 tuple PASS，仅封闭幂等关系可进入修复 |
| v1.123 | 2026-08-03 | `CR-P12-08G1-016`：CR-015 证明 continuation identity/name/type 全为空；终端 summary 的位置、type、name、arguments 与流相等但 call ID 重建，仅在该冗余终端闭合关系中保留首次流式 ID | IN_PROGRESS；3/12 tuple PASS，定向测试与 Full gate 后从 Chat SSE Tool 续跑 |
| v1.124 | 2026-08-03 | `CR-P12-08G1-017`：4d16c3a exact-SHA artifact 使 Chat SSE Tool PASS，随后 Responses JSON Text 以安全 `http_5xx` 停止并完整回滚；登记一次同形单发送及同进程 attempt-stage 读取 | IN_PROGRESS；4/12 tuple PASS，不重发 Chat 四项，先定位 Responses 首个失败边界 |
| v1.125 | 2026-08-03 | `CR-P12-08G1-018`：同形 Responses JSON Text 诊断复现 `http_5xx`，唯一 Attempt 为 `failed/decoder` 且完整回滚；登记一次只读 CC Switch、无值的直连成功响应结构分类 | IN_PROGRESS；4/12 tuple PASS，仅首个已证明 decoder 差异可进入修复 |
| v1.126 | 2026-08-03 | `CR-P12-08G1-019`：直连分类得到 2xx JSON 与有效正文，首个差异为新增 root/message/Usage detail 字段；登记一次只保留字段值类别与封闭关系的最终分类 | IN_PROGRESS；4/12 tuple PASS，区分可忽略元数据与必须映射的 Usage 语义 |
| v1.127 | 2026-08-03 | `CR-P12-08G1-020`：扩展值分类确认部分字段为 null/有序/固定 phase/零 cache write，但嵌套 Tool usage、turn metadata 及浮点 penalties 尚未闭合；登记最后一次深层布尔关系分类 | IN_PROGRESS；4/12 tuple PASS，仅全零/合法相等/已知固定类别可兼容 |
| v1.128 | 2026-08-03 | `CR-P12-08G1-021`：最终分类证明 penalties 为有限零值、retention 已知、Tool usage 全零且 turn metadata 合法相等；登记最小 strict decoder 兼容规则 | IN_PROGRESS；4/12 tuple PASS，定向反例与 Full gate 后生成 exact-SHA artifact 续跑 Responses JSON Text |
| v1.129 | 2026-08-03 | `CR-P12-08G1-022`：CR-021 exact-SHA 续跑仍在 Responses JSON Text decoder 边界失败；登记一次无值直连结构复核，与 CR-020 安全收据比较 | IN_PROGRESS；4/12 tuple PASS，禁止重发已通过 tuple 或未证明放宽 decoder |
| v1.130 | 2026-08-03 | `CR-P12-08G1-023`：CR-022 证明解析后结构未漂移；补齐 Python 分类器与 Rust 预解析边界的重复 JSON 名计数 | IN_PROGRESS；4/12 tuple PASS，仅定位预解析差异，不改 decoder |
| v1.131 | 2026-08-03 | `CR-P12-08G1-024`：CR-023 排除重复 JSON 名；校正直连分类器漏掉的固定 Krill 兼容 User-Agent | IN_PROGRESS；4/12 tuple PASS，仅一次完整请求头等价分类 |
| v1.132 | 2026-08-03 | `CR-P12-08G1-025`：只读证明 G1 v1 图 endpoint/model 与当前 Krill 不等且无本机 provider 可重现；改以当前配置建立 G1 v2 | IN_PROGRESS；目标图已改变，旧 4/12 不沿用，v2 从 0/12 首败即停 |
| v1.133 | 2026-08-03 | `CR-P12-08G1-026`：G1 v2 前 6/12 PASS，第 7 个 Responses JSON Tool 在首败边界停止并回滚；登记单次同进程 Attempt stage 诊断 | IN_PROGRESS；6/12 tuple PASS，仅失败 tuple 可重试 |
| v1.134 | 2026-08-03 | `CR-P12-08G1-027`：CR-026 的一次重试因诊断器使用错误 request ID 前缀而未取得 Attempt；源码确认生产前缀为 `p1-request-` | IN_PROGRESS；6/12 tuple PASS，仅失败 tuple 再发一次完成同进程投影 |
| v1.135 | 2026-08-03 | `CR-P12-08G1-028`：确认 request/event ID 在进程重启后碰撞，使 Attempt 持久与 stage 投影失效；引入进程随机命名空间 | IN_PROGRESS；6/12 tuple PASS，修复审计相关性后再诊断第 7 tuple |
| v1.136 | 2026-08-03 | `CR-P12-08G1-029`：CR-028 artifact 已部署且 restart-unique Request 成功持久化；只读时间线证明诊断事务在 Route 15s 上限内重启，可能终止尚未发射终态的 Attempt；改为终态或 20s 观察上限后才回滚 | IN_PROGRESS；6/12 tuple PASS，仅第 7 tuple 可再发一次，前 6 个不重发 |
| v1.137 | 2026-08-03 | `CR-P12-08G1-030`：终态感知重试得到唯一 `failed/decoder` Attempt，队列与持久化异常计数为零且完整回滚；扩展无值 Responses JSON 分类器，以一次同形 required Tool 直连定位首个封闭差异 | IN_PROGRESS；6/12 tuple PASS，分类不计验收 tuple且不重发任何 G1 tuple |
| v1.138 | 2026-08-03 | `CR-P12-08G1-031`：CR-030 将差异收敛到 Function Call 的双份 turn metadata；同步分类器到现有 root/cache-write 准入，并以一次布尔关系分类证明两份 bounded turn ID 是否相等 | IN_PROGRESS；仅相等关系可支持窄修复，分类不发送 G1 tuple |
| v1.139 | 2026-08-03 | CR-031 最终分类通过：Function Call 两份 turn metadata 均为唯一 bounded ID 且相等；非流式 Tool decoder 仅准入缺失或完整相等 pair，partial/unequal/extra 继续拒绝 | IN_PROGRESS；定向测试与 Full gate 后生成新 exact-SHA artifact，仅续跑第 7 tuple |
| v1.140 | 2026-08-03 | `CR-P12-08G1-032`：`ba54c68` 双架构签名 artifact 成功且 ARM64 独立验证、部署；从第 7 tuple 续跑，若通过则同一固定 harness 继续首次发送 8–12，首败即停并回滚 | IN_PROGRESS；不重发前 6 个 PASS，不触及公开入口、CC Switch 或旧 CPA |
| v1.141 | 2026-08-03 | `CR-P12-08G1-033`：`ba54c68` 使 tuple 7–9 PASS，tuple 10 Messages SSE Text 在成功 SSE bootstrap 后以 lifecycle 类别失败；登记一次只保留事件类型/布尔关系的 loopback classifier | IN_PROGRESS；G1 v2 9/12，仅失败 tuple 10 可诊断重发 |
| v1.142 | 2026-08-03 | `CR-P12-08G1-034`：分类证明 Messages SSE 生命周期完整，仅 input Usage 按既有 deferred-repayment 设计从 start 移至 terminal delta；harness 接受 start 或 terminal 的精确计数，双份必须相等 | IN_PROGRESS；runtime 不变，离线正反例通过后从 tuple 10 续跑 |
| v1.143 | 2026-08-03 | `CR-P12-08G1-035`：tuple 10 通过后，tuple 11 Messages JSON Tool 在零 upstream Attempt 前本地失败；源码证明真实 `any` Tool choice 未被 lossless bridge 映射且 F2 fixture 漏测。新增仅限有 Tool 时的 forced-choice 等价映射（Chat/Responses `required` ↔ Messages `any`），其余 choice 继续 fail closed | IN_PROGRESS；10/12 tuple PASS，需新 exact-SHA artifact 后只续跑 tuple 11，成功才首次发送 tuple 12 |
| v1.144 | 2026-08-03 | `CR-P12-08G1-036`：`ec2fdf6` 首次续跑的 Attempt 查询误读了旧记录；age 复核证明新请求零出网。管理 rollback 会更新 Registry，但 executor 仍绑定启动时 Config Version，因此 re-activate 与最终 rollback 后都必须重启并做真实 HTTP readiness | IN_PROGRESS；复用同一已验签 artifact 与 10/12 receipt，只重试 tuple 11，成功才首次发送 tuple 12 |
| v1.145 | 2026-08-03 | CR-036 修正事务在 re-activate/rollback 后均重启；tuple 11 Messages JSON Tool 与 tuple 12 Messages SSE Tool 各单发送一次并通过，最终两个 Attempt 均 `succeeded`，G1 v2 12/12；predecessor key/图、loopback 与 disabled-at-boot 边界恢复 | P12-08G1 `DONE`；P12-08 `LOCAL_PASS_PENDING_PHASE_GATE`，不授权 P12-09 |
| v1.146 | 2026-08-03 | `CR-P12-09-001`：用户批准开始 P12-09；P12-08 GitHub Full delivery gate 通过后，创建 root-only 生产/回滚 preimage，验证完整 Caddy 候选和三协议 JSON/SSE 回环预检。OpenClaw 当前已无 CPA 引用且无需修改；CC Switch 继续排除。slot 2 由排除法高置信度归为已退休旧 OpenClaw；operator 确认 slot 3 Newapi 必须迁移 | P12-08 `DONE`；P12-09 `IN_PROGRESS_PRE_CUTOVER_GATE`；等待 Newapi 部署位置，生产 Caddy 尚未 reload |
| v1.147 | 2026-08-03 | operator 确认 Newapi 位于 Jakarta VPS；只读 SSH 校验发现现行 RSA/ECDSA/ED25519 主机指纹均与迁移包不符，原 Jakarta client key 对 deploy/root/ubuntu/opc 均被拒。严格身份边界下不覆盖 `known_hosts`、不接受未知主机后修改 Newapi | P12-09 `IN_PROGRESS_PRE_CUTOVER_GATE_EXTERNAL_ACCESS`；须经 VPS console 恢复公钥或提供新 SSH profile + 独立验证指纹，生产 Caddy 尚未 reload |
| v1.148 | 2026-08-03 | 只读检查 operator 放入下载目录的 Jakarta 私钥：文件为 `0600`，但与已安装迁移 key 字节完全相同且公钥指纹相同；该文件直接登录 `deploy` 仍被拒，不能作为新的可信访问凭据 | P12-09 保持 `IN_PROGRESS_PRE_CUTOVER_GATE_EXTERNAL_ACCESS`；生产 Caddy 尚未 reload |
| v1.149 | 2026-08-03 | `CR-P12-09-002`：Clash TUN 排除修正后恢复 Jakarta SSH；Newapi 经官方 API 完成迁移并通过既有两个 Messages alias 的 JSON/SSE 4/4；生产切换、真实回滚与恢复的 Caddy 有效 RTO 分别为 89ms、89ms、88ms。最终 P1 检查发现 OpenAI-compatible Chat SSE 跨请求复用 response ID，使 2 个 Required Usage 事件因全局 response-id 幂等键冲突而隔离；立即执行 89ms 安全回滚并恢复 Newapi preimage。Usage 持久化身份改为网关 request ID 的域分离固定长度摘要，同时继续限制外部 response ID；新增跨请求复用回归并保留同请求冲突、超长 ID 隔离语义 | P12-09 `IN_PROGRESS_P1_FIX_BUILD`；生产入口驻留旧 CPA，P12-10 未开始；须以 exact-revision artifact 部署并从零检查 P1 增量后重验 |
