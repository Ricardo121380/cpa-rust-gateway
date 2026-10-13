# CPAR-38 / #47：验收驱动跨版本完整预演与受控 Chat JSON 诊断

本批当前完成 **LOCAL_SIMULATED 完整预演**，真实观察、启动、Pin/Chat和退出尚未执行。独立两轴审查在执行前单列，不把本地通过作为远端资格。

## 基线、授权与复用

当前基线 `9b6d70615d7507114aeab0007b34ea9db81d6182`；实际checkout为准，没有回退。产品 `bf822b14a62c21cf582a7539e3635f69bd8b7677`，source摘要兼容修复 `5e8768c485dda02b8b84672a7603a97d49f85a1f`。apps/crates/Cargo/工具链及构建脚本对象、工作区产品diff均匹配。binary `cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e`，metadata `d5e87f208ec6ab4265a272d4be5d331bbb0865e30abfa62fa6556671d96cb78c`，manifest `655c037df68c5657b250105bbb07bed665210a7f6b20ea8d8b88aab488157a56`，manifest两行的实际大小和摘要一致。

Task42/42、Phase45/45的gzip及完整原始日志摘要、完整历史两轴审查摘要逐项匹配后复用。产品/全局验收helper不变；依赖安装0、产品构建0、产品Full重跑0。本批资产含Python行为代码，不能称docs-only；使用实际完整驱动回归和补充docs/static检查，未变化产品门槛按用户本批明确要求复用。

独立新epoch `chat-json-rehearsal-20261013`，同一保护根下唯一bin/private/audit/unit。旧拒绝包、tar、停止state和所有失败不变。用户AGENTS和无关tracked/untracked内容保留；本批只提交报告/本批资产。不修改web/prism，不push、CI、发布、部署或生产写入。

最多7次/60分钟只读观察、相邻至少10分钟；仅正常同身份但正剩余期不足可等待。prepare严格 `expires_at_ms > now_ms + 3600000`。正式批次最多fresh Pin1、仅PASS后原合成Chat1；`POST /v1/chat/completions`、grok-4.5、stream=false、max_completion_tokens=4096与精确请求摘要 `245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062`保持。历史35意图/34确认发送独立，不恢复旧余额；费用政策独立于次数，Provider接收与实际费用无证据保持UNKNOWN。

见 [契约](assets/cpar-build-chat-json-rehearsal-20261013/batch-contract.md)、[复用](assets/cpar-build-chat-json-rehearsal-20261013/reuse.json)、[授权](assets/cpar-build-chat-json-rehearsal-20261013/user-authorization.json)。

## 数据来源、完整合成流程与驱动修复

生产source版本 `b0cf36e387b774439acc23830221389eb3ac74d0`/schema31的实际DTO/OpenAPI没有摘要credential_revision；候选版本bf822b14/schema34的该字段可选，但本批候选/实际执行校验强制完整一致。源端缺失/null只适用于摘要，非null在observe/prepare/start各次重读都核对。

候选原生DB保持native revision N；实际候选池提供runtime revision，准备时N+1仅为这个固定handler版本的预期检查。Pin请求使用native N以满足管理CAS；Pin execution.identity提供真实lease revision，冻结计划必须从该执行证据取值且与候选目标相等。Chat的Attempt sidecar再以Request-ID、Attempt原始payload SHA和canonical执行身份精确验证该lease。没有以材料revision推算或填补lease。

完整流程 `observe → upload → prepare → start → pin → chat → exit` 在隔离文件系统、实际SQLite数据库和外部操作替身中执行实际控制器与各阶段顶层程序；实际REMOTE_READ库存收集器、Pin freezer、预算save/fsync、HTTP请求序列化、捕获decoder及Request/Attempt/Usage/ledger检查都运行。systemd、Linux /proc/UID/固定校验材料、时间和HTTP传输替身明确标为LOCAL_SIMULATED；各远端阶段不返回固定PASS。测试自己的PASS资格文件仅为本地测试授权，不能用作实际独立审查。

