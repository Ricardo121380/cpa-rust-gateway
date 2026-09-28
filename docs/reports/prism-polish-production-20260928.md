# Prism 优化与渠道接入合并发布

日期：2026-09-28。生产目标：Oracle `new-vps` 的既有 CPAR 服务与 [Prism 管理界面](https://cpar.142857142.xyz/admin-ui/)。

**已上线并完成登录后的只读浏览器验收。** 当前运行代码为 `b0cf36e387b774439acc23830221389eb3ac74d0`，schema 31 未改变。停止到恢复健康耗时 644ms；17:45（Asia/Shanghai）最终复查服务 `active/running`、自动重启 0、公网与回环 health 均为 200。

## 授权与发布范围

用户明确要求：“上线吧，顺便把另一个会话的改动一起上线。” 本次按当前生产版本 `f283494` 之后的已提交源码差异合并发布。

| 来源 | 提交 | 内容 |
| --- | --- | --- |
| 另一会话 | `5497f92579fceb4291cff9a70b77327e798dbdc8` | API 渠道创建服务与接口、返回账号导入、原生渠道接入指引、多 Kimi Coding 服务显式选择及接入状态延续。见[渠道接入报告](cpar-channel-onboarding-repair-20260927.md)。 |
| 本轮前端优化 | `1a2c3458f82723810d932254a65faba3c93b5b4c` | 珠光默认按钮与显式主操作、单核对入口任务岛、矩形徽章、总览说明与导航/分段选择动效。见[本地验收报告](prism-frontend-polish-20260928.md)。 |
| 发布门槛修正 | `b0cf36e387b774439acc23830221389eb3ac74d0` | Kimi 目标 query 解析改为等价 `let…else`，保留原有失败响应，通过 Clippy。 |

发布分支为 `codex/prism-polish-release-20260928`。生产使用上述准确代码提交；发布完成后的证据文档提交不改变已运行的二进制。原有无关未跟踪文件未纳入本次源码提交。

## 发布门槛

| 检查 | 结果 | 证据与范围 |
| --- | --- | --- |
| 前端完整测试 | PASS | 60 文件、443 测试。类型、契约、严格 CSP、四文件输出及确定性构建已通过。 |
| 正式 Fast / supply-chain / Required | PASS | [准确提交的正式门槛](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36401343994)；Rust workspace 1389 passed、0 failed、12 ignored，121 组。 |
| 双架构签名构建 | PASS | [ARM64 与 x86_64 发布构建](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36401348928)。 |
| 独立签名及供应链核对 | PASS | 两种架构均以预先确定的分支身份与 issuer 验签，并校验 artifact、manifest、SBOM 和 OCI。兼容备用版本也重新验签。[主版本](evidence/prism-polish-production-20260928/verified-artifact.json) / [备用版本](evidence/prism-polish-production-20260928/verified-fallback.json)。 |
| 生产副本隔离演练 | PASS | previous → candidate → fallback → candidate；schema 31 保持一致，新格式事件、checkpoint、规范请求身份、账号、管理员和权限保留。[演练](evidence/prism-polish-production-20260928/rehearsal.json)。 |
| 真实网关隔离认证生命周期 | PASS | 独立合成状态下验证初始化、origin、错误口令、首次改密、CSRF、旧会话撤销、草稿写入/读取、退出和重启。[认证证据](evidence/prism-polish-production-20260928/isolated-auth.json)。 |
| 生产切换与资产字节 | PASS | 公网四份资产与本地已验收构建的 SHA256 一致；CSP、认证边界与回环监听检查通过。[生产回执](evidence/prism-polish-production-20260928/production-receipt.json)。 |
| 最终生产健康 | PASS | 运行提交、current 链接、实际进程二进制哈希、接收请求状态、公网与回环 health 均核对通过。[最后复查](evidence/prism-polish-production-20260928/final-health.json)。 |

首次候选 `1a2c345` 的正式门槛因 `account_channels.rs` 的 `clippy::manual_let_else` 失败，尚未尝试部署。修正并提交 `b0cf36e` 后重新完成全部正式检查和签名；失败候选的签名构建已取消。[首次记录](evidence/prism-polish-production-20260928/first-attempt.json) / [测试计数](evidence/prism-polish-production-20260928/test-summary.json)。

实际 ARM64 进程二进制 SHA256：`0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b`。

## 数据与运行状态保留

- 原有 8 份账号授权、管理员存储与有效权限保留。
- 2391 条历史事件、704 条账本记录和 11 条隔离事件保留；没有补造缺失的请求归属或费用。
- 当前配置与最新轮换凭据保留；schema 31 保持一致。
- 本次生产验收没有配置、账号或密钥写入，没有发起真实 Provider 推理，也没有修改 DNS、Caddy 或 Autoreg。

## 生产浏览器验收

使用 EgoLite TaskSpace 24「Prism combined release 20260928」，用户在真实页面登录后继续验收。全程保持同一页面与内存会话。

| 范围 | 结果 | 说明 |
| --- | --- | --- |
| 生产登录页 | PASS | 1440×900、1280×720、390×844 × 深浅主题，共 6 个状态；输入与主按钮在视口内，文档无横向溢出。[登录检查](evidence/prism-polish-production-20260928/public-browser.json)。 |
| 登录后页面布局 | PASS | 总览、账号、提供商、模型、API 密钥、请求日志、用量与费用、设置，8 页 × 3 尺寸 × 2 主题，共 48 个状态；标题与主操作宽度、文档溢出及导航底板归位检查通过。 |
| 接入工作区 | PASS | 选择 OpenAI 兼容渠道、进入导入表单，再打开“创建服务与接口”；编辑工作区 6 个尺寸/主题状态无输入裁切，取消后正常返回。没有提交表单。 |
| 按钮与徽章 | PASS | 四个资源主页各有一个主入口，普通行操作为次级；徽章为 7px 圆角。深浅切换后等待颜色过渡结束再核对截图。 |
| 只读交互 | PASS | 窄屏菜单正常开关；总览 7 天与平均耗时切换正确，结束时恢复 24 小时/请求量。 |
| 运行错误与生产写入 | PASS | 浏览器 Runtime/console 错误 0、HTTP 错误 0，观测的请求均为 GET，生产 mutation 请求 0。[脱敏结果](evidence/prism-polish-production-20260928/authenticated-browser.json)。 |

主代理查看了深浅主题的桌面/窄屏总览、账号，以及接入工作区代表截图。真实账号工作区截图仅保存在本机受限输出目录，未提交到仓库；仓内证据仅含数量、几何、颜色和状态。原主题与 1793×959 窗口尺寸已恢复，空间已完成，保留总览结果页供用户查看。

## 验收限制与回滚

- **真实完整接入 / 推理：NOT_RUN。** 本次只打开与取消生产接入表单；没有实际创建接口、导入账号、进行 OAuth 交换或调用模型。另一会话的完整接入逻辑仍由本地模拟、单元与 Rust 集成测试支持。
- **生产草稿发布路径：NOT_RUN。** 当前生产没有待应用草稿，未为验收生成草稿。任务岛的单入口、校验与确认发布边界已在本地 fixture 验证。
- **实际动画帧率：NOT_RUN。** 底板最终位置与辅助偏好已验证；EgoLite 的帧节流不能证明实际显示流畅度。
- Toast 编排、面板入场、异步按钮宽度冻结仍未实施，维持[前端报告](prism-frontend-polish-20260928.md)中明确的交付范围。

兼容备用为已签名 `f28349406b7645572e85c95d411a769720092751`，schema 31。已演练保留最新数据、轮换凭据并只切换兼容二进制；不得恢复陈旧数据库。后续用户发布新配置或改变账号后，回滚前应重新核对兼容性。

本次受限发布、切换前备份及演练目录：`/var/tmp/cpar-m4-prism-20260928-b0cf36e`。当前二进制目录：`/opt/cpa-rust-gateway/releases/b0cf36e387b774439acc23830221389eb3ac74d0`。
