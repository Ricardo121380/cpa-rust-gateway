# 独立历史 Spec 补审终态

审计基线 `d3d2e3c5bf0d9521f3fb0d830344b34053f62155`，最后前批生产行为源码 `36079c1fad666c2bbb9c15bd8f8a59518c970b42`。二者间生产/驱动/契约/ADR无diff。后继 `611949b6` 添加T12驱动及独立分支；最终 `c1b304d0` 修复Store对已知旧Usage编码的读取／重放并补充BC-OBS-002。旧Pin/global admission页面、路由资格、native factory及原410断言均未改变。本文件保留历史独立只读 agent 的原终态与覆盖；本批新Store/契约/驱动diff另由最终两轴审查，主代理整合不是历史场景的动态重跑。

## Standards

NOT_IN_SCOPE。本次历史补审只验证Spec及证据归属，Standards在本批最终diff另行独立审查。

## Spec

**PARTIAL；已查 hard findings=0。** #26 COMPLETE；#55 COMPLETE；历史映射仅旧Web/Console各自专属页面回执仍PARTIAL。其他明确缺项已经第二个独立agent逐项补齐。完整覆盖与零hard分别报告，不能把二者混为整批PASS。

- `/root/historical_spec_supplement`：终态partial；独立核139+45份原log实际hash与184份receipt归属；完成#26/#55与主要历史映射。
- `/root/historical_mapping_remaining`：终态partial；完成HTTP12、worker内部子场景、410个原断言及223/223接收关联；旧Web/Console页面仍PARTIAL。
- 两者均未修改文件、未重跑浏览器、未调用真实Provider、未读取真实凭据或生产DB。没有因为PARTIAL机械重开已关闭票。

约定：B=[20261009原索引](../cpar-native-session-batch-20261009/final-evidence-index.json)，C=[20261010补齐索引](../cpar-native-session-completion-20261010/execution-index.json)。`entries/N` 原项保留argv、receipt/raw_log、source_before/after、UTC、exit、SHA、bytes；原SHA均实算匹配。下述时间是原执行UTC，d1批原浏览器精确UTC未取得，不拿归档recorded_at代替执行时间。

| 历史汇总 | 原索引及逐项范围 |
| --- | --- |
| 旧生命周期19项 | B76；clean eab0f044；2026-10-09T15:32:56.719773Z；原19个case ID/result逐项等于final-lifecycle-page.json |
| OAuth fixture18项 | B78；clean eab；15:39:37.010208Z；原18个case逐项等于final-oauth-page.json |
| shared14项 | final-shared-page-summary.json的0…13依次映射B79/80/81/82/83/86/87/88/90/91/92/94/96/98；原scenario/PASS逐项一致；15:43:32–15:46:17 |
| ordinary真实HTTP/TLS/DELETE/历史 | B85；clean eab；15:44:12.462468Z；held stream/新分配/retry零fallback，Request7→11、Usage3→6、ledger3→3且原ID保留；SHA463810bda07369de936cf8627b83322fb3a4b3a715abe5721f9a6fa65410a74c |
| E-05 stock重启 | B102；clean2bb；15:47:00.257496Z；受控推理4/refresh1/fallback0，PID74860→74979、roots4、native删除历史6/4/4保留；SHA5e33864577ac4285e5db0071ba91faee71f389fa0fb0f5e949670415d509b756 |
| 旧stock关闭返回恢复 | B105；clean2bb；15:49:20.187838Z；原close-leave-return PASS对应final-grok-page.json results/1 |
| 新导航RED/GREEN/显式关闭GREEN | C15/14/3；clean c1e8/d05/360；02:23:13.787118/02:33:50.327068/02:37:37.568872Z；原FAIL/PASS/PASS保留 |
| 最新生命周期30项 | C28；clean360；02:36:25.199188Z；原30个case全部匹配，含missing identity、native四类前后漂移、ordinary binding前后；SHA5faaeef3075ff4109b7eff86dd879da53ba21f6d8929b0eba0f42b79a449babd |
| Web/Console共享HTTP | B74；clean eab；15:30:57.930770Z；managed_resource_inventory.rs:1270/1409；各201、observed、调用2/GET0、403/409保护、持久化重开。未找到各自专属页面回执 |
| HTTP12 | B53；原log6–17↔managed_resource_inventory/oauth_identity_matrix.rs:38/40–42；Codex/Claude×same/different/unknown×disabled false/true；same201→200/rev0→1/密文改变；其他409/rev0/密文保留，exchange2/account251/disabled及关系保留 |
| worker刷新及续接 | B33/50；credential_refresh/native_batch_a.rs:32/254–378及runtime/m2_continuity.rs:137/274–410；200/400/403/429/503、disabled sibling0、JSON/SSE刷新/重开/foreign/model/manual/phantom。missing/noncontiguous/conflicting×JSON/SSE只按B50原打印归属，不回填到B33 |
| native worker11项 | B36；p12_10d_native_account_workers.rs源码40/53/107/151/188/216/270/332/354/436/491↔原log9/17/10/16/12/14/15/11/18/25/24；另385四轮、517六种同授权证明、607失败CAS保留 |
| held PATCH/DELETE facade | B38；runtime/batch_a_native.rs:79/85↔原log22/21；242–304库存/审计/effective空/新请求拒绝/旧流自然完成/send1/lease0 |
| 410公共断言与223接收 | B84；clean eab；final-continuity-http.json checks0…409全部逐项映射原driver；provider_calls0…222的七字段逐项匹配，再与all_upstream_receives0…222的POST/path/listener关联；primary58699 Chat63/Responses87/Messages61，fallback58703各4 |

