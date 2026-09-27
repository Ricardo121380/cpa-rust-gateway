# 跨渠道账号修复进度 — 2026-09-27

本轮完成第一批本地实现；全渠道目标仍未完成，未部署、未调用真实 Provider。

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

1. Kiro 身份/额度没有生产 reader。公开 GET 与 POST 协议并存，跨 `codewhisperer`/`q.<region>` 主机，不可把现有运行接口 URL 或 GET 参数直接套进 POST。需落地分认证类型的受控 reader、出站策略、完整数值与邮箱投影后验证；不能保证所有官方响应包含邮箱。
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
