# 跨渠道账号修复进度 — 2026-09-27

最新发布状态：已获授权并部署 `333eae5`；真实元数据部分成功、部分受阻。详见 [生产发布记录](cpar-channel-metadata-production-20260927.md)。以下保留开发阶段证据，不能把历史“未发布”当成当前状态。

已完成三批本地修复：187f172、8d7d39c 及本报告第三批。新增 Grok 三渠道额度读取、Kiro CLI 目录、附加额度与前端接线已通过本地验证。**尚未发布；真实渠道验收未完成，不能称为全渠道可用。**

## 已实现

- Codex/Claude/Kiro/Kimi Coding 导入保留本次 prepare 返回的精确接口，在同一配置任务中绑定服务端返回的去重凭据 ID。API 的“稍后连接”仍保留，重复导入不重启用停用绑定，绑定失败阻止后续批次发布、不自动重放。
- Claude 账号授权/导入默认目标增加 `/models`；旧 canonical 目标缺失路径时，只在本次 revisioned draft 修复。禁用接口或显式不同路径仍冲突，不覆盖操作员选择。“AI 提供商”创建入口已有目录路径，本轮未改该入口。
- Codex/Claude OAuth 额度新增服务端有界读取，按账号租约、当前运行代、出站策略隔离，5 分钟成功缓存。账号列表和详情消费新契约。Codex 的窗口周期来自响应，未知不伪装成 5 小时或 7 天；Claude 仅白名单窗口。无模型权限、计费或调度策略变更。
- 非 OpenAI 格式凭据不再因通用元数据解析失败直接返回 400；渠道额度由运行适配器决定。
- 列表/详情元数据读取共享两个并发槽，避免单页多账号挤满服务端；排队时取消不会发出请求，失败会释放槽位，实际出站请求继续受查询 AbortSignal 约束。
- 修复 Vitest 只收集 `.test.ts`、遗漏 `.test.tsx` 的测试配置。此前报告关于 TSX 额度测试覆盖的结论不足，本轮重新实际执行。

## 本轮验证

- 前端 81 tests / 16 files 通过，包含 Kimi 和通用额度 TSX 测试。
- 管理接口集成 40 tests 通过；额度解析 2 tests 通过。
- strict Clippy、TypeScript/确定性四文件嵌入、crate boundaries、contract references 通过。
- EgoLite space15：模拟 Codex 导入→完成→一个 Responses 连接→详情额度 25%；1440×900、1280×720、390×844 截图检查，手机弹窗无文字贴边/裁切。该流程只使用开发 fixtures，不证明真实账号或生产服务。
- 证据 `output/channel-repair-20260927/`。生产配置/凭据/历史账本未改，真实推理使用 0 次。

## 第一批结束时的缺口（历史记录，最新状态见第三批）

1. Kiro 现已接通有界 GET 身份/基础额度 reader（见第二批）。CLI 目录仍未接入；试用/奖励等附加额度未完整投影；企业 profileArn 型及需要不同协议的上游仍需真实验证，不能保证官方响应均含邮箱。
2. Grok Console SSO quota 需要 DPoP token + proof 的完整读取链，不能复用 Build Bearer。Web 的 rate-limits / gRPC-Web quota 与现有 fixture decoder 不是同一生产协议，仍待接入及故障回归。
3. Grok Build 目前套餐不等于额度；billing/credits 读取和数值单位仍待实现。
4. Web/Console 的参考 grok2api 本身使用内置静态 catalog，并不存在已确认的完整上游动态目录。CPAR 需显示“适配器支持列表”及来源限制，不能编造账号级动态模型列表。
5. 新 runtime quota 出站路径尚未真实验证；完整导入→运行池→目录→真实元数据验收仍待。前端列表/详情已共享两个并发的读取队列，等待项随查询取消；跨浏览器的服务端繁忙仍明确返回不可用，不伪装额度。
6. 本轮 live_quota_error 将未接入与未连接合并表示，尚未完成更细能力契约。真实目录与额度失败不能只靠文案判定原因。

## 后续依据

- Claude models：https://platform.claude.com/docs/en/api/models/list
- Claude/Codex quota：https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/constants.ts
- Grok Console DPoP/usage：https://github.com/chenyme/grok2api/blob/main/backend/internal/infra/provider/console/quota.go
- Web quota：https://github.com/chenyme/grok2api/blob/main/backend/internal/infra/provider/web/quota.go
- Kiro 协议线索（第三方，未实测）：https://github.com/QiXia881/xkiro.rs/blob/master/src/kiro/endpoint/ide.rs

这些是后续实现证据，不能冒充已部署或真实渠道通过。本轮不重复申请发布许可；前一轮发布确认仍待答复，当前“开始修复”不解释为上线。

## 第二批：Kiro 元数据读取（本地）

