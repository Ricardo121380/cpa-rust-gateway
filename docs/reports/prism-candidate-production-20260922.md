# Prism 候选正式发布 — 2026-09-22

## 结论

Oracle 现有服务已部署签名提交 `ca75a44351374aac62a40b13ba878a3487345db6`，schema28不变。人工验收入口：[Prism](https://cpar.142857142.xyz/admin-ui/#/unlock)，继续使用现有管理员。

本轮交付累积前端工作区改进、用量口径与会话验收，并修复重复点击账号当前视图清空筛选的问题。相对上一生产版本，无新增后端生产逻辑或数据库迁移。

## 验证

- 本地前端49文件393单测通过；最终串行 fast gate 1282条 Rust 通过、9条忽略，契约、四文件构建、嵌入、边界与供应链检查通过。
- E2E首次293通过5失败；修复后第二次298通过1失败，最后修正绑定文案断言并单跑相关 batch-d 6/6通过。不是一次299/299通过；具体失败成因和修复见[整体验收报告](prism-candidate-integration-20260921.md)。
- [正式门禁35621951153](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35621951153)通过；[双架构签名构建35621976728](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35621976728)通过。独立核对ARM64/x64的签名、SBOM、manifest及提交。
- ARM64 SHA256：`25389fcaabb301bb8ef3619bec682ac516bce2d7311f05470df83d19a57e318f`；前端资源版本 `2be9c06023713b03f58fe5c6`。
- 本地真实 gateway + loopback TLS mock 完成登录改密、目录分页、手动开放模型、界面签发受限密钥、越权拒绝、请求终态及缺价账本物化；会话撤销三个时序通过。14入口×三尺寸×深浅色共84项结构观测无JS异常/溢出，不等同每种业务操作均验。
- 生产副本断网演练 fallback→candidate→fallback→candidate通过，权限与历史数据保持。正式切换至就绪1022ms，7个账号、2229条历史事件、593条账本记录、管理员存储及有效权限保留。
- 公网四文件哈希、CSP、鉴权隔离通过。EgoLite沿用发布前标签页正常刷新，无清缓存；1440×900、1280×720、390×844均可见登录表单，加载新版资源，无JS异常、无横向溢出；桌面及手机截图人工查看通过。

## 边界与回滚

本次生产浏览器只验登录页，生产受保护接口通过服务器本地只读核对；完整业务链路在隔离真实网关验证。未新增真实 Provider 推理或官方授权同意，不将mock结果当作真实渠道授权结果。

此前会话并发空画布未复现、尚未归因，保留风险，不宣称修复。发布前曾遇公网vendor传输不完整；后续完整传输及页面加载成功，未修改网络配置，也未据此推定应用根因。

兼容二进制回退为 `b518d925fdaf03c92d235fa6ca4a7c5565d04043`，保留最新schema28状态，禁止回滚历史库。远端 `/var/tmp/cpar-candidate-ca75a4435137` 保存受保护切换备份与回执；未触发回退。未改DNS/Caddy/Autoreg。

## 证据与人工验收

[上线回执](evidence/prism-candidate-production-20260922/production-receipt.json)、[隔离演练](evidence/prism-candidate-production-20260922/rehearsal.json)、[EgoLite三尺寸结果](evidence/prism-candidate-production-20260922/browser-after.json)、[桌面截图](evidence/prism-candidate-production-20260922/public-login-1440.png)、[手机截图](evidence/prism-candidate-production-20260922/public-login-390.png)。

建议人工依次检查账号搜索与视图切换、渠道授权表单、提供商与模型目录、密钥模型权限、请求及用量口径。真实官方登录若需本人同意仍由本人操作；本报告不宣称全部历史计划和所有渠道人工验收完成。
