

---

### 2026-10-11 CPAR-38 Chat JSON 502 本批交付（local PASS / real BLOCKED_BEFORE_LAUNCH）

本条追加记录CPAR-38的本批局部工程交付；保留本票原AC、全部历史、元数据和依赖，不关闭#47。

已完成证据支持的最小产品修复、实际公共入口本地回归与独立完整 Standards / 本批 Spec 审查。**旧真实 Chat JSON 502 的具体根因仍 UNKNOWN**：旧上游原文未捕获，本批没有新原生样本。native-metadata 离线拒绝测试不是旧响应精确重放；未据此认定某个字段为根因，也未将 Usage0/账本0解释为免费或零消耗。

产品修复保留安全固定阶段和六类闭集拒绝原因；在实际建立 Request 身份后统一错误 Request-ID，并验证与 Request/Attempt 精确关联，准入前不伪造身份。保留完整验证且有来源的已解码 Usage，partial/absent 字段保持未知，快照不重复累加，事件及账本去重；Attempt succeeded 与最终 Request failed 不混同。Chat/Messages/Responses JSON/SSE 的成功及严格拒绝、已知/未知 Usage、重放去重、账本和隐私均经公共 HTTP/runtime/SQLite/billing 路径回归。保真能力保留原严格边界；没有删除 native items、reasoning/citation/signature、工具结果、call_id、顺序或历史，没有关闭声明能力、改变4096上限或降低断言。

冻结的本地产品候选 source **bf822b14a62c21cf582a7539e3635f69bd8b7677**，Linux ARM64 binary SHA256 **cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e**。修复后驱动 source **c23d2c356ec0253d93e41ca94aac821ad167fcd6**：Task **42/42 PASS**、Phase **45/45 PASS**，每轮 Rust **1510 passed / 0 failed / 12 ignored**，诊断测试 **18/18**、既有 oracle **54/54**；Phase14条既有重复依赖警告保留。Standards 与本批 Spec 均 **COMPLETE PASS**，完整范围及独立辅助闭合的 review target **74de66f3ff95ec78364c01d231b9617f482d5353**，精确覆盖141个路径；原 FAIL/PARTIAL/中断记录保留。unsigned两载荷候选仅为本地候选，非P12正式签名release资格。隔离构建复用既有配方与依赖，没有重复构建准备；后续仅驱动变化，产品/build-release字节未变。

本批唯一新真实诊断 epoch 在 **systemd启动前** 失败：已审查上传的驱动 **38504a1eabeaacfcb548e23de7a61a545351b35b** 错把既有 `--property=Environment=` 的 guard/audit 注入形式当作 `--setenv=LD_PRELOAD` 锚点。该启动驱动缺陷已通过实际argv形状的network-free AST RED证明并最小本地修复；另补启动前拒绝stopped epoch。**修复后的驱动没有上传、重启或真实复测**。新实例从未启动；新Pin和原Chat JSON均 **NOT_RUN**，实际新lease/pool/serving declaration/Pin资格未建立。它不是 NOT_REPRODUCED，也不是新一次HTTP502响应。诊断默认关闭、候选/请求选定及0700/0600私有捕获约束已本地验证；真实捕获为空，原生内容和认证材料未入Git/GitHub/外部模型。

推理对账：历史意图 **35**、网关/Pin确认发送 **34**；本批新增 **0/0**（Pin0、Chat0），累计仍 **35/34**。Provider独立接收数及实际费用继续 **UNKNOWN**。旧剩余58已经终止，本批未用2也随失败epoch永久终止，不续用、不重试、不fallback、不主动刷新/reauth/换账号、不继续其他协议、扩展或修复后的真实复测。新Request/Attempt/Usage/账本均0是never-started事实，不构成旧消费的推断。

退出及交付只读核验 PASS：`NOT_STARTED_VERIFIED`，临时unit MainPID0/not-found、19180/19181无监听、audit不存在且5秒保持、无竞争state进程；334个保护历史文件与14个旧上传载荷哈希保留，29个私有文件/4个子目录为0600/0700且无symlink。生产进程/unit/binary/current/Caddy保持初始身份，活动配置在退出时核验保持；Colima两个profile恢复原Stopped并与初始六项配置一致。48,185个原有脏路径独立重新核验0改变/缺失/错误，用户AGENTS保留不暂存。本批不修改web/prism，历史候选/epoch/失败回执保留。审计限制：本批初次runner的完整FAIL aggregate log被误覆盖，仅工具摘录保留；最终完整PASS原文及双摘要保留，旧真实失败材料不受影响。

本批完整六组合基础矩阵 **NOT_RUN（0/6）**。旧source1b6023f5的Responses JSON/SSE各9/9仍只证明旧身份 **2/6组合、6/18场景**；不迁移为本批或新候选通过。关联#47/#57/#9全部继续 **OPEN/PARTIAL**，本批局部修复不满足三票全AC或父规格关闭条件。远端CI、正式发布、正式部署、切生产及生产验收均 **NOT_RUN**，没有push。

后续先另行明确授权新的最多 **2个诊断意图**（fresh Pin1，PASS后原Chat JSON1；任何结果停止），使用新自有epoch，重新核对实际launch/guard/后台出口/全部字节以及账号连续性、实际进程/config/route/model/Client Key/lease/egress/声明/有效期，不能复用旧Pin/lease/预算。新证据若不可保真，保留明确拒绝并提交具体产品规则决策。诊断阻碍与保真规则解决、候选重新满足本地门槛及独立完整审查和全身份资格后，再另行授权基础矩阵上限 **60新意图＝6个fresh Pin＋54个基础请求**；扩展另批。这是请求预算建议，不是费用估算或本次继续调用授权；本批结束不自动进入下一轮。

本地报告 `docs/reports/cpar-build-json-502-20261011.md`；证据目录 `docs/reports/assets/cpar-build-json-502-20261011/`；关键回执 `review-final.json`、`quality-gates-c23d2c35.json`、`remote-prelaunch-exit.json`、`remote-delivery-readback.json`。核心冻结与资源退出证据提交 **65c1ab124ccada3773d6353cc20989e11b703723** 仅本地，未push，不伪装为远端可读取的代码/CI/发布证据。
