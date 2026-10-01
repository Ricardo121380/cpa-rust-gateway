# 当前真实条件及下一执行计划

观察基准：[current-environment.json](current-environment.json)，2026-10-01 15:22:19 UTC；
最终验收工具/测试源码 ff4b59ed。**现在完整新构建真实组合 0；共通阻断未解除前不发送推理。**
当前清单是连续 GET 观察，非原子 snapshot；必须在实际执行前重读，版本观察有效期十分钟。

## 共通条件

| 条件 | 当前事实/状态 | 可操作缺口 |
| --- | --- | --- |
| 实际运行构建 | 旧 b0cf36e387b774439acc23830221389eb3ac74d0；/proc/exe 的 SHA256、PID/start 已读 | 缺经过审核的新 Linux aarch64 release 和 manifest；source/artifact 必须匹配，不靠运行参数 |
| 目标 route/candidate/model | 5 candidate；gpt-6-sol、Codex、Build、Console 启用；gpt-5.5 candidate disabled | 逐组合固定 exact endpoint/candidate/upstream model；不允许 disabled/其他成功替代 |
| 配置 | 管理 active rev-1、观察期间稳定；实际 executing loaded graph revision 未取得 | 在请求/attempt 执行 seam 保留实际 config version/revision，别用事后 active 配置补历史 |
| 账号修订 | managed revision 与 native account revision 已投影；actual lease credential revision 未取得 | 每个 actual attempt 的 executing revision 要可取得；pool/管理修订不默认等价 |
| 精确归属 | 按 response-ID→唯一 Usage request-ID→Attempt、ledger source_event 精确采集已运行 | 当前 AttemptEvent 缺 channel、credential revision、config version/revision、egress identity/revision 共六项，保持 null/BLOCKED |
| Usage | 新源码 cpar_usage 八字段和 input_accounting oracle 已对齐；旧 DB 元数据列可能缺失 | 新构建迁移/实际 Usage 与账本必须重新核验；旧记录缺字段不能补零/推定 measured |
| 实际声明 | public capabilities={}；candidate override 仅配置事实 | 六项 effective declaration 均未知；用获准的当前模型/能力读取和确切 access group 固定，未知不能置 false |
| 权限 | 3 个 Client Key/AccessGroup 配置与 route grants 已读 | 没有使用 client secret 进行 data-plane model view；执行前核对所用 key 对具体 route/model 的权限 |
| 出口 | provider 状态有 available/direct 线索；5 域 DNS/TCP/TLS443 PASS | 探测为主机直连，未验证实际 gateway proxy/egress revision、Provider 认证、配额或模型能力 |
| 边界/扩展 | 实现能 audit 受控原始采样；本地 negatives 已验证 | 真实取消、超时、重试与声明扩展采样尚 NOT_RUN；重启/删除/跨 owner 操作各按具体授权 |

缺 executing revision 的具体软件 seam：`crates/gateway-core/src/gateway_event.rs` 的 AttemptEvent
当前只保留 route/candidate/credential ID、endpoint/upstream/model、时间/结果/retry；
`scripts/acceptance/cpar_new_build_acceptance.py` 的 collector 将缺失字段原样 null。
下一实现可在实际 selection/credential lease/config snapshot/egress resolution 处增加有界、无正文/
秘密的 request+attempt 观察，或复用能提供同等身份的保护采集器。必须同时验证三协议路径、刷新
修订、错误候选和未知字段，不能在验收脚本里从 current inventory 倒填。

## 固定候选与环境阻断

完整 opaque 账号身份在 inventory 和组合清单，本文用摘要前缀；前缀不作为执行身份。

