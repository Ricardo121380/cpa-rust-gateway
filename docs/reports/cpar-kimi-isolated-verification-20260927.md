# Kimi 隔离诊断与发布前修复 — 2026-09-27

## 结论

**隔离诊断完成，进入正式发布验证。** 真实身份及 4 个模型读取正常，真实额度
接口返回空 JSON 对象。用户补充该账号为 Free；对照 CPA 与官方 Management
Center 后，成功空响应按普通空态处理，不再把数值额度作为发布前置条件。
这不证明所有 Free 账号都没有额度，也不将真实数值额度标为已验证。

## 实际过程与证据

- 新增 `kimi-metadata-check`：必须显式隔离标记，复用真实 runtime/catalog
  facade，不启动监听、后台维护、OAuth 续期或推理。提交 `c40fab3`。
- Oracle `/var/tmp/cpar-kimi-isolated-20260927` 独立构建候选；每次使用新鲜的
  主机内 SQLite backup 与 credential 副本，原文件不改。隔离副本权限 0700/0600，
  检查结束后回收为 root 可读；秘密未离开 VPS。
- 初次程序启动因源目录 0700 阻止 service account 遍历而失败，未发上游请求；
  改为复制程序至私有隔离目录，不放宽生产凭据目录权限。
- 三次成功执行检查，全部 `identity_present=true`、`profile_available=true`、
  `model_count=4`、`quota_available=false`。第一轮定位为成功 JSON 无可识别窗口；
  第二轮仅记录结构/类型，确认空对象；第三轮加入现有 CPAR 设备头对照仍为空。
- 临时结构探针只存在隔离构建中，不进入正式代码、日志或契约；不输出正文值。
  无效的设备头实验已从工作树撤去，未冒充修复。
- 三份回执均确认生产 PID、release symlink、配置指纹前后不变。线上仍为
  `333eae57ce33e3417bb6218410021cdfe4683201`；推理请求 0，授权续期尝试 0。
- 脱敏回执：[检查 1](evidence/cpar-kimi-isolated-20260927/check-1.json)、
  [结构检查](evidence/cpar-kimi-isolated-20260927/check-2.json)、
  [设备头对照](evidence/cpar-kimi-isolated-20260927/check-3.json)。

## 本地修复

- 原解析器只支持 TypeScript 官方客户端的 `usages.limit_*`。增加 Python
  官方 OAuth 客户端的 `usage` 周汇总和 `limits[].detail` 计数格式兼容；不把缺值、
  非法数值、零分母转成 0%。两种格式同时存在时去重并优先保留直接比例。
- 新增 `empty_response` 安全错误分类；前端说明官方未提供数据，不再只有泛化的
  “未知”。契约通过权威 OpenAPI → sync-contract 更新。
- 账号删除入口 `be5f30e` 仍待与最终验证通过的修复一起发布，不删除生产账号。

## 验证

- PASS：6 项 provider Kimi 单测，含两种格式、重复窗口、缺值、非法值及空响应。
- PASS：8 项前端额度状态测试，空响应不显示假 0%。
- PASS：隔离命令解析和未标记目录拒绝两个定向测试。
- PASS：provider/gateway 严格 Clippy；权威契约同步、TypeScript 与前端四文件双构建嵌入门禁（153 operations）。
- 未观测：此账号的真实数值额度；已验证成功空响应，不编造 0% 或套餐限制。
- NOT_RUN：本批正式签名发布、生产切换、生产新 UI 验收、真实推理。

## 下一步

1. 空响应显示“上游暂未提供额度数据。”普通状态；HTTP/网络/解析失败仍报错，
   新错误优先于旧空态。用户的 Free 描述不硬编码到账号 DTO。
2. 完成针对性回归后执行正式签名构建、生产副本回退演练、发布及 EgoLite 复查。
3. 不扩大模型/Key 权限，不改变 DNS/Caddy/Autoreg，不恢复旧令牌数据库。

参考行为：官方 Management Center 的 `src/features/quota/providers/kimi/data.ts`
不按 Free/付费过滤账号；`KimiQuotaBody.tsx` 对空 rows 渲染 empty_data。
CPA 的 `internal/runtime/executor/kimi_executor.go` 不提供 Free 数值额度替代值。
