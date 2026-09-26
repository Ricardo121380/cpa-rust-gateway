# CPAR M4：容量、真实验收与发布

状态：进行中。实施基线 `fa5f280fe897158aac5130c7d7f6324f1aa2c550`。
对应[计划](../handoffs/cpar-reliability-alignment-plan-20260926.md)和[验收清单](../handoffs/cpar-reliability-alignment-acceptance-20260926.md)。

## 当前进度

- M4-01/02：10万/100万精确查询、容量采样及告警通过本轮本地验收。
- M4-03/04：`3de0319` 正式fast/supply-chain通过，Rust 1,356项、前端415项；双架构签名与独立校验通过。EgoLite真实嵌入应用48个工作区状态、18个弹窗状态及模型→受限Key→mock请求→账本通过。
- M4-05：完整生产副本28→31→兼容30→31通过；新格式事件/checkpoint、管理员、历史、轮转密文和权限保留，演练无网络。
- M4-06：真实推理 **6/12**，Build首轮工具调用成功并入账；续轮文本metadata拒绝，四轮闭环未完成。每次均1attempt，无自动重试。真实目录读取单独记录为元数据验证。
- M4-07：`3de0319` 已部署，schema31，切换到就绪1,031ms；公网4资源hash/CSP/鉴权、7账号、原有2,356事件/699账本与权限保留核验通过。DNS/Caddy/Autoreg未改变。
- M4-08：生产EgoLite登录页已交给用户，等待本人登录；不能用登录页或本地矩阵代替生产登录态验收。
- M4-09：报告持续更新；8916032加密reasoning修复已发布；新增文本metadata拒绝及生产登录复验未完成，**M4未完成**。

## 测量与安全边界

沿用 M0 的固定规模、seed=20260926、分布、一次预热五次测量及阈值。
测试库由脚本仅在未占用本地路径创建，合成数据不涉及真实账号或秘密。
不会删除生产历史、恢复旧轮转凭据、改 DNS/Caddy/Autoreg，或沿用已用完的旧真实调用额度。

## 大样本查询证据

[固定数据与全量分页回执](evidence/cpar-reliability-m4-20260926/query-benchmark.json)包含机器、seed、查询输入、一次预热/五次原始计时、独立总数、全部分页摘要和整进程RSS；[SQL计划](evidence/cpar-reliability-m4-20260926/query-plan.json)按实际SQL由本机SQLite解释，明确区别源代码统计的SQL次数和运行的执行计划。

| 百万请求查询 | 中位数 | 五次最大值 | 正确性 |
|---|---:|---:|---|
| 5分钟窄窗 | 2.54ms | 2.59ms | 110请求，全页/账本/分位数一致 |
| 24小时 | 168.84ms | 170.52ms | 31,667请求，全页一致 |
| 全部有终态历史 | 1,908.98ms | 2,186.09ms | 950,000请求，9,500页 |
| 全历史单页，不聚合 | 1.79ms | 1.87ms | 100行/页，完整翻页一致 |
| 包含未知历史 | 2,594.68ms | 2,728.26ms | 1,000,000请求，10,000页 |
| 模型筛选 | 1,250.64ms | 1,515.25ms | 45,573请求，全页一致 |
| 模型/账号/Key组合 | 1,784.37ms | 2,181.45ms | 4请求，5attempts |

10万和100万共14组均通过。独立Python按固定seed公式逐条计算，不读取SQL结果作基准；全页摘要同时覆盖ID、attempt数、账本行数、费用与置信度。整进程峰值RSS最大26,017,792字节（约24.8MiB，保守高于“额外RSS”），小于256MiB门槛。分页3条SQL，附加完整汇总共4条，不含事务控制；金额账本整页批读，未做N+1。SQLite内部的索引子查询仍按匹配行执行，未声称扫描成本恒定。

初始百万全历史中位数约13.34秒；优化初期最大值仍曾15.38秒，未当作通过。后续以终态覆盖索引、精确时延频数分位数、单次聚合集合和账本批读降低成本；不截断/近似。完整翻页另暴露双ordinal上界造成的深页扫描，固定为snapshot/cursor共同上界后重新运行全矩阵。测试中的所有旧失败回执留在本机output目录。验收后仅删除本轮两个可重建的合成数据库以释放磁盘；脚本、输入和回执保留。

容量使用独立单线程采样，HTTP只读内存；与TTL清理解耦，初次无观测、失败保留旧值、过期均区分。原生文件系统I/O不可保证中断，停止最多等待3秒并明确记录未退出，不把它描述为可强制取消。低磁盘/WAL/队列与物化序号跨度仅告警，不删除历史或扩大准入策略。操作说明见[容量手册](../handoffs/cpar-capacity-runbook.md)。


## 发布候选兼容修复

首候选 `c50e11a` 的release-artifact运行36249803472被ARM Debian容器冒烟正确拦截：缺少`GLIBC_2.39`；未签发ARM产物、未部署。x64在较旧构建宿主通过，不能代替ARM验收。移除新增的`Command`/df进程路径，改为固定rustix 1.1.4的安全statvfs封装；不抬高运行镜像/生产系统要求，不放宽unsafe或跳过冒烟。后续候选必须重新完成双架构及正式门禁。

