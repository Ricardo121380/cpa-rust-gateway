# Prism 域名访问交付（2026-09-10）

用户要求先明确项目状态，再通过既有 `cpar` 子域从其他设备人工验收。本轮已接通该域名的
HTTPS Prism 入口；Cloudflare 名称服务器及已有 DNS 解析已确认，无 DNS/CDN 配置变更。
完整公网地址由任务交付消息提供，仓库样例仍使用占位域名。

## 项目状态与本轮范围

V4 的14个栏目和解锁页、有效模型/候选管理、会话正确性、新请求计费和限定 TTL 维护已有
正式实现与本地真实网关验收，见 [V4交付](prism-v4-delivery.md) 和 [操作UAT](prism-v4-user-acceptance.md)。
这不等于所有生产渠道、浏览器和历史账务均完成验收。

仍然保留的项目事项：87条旧格式历史 Usage 尚待兼容迁移；此前2次准入503的内部根因未完全定位；
生产测试的4条新账本没有价格配置，保留 unpriced/null。Safari/Firefox 实机及用户其他物理设备未宣称通过。
请求趋势、延迟分析等仍是已约定的下一期范围。本次域名接入不冒充修复这些事项。

## 已部署改动

- 当前代码：`c7cfd2c0d8771187775d2cd846cb49a8cf536046`，上一版为 `18f29a3`。
- ARM64 binary SHA256：`fc3dcf2e2ae6251acdab72c831d9075154ed614cd0ba5be755f5234e06a4c1f9`。
- `serve --management-origin` 显式指定一个 canonical HTTPS origin；省略仍为原 HTTP loopback 默认值。
  管理 Key、CSRF、实际 peer 的 loopback 限制保持有效，不信任转发头来推断浏览器来源。
- 仅修改既有 CPAR 站点：根路径302到 `/admin-ui/`；`/admin-ui`、`/admin` 及其子路径转管理 listener；
  保留数据面 fallback。没有向代理注入密钥或改写 Origin。两 listener 仍绑定127.0.0.1。
- 基础 systemd 模板未改。生产使用专用 drop-in，保留原 argv/proxy 参数和 `%d` 凭据目录。
- SPA 四文件哈希与上一版一致，前端视觉、路由和 HTTP DTO 未改变。没有手改生成代码。

实现约定：[CR-PRISM-DOMAIN-001](../change-requests/CR-PRISM-DOMAIN-001.md)。
操作说明：[域名访问交接](../handoffs/prism-domain-access.md)。

## 本次证据

| 验证 | 实际结果 |
|---|---|
| CLI与装配 | 10项 deployment 测试通过，包含新增的 HTTPS origin 解析与真实安全中间件装配 |
| 既有鉴权回归 | 4项 management-security 测试通过 |
| 代码门禁 | gateway 全目标 Clippy、格式、docs、基础 systemd 静态检查通过 |
| 本地真实进程 | 显式 HTTPS origin 的草稿写入/重读成功；匿名、错误 Origin+伪造转发头、缺少CSRF拒绝 |
| 签名发布 | [34456280809](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34456280809)：ARM64/x86_64成功；独立Cosign及仓库artifact/SBOM/receipt验证通过，服务器SHA匹配 |
| 正式门禁 | [34456283504](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34456283504)：Fast和供应链全部通过 |
| 服务器隔离 | 仅lo、无默认路由的network namespace，cpa-gateway身份；四文件、HTTPS origin写入/重读及拒绝路径通过 |
| Caddy | 候选配置validate通过；对比排除目标站点后其他配置一致，仅规范化自动生成的group编号及配置文件自动隐藏路径 |
| 正式公网 | TLS、根重定向、HTML/JS/CSS、CSP通过；带真实管理凭据的专用草稿写入与重读成功；匿名/错误来源/缺CSRF拒绝 |
| 既有数据面 | 公网healthz通过，现有Client Key读取 `/v1/models` 成功，4个可见模型；本轮新增Provider推理请求为0 |
| 浏览器 | Codex内置浏览器通过公网HTTPS显示真实解锁页，无捕获到的警告/错误；不以此代替其他设备的人工确认 |
| 保留状态 | schema22、87条历史待处理、原active配置ID/revision保留；Autoreg进程及active时间一致；未操作Jakarta |

最终切换的停止至健康恢复脚本计时为1734ms。保留未发布验收草稿 `prism-domain-check-c7cfd2c`，
可以查看其创建与读回；未发布或替换现有active配置。此次账务不使用重新执行的旧测试数量作证据。

## 失败记录与恢复

隔离脚本首次将API的带引号revision直接转整数，收据生成失败；改为读取并核对持久active revision后复验通过。
首次域名切换的验收脚本又错误假设不存在的Config Version返回404，现有契约实际为409
`management_lifecycle_conflict`；流程据此自动回到18f29a3。改为通过版本列表判断是否需要创建专用草稿后，
完整复验通过。没有把失败计作通过，没有改变API契约或对409自动重放写请求。

现场目录 `/var/backups/cpa-rust-gateway/prism-domain-20260910` 保留原Caddy配置、argv、凭据与一致DB备份，
候选/签名收据、隔离日志/收据、首次回滚收据、最终切换/公网验收收据。两版都是schema22，
恢复时只回退binary、专用drop-in和该站点路由，不能使用旧V4的down22流程。
本机value-free脚本和收据位于 `output/prism-domain-20260910/`。生产秘密未写入聊天或仓库。

## 人工验收入口

现在使用既有 `cpar` HTTPS 域名即可，根路径自动进入Prism；其他设备不需要SSH隧道。
此前本轮建立的本机18181临时SSH隧道已关闭，公网入口继续独立可用。
使用现有Management Key和CSRF Token解锁，刷新后重新输入。Client Key和Grok Provider凭据不能代替管理凭据。

建议先在active版本只读浏览账号、模型、诊断和账本，再在独立草稿检查维护操作。当前配置发布后的
运行装配仍遵循既有重启serve要求，界面确认提示与项目交付说明保持一致。
用户在手机、平板、其他电脑上的实际操作结果尚待人工确认；本次只宣称入口和上述自动验收已完成。
