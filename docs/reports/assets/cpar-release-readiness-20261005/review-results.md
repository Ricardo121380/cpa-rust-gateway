# A／B 补齐审查结果（2026-10-05）

审查覆盖 **COMPLETE**；A Spec **FAIL**，B Standards **FAIL**，B Spec **FAIL**。这是完整读取后的结论；静态确认的源代码路径没有执行新复现。测试／契约补证保持 **PARTIAL**，真实 Provider 与新构建生产 **NOT_RUN**。候选与完整方法见 [结构化回执](review-results.json) 和 [覆盖表](review-coverage.md)。

## Standards

A 原 complete 审查按既有范围复用。本轮 B 39 个代码文件及 34 个文档／fixture／回执文件的原实现、修复端点和候选变化均已读。证据探子完成所有 diff 后因错误 schema 名中断；主代理核验真实 RequestHistoryRow、三份契约及生成字节、B 同提交跨界日志／trailer和后续执行 helper，补齐其剩余检查。文档规则发现 S-01；H-01/H-02 仅为维护建议。

- **S-01 / P3** `docs/adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md:3`：ADR lacks mandatory Context/Decision/Consequences/Alternatives/Validation and rollback structure, especially explicit rollback contract.

- **H-01 / advisory** `crates/protocol-anthropic/src/response.rs:962`：Unused deferred-input bool/field and obsolete terminal comments survive deleted Usage narrowing; remove if touched for a related repair.

- **H-02 / advisory** `crates/gateway-router/src/tool_controls.rs:150`：choice_name duplicates protocol control spellings in protocol_transform.rs:388/843; reuse existing mapping if related change justifies it.

## Spec

A 原50文件探子中断且未提供终态，记零覆盖；替换四个单轮组覆盖41文件，主代理覆盖其余9个UI契约文件。A 后端36文件完整；主代理亲读10个基础／报告文件与全部结构化回执。合计96/96。B 两组原实现完整diff＋修复端点＋相关候选变化已读；第二组原来邻接遗漏由一个新单轮探子完整补齐。合计73/73。原发现的修复保留，但没有覆盖以下分支。

所有13项P2来自确定的源码分支和规格对照。这里没有把静态反例写成新执行失败；最小必要回归由后续修复任务实施。

### A-01 / P2 / crates/gateway-control/src/management_mutation_service.rs:1288

触发：Existing Codex credential callback supplies valid authorization of another identity or one without comparable identity, with unchanged expected revision. Registered callback at management_resources.rs:2805 reaches persistence at :6923.

影响：Original credential ciphertext is replaced while identity-bearing bindings remain. New enrollment guards do not protect this legacy callback.

规格：#14:18; #9 same-identity reauthorization; ADR-0099:25。

最小修复：Use the existing old/new identity guard in interactive callback persistence; keep normal refresh semantics distinct.

必要验证：foreign/unknown identity returns409; exact original ciphertext, revision, Disabled and bindings unchanged; same identity succeeds; CAS conflict fails。本轮 **NOT_RUN**。

### A-02 / P2 / web/prism/src/features/accounts/AuthorizationRecovery.tsx:24

触发：Exact completed receipt is read, then getConfigVersion/getCredential fails. Also AddAccountDialog.tsx:228–240.

影响：Confirmed saving is dropped and UI reverts to unknown because downstream read is part of receipt success handling.

规格：#10:18; #9:177; ADR-0099:15。

最小修复：Retain confirmed receipt before follow-up reads; failures block connection/application confirmation only.

必要验证：completed receipt+follow-up503 retains credential ID and saved fact; no second exchange/import。本轮 **NOT_RUN**。

### A-03 / P2 / web/prism/src/features/config-versions/configurationTask.ts:117

触发：Automatic API setup publishes successfully; user closes its result; ProviderDialog.tsx:86 calls rememberPending(active).

影响：Pending draft remains because versionStore.ts:36 ignores active summaries. Dock says current service unchanged; next task rejects the now-active pending ID at configurationTask.ts:46.

规格：#9:175,177; #13:20。

最小修复：Settle the acknowledged applied version and its matching pending state independently of a review/navigation action.

必要验证：auto import→ordinary close→second import succeeds; active context and dock correct。本轮 **NOT_RUN**。

