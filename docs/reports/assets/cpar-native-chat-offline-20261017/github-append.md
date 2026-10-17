

## 2026-10-17 — Grok Build 原生响应离线诊断与严格 Usage 修复

本批真实 Provider 调用0、新Pin0、真实Chat0、远端验收实例0、部署0；没有复用旧停止预算。基线ec534918仅用于比较。旧AGENTS及其他用户内容、旧候选和原始失败保留。本地产品提交b98322ef，代码/本地材料审查固定ec534918→7a876527；未push，没有新release candidate或manifest。

原chat-json-serving-20261015的完整gzip捕获只读取回本地程序：673字节，SHA256 7b5a3c2766766397af638665aaa7160f351780232a2c06f11d8f784ebbfdaa96；解压1393字节。selection/start/Request/Attempt/Pin实际lease153/sidecar/ledger摘要关联一致，旧unit not-found/PID0、budget stopped。原POST /v1/chat/completions、grok-4.5、stream=false、max_completion_tokens=4096及原字节SHA245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062未改。原生内容、实际私有值和价格没有进入Git/GitHub/其他模型。

首拒绝已确证：11个Canonical事件中的零基序号3 ReasoningDelta来自output[0].summary[0].text，原生part元数据field=summary/index=0；project_native_identity要求content，返回UnknownExtensions，公共Chat502/UpstreamProtocolError。序号4结束metadata还会拒绝丰富summary。它符合ADR-0101/0105，不能删未请求的reasoning或summary冒充body，不属于已有无损映射却误拒的证据。

同一完整响应另有四个未映射Usage字段（两个零计数、非零原生price字段及语义未解释object）；旧Build会丢弃它们，旧Responses投影PASS不能证明整包保真。另有合成负例证明total未校验。最小修复保留完整Usage到既有RawExtensions并由strict projector拒绝、校验可确证checked total；若被拒事件本身是Usage，runtime缓存其已取出快照供既有失败flush读取。无追加上游读取/重试，无新wire映射或结算推断。原样本最终Chat/Messages仍事件3拒绝，Responses现于事件8 Usage拒绝。已知input228/output159/reasoning158/cached128仍保留；缺失计数null、账本costnull、Provider独立接收/实际费用UNKNOWN。原生model与public model不等及未解释字段语义也保留待核对，不修改原正式oracle迁就它。

本地真实公共factory/compiler/lease/auth/handler matrix：两类合成形状×三协议×JSON/SSE，共12组通过。各组模拟receive1；安全错误/RequestID/Attempt succeeded与Request failed区分、无伪成功终态；主动重复Usage投递后Usage/source-event/ledger仍各1，costnull。受支持的标准Usage Responses fixture逐项保留summary/message原生结构；未知item/part、身份、终态、total及Usage的0/null/空数组负例严格。原始私有重放仅本地程序输出白名单事件/类型/数量/hash。

本地Full实测FAIL：43步PASS，Dependency policy FAIL，最后RustSec audit NOT_RUN；Rust1515 PASS/0 FAIL/13 IGNORED，Clippy/格式/本地loopback进程/双build/秘密扫描等已过。未变Cargo.lock中rustls0.23.45命中RUSTSEC-2026-0346/0347/0348/0349；不在本批擅自升级/豁免。原RED、Usage丢失RED、驱动/初次clippy失败及Full FAIL全部保留；13份原日志字节未改，并做确定性gzip逐字节归档，不以定向绿测覆盖Full。

独立双轴：Standards PASS（硬发现0/建议0），Spec PARTIAL/BLOCKED（新实现违约0/scope creep0，summary/完整Usage/模型等价、真实矩阵及Full阻塞未解）。初次探子路径检索错误的不完整状态保留，收窄补完终态complete。旧ARM64 binary cdadcb2c…/产品bf822b14不含本次修复；新候选必须绑定新源码、binary/manifest/metadata及受影响Task/Phase/同候选Linux/egress/reader验证，旧资格不自动覆盖。

下一步唯一优先事项：先决定有类型summary/native Usage/原生metadata的无损公开字段契约与模型映射验收，或由用户明确调整相应承诺范围；候选字段只是报告中的未批准建议。当前映射不变时真实预算应为0。只有决策/实现/新候选门槛完成并另行授权后，才建议新epoch最多fresh Pin1＋原Chat1（总2），首FAIL/UNKNOWN/矛盾永久停止，无retry/fallback/换账号模型/refresh/修复后追加。本批不自动开启下一轮。

报告 docs/reports/cpar-native-chat-offline-20261017.md；同名assets含白名单身份/结构、合成fixture、完整失败日志、local-validation及固定审查。原真实Chat FAIL不改写，完整协议/六组合/声明扩展/远端CI/发布/生产仍未完成；本次不关闭#47/#57/#9，不改票面AC、正文、metadata或依赖。
