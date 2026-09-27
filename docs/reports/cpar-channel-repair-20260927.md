# 跨渠道账号修复进度 — 2026-09-27

已完成第一批（187f172）及第二批 Kiro 本地实现；全渠道目标仍未完成，未部署、未调用真实 Provider。

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

## 仍待完成的明确缺口

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
