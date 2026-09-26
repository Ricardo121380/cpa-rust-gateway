# CR-M4-OWNED-BUILD-REASONING-001

日期：2026-09-27。状态：本地实现及专项验证通过；待正式发布门禁和真实闭环。

## 证据与边界

M4 第4次真实 Pi/Build 请求（f1e576c，grok-4.5，low，512输出上限，无自动重试）在 reasoning item 的 metadata 校验遇到非空 encrypted_content，终态 UpstreamProtocolError。Pi 本机 SDK 请求 include=reasoning.encrypted_content；现有 Build 构造器透传此 selector，但输出与历史解析拒绝密文。现有 P6 replay 存储只有独立 API，不能当作已装配的归属验证。

修复属于 M2/M4 已承诺的原生历史与真实客户端闭环。仅修改数据面；管理 OpenAPI/生成客户端无变化，不新增渠道、目录权限、推理额度或持久历史策略。

## 决定

- `cpar_reasoning_v1.` 为网关自有封套，不是上游 cipher，也不是既有 compaction locator。客户端只需原样携带 reasoning item。
- 复用部署 SecretStore 的 XChaCha20-Poly1305，独立 AAD domain，绑定已认证 Client Key、公开模型与 item ID；认证载荷包含 exact upstream model、配置版本、Provider/Upstream/Endpoint/Route/Candidate、Credential ID/revision、签发时间及30天期限。
- provider cipher 最大64KiB，wire封套最大128KiB，继续服从请求/item整体已有上限。所有 Debug/错误均脱敏。签发时不写 SQLite；store:false保持不存储响应历史，必需请求/用量元数据记录不受影响。
- 语法解析仅承认有界网关封套，不是认证。运行装配在租约前验证所有封套；混合绑定、异主、篡改、过期或配置/模型不匹配拒绝。历史修订号取最旧值，精确租约仍要求原凭据或 M2 已证明的同 grant 轮转范围；不依据邮箱、不自动换账号或重放。
- Build adapter 在已选租约范围内解封后才能构造请求；普通 builder/generic encoder 不得透传封套。无归属上下文的 Build decoder 继续拒绝原始密文。
- JSON/SSE 通过同一原生 lifecycle 校验；起始 item 不携密文，完成 item 携封套，终态与已完成 item 的原始 cipher 必须一致。真实 reasoning 文本、工具调用及结果保留。
- stored/compaction/WebSocket 与封套组合时，保留原 continuation kind 的能力检查，额外要求 Build+Reasoning 能力。

## 验证与回退

必须覆盖 JSON/SSE 分块、原生工具回传、store:false无响应落盘、跨Key/model/item/credential、混合grant、篡改/TTL/重启、轮转有无证明、默认拒绝、终态cipher漂移、原续接能力保留及正式运行装配。

不迁移 schema，不更换生产主密钥或历史数据。相同密钥重启可继续解封；移除旧密钥会使旧封套不可用，不提供跨密钥绕行。兼容旧二进制回退保留最新数据库/轮转凭据，但旧版不支持新封套，新产生的对话需要重新开始；不得为回退剥除密文或恢复旧数据。真实验收仍计入同一12次总额。
