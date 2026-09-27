# M4 收尾：阶段标签与渠道就绪核对

日期：2026-09-27。生产仍为 `a715d6b` / schema31。本轮仅修改本地文案及只读核对生产；没有发布、更新凭据、变更模型权限或调用Provider。真实推理累计仍为10/12。

## 修复与验证

M4-UI-01已在本地修复：`web/prism/src/features/monitoring/AttemptTimeline.tsx` 将共用标签“失败阶段”改为“观测阶段”，同时覆盖请求详情与失败诊断；错误原因、请求终态和重试决定保持原义。**尚未部署，线上仍保留原文案。**

- sync-contract无契约变化，check通过。
- monitoring/model与RequestHistory两组既有测试共25项通过；没有为纯文案新增镜像测试。
- 类型检查及四文件生产构建通过；`node scripts/check-management-spa.mjs` 的契约与双构建一致性检查通过。现有主包体积告警仍存在，未为消除告警破坏四文件约束。
- 纯文案没有新增浏览器或真实推理验证；上一轮生产视觉证据不冒充新文案已经上线。

## 当前渠道条件

通过既有管理鉴权机制在Oracle管理listener执行GET；仅使用管理认证，不读取Provider秘密。分页有界，配置版本前后保持一致，输出只保留渠道、状态、数量和原始模型名。证据：[渠道快照](evidence/cpar-reliability-m4-20260926/followup-channels.json)、[清单与水位](evidence/cpar-reliability-m4-20260926/followup-readiness.json)。

| 渠道 | 本次只读观察 | 下一步条件 |
|---|---|---|
| Codex | 1个账号；运行态expired，目录missing/authentication | 本人重新授权，之后核对身份、运行态和目录；未恢复前不推理 |
| Grok Build | 3个账号绑定：1 available、2 expired；一份fresh目录4模型，另有stale目录1模型 | 已通过的四轮不重复消耗额度；需要使用过期账号时分别重授权 |
| Grok Console | 2个账号当前均available，身份未观测；这是与旧快照的差异 | 旧CredentialUnauthorized仍是有效失败证据；本地available不等于SSO已被上游接受。验证前须明确目标授权/账号选择，不能靠随机换号试到成功 |
| Krill | 2个接口均available，各自fresh目录21模型；两份完整缓存均不含gpt-5.5，公开模型仍含gpt-5.5 | 先确认管理员希望开放的目录exact ID及对应Key权限；不能自行替换模型或扩大现有Key权限。缓存缺失不能单独证明上游永不支持该ID，但现有证据不足以选它验收 |
| Claude/Kimi/Kiro/Grok Web | 完整管理清单7项中无独立账号 | 配置对应有效授权后才具备真实验收条件；Krill目录里的模型不能当作这些独立渠道的账号 |

计费源与checkpoint均2391，11条隔离记录保留，状态为needs_repair；未把隔离状态误写为已补账或新积压。

## 调用与发布安排

本轮Provider调用0次，剩余2次保留。当前没有具备完整前置证据的新目标，因此不盲试、不自动重试、不新增费用。后续有有效授权/明确模型权限时，再登记具体目标和一次尝试；官方登录仍由本人完成。

文案修复可随下一次批准的完整签名gateway发布交付，不能单独复制dist上线。本轮没有发布请求，不为一处文案重启生产服务。