| 渠道 | endpoint / route / candidate / upstream model | 当前账号及独立阻断 |
| --- | --- | --- |
| openai-compatible | p12-12-production-krill-responses-endpoint / route-0b9a2bb7-3f85-487a-bba2-0871bacc46ca / candidate-95857499-eff9-4b29-8bb5-34e55dbabcb0 / gpt-6-sol | cef2ecd2… managed revision0，runtime available；专用 grant 3d761b98… 已读。仅共通条件后优先 |
| grok.build | p12-12-production-grok-build-endpoint / p12-12-production-grok-build-route / p12-12-production-grok-build-candidate / grok-4.5 | 5beff1b6… runtime available、native revision96；其他两个 expired。必须固定该账号的 executing revision，不换账号冒充 |
| codex | p12-06-codex-bridge-endpoint / p12-06-codex-bridge-route / p12-06-codex-bridge-candidate / gpt-5.6-terra | 279f8dd3… managed rev1/status active，但 runtime expired；恢复授权/凭据未执行 |
| grok.console | p12-12-production-grok-console-endpoint / p12-12-production-grok-console-route / p12-12-production-grok-console-candidate / grok-4.20-0309 | 两账号 credential_unauthorized，session absent。native account revision0 与 egress credential/session revision1 不混同 |
| kimi-coding | kimi-coding-responses；无 route/candidate/model | aedc86f7… managed rev847，runtime available；binding enabled、route_ids=[]。需要专用候选、模型和 grant；后台刷新不是本轮恢复操作 |
| anthropic-compatible | 无现有账号/endpoint/route | 账号、合法目标与授权材料缺失 |
| claude | 无现有账号/endpoint/route | 同上 |
| grok.official | 无现有账号/endpoint/route | 同上；有富原生元数据时 Chat/Messages 桥接独立条件性阻断 |
| grok.web | 无现有账号/endpoint/route | 原生多轮、Messages hard cap 软件阻断；工具 unsupported 仅既定限制 |
| kimi-api | 无现有账号/endpoint/route | 账号、合法目标与授权材料缺失 |
| kiro | 无现有账号/endpoint/route | 原生 hard cap 与 Messages 必填输出上限的软件阻断，不能删参数/提示词模拟/下游截断 |

Krill 另有 `p12-12-production-krill-chat-endpoint`，同账号可用，但旧 gpt-5.5 candidate 指向它且 disabled。
本轮目标的 Responses endpoint 不被它的成功替代。routes max_attempts 当前均为 1；受控 retry 检查
必须在经批准的独立场景记录自己的真实预算，不能擅自修改线上预算。

## 下一执行顺序

1. 补 exact attempt 修订/出口观察，读取实际声明及执行 key 的模型权限；本地回归验证新观察。
   当前脚本/模板现成可用，缺证据时停止而非先发完整组合。
2. 在有权限的构建环境使用 `scripts/build-release.sh` 生成指定源码的
   `aarch64-unknown-linux-gnu` 产物/manifest；核对候选及校验值，保留原 release/current 指针和
   DB/状态兼容、恢复方案。release 产物本轮 NOT_RUN，不把 Darwin Full 当作 Linux release 证明。
3. 部署/必要服务重启的具体目标为已有 `new-vps` 上 `cpa-rust-gateway.service`，仅替换指定 release；
   当前旧 release 为 b0cf36e…。这需要针对目标的明确授权。先完成候选制品、迁移兼容/备份和回退
   准备再提出审批；本轮没有执行或请求笼统授权。账号恢复、route/grant 发布是独立操作，不捆绑。
4. 授权切换后重新 `inspect --manifest`，补齐
   [Krill Responses JSON 具体计划](next-krill-responses-json-plan.json) 中 response_model、
   executing egress revision/identity、实际声明与时间等 null。管理 config/revision 是目标快照，
   actual Attempt 必须验证同一修订。使用已有环境变量 client secret，不外发账号材料。
5. 先 Krill gpt-6-sol：Responses JSON、Responses SSE、Chat JSON、Chat SSE、Messages JSON、Messages SSE。
   每组 7 请求，无 runner retry；Messages 保留实际批准的 max_tokens，其他协议保留固定参数。
   逐轮 source/runtime/目标/Usage/ledger 校验，首错停发。完成基础后独立采集九类边界和全部实际声明扩展。
6. 再固定 Grok Build available 账号/grok-4.5，同六组合顺序。实际富原生 metadata 的非等价桥接
   仍为 BLOCKED，不能以普通文本正例覆盖该条件格。之后按独立恢复/发布授权处理 Codex、Console、Kimi。
7. 逐格落 LIVE_NEW_BUILD 回执后再审查 C、#57 和父 #9 的各自完成标准；生产证据另列，不提前关单。

精确命令/输入 schema 在 [正式工具文档](../../../../scripts/acceptance/cpar-new-build-acceptance.md)。
本轮预检 [current-runtime-preflight.json](current-runtime-preflight.json) 确实因旧 actual source
返回 FAIL/wrong_runtime_version，且无 `--execute`、未读 client secret、未发推理。

## 范围和归集

[组合清单](combination-readiness.json) 继承旧 ledger 的 198 基础格、24 Kiro capped、12 条件性
metadata、49 扩展最低准备行，保留旧来源 hash 与 accepted restrictions。186 applicable 基础格
因当前前置条件 BLOCKED，12 个 Web 工具限制格 inapplicable/NOT_RUN；不是把全批业务判为回归失败。
声明未知行保留 actual_declared=null；读到额外声明后必须扩展，49 不是自动封闭全集。

未部署、未启停服务、未发布配置、未恢复账号/改权限、未真实推理；远程 CI、push、工单回填/关单
NOT_RUN。历史读取和 TLS 探测是当前前置条件证据；它们不是新构建/生产验收回执。
