# 全站资源名称优化上线记录（2026-09-12）

按用户“上线让我人工验收”的明确授权，已将 `743276393486b672703e662d4798b3dc56897ab3`
部署到 Oracle 新加坡现有 CPAR 服务，替换 `5b92e15`。
正式入口：[Prism 账号管理](https://cpar.142857142.xyz/admin-ui/#/accounts)。
继续使用原管理员账号与密码，刷新旧页面后重新登录即可。

## 本次上线内容

全站列表、详情、资源选择、编辑与确认框使用可读名称，移除旧测试前缀和生成编号；
账号显示已观测身份，提供商显示实际渠道，接口显示协议/主机。
精确原 ID 仍用于 API、URL、原始导出及明确的“复制内部引用”操作。
代码与本地三尺寸验收见 [实现报告](prism-resource-presentation-20260912.md)。

此前整理的活动配置 `production-accounts-20260912` 保持不变。上线后鉴权读回确认：
Codex **1 份授权、1 条运行连接**，全部运行连接 8 条，原生 Grok 账号 5 个。
原管理员库、2,229 条历史事件、593 条账本、87 条待处理物化记录及 11 条测试草稿退役审计保留。
本次没有新增数据库迁移、凭据格式变更、配置发布或数据清理。

## 发布验证

| 检查 | 本次结果 |
|---|---|
| [签名构建](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34694716386) | ARM64、x86_64 均通过，绑定提交 `7432763` |
| [正式发布检查](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34694717977) | Fast、供应链及 Required delivery gate 均通过，提交一致 |
| ARM64 制品 | manifest、revision、SBOM、receipt 和独立 Cosign 身份/签名校验通过 |
| ARM64 实际程序 | 合成账号初始化、首次改密、Origin/CSRF、注销、重启和四个嵌入文件校验通过 |
| 生产副本 | 网络隔离、服务用户运行；新版启动与旧二进制回退通过，schema 均为 25 |
| 正式切换 | 停服务至就绪 **1,025 ms**，进程 SHA256 与已验证制品一致 |
| 公网与管理 API | 服务器及本机网络的四文件哈希一致；health、CSP、匿名拒绝及鉴权目录/运行状态读回通过 |
| 浏览器 | EgoLite / Chromium 152 刷新正式登录页，正常显示，已保留给人工验收 |

实际进程 SHA256：`3bcf31b555cdca80ecc0b5a83b30c1c240bc8bd67d5f432439516bc3b4902916`。
本次生产浏览器检查到登录页，没有读取或代填管理员密码；登录后页面的功能依据本地真实网关验收、
线上鉴权数据读回与新前端文件一致性分别记录，不冒充已经完成用户的人工验收。

## 人工验收与范围

可以重点核对“全部账号”和“运行状态”中的 Codex 单例，以及账号详情、AI 提供商、路由/候选、
请求详情和资源筛选中的可读名称。已有浏览器页面需要刷新并重新登录。
两个 Console 账号的邮箱获取限制未由此次发布解决，本次没有再次发起资料查询、刷新、授权或推理。
没有变更 DNS、Caddy、Autoreg 或 CPAR 的 loopback 监听；原服务后台任务继续按既有规则运行。

## 备份与回滚

服务器私有目录：`/var/backups/cpa-rust-gateway/prism-resource-presentation-20260912`。
已保存切换时凭据，停服务后通过 SQLite backup 保存一致的控制库和管理员库；旧 release 保留。
隔离演练确认配置、授权、原生账号、事件、账本、身份观察表及管理员库保持一致。

本次回退目标 `5b92e15` 与新版均使用 schema 25，因此只需在新的备份后切换旧二进制并启动，
保留当时最新的数据库和令牌。**不要运行上一批的 schema 25→24 降级脚本，也不要恢复陈旧数据库。**
这一回退已在隔离副本实际执行；生产未执行回退。

证据：[脱敏发布记录](evidence/prism-resource-presentation-production-20260912.json)、
[正式登录页](evidence/prism-resource-presentation-production-20260912.png)。
本机编排脚本位于 `output/prism-resource-presentation-release-20260912/`，哈希已纳入证据。