18项测试包含多个边界子场景：缺失/null source成功；source非null漂移；候选缺失/null/漂移；fresh Pin lease缺失/漂移；prepare不足整小时或未知；启动认证首次失败不重试、启动未知和审计新增；Pin/Chat未知永久停止；操作槽与预算发送前持久化；重入/停止后拒绝；冻结字节漂移；Attempt/request/payload/lease及账本source_event错配；严格502拒绝的原生Usage保留；失败/缺证据发送计数UNKNOWN独立于资源退出；prepare/start失败无start receipt时退出。

保留RED后做三处本批驱动最小修复：source摘要非null校验补到prepare/start；Chat `error:null`不会中断发送后证据收集，HTTP200仍要求无具体错误、成功Attempt与Finished；无成功start receipt时走只读资源证明，使用本批观察持久化的新鲜生产基线，保留启动失败的实际停机状态，不停止身份不明进程。不改Rust产品、生产schema或保真规则。

第一次完整审查覆盖36/36对象与8/8完整gzip：Standards PASS，Spec FAIL，保留原结论后实际回归得到16项/2 FAIL。最小修复将观察返回的生产基线在资格判断前持久化，并绑定、接入未上传时的只读退出程序；没有此前观察基线时明确标记NOT_OBSERVED，同时检查当次新鲜生产状态稳定，不虚构恢复目标。对应版本DTO夹具扩展到全部引用闭包（source8、candidate12），完整校验池字段、Pin请求和响应：补request_id/attempt_id，成功不输出非法stage=completed。源端摘要compat字段与旧DTO分开说明；候选和实际lease仍严格。修复后完整流程17/17 PASS，总计30/30。未发生真实观察或发送。

见 [第一次完整审查](assets/cpar-build-chat-json-rehearsal-20261013/review-preparation-first.json)、[审查缺口RED](assets/cpar-build-chat-json-rehearsal-20261013/review-gap-red.log.gz)和[修复GREEN](assets/cpar-build-chat-json-rehearsal-20261013/review-gap-green.log.gz)。

见 [对应版本的契约与脱敏观察夹具](assets/cpar-build-chat-json-rehearsal-20261013/contract-fixtures.json)、[完整流程测试](assets/cpar-build-chat-json-rehearsal-20261013/tools/full-flow-tests.py)、[初始RED](assets/cpar-build-chat-json-rehearsal-20261013/full-flow-red.log.gz)、[无start退出RED](assets/cpar-build-chat-json-rehearsal-20261013/prestart-exit-red.log.gz)、[完整GREEN](assets/cpar-build-chat-json-rehearsal-20261013/full-flow-green-bound.log.gz)。

第二轮完整Spec审查覆盖45/45对象、14/14 gzip，精确DTO与载荷绑定通过，但仍发现上传失败后的退出不可达（Spec FAIL）；Standards未取得终态，记录PARTIAL。保留第二轮结论和18项/1 FAIL的上传退出RED后，退出分支按是否已进入prepare选择：尚未prepare时只执行本地冻结的只读资源证明，不依赖远端上传完整性。真实上传程序注入创建前拒绝、部分tar和完整上传后结果未知三种故障：禁止重入/prepare，保留上传字节并报告NOT_UPLOADED/PARTIAL/PASS，资源证明独立于载荷完整性。完整流程18/18、受影响控制器2/2新通过；资格9/9和启动2/2代码未变复用，总计31/31 LOCAL_SIMULATED。

见 [第二轮审查](assets/cpar-build-chat-json-rehearsal-20261013/review-preparation-second.json)、[上传退出RED](assets/cpar-build-chat-json-rehearsal-20261013/upload-exit-red.log.gz)、[上传退出GREEN](assets/cpar-build-chat-json-rehearsal-20261013/upload-exit-green.log.gz)。

## 本批执行与剩余条件

实际观察/远端创建/启动/fresh Pin/Chat目前NOT_RUN；真实根因、Request/Attempt/Usage/账本、发送和退出结果在新鲜回执后追加。旧502根因仍UNKNOWN，没有NOT_REPRODUCED或六组合PASS。

诊断与保真规则缺口消除后才提出下一轮基础矩阵。六组合最多60新意图及扩展仍需独立授权，本批不执行。#47/#57/#9保持正文/AC/依赖/状态，只在本批真实停止交付后追加既有progress并独立读回，不按局部结果关闭整票。
