# Kimi Coding 账号闭环修复 — 2026-09-27

## 结论与生产事实

本地修复已实现并完成针对性验证，尚未发布；不能据此宣布生产账号已可用。
生产只读检查发现 1 个已启用 Kimi 账号、1 个 api.kimi.com 接口，但没有启用的账号绑定。
原授权弹窗丢弃 prepare 返回的 endpoint_id，导致凭据已保存但没有进入接口账号池，目录 worker 因此没有该账号的发现目标。
身份与额度也没有对应的官方元数据读取实现。此次未重新授权、未读取/导出 Provider secret、未执行真实推理，既有调用额度未变化。
生产基线 a715d6b，schema 31；本地原基线 67f15be。旧历史请求、账本与 11 条隔离事件未更改。

## 实现

- `web/prism/src/features/accounts/KimiDeviceDialog.tsx`：首次授权保存 prepared endpoint，成功后先创建精确凭据绑定再校验/发布。已有绑定不重复创建；重新授权不修改原连接/启用状态；终态失败不重放授权轮询。
- `crates/provider-openai-compatible/src/kimi_metadata.rs`：白名单解析真实邮箱、电话、用户名、官方套餐及各额度窗口。电话保持上游脱敏；不把 opaque user_id 当身份，不把未知额度记为零。
- `apps/gateway/src/runtime/kimi_metadata.rs`：固定官方 `/coding/v1/me` 与 `/coding/v1/usages`，复用出站策略、连接池、精确可租用凭据。单请求 8 秒、64 KiB 上限，HTTP 入口整体 10 秒并有并发上限；配置切换保护；完整结果 5 分钟缓存，部分失败 30 秒。缓存只存在当前运行代，不改变凭据或权限。
- `crates/gateway-http-actix/src/management_resources.rs`：既有管理鉴权 metadata GET 扩展 kimi/kimi_error，Cache-Control no-store；不让浏览器携带上游秘密。
- 账号目录、运行状态与详情接入观测身份，目录分页含 metadata revision，变更后拒绝混合快照游标。
- 账号详情额度页显示官方时间窗口使用比例、重置时间、观测时间和失败状态；模型页按精确账号/接口列出原始 ID，超过 100 项链接完整目录。目录与手动开放权限仍分离。
- 权威 OpenAPI 已更新并执行 sync-contract；四文件嵌入约束不变。

官方依据：
- [Kimi profile reader](https://github.com/MoonshotAI/kimi-code/blob/main/packages/oauth/src/managed-userinfo.ts)
- [Kimi usage reader](https://github.com/MoonshotAI/kimi-code/blob/main/packages/oauth/src/managed-usage.ts)
- [Kimi model integration](https://github.com/MoonshotAI/kimi-code/blob/main/packages/oauth/src/managed-kimi-code.ts)

## 本轮验证证据

本地证据目录：`output/kimi-metadata-20260927/`（不提交原始输出）。

| 验证 | 本次结果 | 证据 |
| --- | --- | --- |
| Kimi 元数据解析 | 2 通过：脱敏/白名单、缺失与零区分 | provider-tests-final.log |
| Kimi HTTP/授权回归 | 5 通过：管理鉴权、身份搜索、游标冲突、授权渠道归属及终态 | http-tests-final.log |
| 前端账号/上游单测 | 67 通过，13 文件 | frontend-tests-final.log |
| TypeScript、构建、契约、确定性四文件嵌入 | 通过，152 operations | embed-check-final.log |
| Rust strict Clippy | 通过 | clippy-final.log |
| crate boundaries / contract references | 通过，21 packages / 107 references | 本轮命令输出 |
| EgoLite 真实浏览器、开发 fixtures | 完成模拟授权→一份绑定→身份→额度→两项合成模型 | browser-layouts.json、quota-*.png、models-390.png |

EgoLite 覆盖 1440×900、1280×720、390×844；手机左右 12px，无横向溢出；深浅色、关闭/退出交互通过。发现额度重置文字贴边后已修正间距。浏览器使用合成邮箱与模型，绝不表示用户真实 Kimi 账号已返回这些数据。

## 待生产执行及停止条件

需要明确覆盖此次 Oracle 发布与配置修复的授权后，执行以下有边界的步骤：

1. 按现有签名 release/回滚流程部署精确提交；保留管理员、凭据、历史事件、账本和现有 model/Key 权限。包含此前尚未发布的 observation-stage 文案修正，勿声称只有本次一个提交。
2. 重新读取当前 active、revision、lifecycle（不使用本报告旧值）；枚举 Kimi 所属 upstream/endpoint/credential。只为已确认的单个启用 OAuth 账号补其同 owner 的官方接口缺失绑定。存在多个候选、已停用绑定或用户并发修改时停止自动修复，不随意选择或重启用。
3. fork active、添加绑定、校验、按 expected-active/lifecycle 与 If-Match 发布、runtime apply；保留上一 active 供回滚。不改 credential 内容，不创建新授权，不放开模型权限，不触碰 DNS/Caddy/Autoreg。
4. 只读真实 `/me`、`/usages`，刷新该账号目录并逐页核对原始 ID。记录是否返回邮箱或脱敏电话、套餐及各额度窗口，不在报告写个人身份或秘密。失败不得记为零或空成功。
5. 核对客户端 Key 有效模型前后不扩权。官方目录可能声明 `anthropic` 或 `response` 协议；当前准备的接口是 Responses。必须按实际目录核对协议，不能把目录可见等同于所有模型都可推理；协议不匹配另行修复后才开放。
6. 使用 EgoLite 检查生产账号列表、运行状态和详情。此次不做真实推理；若以后需要，另按剩余授权额度与合成内容约束执行。

目前未验证：正式 gateway 出站传输对真实 Kimi profile/usages 的响应、生产已授权账号目录/协议及真实推理。缓存重启后需重新观测；手动“重新读取”在缓存有效期内可返回带时间戳缓存。不得用本地 fixture 覆盖这些缺口。
