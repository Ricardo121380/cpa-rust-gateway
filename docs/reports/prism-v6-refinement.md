# Prism V6：K3 max 设计与生产资源名称

**V6 与最终名称修正均已上线。** 直接刷新原域名 [Prism 管理台](https://cpar.142857142.xyz/admin-ui/#/unlock)，使用原管理员账号密码登录。当前运行 `8a1b5377594919690c53d35a390a0c992aaf41d3`；名称展示、V6 布局与最后的空格名称修正分别为 `c839321`、`6b4e9a7`、`8a1b537`，均在 `codex/prism-v4-delivery` 分支。

## 本轮改变

- 用业务名称或可读名称替代列表中的 P12 阶段前缀、测试时间戳和长哈希，以短编号区分同名对象。完整 ID 在技术详情和复制中保留，API 参数、模型 exact ID、路由和审计引用保持原值。
- 总览改为分层指标和运营／资源索引分栏；计费修复状态醒目，口径说明按需展开。
- 账号池按上游分组，减少逐行重复信息；同一账号的不同绑定仍分别展示，计数明确为绑定。
- 顶栏、侧栏、内容层统一为银白／石墨的磨砂质感；内部使用细分隔线，减少层叠边框。保留三面 chrome 和共享工作区模糊，行与小组件没有额外模糊。
- 配置 ID、revision、来源集中到一个技术详情区；手机配置列表纵向排列。原有草稿、差异、校验、发布、回滚和确认流程保留。

本次不清理历史请求、账本或认证数据；之前确认的 11 个测试草稿清理结果保留。没有改写稳定资源 ID，也没有通过隐藏记录解决命名问题。

## 确实使用的设计模型

通过 OpenDesign MCP 既有项目 `prism-gateway-console-redesign-a735`，调用 Pi 的
`cc-switch-kimi-for-coding/k3:max`，使用 `frontend-design` skill。该路由是用户在原 OpenCode Go
额度耗尽后明确选择的 Kimi for Coding K3；`max` 通过 Pi 的实际模型参数解析生效。

运行 `101cbe62-d3f6-425b-b171-ecacb2e37285` 已结束。K3 的原型和规格经 MCP 读取，Codex
修正了原型的一处表格拼接类型错误，再保存回读，并在内置浏览器检查全部 14 个入口。
[来源与哈希](../design/prism-v6-provenance.json)、[可运行原型](../design/prism-liquid-glass-v6.html)、
[生产适配说明](../handoffs/prism-opendesign-v6.md)记录了完整来源及适配边界。

正式实现采用 K3 的分节与布局，并延续用户要求的统一通透材质。模型原稿中的合成待办、
固定授权、账号合并和一对一模型路由关系没有迁入真实 API 或生产默认数据。

![K3 原始总览](../design/prism-v6-evidence/k3-overview-light.png)
![正式真实网关总览](../design/prism-v6-evidence/1440-light-overview.png)
![修改前账号列表](../design/prism-v6-evidence/before-accounts-dark.png)
![正式真实网关账号列表](../design/prism-v6-evidence/1280-dark-accounts.png)
![手机账号池](../design/prism-v6-evidence/390-dark-accounts.png)
![手机配置管理](../design/prism-v6-evidence/390-dark-versions.png)

## 全部入口

以下为本次正式嵌入式 React 应用的真实 gateway 检查，不是独立 HTML 或 mock API 的替代证明。
每个管理入口均在 1440×900、1280×720、390×844 下检查浅／深色，共 84 个页面视图，
另有 6 个登录页视图；[逐页记录](../design/prism-v6-evidence/browser-audit.json)保留标题、路由、
尺寸、错误及溢出检查结果。

| 栏目 | 正式路由 | 本轮落点 |
|---|---|---|
| 总览 | `#/` | 分层指标、计费状态、资源索引分栏 |
| 请求与失败 | `#/monitoring` | 统一表格与来源名称、保留筛选／尝试／诊断深链 |
| 用量分析 | `#/usage` | 统一数据面与分组名称、六类 token 置信度保留 |
| 计费与价格 | `#/billing` | 价格引用名称、真实处理状态与异常 |
| 账号池 | `#/accounts` | 上游分组、绑定独立行、短编号与技术详情 |
| 上游 | `#/upstreams` | 业务名称、子资源与凭据引用 |
| 模型目录 | `#/catalog` | 目标名称与有效模型上下文 |
| 模型与路由 | `#/models` | 原模型 exact ID、完整资源枚举与工作台 |
| 访问控制 | `#/access` | 组名称与原始授权值 |
| 运行诊断 | `#/runtime` | 目标名称、账号、候选与 Explain 引用 |
| 出口策略 | `#/egress` | 策略、代理池、节点与绑定名称 |
| 配置版本 | `#/versions` | 技术信息折叠、手机纵向布局、完整生命周期操作 |
| 审计与备份 | `#/audit` | 资源引用可读，原始事实可查 |
| 设置 | `#/settings` | 统一控件、原外观／辅助／会话功能 |
| 管理员登录 | `#/unlock` | 简洁账号密码表单、真实会话及首次改密 |

## 本次验证

下列完整本地验收对应 V6 主体 `6b4e9a7`；随后仅有名称格式辅助函数的修正，增量验证与最终发布证据列在下一节，不把先前的完整测试数量当作补丁重跑结果。

- 前端类型检查、**262 项单测**、**128 项完整 E2E**通过。
- `node scripts/check-management-spa.mjs` 通过：**111 个权威操作**、生成客户端无漂移、
  CSP 与四文件双构建一致。构建仍为 `index.html`、`assets/main.js`、`assets/vendor.js`、`assets/index.css`。
- **3 项 Rust 嵌入测试**通过，包含精确资源、加固响应头和管理／数据面路由隔离。
- `scripts/prism-v4-local-acceptance.py --priced --browser --browser-flow --preview` 通过，
  使用临时合成状态、真实 `gateway serve` 和 TLS loopback mock Provider。
- 真实浏览器 **10 阶段操作**通过：授权模型→草稿→候选 CRUD→授权→校验／发布→重读与审计；
  账号动作、失败→attempt→target／Explain、配置差异、生命周期旧确认拒绝、会话清理均在链路中。
- 实际请求经过数据 listener、持久事件、计费物化和账本；重启 checkpoint 幂等、既有 TTL 维护也通过。
- 内置浏览器另核对手机账号详情：宽 366px、左右各 12px，Esc 后焦点返回触发按钮。
- 实测 Chromium **151.0.7922.34**。Safari／Firefox 仍仅有已有能力回退分支测试，不宣称实体浏览器验收。

一次早期完整 E2E 中，标签切换尚未完成时测试填写了即将卸载的旧表单。
增加“失败归因标签已选中”的界面检查后，9 项监控用例和最终全部 128 项 E2E 通过。
原型的表格错误同样记录并修复，没有把失败结果计入通过证据。

原始日志位于 `output/prism-v6/`：`v6-final-unit.log`、`v6-final-e2e.log`、
`v6-final-spa-gate.log`、`v6-final-rust.log`、`v6-final-real-gateway.log`。
当前可查看[本地真实应用](http://127.0.0.1:57092/admin-ui/#/)；这是合成验收环境，
与生产管理员账号无关。临时状态和仅本人可读的预览凭据保存在该次验收目录。

## 正式发布

首轮 V6 `6b4e9a7` 的双架构签名构建、正式门禁和隔离 ARM64 验收通过，原服务已切换，停止到恢复健康 1239 ms。公网四个资源及 CSP、CLI 读回、schema 22、原管理员认证库、2217 个事件、589 条账本、87 条历史未修复记录与 11 个测试草稿归档保留。没有访问生产密码，也没有改变 Caddy／DNS／Autoreg。

上线后的只读命名复查发现三个人工显示名用空格分隔（如 `P12-06 official ChatGPT Codex`），还残留数字阶段号。补充规则后，37 处线上身份／名称引用的展示文字全部消除 P12 与数字阶段前缀，原始 ID 未更改。这里的 37 是引用检查次数，包含重复引用，不是独立账号数量。

最终 `8a1b537` 已完成以下检查与发布：

- **3 项名称单测、6 项身份／详情 E2E**，以及 111 操作权威契约、生成客户端和四文件双构建门禁通过。增量日志为 `output/prism-v6/spaced-names-{unit,e2e,spa-gate}.log`。
- [双架构签名构建](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34504176897)与[正式门禁](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34504181369)均在该完整提交上成功，包含 Fast、Full supply-chain 和 Required delivery gate。
- 下载 ARM64 成品后，以限定仓库工作流身份和 GitHub OIDC issuer 独立执行 Cosign，结果 `Verified OK`；随后通过 manifest、二进制、SBOM、OCI 与签名回执结构核验。
- 在 `new-vps` 的临时合成状态中启动真实 ARM64 gateway，验证四个嵌入资源逐字节一致、首次改密、错误 Origin／密码／CSRF 拒绝、CLI 兼容、草稿写入／重读、退出撤销及重启后的密码与会话语义。没有使用生产密码或发送 Provider 请求。
- 原 CPAR 服务切换并恢复健康耗时 **1226 ms**。运行进程的二进制 SHA-256 为 `e7f7f3abe81df0ab807efad2bbdcdccc26af9daf68b04b31eeeb0dac9cc12e22`，与签名产物一致。公网四资源摘要、CSP、匿名拒绝、CLI 读回、loopback 绑定和 `/healthz` 均通过。
- 当前配置、schema 22、管理员认证库、切换前 **2217 个持久事件、589 条账本、87 条历史未修复记录**均保留，11 个测试草稿归档记录不变。2217 是事件数，不冒充请求数；87 条历史记录也不因本轮界面改动宣称解决。
- 内置 Chromium 重新加载正式 HTTPS 登录页，账号／密码／登录控件与布局正常；[公网截图](../design/prism-v6-evidence/public-login.png)已归档。公网未登录进入生产管理页面；完整认证功能由本地及隔离真实网关验证。

[脱敏发布回执](evidence/prism-v6-production-20260911.json)保存最终提交、签名与门禁、资源摘要、隔离检查和保留性结果。原始产物及日志位于 `output/prism-v6/names-fix/`。线上私有备份与回执目录为 `/var/backups/cpa-rust-gateway/prism-v6-names-20260911`，其访问权限保持 0700／0600。

本轮回滚点是上一份 V6 二进制 `6b4e9a7abad2d1df8a746d1f023fc88d69b5be6a`，SHA-256 `3c3d3d7468fa862db15886fb374f18ac467b8d75035c4c333ea21fe5280b0682`，保留于 `/opt/cpa-rust-gateway/releases/6b4e9a7abad2d1df8a746d1f023fc88d69b5be6a`。若本轮验证失败，预设恢复仅切回二进制并重启 CPAR；本次没有触发回滚，也没有恢复数据库、覆盖管理员认证或改变 DNS／Caddy／Autoreg。历史数据备份用于恢复保障，不用于覆盖运行中的新增数据。

请求趋势／延迟分析、完整英文、生产真实 Provider 自动巡检、在线备份恢复等仍按原范围留待后续。
