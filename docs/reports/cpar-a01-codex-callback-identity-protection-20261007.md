# A-01 Codex 授权回调身份保护修复

日期：2026-10-07，Asia/Shanghai。**A-01 / P2：RESOLVED_LOCAL**。实际 callback 回归、独立 Standards／Spec 及新源码的本地 Full 均已通过。本轮交付本地修复检查点；项目发布仍 **BLOCKED**。

## 任务身份与当前基线

范围是 2026-10-05 发布复核的 A-01，来源 CPAR-05／GitHub #14、父规格 #9、ADR-0099；与 CPAR-01／#10、AC-01 历史凭据修复不同。固定起点与验收边界见 [scope](assets/cpar-a01-20261007/scope.md)。

| 对象 | 本轮核对结果 |
| --- | --- |
| 起点 HEAD | `602874b9ed9943c146c8d12f77ab1cc1adbfa167`，分支 `codex/prism-v4-delivery` |
| 原候选源码 | `80f5838902eb56be6dccc3ce047eb227d6c18e6c`；后续到起点仅为报告变化，无新的 A-01 修复 |
| 本轮修复源码 | `42bc473f2eba8c34484b54aaf09fff455aa13c9c`，父提交即固定起点 |
| 原始工作区 | 用户修改的 AGENTS.md 和无关未跟踪内容保留，不进入本轮提交 |
| 工单 | [#14](assets/cpar-a01-20261007/issue-14.md)、[#9](assets/cpar-a01-20261007/issue-9.md)、[#57](assets/cpar-a01-20261007/issue-57.md) 只读快照均 OPEN，无评论；未作远程更新 |
| 原审查 | [2026-10-05 review-results](assets/cpar-release-readiness-20261005/review-results.md) 和原始失败证据保留 |

适用项目规则、issue-tracker、相关最新跨界记录、发布报告、基础领域／架构资料与身份／凭据／并发契约由主代理亲读。已修改的完整代码由主代理亲读、修改和最终验证；探子只做定位与只读评审。

## 原缺陷与 RED

实际注册路径是 `POST /admin/credentials/{credential_id}/oauth/callback` → `complete_credential_oauth` → 原 `persist_oauth_credential_if_revision`。管理鉴权、版本、既有 Codex OAuth 材料与 OAuth state 均有效时，受控 exchange 返回另一稳定账号 ID，即使邮箱相同，旧实现仍进入持久化；它只有 revision/CAS，没有已有 enrollment 的 old/new identity guard。

[RED 原始回执](assets/cpar-a01-20261007/red-foreign.md) 在旧产品源码 `602874b9` 加第一条 HTTP 回归时运行：

```sh
cargo test --locked -p gateway-http-actix --test managed_resource_inventory legacy_codex_callback_rejects_different_identity_without_changing_account -- --exact
```

退出码 **101**，callback 实际 **202**，预期 **409**。失败发生在公开 HTTP 状态断言，证明报告中的覆盖缺口能在真实路由与持久化路径上触发。后续 GREEN 还检查公开状态和真实数据库材料／关系。初次编译失败、测试错误对象形状校正和首轮 Clippy 修正不计作 RED；其日志仅留在本地 output。

测试只替换已有可注入的外部 token exchange，使用合成材料；Actix 的实际资源注册、管理鉴权、磁盘 SQLite、SecretStore AEAD 和事务 CAS 均真实执行。这里使用 Actix test service，未绑定网络监听端口，也未执行浏览器或真实 OAuth。

## 最小修复与数据边界

1. 将 enrollment 的原 `same_observed_account` 规则原样移至 `AccountIdentity::same_observed_oauth_account`，两个交互式路径复用同一规则。旧稳定 account ID 存在时，新值必须一致；旧已观察 email 存在时也必须一致；旧两者都缺失则拒绝。保留 ADR-0099 既有可比较 ID／观察 email 语义，未引入显示名，也不允许相同邮箱替代一个冲突或缺失的新稳定 ID。
2. 新增 `reauthorize_oauth_credential_if_revision`：按精确 version／credential ID 读取旧记录，先检查期望 credential revision，再使用该 version／ID／upstream 的 AAD 解密同一记录，执行 identity guard。失败返回既有 **409 `management_credential_identity_unverified`**，在 seal 和数据库写入前退出。
3. legacy callback 的唯一持久化调用改为上述交互式入口。通过 identity guard 后，沿用原 persistence，并再次检查同一 credential revision；最终 Draft 配置 CAS 或 Active credential CAS 保护相同对象。核对不是放入写事务的新模型；单调 revision 和最终 CAS 防止两次读取之间合法更新被覆盖。
4. Draft 写入保持 immediate transaction 的配置 revision CAS，并使 credential／config revision 各增加一。Active 使用 version／credential ID／upstream／旧 credential revision 条件更新，只增加 credential revision，保持配置图 revision。不能将 Active 描述为“双 revision CAS”。
5. 同身份更新保留 Disabled 停用意图和 owner、连接、binding、路由、访问组、Client Key 权限及其他账号。拒绝／CAS 失败保留对应基线中的材料和关系；不删除绑定、不新建账号、不留下部分凭据写入。审计属于既有事务合同，不作“全库所有字段不变”的断言。

首次接入继续通过 enrollment 原 import 分支；显式管理 renewal 仍调用原 `persist_oauth_credential_if_revision`；后台续期仍使用 provider 的 refresh 元数据合并及 durable claim。交互式身份缺失拒绝规则未扩大到正常续期。

## GREEN 与验收矩阵

| 场景 | 组合数 | 公开行为与状态断言 | 结果 |
| --- | ---: | --- | --- |
| 不同稳定 ID，邮箱相同 | 4 | Draft／Active × 启用／停用；409 identity conflict；ciphertext、key version、credential/config revision、status 和关系不变 | PASS |
| 缺失或不能可靠比较的身份 | 24 | 6 组 old/new × 上述 4；涵盖旧无观察、新无 ID、新无原 email、email 冲突、双方未知；明确拒绝且保存原状态 | PASS |
| 同身份重新授权 | 4 | 202 complete；进程内确认保存 exact exchange 材料；符合两类 revision 合同；停用、连接及权限保留 | PASS |
| exchange 期间凭据更新获胜 | 4 | 409 credential revision conflict；以 winner 完成后的状态为基线，材料、关系及合法 Draft 连接修改均保留 | PASS |
| exchange 期间仅 Draft 连接更新获胜 | 2 | 409 config revision conflict；credential revision 未先变化，仍由最终配置 CAS 拒绝；无部分写入，winner 关系保留 | PASS |

共 **5 条 HTTP 回归、38 个组合**。秘密材料、密文及解密结果仅在测试进程内通过 boolean 一致性断言检查；运行回执和报告不输出这些值。公开 GET 和 OAuth status 与 SQLite readback 分别验证结果。测试及完整命令见 [执行证据](assets/cpar-a01-20261007/execution.md)。

| 验证 | 结果 | 提交／证据边界 |
| --- | --- | --- |
| 5 条定向 callback 回归 | 5/5 PASS | 开发验证后以 SHA256 核对 5 个 Rust 文件与修复提交一致；Full 在不可变提交重跑 |
| 管理 HTTP 集成 | 49/49 PASS | 包含首次 Codex 授权、enrollment 与既有相邻管理行为；Full 再次 49/49 |
| 正常 refresh provider 回归 | 19/19 PASS | `provider-openai-compatible runtime_credential::tests::` |
| refresh store／后台 worker | 6/6、7/7 PASS | `gateway-store credential_refresh::tests::`／`gateway credential_refresh::ordinary::tests::` |
| 原 OAuth persistence／enrollment guard | 2/2、2/2 PASS | revision 合同与旧 identity 规则 |
| 定向静态检查 | PASS | locked Clippy、fmt、source policy、crate boundaries、contract tests、secret scanner 和 whitespace |
| 新源码本地 Full | **45/45 PASS** | clean detached checkout `42bc473f`，2026-10-07 17:44:16–17:49:09 +08:00，293.398 秒 |
| Full Rust tests | **1458 passed，0 failed，12 ignored** | 汇总 Rust tests 步骤的 124 个 harness；已有需专用配置／授权的 ignored 项列于执行证据，未作为已运行验收 |
| 本地文档收口 | **7/7 PASS** | `./scripts/check.sh docs`，2026-10-07 18:02:32–18:03:08 +08:00，36.427 秒；只含本轮 staged 报告证据 |

Full 执行 `./scripts/check.sh full`，覆盖完整 Fast 和供应链门禁，包括 workspace Clippy、workspace tests、格式、文档／契约／源码／边界／秘密检查、`cargo deny` 和 `cargo audit`。前后 checkout 均 clean；只复用编译缓存，不使用主工作区未跟踪源码。[45 步回执](assets/cpar-a01-20261007/full-check.md)、[运行身份与时间](assets/cpar-a01-20261007/full-receipt.json)、[机器可读证据](assets/cpar-a01-20261007/verification.json) 绑定上述精确源码提交。供应链检查有 14 条已有 duplicate-version 告警；依赖未修改，licenses／sources／bans／advisories 均通过，未顺带升级依赖。

## 独立两轴评审

固定范围是 `602874b9ed9943c146c8d12f77ab1cc1adbfa167...42bc473f2eba8c34484b54aaf09fff455aa13c9c`，完整 11 个文件，含 5 个 Rust 文件、5 个范围／规格／RED 资料及源码检查点报告。两个单轮只读探子独立完整阅读 diff；必要相邻路径限定为首次接入、正常续期、Draft／Active CAS 与其契约，没有扩大到整个 A／B／C／D。

| 轴 | 覆盖 | 行为／规则结论 | 发现 |
| --- | --- | --- | --- |
| Standards | COMPLETE，11/11 | PASS | 硬违规 0，可行动 heuristic 0 |
| Spec | COMPLETE，11/11 | PASS，限定 A-01/P2 实现 | 缺失／部分 0、scope creep 0、错误实现 0 |

原样或轻整理的两轴结论分别保存于 [review](assets/cpar-a01-20261007/review.md) 和 [review.json](assets/cpar-a01-20261007/review.json)。探子未执行测试，原结论明确将 Full 与报告收口留给主代理；主代理已取得 Full 终态，并对关键身份／CAS 引用和最终交付 diff 点验。后续交付提交只汇总文档证据，产品源码和测试与已评审／已运行 Full 的 `42bc473f` 相同；该文档收口由主代理静态检查，不虚增探子的覆盖。

## 本地交付与剩余边界

[本地 #57 补充记录](assets/cpar-a01-20261007/issue-57-evidence.md) 将 **A-01** 标为 **RESOLVED_LOCAL**，保留原发现和 RED；其他 **12 项 P2（A-02–08、B-01–05）**、S-01 及 E-01–05 维持原状态。这里不表示 #14 的全部场景完成、A 批完成、#57／父规格 #9 可关闭或项目可以发布。

本轮本地受影响 Task 检查和 Full 已完成；未建立新 Phase tag、未推送触发远程 Fast／Full Required Gate。正式远程 Phase Gate、新候选构建与签名、真实 Provider 验收、生产迁移／部署均 **NOT_RUN**，属于后续冻结和独立授权范围。Full 内已有 harness／oracle 的通过仅证明本地受控行为，不表示同名生产或真实渠道操作已执行。未修改 web/prism、Responses same-grant/revision 合同或 E-05。

原 source80 的 Full、签名制品、迁移演练与 AC-01 证据仍作为历史事实；没有将它们重新绑定至本修复提交，也没有生成新的发布候选。本地提交只含本轮必要源码、测试和报告；用户原 AGENTS.md 及无关工作区内容保留。收口时核对原已记录的 56 个文件 SHA256 全部不变，产品／测试相对已评审源码提交的差异为空；最终报告结果补记另作 JSON 解析、引用与 whitespace 检查。

下一项建议是 **B-01：Responses 同协议多 text part 输出回传下一轮**。它是现有后端阻断中可独立切出的最小路径：以受控 JSON／SSE 输出两段有序文本，再提交同协议 history，先复现 `ToolHistoryUnsupported`，修复并验证内容、顺序和元数据保留；继续保留 tool-history 和 closed-field validation，不触及 E-05。
