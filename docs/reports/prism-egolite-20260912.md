# EgoLite 账号管理验收（2026-09-12）

已使用 EgoLite（macOS，Chromium 152）操作本地真实 gateway 的正式嵌入 Prism，完成本轮
账号目录与身份显示复验。没有启用 fixture 后端。发现并修复 Codex 重新授权弹窗仍展示
内部测试 ID 的遗漏；重新构建 gateway 后在同一 EgoLite 任务空间复验通过。未部署生产。

## 实际操作与结果

| 检查 | 本轮证据 |
| --- | --- |
| 管理员登录 | 两套本地网关均从账号密码表单登录；密码只从本机私有文件进入表单 |
| 真实 Grok 身份 | 61700 的既有真实 Build 账号显示授权服务返回的邮箱，主列表无随机 ID；详情、重新授权入口可打开并取消 |
| 六类分组 | 54694 的合成状态有 5 个普通账号与 3 个原生账号；API、Codex / ChatGPT、Claude、Kimi、Kiro 各一组，Grok 内 Web / Console / Build 各一组 |
| 统一格式 | 八条账号均使用同一 AccountList；桌面表格、手机纵向行，无 Grok 独立卡片组件 |
| 搜索与来源 | 邮箱搜索只返回对应已加载账号；无匹配时不显示账号行。Autoreg 仅标来源；不把导入标记当姓名 |
| 接口连接 | 实际打开 Responses 连接详情，显示协议、合成主机名和配置停用状态；说明接口用途与配置状态不等于正在使用。Grok 说明渠道池关系 |
| 账号状态写入 | 合成 API 账号停用成功，刷新后重读仍停用，再启用恢复；仅修改未发布草稿，配置 revision 20→22 |
| 添加入口 | 检查现有 9 个渠道选项；7 个显示导入表单，缺少对应提供商的 Anthropic 兼容与 Grok Official 引导先配置提供商 |
| 首次授权入口 | Grok Build 的“授权登录”进入无名称字段的授权向导；关闭返回导入页，未点击启动真实授权 |
| Codex 重新授权 | 列表直达与详情内入口均显示邮箱，弹窗不再显示内部 ID；账号详情显示 Codex，技术 ID 默认折叠 |
| 尺寸与辅助偏好 | 1440×900、1280×720、390×844 无横向溢出；浅/深色、减少动态效果/透明度及增强对比度已实际检查 |
| 面板与键盘 | 桌面连接详情宽 520px；手机面板宽 366px、左右各 12px；Tab/Shift+Tab 保持面板内焦点，Escape 可关闭 |
| 会话清理 | 本地服务重启并刷新后回到登录；重新登录后主动退出，直达账号 URL 仍被锁定，账号文字与密码清除，localStorage/sessionStorage 均为空 |

## 发现与修复

原 `OAuthWizard` 直接把 credential ID 拼进标题；共用 `CredentialSheet` 也会把历史
资源名和短编号当作标题。现在账号目录把后端投影的身份与真实渠道名传给详情和授权向导，
其他详情入口使用已有元数据邮箱；缺失时仍明确表示未提供身份。技术 ID、API 参数、
缓存键、OAuth 流程及权限边界不变。

本次源码基于 `b369700`，修复与本报告在同一后续本地提交。相关前端类型检查和 13 项
E2E 通过，包含列表/详情两个授权入口以及共用详情原有功能。契约同步无变化，Prism
静态门禁、`cargo build -p gateway --bin gateway` 和
`node scripts/check-management-spa.mjs` 通过：122 个操作、四文件、两次构建一致。
没有重复运行与这次展示修复无关的 Rust 全套测试。

## 证据与范围

[脱敏验收收据](evidence/prism-egolite-20260912.json)记录分组、尺寸、会话、真实身份存在性
及修复后读回。以下截图全部来自合成账号的真实网关，未保存真实账号邮箱或秘密：

- [1440px 浅色账号目录](evidence/prism-egolite-20260912/accounts-1440-light.png)
- [390px 深色 Grok 分组](evidence/prism-egolite-20260912/accounts-390-dark.png)
- [修复后的 Codex 账号详情](evidence/prism-egolite-20260912/codex-detail-fixed.png)

本地预览为 `http://127.0.0.1:54694/admin-ui/`，测试结束时服务仍运行，需要本机管理员
登录并选择既有合成草稿。其状态位于临时目录，不是远程设备可访问的生产服务。
EgoLite 任务空间在验收结束后按技能要求关闭。

此次没有新增真实 OAuth grant、真实 Provider 请求或生产修改。真实 Grok 邮箱来自
[上一轮实际续期及 userinfo 获取](prism-authorization-identity-20260912.md)，本轮检验其
正式浏览器显示；不能据此声称重新跑完所有渠道的首次与重复授权。只有不含身份资料的
Web / Console SSO 时仍显示“未提供账号身份”；Claude/Kiro 尚未接通的首次网页 OAuth
以及 Kimi 独立授权入口不在这次验收中冒充已实现。

Chrome 扩展占用的历史工具限制仍如前述报告所记；用户这次指定 EgoLite 后，已由实际
EgoLite 操作补齐浏览器复验。上线仍需处理上一轮报告注明的 Grok 紧凑凭据 v2 回滚兼容性。