本轮EgoLite在嵌入gateway完成48个工作区状态、18个主要弹窗状态，及目录手动开放→Key显式权限→mock调用→unpriced账本；held晚到失效响应正确清除会话。容量低磁盘真实采样提示已检查390px辅助偏好布局。上述前端资产未被原生采样修复改变；最终签名资产仍须核对。

## 本次发布、目录及真实失败证据

- 正式门禁 [36250575121](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36250575121)，签名构建 [36250577536](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36250577536)。[签名回执](evidence/cpar-reliability-m4-20260926/verified-artifact.json)、[兼容回退](evidence/cpar-reliability-m4-20260926/verified-fallback.json)、[演练](evidence/cpar-reliability-m4-20260926/rehearsal.json)、[切换回执](evidence/cpar-reliability-m4-20260926/production-receipt.json)。回退目标fa5f280/schema30，保留最新状态，仅撤销31的两索引及迁移标记；不能切回旧schema28或恢复旧凭据库。
- [真实目录](evidence/cpar-reliability-m4-20260926/real-catalog-receipt.json)：不启后台续期的隔离副本中，一个Build账号返回4个exact模型，Krill两个已配置接口各返回相同21个模型；另两个Build读取Busy，Codex读取authentication。数量是实取结果，不预设为产品容量。Krill现服务的gpt-5.5不在本次目录且测试Key不可见，不能宣称其推理已验收。
- 目录验收脚本最初错误地用SELECT *比较schema28/31行，新增两个NULL续接字段导致断言失败。已按原列重查六个保护表全部相同；[更正证据](evidence/cpar-reliability-m4-20260926/metadata-preservation.json)。没有为修正收据重复访问Provider。
- 首次Pi发送前的外部preflight误把public-models数组当分页，失败后主线程仍启动了1次受控请求，这是验收编排缺陷。现补发送器内强制前置校验：精确revision、10分钟内观测、目标route max_attempts=1；缺失/格式/版本/陈旧/未来/次数/缺route七种错误均零SDK调用、零额度预留。实际请求回读确认只有1个attempt；本次计入1/12，不重置。
- [Pi结果](evidence/cpar-reliability-m4-20260926/pi-live-receipt.json)、[持久回读](evidence/cpar-reliability-m4-20260926/pi-first-readback.json)：HTTP200，首内容2309ms，总2999ms，终态failed/UpstreamProtocolError；未观测usage/账本，不记零。attempt仅表示HTTP建立成功(stage=http_status)，不冒充整次请求成功。旧11隔离事件仍保留，水位已追平。
- 离线发现Build未处理response.incomplete。现补JSON/SSE明确max_output_tokens/content_filter终态、实际usage和不完整文本状态，保留错误身份、未知原因及残缺工具参数的拒绝；19个Build专项、两相关crate共264项通过(5既有ignored)。该缺口有[xAI Responses状态规范](https://docs.x.ai/developers/rest-api-reference/inference/responses)依据，但**没有原始上游帧证据证明它就是本次失败原因**。补安全固定标签日志用于后续定位，不输出上游正文/标识。

独立复核另发现验收器不能仅以Pi的done事件算通过：已在本地验收工具补严格rawStopReason=completed及对应toolUse/stop要求，两个合成incomplete done用例均停止首轮并保持passed=false；加上七个前置检查，共9项脚本测试通过。该工具修订不改变a49c491的网关二进制。

## 流修复候选门禁

a49c491 的正式门禁36252515389在密钥扫描处失败：合成脚本值触发扫描；未部署该候选。将测试值改为明确的非秘密短占位符，未修改扫描规则。079a683ea46f72711ddfc4602d3bde06b08927fc已通过[完整门禁36253429923](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36253429923)及[双架构签名构建36253432916](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36253432916)。产物独立校验、同schema31副本演练和部署仍需后续回执，不能由CI成功推定完成。

## 2026-09-27 流修复发布与第二、三次尝试

079a683已发布，schema31；[签名校验](evidence/cpar-reliability-m4-20260926/stream-fix-artifact.json)、[无网络同schema回退演练](evidence/cpar-reliability-m4-20260926/stream-fix-rehearsal.json)、[隔离登录测试](evidence/cpar-reliability-m4-20260926/stream-fix-auth.json)、[生产切换](evidence/cpar-reliability-m4-20260926/stream-fix-production.json)均通过。切换就绪650ms，保留2,359条既有事件、699账本、7账号、11隔离记录及权限；四文件hash与首批相同，没有DNS/Caddy/Autoreg变更。

- 第2次：Pi/Build/grok-4.5/Responses SSE，修复后单次发送仍HTTP502/UpstreamProtocolError，2,243ms、无首内容。持久记录1attempt/failed、usage和费用未知，无账本。安全日志确认output_item_done、item状态completed；不能把先前incomplete修复说成本次根因。见[客户端](evidence/cpar-reliability-m4-20260926/pi-second-receipt.json)和[持久回读](evidence/cpar-reliability-m4-20260926/pi-second-readback.json)。
- 第3次：Grok Console/grok-4.20-0309/Responses JSON，512上限、无重试，HTTP502。持久回读明确CredentialUnauthorized/credential/non_retryable，366ms、1attempt，需有效授权；harness白名单外编码为other，不能仅凭该值归因。见[客户端](evidence/cpar-reliability-m4-20260926/console-smoke.json)和[持久回读](evidence/cpar-reliability-m4-20260926/console-readback.json)。未再次发送。
- 总计3/12；剩余9次。Build新增固定校验阶段、item类型和密文存在位诊断，绝不输出ID/正文/密文或放宽归属校验。6种坏item边界合成测试、19项Build回归及严格Clippy通过；f1e576c为诊断候选，尚待正式门禁与发布。

当前生产登录态EgoLite仍等待本人登录；M4保持未完成。后续需要真实完成的多轮链路，不能以本地或服务健康代替。

渠道只读复查见[运行状态](evidence/cpar-reliability-m4-20260926/channel-availability.json)：Codex授权已过期，未浪费推理额度验证已知过期；Build三连接中一可用、两过期；Console命中连接为unauthorized，另一连接为available但尚未真实验证，不能断言所有Console账号失效。Krill两协议连接可用仅为运行态观测，目录中没有当前开放gpt-5.5且验收Key不可见，不扩权凑验收。


## 2026-09-27 第四次诊断与原生加密历史修复

f1e576cf07593f648852dce718b8ad4a9102e43a通过正式门禁36259338866、双架构签名36259341429、独立产物校验、无网络副本回退/重升级和隔离登录测试后发布。schema31，切换就绪632ms，保留原有2365事件、699账本、7账号及11隔离记录；前端四文件未变化。证据前缀为本报告evidence目录的item-diagnostic-*。

第4次受控单轮诊断为Pi/Build/grok-4.5/Responses SSE，low、512上限、无重试；HTTP502，2003ms，1attempt、failed，无首内容/usage/账本。安全诊断确认reasoning的metadata拒绝非空encrypted_content；原固定词提取额外误中JSON字段名item_type/message；已只读重查限定时间窗并按MESSAGE.fields精确解析，确认仅1条metadata/reasoning/cipher_present=true，回执已更正。回执为pi-fourth-*。总额4/12，剩余8，不重置。

修复按[CR](../change-requests/CR-M4-OWNED-BUILD-REASONING-001.md)实现stateless AEAD封套、租约前归属验证及精确凭据续接；store:false不转为持久历史。独立复核指出已有continuation kind不可被覆盖，已保留其能力检查并额外要求Build/Reasoning。本地专项与运行装配验证进行中；不得将此实现称为已生产验证。

本地验证：四相关crate共498项通过（5项既有ignored）；新增归属专项3项、真实HTTP＋原生Build mock的stored/stateless两项（各JSON/SSE）及严格Clippy通过。覆盖混合grant、旧revision无证明拒绝/有证明可租约、原continuation kind保留、cipher终态漂移和跨Key零上游发送。以上为本地合成验证，不是真实渠道成功证据。


## 2026-09-27 加密历史发布及第五、六次真实尝试

8916032ef79ea5cc944a0e609ddcea6caf4e38d1已通过完整门禁36262228082、双架构签名36262228134及独立产物验证。无网络schema31副本完成升级/回退/重升级与隔离登录；切换669ms，保留2368既有事件、699既有账本、7账号、11隔离记录和权限。证据为owned-reasoning-*。回退至f1时保留最新状态；旧版不能接受新加密历史封套，相关会话需重启，不丢弃密文伪装续接。

Pi原始SDK默认加密reasoning路径：第5次工具调用成功，HTTP200、completed/toolUse，输出115token；持久记录succeeded/1attempt、2585ms、首内容1956ms，327输入/115输出/99reasoning，1条unpriced账本。第6次工具结果回传后收到文本，但终态failed/UpstreamProtocolError，1160ms、首内容1120ms，1attempt，无可靠usage或账本。已停止剩余两轮，不把HTTP200或局部文本算成功。累计6/12，余6。

限定日志精确确认第6次为metadata/message、encrypted_content_present=false，不能由此推断具体字段。新增固定枚举的metadata拒绝分类（顶层扩展、part类型/扩展、logprobs形状、annotations形状等），只记固定标签，不记录动态字段名、正文、密文；保留原校验策略。两相关crate回归通过，新增分类回归及严格Clippy通过；诊断候选尚待发布。回执pi-fifth-sixth-*及pi-sixth-diagnostic.json。持久处理水位2375已追平，原11隔离保持。

官方xAI Responses当前示例明确包含output_text.logprobs=null，现有闭集此前会拒绝这种合法空值。补充null/空数组的原样保留，JSON/SSE/最终快照/请求回读专项通过；非空或类型错误仍拒绝。此为有规范依据的兼容修复，不以它反推第6次实际字段。8cc2c76仅诊断候选的两项CI主动取消、未部署，改为诊断与空值修复合并验收，避免额外发布和推理消耗。