- `provider-kiro/src/account_usage.rs` 构造 IDE/CLI 分开的固定 GET URL，保留当前区域及 AI_EDITOR/KIRO_CLI origin；OAuth/API Key 使用各自凭据接口。无 POST 回退、区域切换、推测 profileArn 或自动重试。
- CLI 元数据 target 与自动模型发现 target 分离：额度可读取不代表 CLI 动态目录已支持，更不开放任何模型。
- 新建 Kiro canonical policy 同时允许推理 host 和该区域 `q` 元数据 host。旧 canonical policy 仅在显式 prepare 的 revisioned draft 中升级；非 canonical/custom policy 不被静默扩权。生产现存配置本轮不迁移。
- `live_quota` 添加可空 email/plan/原始 used/limit/unit 和 partial。只采用真实响应 email，复用身份过滤；不把 userId 当邮箱。精度字段优先，缺失用量或零分母不造 0%。最多投影六个基础资源；试用、奖励或无法投影的条目明确 partial，不等同全部剩余额度。
- 账号列表通过按配置代际隔离的缓存消费邮箱/套餐，观测变更更新分页 revision；详情保留数值单位。缓存不进入凭据密文，不改变计费/授权/调度。
- 当前验证：Kiro 请求构造 1 项；管理接口 40 项，追加 canonical policy 升级/自定义冲突与缓存身份搜索断言单独复跑；额度解析 3 项；前端 82 项；TypeScript/四文件嵌入和 strict Clippy。均为本地合成数据，未做真实 Kiro 请求或新一轮浏览器验收。
- 协议依据为 xkiro.rs 的 `endpoint/{ide,cli}.rs` 当前公开实现（第三方，不是官方保证）。旧 IDE egress 如不允许 codewhisperer host 仍按策略拒绝，不能绕过出站策略读取。
- Grok Build/Console/Web 真实额度 reader 仍未完成；本次没有将其解析器宣称为已接线。


## 第三批：原生 Grok 与剩余元数据接线

### 实现与证据位置

- `apps/gateway/src/native_account_usage.rs` 与 `native_account_usage/grpc.rs`：Build 固定 billing credits URL；Console SSO → DPoP exchange → usage；Web REST auto/fast 加可选 gRPC weekly。固定 HTTPS 主机、禁止重定向、有界超时和 64 KiB 响应，无推理或自动重试。缺失/零分母使用率为 null；不推算未经观测的重置时刻。可选 weekly 失败（包括 Cookie 路径不覆盖）保留 REST 成功窗口。
- `management_resources/native_accounts/usage.rs`：管理鉴权、精确账号密文快照及 revision 前后复核，并发有界；账号变更后的晚到响应返回 409，无缓存头。403、401、无效响应、未接入、繁忙分别返回安全状态。
- `NativeQuotaEvidence.tsx`、`AccountsPage.tsx`：列表和详情共享精确账号/revision 查询缓存与双槽读取队列；详情展示原始单位、部分观测和具体失败原因。
- `provider-grok/src/{console_responses,web_production}.rs`：支持模型来自推理适配器同一映射表；账号详情标明“适配器支持的模型”。它不是动态上游目录，也不等同该账号权限；Build 保留真实动态目录路径。
- `provider-kiro/src/catalog.rs`、`runtime/catalog_refresh.rs`：CLI 使用区域 management JSON RPC，先获取唯一 ACTIVE KIRO profile 再完整分页；未取得可靠 profile、分页异常或结果越界即失败，不使用猜测 ARN、不回退 IDE，不发布部分结果。canonical 出站策略仅在显式 prepare 草稿时补 management 主机，不改自定义策略。
- `management_resources/account_quota.rs`：Kiro 基础、ACTIVE trial、bonus、已观测 overage 分开投影，总窗口有界；超出/异常标记 partial，未知用量不造零；精准额度字段优先。
- `runtime/account_quota.rs`：明确区分认证失效、上游拒绝、出站策略拒绝、响应无效与服务繁忙。权威契约同步生成，共 153 operations；修正过程中发现并恢复登录接口原有匿名 security 声明，完整前端回归已复跑。

### 本次验收

| 层级 | 结果 | 当前证据 |
| --- | --- | --- |
| 前端 | PASS | 435 tests / 60 files；TypeScript 通过 |
| HTTP | PASS | 40 管理集成测试；新增 native usage 晚到 revision 冲突测试单独通过 |
| Provider/runtime | PASS | Kiro 目录 3、运行分页 3、普通额度解析 4、Grok crate 28；native parser/可选读取回归见日志 |
| 静态/契约/嵌入 | PASS | strict Clippy、21 crates 边界、107 契约引用；153 operations 确定性双构建、四文件门禁 |
| EgoLite | PASS，仅 fixtures | space17：Console 合成 SSO 导入 → 身份 → 列表 25% → 详情额度与适配器模型；1440×900、1280×720、390×844；手机深浅色详情无横向溢出 |
| 真实 metadata | NOT_RUN | 未用本地模拟结果证明私人接口/WAF兼容 |
| 生产发布/配置迁移 | NOT_RUN | 未修改线上账号、配置、模型权限和历史数据 |
| 真实推理 | NOT_RUN | 本轮使用 0 次，不重置先前额度 |

EgoLite 截图与命令日志：`output/channel-repair-20260927/native/`。验收 fixture 服务已停止，已有用户 gateway 未动。输出目录含本地证据，不纳入代码提交。

### 仍需关闭的验收边界

1. 发布后用现有账号核对真实身份、目录及额度响应；当前仅有第三方协议源码证据与合成回归，不能保证上游私人接口或 WAF 行为。
2. Kiro API Key 的模型目录没有已验证协议；Builder ID 若拒绝 profile 枚举或返回多个可选 profile，当前明确失败，不硬编码共享 ARN。需要对应脱敏真实响应/官方协议才能补齐，不能称这类账号动态目录完成。
3. Web 额度明确 partial，未覆盖媒体和产品子池；未实测 Statsig/出站会话兼容性。不会以聊天/weekly 窗口冒充完整剩余额度。
4. 现存生产 Kimi 连接、Claude models path、Kiro canonical egress 的修复需发布后在可审查配置事务中执行；本地实现没有偷偷迁移或开放模型。
5. Console used/limit/remaining 若出现相互矛盾，现有解析拒绝越界 remaining，但尚无真实样本证明三者一致性规则；实测需核对，不以推测改写数值。

本轮代码与本地验收完成不等于整个跨渠道目标完成。生产发布仍等待明确授权；授权前继续保留现有服务与数据。
