# #61 定向一手补证：cost 字段继续 NOT_ADMITTED

日期：2026-10-18（Asia/Shanghai）。**第一阶段 B：限定检索已完成；字段 NOT_ADMITTED；#61 OPEN / BLOCKED。**
本轮没有取得满足现有资格要求的一手依据，因此没有进入成功元数据实施阶段。
[cost 契约草案](cpar-61-cost-contract-draft-20261018.md)继续为 PROPOSED；用户此前选择
“保持 NOT_ADMITTED，等待补充一手依据”的决定保持。该草案与 ADR-0107 的 Accepted 方向均不批准具体字段。

## 承接与范围

实际起点为 `codex/prism-v4-delivery` / `db68928e3f3c6cd78ddca152d205b047b41147a7`，与参考一致；
`fc1c4755c05bdfa7d91812fd77b9494225cc161a`仅用于定位，未回退。索引最初为空，9 个既有跟踪文档改动保留。
已完整阅读指定的 continuation、cost draft、field qualification、field-decision、review-final，
以及相关 ADR-0107/0109/0110/0111/0112、BC-PROVIDER-003/024、BC-OBS-006、CONTEXT 与仓库规则。
重新读取 #58/#61 正文和全部评论；#61 最新评论仍要求保留字段阻断。
原生依赖逐页核对：#59/#60 CLOSED；#61 OPEN；#62/#63 OPEN 且仍被 #61 阻塞。

研究限定为实际 `cost_in_usd_ticks`，固定渠道 `grok.build.responses`、
`https://cli-chat-proxy.grok.com/v1/responses`、production profile **1.0.50**、标准 Responses JSON/SSE。
两个一次性只读探子分别检查版本发布依据、官方字段文档；主代理亲自阅读基础材料、点验承重出处并保存本报告。
未重复冻结归档的 4,487 文件扫描；公开 main 未变，未制造无新增源码的全仓差异检索。
来源 URL、UTC、HTTP 状态、字节数、SHA256、查询及访问限制见
[来源与范围回执](assets/cpar-61-targeted-evidence-20261018/public-source-manifest.json)。
当前网页以取得字节的哈希固定快照；这不表示网页内容属于 production 1.0.50。

## 证据表

