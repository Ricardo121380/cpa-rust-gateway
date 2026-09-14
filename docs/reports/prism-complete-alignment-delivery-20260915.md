# CPAR 功能与交互对齐交付

2026-09-15。开发、自动验收、生产迁移与上线完成；现网签名修订 `e77bcb5b79a81fac1db33aec2a1912eb5c9e50fe`，schema28。需本人官方登录的验证单列如下，没有冒充完成。

## 工作区行为

以 CPAMP 日常工作流为主线、官方 Management Center 配置交互为补充，CLIProxyAPI 核对后端行为。固定参考修订和映射见[执行清单](../handoffs/prism-complete-alignment-execution.md)、[行为矩阵](prism-cpa-alignment-matrix-20260912.md)。保留 Apple Liquid Glass、桌面紧凑列表和手机统一卡片。

| 工作区 | 实现行为 | 本轮证据 |
|---|---|---|
| 仪表盘 `#/` | 同时间窗真实请求趋势、成功率、P50/P95、首内容延迟、费用与筛选深链 | 受控5请求：2成功、2失败、1取消、5attempts；终态聚合 |
| 账号 `#/accounts` | 统一身份/渠道/协议；全量搜索、类别/状态/套餐、排序分页；批量、授权导入、更新、启停；五分区详情 | 250条HTTP目录末页搜索及冲突；EgoLite套餐筛选、五分区与Kiro重新授权入口；注入交换回归 |
| 提供商 `#/upstreams` | 紧凑提供商/接口/账号工作区、正式名称、真实协议、目录数与已开放数分离 | EgoLite创建/展开/编辑；真实Build和Krill目录核对 |
| 模型 `#/models`、`#/catalog` | exact ID、多来源、跨页选择、批量开放/关闭、来源和自定义别名；发现不扩权 | 三模型两页刷新；UI开放、别名增删重读；批量第二项失败后停止；旧Key越权404 |
| 密钥 `#/access` | 名称、有效期、显式模型、全选当前、编辑/停用/撤销、最近请求 | UI创建与修改单模型权限，其他Key不变；持久终态索引回归 |
| 请求 `#/monitoring` | 过滤/详情、重试链、终态、时延、脱敏已加载记录导出 | JSON/SSE/错误/截断/取消到持久聚合；100001历史对窄窗回归 |
| 用量 `#/usage`、价格 `#/billing` | 同时间及维度、六类独立置信度、缺价；报价读取、编辑、差异确认与导入 | models.dev实际10来源报价；EgoLite补全合成报价、确认未来目录并保留旧目录；2账本物化 |
| 设置 `#/settings` | 系统、外观、辅助、会话、高级入口 | 深浅色、降透明/高对比/减动画、失效锁定、未保存保护 |
| 高级 `#/runtime`、`#/egress`、`#/versions`、`#/audit` | 诊断、出口、配置生命周期、审计/备份预检保留，普通保存自动管理配置 | 手机动作可见；原查询/确认保护保留；迁移走真实CRUD/校验/发布 |
| 登录 `#/unlock` | 管理员密码登录和会话失效清理 | 实际本地重新登录；签名ARM64首次改密、退出、重启与来源边界通过；EgoLite公网登录页 |

## 验证边界

- 真实目录先在隔离生产副本检查，再通过现网正式刷新接口复验。两个Build账号分别返回1/2模型；第三个运行观测明确为expired，因此返回429且未刷新，不拿其他账号目录填补。Krill两个接口各33模型，完整返回与持久exact ID计数相同。这是当时观测，非硬编码数量。刷新前后有效模型权限不变。
- 真实AWS设备授权启动、pending、取消已验证。本轮Kiro/Claude/Codex官方登录尚未由用户完成；成功交换、重复导入、重新授权和旧回调拒绝使用注入交换HTTP回归，不能冒充真实官方登录。此前真实Grok授权单独记录。
- 额度余额没有观测就保持未知；现有runtime拒绝非空Access Group limits，本轮未虚构共享额度执行器。公开报价缺项须补充，确认币种后导入。历史无时延保持未知；请求数是已接纳推理请求，不是全部HTTP流量。
- 推理全部使用自有loopback TLS mock，真实Provider推理0次。无目录接口的渠道明确来源限制并允许手填原始ID；没有新增Gemini/Vertex/Antigravity渠道。

## 正确性和兼容

BE-FE-01有效授权投影、BE-FE-02版本一致路由/候选/别名维护，以及B1可停止物化、B2存储筛选、B3目录硬过期、B4既有TTL有界维护均保留。原实现与专项证据见[原V4报告](prism-v4-delivery.md)；本次另验请求终态到物化与兼容回退，不以历史测试数代替本次结果。

schema28仅增加Key终态查询索引。兼容回退为签名`b792a99`，支持schema28与当前凭据格式；不降schema、不回灌旧凭据。Rustls更新0.23.45修复本次正式门禁发现的RUSTSEC-2026-0285，未加忽略。

## 验证和产物

296项前端单测；Kiro创建/替换/停用与连接保持/晚到冲突HTTP回归；workspace all-target/all-feature Clippy；145operation同步、类型、四文件双构建和嵌入门禁通过。EgoLite16读取真实本地嵌入应用，14入口在1440×900、1280×720、390×844深浅色逐页检查，修复手机目录、访问组、运行矩阵、审计和出口操作溢出；另验辅助、焦点、未保存取消及会话失效。

可审查的脱敏收据保存在[evidence/prism-complete-alignment-final](evidence/prism-complete-alignment-final/production-receipt.json)，完整本地截图在`output/prism-final-qa-20260915/`，发布与演练材料在`output/prism-complete-release-20260915/`。凭据、数据库和账号身份不进入报告。

## 发布状态

现网入口：[Prism 管理端](https://cpar.142857142.xyz/admin-ui/#/unlock)。EgoLite已保留此页供本人验收，使用原管理员密码。

- 双架构签名构建：[34880624683](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34880624683)；最终正式门禁：[34880628832](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34880628832)。主版本和回退的ARM64/x64产物、SBOM与Cosign身份均独立验证。
- 网络隔离副本实际完成27→28→正式命名/别名迁移→兼容回退→候选，终态记录及物化checkpoint保留，管理员存储字节及凭据材料保留。
- 生产切换就绪1027ms；活动配置`production-aligned-20260915`，正式提供商名Codex/Grok Build/Grok Console/Krill，六旧别名从活动配置删除；历史配置与正常自定义别名能力保留。原始模型、来源与所有有效Key权限保持。
- 7个账号、2229条事件、593条账本及原管理员保留；公网四资源哈希、CSP、未认证404、health、进程二进制哈希与双loopback监听通过。未修改DNS/Caddy/Autoreg，没有真实推理调用。
- 远端私密演练/停服前备份/迁移收据：`/var/backups/cpa-rust-gateway/prism-complete-20260915`。回退只切换兼容`b792a9920b515436d0c745424f91157cd6274ff2`，保留最新数据库和旋转凭据，不切回schema27旧版、不还原陈旧备份。若用户已发布新配置，先核对兼容性再回退。

## 人工验证与明确限制

已过期的一个Build账号需要本人重新授权；Kiro/Claude/Codex官方账户登录完成后的真实交换仍需本人参与，不能由本轮自动化代替。其余已授权实施和发布事项没有作为“下一期”遗留。未新增的渠道、共享配额执行器、自动真实推理巡检、在线秘密导出和恢复仍不声称支持。旧客户端必须使用原始模型ID；本轮没有自动开放新发现模型。
