# A-01 固定修复范围的独立两轴评审

日期：2026-10-07。固定起点 `602874b9ed9943c146c8d12f77ab1cc1adbfa167`；最终产品修复提交 `42bc473f2eba8c34484b54aaf09fff455aa13c9c`。唯一源码提交：`fix(cpar): protect A-01 Codex callback account identity`。

以下分别保留两个单轮只读探子的终态结论，作轻量格式整理。任务身份是发布复核 A-01／CPAR-05／#14，范围及文件见 [scope](scope.md)、[review.json](review.json)。两轴均未重新审查整个 A／B／C／D，均未运行测试或改文件。

## Standards

终态：complete。Coverage **COMPLETE（11/11）**；Standards **PASS**。完整 diff 已读；**硬违规 0，可行动 heuristic 0**。

- `crates/gateway-control/src/management_mutation_service.rs:1267–1303`：新入口绑定 version、credential ID、credential revision，以原 AAD 解密并核对身份，再交给既有持久化 CAS；秘密与事务责任符合 `docs/adr/ADR-0005-versioned-control-plane-repository-service.md:24–30`。
- `crates/gateway-store/src/account_identity.rs:147–164` 的身份规则由原 enrollment 迁移；`crates/gateway-http-actix/src/management_resources/codex_enrollment.rs:94` 与 `crates/gateway-http-actix/src/management_resources.rs:6923` 共用。正常 renewal 在后者 `:7030` 仍调用原持久化入口。与 `docs/adr/ADR-0099-account-workflow-evidence-and-retirement.md:25` 一致，无新增依赖方向、持久化模型或公开契约。
- `crates/gateway-http-actix/tests/managed_resource_inventory.rs:2356–2567` 覆盖 5 条 callback、38 组合；秘密比较使用 boolean。人工 fixture 符合 `docs/06-development-plan.md:4059–4060`。现有运行日志确认 HTTP 49/49、callback 5/5。
- 源码检查点报告正确区分开发验证、固定提交 Full、两轴审查与发布状态。审查时 PENDING 骨架符合 `docs/06-development-plan.md:294–297`，不算漏做；探子当时未取得 Full 终态回执，**不计 Full PASS**。

覆盖：5 个 Rust 文件；本目录的 `issue-14.md`、`issue-57.md`、`issue-9.md`、`red-foreign.md`、`scope.md`；A-01 源码检查点报告。已读取指定标准及完整 smell baseline；未扩大重审其他发现。

## Spec

终态：complete。Coverage **COMPLETE（11/11 文件完整 diff）**；Spec **PASS（A-01/P2 实现范围）**。

**实现缺失／部分 0；scope creep 0；错误实现 0。** 全部 11 文件及必要相邻调用／事务已点验，结论不扩展至其他 A／B／C／D 发现。

- 身份：`issue-14.md:16` 要求“身份未知先核对、冲突拒绝覆盖”；ADR-0099:25 要求可比较 account ID 或 email。`management_mutation_service.rs:1267` 的新入口按 version／ID 查找，核对旧 revision，以精确 AAD 解密，拒绝后尚未进入 seal／写入。注册 callback 在 `management_resources.rs:2805,6923` 实际调用该入口；同渠道检查仍在 `:6673`。
- 规则复用：`account_identity.rs:147` 与移除的 enrollment guard 逻辑一致。旧稳定 ID 缺失／冲突不能靠相同邮箱通过；旧已观察 email 也必须匹配，未新增显示名匹配。此处“缺失”指在旧稳定 ID 已存在时新材料缺少该 ID，未改变旧契约允许的 email-only 观察比较。
- 并发：`scope.md:16` 要求晚到回调不得覆盖合法并发获胜状态。旧 revision 贯穿新入口与原 persistence；Draft 使用 `control_plane.rs:2088,2184` 的 immediate 配置 CAS；Active 使用 `:2606` 的 version／ID／upstream／revision 条件更新。Disabled 保留在 service `:1344`；Active／Draft revision 合同保持。
- 回归：`managed_resource_inventory.rs:2212,2356,2387,2432,2486,2534` 穿过注册 HTTP、管理鉴权、磁盘 SQLite、真实 AEAD，覆盖 38 组合、关系、其他账号及并发 winner。`:2323,2344,2478,2523` 用 boolean 比较敏感材料；未断言审计或全库零变化。
- 首次登记走 enrollment `:109` 的原 import 分支；正常 HTTP renewal 在 `management_resources.rs:7030` 保持旧 persistence，后台 `apps/gateway/src/credential_refresh/ordinary.rs:418–446` 保持 durable claim。已有日志为 HTTP 49/49、callback 5/5。

探子明确留下主代理收口项：`scope.md:20` 要求本地 Full、独立 Standards／Spec、精确提交与本地报告交付；审查时报告为 PENDING。Spec 判断不代替 Full、最终报告或发布结论。

必要相邻阅读：`control_plane.rs`、`credential_refresh.rs`、后台 `ordinary.rs`、ADR-0099、ADR-0005、行为合同 §14、P12 Codex 管理契约。

## 主代理收口边界

主代理已核对关键身份与 CAS 原文，取得 `42bc473f` 的 [Full 终态回执](full-receipt.json)，并汇总 [执行证据](execution.md) 和最终报告。后续交付 diff 只有报告证据，产品代码／测试不再变化。探子覆盖仍限定冻结的 11 个文件；后续文档由主代理检查，不计入探子覆盖。审查覆盖 COMPLETE 与实现 PASS、执行 PASS、发布 BLOCKED 分别记录。

Standards：0 项发现，最严重问题无；Spec：0 项发现，最严重问题无。