| 待证明事项 | 一手来源与固定版本 | 适用范围 | 已证明内容 | 尚未证明内容 |
| --- | --- | --- | --- | --- |
| 冻结源码或正式发布与 production 1.0.50 对应 | [固定 README:31](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/README.md#L31)、[version:11](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-version/src/lib.rs#L11)；[refs](https://api.github.com/repos/xai-org/grok-build/git/refs?per_page=100&page=1)、[tags](https://api.github.com/repos/xai-org/grok-build/tags?per_page=100&page=1)、[releases](https://api.github.com/repos/xai-org/grok-build/releases?per_page=100&page=1)及当前 installer 快照 | 此公开仓库的列出范围、当前安装脚本文本 | main 仍为 `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`；SOURCE_REV 是 monorepo revision；发布版本由 GROK_VERSION 注入；tags/releases 返回空页 | 没有把该树/SOURCE_REV 或正式制品绑定至 1.0.50 的发布依据，也没有目标服务端适用声明 |
| 非流式 JSON 的字段位置、形状与语义 | [固定 client.rs:1276/1374](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/client.rs#L1374)；[Build headless](https://docs.x.ai/build/cli/headless-scripting.md)当前快照 | 前者为 typed Response 消费；后者为 CLI stdout | JSON 使用 typed Response；CLI 文档描述 JSON/换行 JSON 输出 | typed client 忽略字段、CLI 输出及既有单次样本均不定义目标 `$.usage.cost_in_usd_ticks` 的服务端契约；JSON 的字段/容器形状、单位与 profile 关系仍缺依据 |
| SSE terminal 的位置、单位与处理范围 | [固定 client.rs:124](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/client.rs#L124)、[conversation.rs:830](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation.rs#L830) | 冻结客户端对 completed/incomplete SSE 的 cost 提取 | 路径为 `/response/usage/cost_in_usd_ticks`，经 `as_i64` 后保留正值；文档注释定义 1 USD = 10^10 ticks | 仅有客户端范围的语义证据；没有 production 1.0.50 的服务端 schema、完整 phase 范围或 JSON 共用声明，不能外推 |
| missing/null/zero、数值边界与异常形状 | [固定 conversation.rs:858](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampling-types/src/conversation.rs#L858)和上述提取路径 | 客户端捕获和归一化规则 | 注释说明 REST 会以 0 回填未报告成本、负 ticks 无效；该客户端将非正值归为未报告，而非免费 | 未定义固定 Build 服务端的缺失/null 合法性、实际数值上下界、精确数值表示及异常容器/类型规则；i64 读取能力不是服务端合法域，草案边界未获批准 |
| 与回答、原生 summary、工具、标准 Usage、模型身份独立 | [固定 stream/responses.rs:615](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/stream/responses.rs#L615) | 冻结 SSE 客户端的内部处理路径 | 标准计数、cost 提取和 response item 转换分别处理，支持该客户端路径中成本与内容分开的判断 | 不能据此宣布目标 JSON/SSE/profile 的共同资格、全部标准语义不受影响或模型别名成立；获准后的公共行为/持久化证明仍需实施 |
| 新官方 schema 能否填补上述缺口 | [llms.txt](https://docs.x.ai/llms.txt)、[OpenAPI](https://docs.x.ai/openapi.json)、[Cost Tracking](https://docs.x.ai/developers/cost-tracking.md)当前哈希快照 | 官方明确列出的 `https://api.x.ai` 公共 REST API | OpenAPI 为公共 API；不同 schema 的同名字段存在 integer 与 integer/null 差异；取得的 schema 未出现目标 proxy/profile | 没有适用于 OAuth Build proxy 的声明；这些内容仅用于排除错误适用范围，**不作为 Build 缺失依据的替代品** |

## 新查范围与限制

- 公开仓库 refs、branches、tags、releases 以每页 100 检查至空页，最长两页，未触达预设三页上限。
  未发现新的公开源码 ref 或 1.0.50 release 映射；这是本次列出范围的结果。
- 检查官方 [install.sh](https://x.ai/cli/install.sh)、[install.ps1](https://x.ai/cli/install.ps1)
  以及官方安装入口中的发布映射链接。脚本可选择显式版本，但未提供所需源码对应；PowerShell 的 checksum 属于
  bundled MinGit ZIP，不能用来证明 Grok 主制品。没有下载、运行 installer 或正式二进制。
- 两个官方 stable 指针本次均为 **1.0.62**，主代理独立点验一致；历史 1.0.50 观测保持。
  当前指针既不固定旧版源码，也不授权改 profile，本轮保持 1.0.50。
- 从官方 sitemap/llms.txt 定位并读取 Build overview、headless-scripting、settings/reference、status-line 四页，
  以及其公开 OpenAPI/Cost Tracking。CLI JSON 是 stdout；状态栏 cost 是当前进程统计，恢复时重新累计。
  二者均不提供目标 HTTP 字段的生产契约。没有抓取全部文档或 llms-full.txt。
- 两个探子分别执行 5 个版本映射查询、6 个字段/端点精准查询，见来源回执。搜索结果与第三方材料未作为准入依据。
- changelog 的直接 GET 返回 **403**；网页提取服务能读取其 1.0.50 / 2026-10-06 版本章节，
  但未提供原始 HTTP 状态/响应哈希。该版本章节仅证明发布说明中的版本记录，不能当作制品或服务端映射。
  两个试探文档入口的网页工具读取失败但未给出 HTTP 状态，未伪称 404；随后经 sitemap 找到实际入口。

主代理重新取得的三个关键 cost 源码文件、version 与 SOURCE_REV 的哈希全部与前轮 manifest 相同；
只点验相应行段，没有重做归档检索。公开 OpenAPI 的服务范围、两个 Build 文档关键位置与当前 main/stable
也完成独立点验。否定结论仅覆盖记录的来源、查询和快照，不能推断所有公开或未公开资料不存在。

## 最小所需外部材料

需要生产端所有者提供一份**可固定版本/日期、明确适用于目标 endpoint、production profile 1.0.50 和标准 Responses 的一手字段契约**。
它可以直接作出正式适用声明；若通过源码/制品证明，则需给出可核验的 release/source 对应，不能只给当前 stable 或孤立 checksum。
同一材料可以同时覆盖以下四项，不要求新 Provider 推理或新的样本：

1. 分别声明 JSON `$.usage.cost_in_usd_ticks` 与 SSE `$.response.usage.cost_in_usd_ticks` 的字段及容器 schema，
   以及 SSE 可出现的事件/终态范围；共用定义必须显式说明两种传输适用。
2. 明确单位、每请求/每 turn/累计范围，以及与工具执行、折扣或其他费用度量的关系。
3. 说明 missing、null、zero 的含义，数值类型与上下界、数值表示和负值/非整数/字符串/超界/异常容器的规则。
4. 提供其独立于回答、summary、工具名称/ID/参数/结果关联、标准 Usage 与模型身份的依据。

取得这些材料后仍需重新作字段资格判断；资格成立才实施成功 bytes/终态前确认、完整原始 Usage 留存和授权读回。
本轮不再要求用户批准已选择暂不批准的草案。

## 交付与未完成项

[资格决定](assets/cpar-61-targeted-evidence-20261018/field-decision.json)记录第一阶段 B。
全部 14 条 AC 的继承状态不变：AC1/3/5/10/13/14 PARTIAL；AC2/7/9/11 BLOCKED；
AC4/6/8/12 保留前轮的 scoped PASS 和适用限制，**不是本轮重跑结果**；成功获准元数据变体仍缺失。
此前严格 root/status/phase、失败 Usage、标准工具循环和取消/RST/EOF/timeout/资源释放成果保持；
旧 Full 45/45、Rust 1542/0/13 继续仅绑定原候选，不冒充本轮测试。

本轮只有研究文档和收据增量。适用验证为文档链接、JSON 结构、暂存秘密扫描、空白与所有权/保留核对，
见[验证与保留回执](assets/cpar-61-targeted-evidence-20261018/verification.json)。没有产品代码改动，
按第一阶段 B 未触发 `$implement`、行为测试、Full 或第二阶段候选的 `$code-review`。
未修改契约/ADR 状态、schema 35、observer/事务确认/AEAD/reader 或任何字段准入。

已仅向 [#61 回写一条本轮结果](https://github.com/Ricardo121380/cpa-rust-gateway/issues/61#issuecomment-6251598716)并独立读回；正文、元数据、原评论和原生依赖的核对见
[远端回执](assets/cpar-61-targeted-evidence-20261018/remote-result.json)。
其他未知字段、额外计数、匿名 Usage object 和未经证明的真实别名继续阻断。
真实 Provider 推理、新增真实账号请求预算、私有 capture/真实凭据读取、实际客户端验收、push、部署和发布均 NOT_RUN；
没有启动 #62/#63，也没有修改其他工单。
