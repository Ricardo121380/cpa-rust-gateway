# 渠道元数据修复发布 — 2026-09-27

## 当前结论

用户明确确认发布后，`333eae57ce33e3417bb6218410021cdfe4683201` 已部署到现有 Oracle `new-vps` CPAR 服务及 `https://cpar.142857142.xyz/admin-ui/`。这次发布包含 Kimi 元数据、导入绑定、普通 OAuth 额度、Kiro 及 native Grok 修复累计变更；不能称为仅一个前端补丁。

- 原版本/兼容回滚：`a715d6b952042d2e7c2775c0c7ab91895ee26e59`；schema 31，无新增数据库迁移。
- 实际二进制 SHA-256：`78fac8bc13a9e819d987184b199f1135e80cc85046ea7db226607934696ff410`。
- 停止到就绪 639ms；公网健康、CSP、匿名管理访问拒绝、四文件哈希、当前进程二进制均通过。
- 保留管理员、8 个账号、2391 条历史事件、704 条账本及有效模型/Key 权限。未改 DNS/Caddy/Autoreg，未发推理请求。

## 发布证据

- [正式门禁](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36309652738)：精确 revision Fast + Full supply-chain success。
- [双架构签名构建](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36309655679)：ARM64/x86_64 success；本机独立验证 manifest、SBOM、收据及 Cosign 身份。
- 回滚产物重新校验，按旧运行的实际 `codex/prism-v4-delivery` 签名身份验证；未沿用历史脚本里另一条分支。
- 生产副本置于独立网络 namespace，旧版→新版→回滚→新版通过；管理员库、账号/凭据、历史与权限保留，新增合成终态可在回滚版本继续处理。
- 隔离签名二进制真实管理员初始化、改密、CSRF、退出撤销及重启验证通过，只使用合成凭据。
- 机器证据：`evidence/cpar-channel-metadata-release-20260927/`。远端受限备份/演练目录 `/var/tmp/cpar-m4-metadata-20260927-333eae5`，包含敏感副本，不得公开或带入代码仓库。

## 真实元数据结果（与发布成功分开）

| 渠道 | 当前结果 | 限制/下一步 |
| --- | --- | --- |
| Grok Build | 1 份账号返回 3 个额度窗口；身份已存在 | 另外 2 份返回 unauthorized，未自动重新授权 |
| Grok Console | 两份额度读取均返回 unauthorized；适配器模型列表 6 项 | 6 项是静态适配器支持，不是账号级目录成功；身份仍未观测 |
| Kimi | not_connected，启用 OAuth 账号没有接口绑定 | 只读定位到唯一同 owner、启用的官方 Responses 接口且绑定为零；已准备单条绑定修复，待配置变更授权 |
| Codex | 额度 busy；目录保留 authentication 失败记录 | busy 不能直接证明临时网络拥塞；确切凭据租约不可用也映射到该状态。未重新授权 |
| Krill/API | 两个接口各 21 项 fresh 目录，分页读取成功 | API Key 不提供通用邮箱或订阅额度；未做推理 |
| Kiro/Claude/Grok Web | 生产当前没有对应账号可验 | 本地测试不能替代真实渠道成功 |

Build 两份已保存目录分别 1/4 项，本次读取时均 stale，不宣称已完成最新完整目录发现。未手动刷新配置或更改模型权限。

用户登录后，EgoLite space19 已完成本次生产账号页面检查：8 份授权正常加载；Build 列表与详情显示真实 3 个额度窗口；Console 授权失败有明确提示，6 个模型标注为适配器支持；Build 目录显示真实保存的 4 个 exact ID、观测时间及“陈旧”；Kimi 明确提示未连接接口。

1440×900、1280×720、390×844 的额度详情页面无横向溢出；桌面详情宽 520px，手机左右 12px。查看了桌面/手机浅色额度截图和手机深色 Console 模型截图，未见文本裁切。关闭、切换分类、主题及返回操作正常。本次仅覆盖账号相关页面，不据此声明八工作区全量验收。发现 Kimi 额度页同一未连接提示重复出现，记录为文案层级待优化；Codex 的 busy 错误归因限制仍保留。

截图保存在本地 `output/channel-metadata-release-20260927/browser/`，包含真实账号身份，不提交截图。脱敏布局证据已落盘。TaskSpace 已 finish，保留已登录 Build 额度页供用户查看。

## 回滚与待办

回滚已演练：仅将 current 切回上面的 a715d6b 并重启 CPAR，保留最新数据库和滚动凭据，不恢复旧快照覆盖新数据。备份完整性与权限检查均在切换流程内执行。

待完成：获准后补齐 Kimi 单个缺失绑定，再检查身份/额度/目录；核对失效授权；Kimi 重复提示与 Codex 额度错误归因仍待优化。当前状态为“发布成功，部分真实渠道仍受阻”，不是“全渠道全部修好”。