### A-04 / P2 / web/prism/src/features/config-versions/configurationTask.ts:54

触发：Selected or tracked draft exists but its priorChanges is empty.

影响：Draft existence disables automatic application despite there being no other pending change. Existing test :36–42 expects this incorrect behavior.

规格：#9:175; #11:17。

最小修复：Base auto-application on actual prior changes, maintaining ownership and publication CAS.

必要验证：empty selected/tracked draft auto-applies; nonempty draft still requires complete review。本轮 **NOT_RUN**。

### A-05 / P2 / web/prism/src/features/accounts/KiroDeviceDialog.tsx:53

触发：Device poll result is unknown; user clicks close-and-mark-unconfirmed. KimiDeviceDialog.tsx:161–172 has same branch; OAuthPage.tsx:26–27 unmounts it.

影响：Close drops the exact session/task recovery handle and gives no persistent pending-result notice. It cannot return to that exact retained receipt from this flow.

规格：#15:19–20; #9:180。

最小修复：Hand off a value-free recovery key and pending notification to an owning task before unmount.

必要验证：lost poll→close→return uses original session_id, reads only, no fresh exchange。本轮 **NOT_RUN**。

### A-06 / P2 / web/prism/src/features/accounts/accountStatus.ts:6

触发：API Key becomes unauthorized/revoked/expired; AccountsPage.tsx:147 does not pass material type.

影响：Main account status incorrectly instructs reauthorization instead of replacement of the API Key.

规格：#12:18; #9:203。

最小修复：Carry actual authentication-material classification into the primary status.

必要验证：API Key unauthorized→replace Key; OAuth unauthorized→reauthorize。本轮 **NOT_RUN**。

### A-07 / P2 / web/prism/src/features/accounts/AccountModelsDialog.tsx:75

触发：Successful invocation leaves evidence.data=true; a refetch is pending or errors with retained data.

影响：New-result title still treats retained true as current identity evidence; neighboring retained-receipt branch correctly checks isFetching/isError.

规格：#26:21; ADR-0099:19; A report:78。

最小修复：Apply the same current-evidence guard to the immediate result heading.

必要验证：true→refetch pending/error shows historical/pending; no extra Pin。本轮 **NOT_RUN**。

### A-08 / P2 / web/prism/src/features/accounts/AccountRuntimeSummary.tsx:39

触发：Reauthorization establishes a new revision, then runtime read fails with retained expired rows.

影响：Old runtime authProblem overrides new authorization and primaryAccountStatus prioritizes it above read failure.

规格：#20:19–20; #9:194–204。

最小修复：Do not let an invalid/stale runtime snapshot override the current authorization; retain old observations with time/scope.

必要验证：new active revision+old expired rows+read failure retains current auth and pending runtime status。本轮 **NOT_RUN**。

### B-01 / P2 / crates/gateway-router/src/protocol_transform.rs:950

触发：Responses assistant output contains two ordered output_text parts; its native decoder accepts and preserves them, then caller resubmits that output in same-protocol history.

影响：valid_assistant_tool_history allows subsequent text only for Messages (:958), so next Responses turn is rejected as ToolHistoryUnsupported.

规格：#27; #31 same-protocol roles/content/order。

最小修复：Permit ordered Responses text parts while retaining tool-history and closed-field validation.

必要验证：two-part Responses JSON/SSE output→next input preserves both parts, order and metadata。本轮 **NOT_RUN**。

### B-02 / P2 / crates/protocol-openai-responses/src/upstream_response.rs:623

触发：Valid deltas are followed by contradictory response.output_text.done / content_part.done / reasoning done events.

影响：These events unconditionally return Ok(()), so contradictory terminal confirmations and fields are ignored.

规格：#9:223 lifecycle; #27; #31。

最小修复：Validate closed fields, item/index correlation, legal phase and accumulated-versus-done values.

必要验证：contradictory text/reasoning/part confirmations fail; no successful terminal, saved response or WS root。本轮 **NOT_RUN**。

### B-03 / P2 / crates/protocol-openai-responses/src/upstream_response.rs:1143

触发：Two completed native output items arrive in A,B order; final response.output lists B,A.

影响：Terminal check compares BTreeSet IDs; native_output.rs:322 re-emits start order. Conflicting order is silently rewritten.

