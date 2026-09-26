# CPAR M4：容量、真实验收与发布

状态：进行中。实施基线 `fa5f280fe897158aac5130c7d7f6324f1aa2c550`。
对应[计划](../handoffs/cpar-reliability-alignment-plan-20260926.md)和[验收清单](../handoffs/cpar-reliability-alignment-acceptance-20260926.md)。

## 当前进度

- M4-01/02：10万/100万精确查询、容量采样及告警通过本轮本地验收。
- M4-03/04：`3de0319` 正式fast/supply-chain通过，Rust 1,356项、前端415项；双架构签名与独立校验通过。EgoLite真实嵌入应用48个工作区状态、18个弹窗状态及模型→受限Key→mock请求→账本通过。
- M4-05：完整生产副本28→31→兼容30→31通过；新格式事件/checkpoint、管理员、历史、轮转密文和权限保留，演练无网络。
- M4-06：真实推理 **1/12**，首个Pi Grok Build请求在HTTP200后流错误，准确持久为失败，无自动重试；多轮闭环未完成。真实目录读取单独记录为元数据验证。
- M4-07：`3de0319` 已部署，schema31，切换到就绪1,031ms；公网4资源hash/CSP/鉴权、7账号、原有2,356事件/699账本与权限保留核验通过。DNS/Caddy/Autoreg未改变。
- M4-08：生产EgoLite登录页已交给用户，等待本人登录；不能用登录页或本地矩阵代替生产登录态验收。
- M4-09：报告持续更新；新增流协议修复尚待签名发布和真实核验，**M4未完成**。

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