B33/36/38/50/53的实际source为85a168da dirty，不能换成clean eab。B84原receipt与归档JSON字节一致；两个driver在原eab→d3区间无diff。本轮新增CPAR_T12_ONLY分支默认不进入，也没有改旧410断言或Provider fixture；复用归属仍是原执行。

410断言映射，H=`scripts/acceptance/cpar-batch-b-http.py`，E=`cpar_execution_evidence_http.py`：0→H644；1–61→E65；62–73→H651；74–217→H684；218–236→H709；237–266→H741；267–283→H772；284–289→H819；290–296→H838；297–319→H875；320–327→H985；328–337→H1014；338–373→H1048；374–376→H1090；377–382→H1104；383–398→E112；399–409→E158。行号指原执行源码。

## #26：COMPLETE

旧Pin回执B91：2026-10-09T15:45:02.974121Z，clean eab，SHA43b7f4f532977cf16ec52123f7a041dc243944adf39c035448e64a3c9fb0baf8。cpar-batch-a.mjs:348打开0/352选择0/356显式1/360协议变动失效/367重开仍1；execute_channel_pin在eab→360逐字相同，相关driver/callEvidence/router无diff。

credential_scheduler.rs:636验证exact provider/channel/credential、唯一candidate、health/quota/exact lease；1609覆盖health/quota/capacity；1698 expiry100且now100，拒绝无lease。attempt_orchestrator.rs:772显式一次；2109–2132失败只有原binding一次、RetryClosed、sibling未消费。C23原Full1135/1164/1166/1168行实际执行具名测试，SHAda2f48f1b7de4cb5eda65f79f928845d2a7e5044a47c31817bcc7ef8913aa5db；C21原前端log45行执行callEvidence 3 tests。原#26没有任意evidence TTL要求；未新增该要求。

## #55：COMPLETE

原cpar-batch-d-review-fixes-20261001/maintenance.json checks/3为failed system refresh retains observed admission state with action gated。browser-maintenance.txt原source d1ee7702937a87777cc53e3d0de392d1e2528b84、6 checks PASS；原stdout SHA5ca7e3aad36a9eee5e30135df32e88d00f9fbaed8f74d25df8fbe93939016cac，JSON SHA3d8959ab781c918406675a4868f7e0d174b0164cc61839b3530bacc94b94ded4，分别匹配evidence-integrity.json 67–68/247–248。

SystemInformation.tsx:13–14的true/false/unknown及失败/fetching apply门槛，连同RuntimeApplyNotice d1→360无diff；D原driver段等于当前cpar-batch-d.mjs:206–220，段SHA588416429aa21487e2a145fdb38a6b63cc2108688f19c0146cb5e256854f50ef。本轮仅静态复用；没有新的global admission浏览器执行。

## 剩余覆盖与最小下一步

旧报告80行的Web/Console“旧证据保留”能够映射到B74共享HTTP，没有各自专属页面receipt。独立检索已检查B全部139项label及旧cpar-batch-a-20260930/browser.json场景清单；没有找到不能改写为已经执行。若其所承诺层级确需单项页面证明，应取得原回执，或下一轮在具体页面行为范围补EgoLite场景。该证据缺口不自动证明产品缺陷，也不自动重开#16。整批Spec保持PARTIAL。