规格：#27 exact role/content/order; #31 native item preservation。

最小修复：Record ordered item IDs/output_index and verify terminal sequence rather than set equality alone.

必要验证：reversed final order fails; valid interleaved items preserve ordered output。本轮 **NOT_RUN**。

### B-04 / P2 / apps/gateway/src/runtime.rs:6060

触发：First bootstrap read contains ResponseStart plus valid Usage and malformed later frame in one transport chunk.

影响：decoder exposes pending Usage, but begin() errors before returning source; runtime.rs:4857 uses ?, dropping the only Usage carrier before observer attachment. Historical regression emits its first semantic chunk separately.

规格：#32; ADR-0100:55–60。

最小修复：Carry already-validated bootstrap Usage on this exact failed Attempt without replay or guessed counters.

必要验证：intro+known Usage+bad frame in first read for Chat/Responses/Messages; exact failed Attempt and one complete source-counter ledger record。本轮 **NOT_RUN**。

### B-05 / P2 / apps/gateway/src/deployment.rs:471

触发：Serve restarts after external Master Key rotation with old records and a new active key.

影响：Serve only loads one master-key and fixes its KeyVersion to1. It cannot express the old+active ring that store unit tests manually inject. This is a missing process capability, not merely real-provider evidence.

规格：#35:19; #9:234; ADR-0089:64–66。

最小修复：Implement versioned external ring loading and explicit active version without embedding secrets or rewriting ownership.

必要验证：actual process restart with old+active keys; public old record/compact reads and active-version writes, foreign-owner safety。本轮 **NOT_RUN**。

## 审查已完成，执行证据仍有缺口

| 编号 | 范围 | 状态 | 已核对的缺口 | 最小补证／责任 |
| --- | --- | --- | --- | --- |
| E-01 | #13 four API presets | PARTIAL | cpar-batch-a.mjs:156–185 never switches preset; fixtures.ts:831–832 always Responses. | parameterized HTTP/EgoLite positive+negative cases for all four presets |
| E-02 | #15 completed device exchange lost response | PARTIAL | fixtures.ts:1860/1873 sets unknown before saving; existing unknown test does not cover commit-then-loss. | saved-then-lost response, receipt recovery, single exchange assertion |
| E-03 | #12/#15 Kiro fixture authentication material | FAIL_FIXTURE | fixtures.ts:1875 stores bearer and omits oauthCredentials; inventory :2730–2733 returns authentication null/no reauthorize. | correct fixture OAuth classification and test inventory/action contract |
| E-04 | #34 public isolation matrix | PARTIAL | registry checks and one Responses→Messages fixture do not supply all public same-account/different-connection, different-model and shared-Upstream Chat cells. | bounded local HTTP matrix linked to candidate; real Provider not needed for these deterministic checks |
| E-05 | #36 revision mismatch | PARTIAL_SPEC_CONFLICT | issue-36:20 requires revision changes fail; inherited BC-RESP-003:27–48 expressly allows proven same-grant Build refresh; credential_pool.rs:407–417 implements inherited exception. | spec owner resolves literal requirement versus inherited contract without silently adding an exception; apply and regress exact approved rule |

E-05 保留明确冲突；既有 same-grant 合同没有被当作本轮新豁免，#36 的字面要求也没有被删除。E-03 是 fixture 缺陷，不能据此说生产真实 Kiro 保存错材料。原 D、C 和 execution 切片 PASS 保留其固定范围，不覆盖这些发现。

## 历史修复存续

A 的精确导入ID回执、授权恢复补绑定／再次核对、Disabled 保持、Key 默认关闭与 sibling 隔离、目录空成功与分页失败、执行证据身份失效门槛、RCU相同快照保留均仍在候选；A-01/A-02/A-03/A-07 是同需求的另一条未覆盖路径。B reasoning-only history、pending-tool阻断、store能力准入、完成后／已建立source的Usage补偿、WS保存／交付顺序以及删除旧Usage narrowing仍在；B-04 是source尚未建立的bootstrap边界。

独立探子遵守 generic／fork_turns=none／只读／单轮；派发后主代理只等待并收敛。主代理核定发现、责任与最终状态。没有产品修改、浏览器执行、Full、重新签名或真实请求。
