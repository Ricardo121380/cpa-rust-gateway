# CPA-Rust / Prism 三协议字段映射决策与依赖门禁修复

**本批依赖修复及本地 Full 已完成，三协议 rich 语义的实施仍待下一批。**
用户已确认 opt-in native-v1＋专用消费者、完整非结算 Usage 证据，以及有证据的模型别名规则。
这批准了交付形式和下一批实施方向；没有改变当前协议行为，没有证明本样本模型等价，
也没有将标准客户端、真实渠道或 #9 的全范围验收写成 PASS。

本批真实 Provider、fresh Pin、真实 Chat、远端验收实例、正式候选、远端 CI、部署、发布均为0/NOT_RUN。
旧真实 FAIL、原响应、候选和 stopped 预算保持。所有新增消费者和 TLS 输入为合成数据。

## Actual baseline and preservation

开始 HEAD=`312584184e7680594878460499c39af0a244d440`，恰等于比较检查点；没有回退。
适用 AGENTS、single-context 领域入口、ADR-0100/0101/0105、两份指定 BC、quality gates、
#47/#57/#9 的正文/既有评论/双向分页依赖均已实际读取。

上一轮六个受测 Rust 文件的SHA与当前一致；13份压缩原日志解压长度/hash全部一致，
27份 GitHub 原件/载荷绑定重新核对通过。没有重新捕获或重跑原样本首拒点；旧证据保持原结论。
详见 [baseline](assets/cpar-field-contract-dependency-20261017/baseline.json)、
[history check](assets/cpar-field-contract-dependency-20261017/history-check.json)。

开始冻结55,218个tracked/nonignored untracked文件。除本批拥有的 Cargo.lock、ADR索引、
ADR-0101新增链接和traceability四条既有路径，55,214个文件的字节/长度/权限均未变；
用户 AGENTS 和所有其他原有未跟踪内容保留。[preservation](assets/cpar-field-contract-dependency-20261017/preservation.json)
记录完整本地manifest摘要及检查数量；[最终保留核对](assets/cpar-field-contract-dependency-20261017/preservation-final.json)
再次核对同一批原有文件。55,204个普通文件及10个符号链接均保持原字节/长度/权限；
末态脚本初次跟随链接与初始link-byte清单比较的误报回执独立保留，未改变任何旧链接或目标。
新材料仅在本批ADR、报告/assets与独立output目录。

只读探子的两项不完整结果没有记为执行PASS：SDK安装/保存受只读角色约束，初始Python无tomllib。
主代理补完固定SDK实际执行、锁差异及约束核对；另直接定位冻结driver里的
`verify_success_fidelity`，没有把普通scripts范围无该符号误当成历史oracle不存在。
[research verification](assets/cpar-field-contract-dependency-20261017/research-verification.json)保留边界。

## Dependency delivery and Full

| 修改 | 本地不可变提交 | 实际范围 |
|---|---|---|
| rustls 0.23.45→0.23.46 | `28568553ce5d8754f4063884c1c9c37821f68c9f` | Cargo.lock仅版本和checksum |
| quinn-proto 0.11.16→0.11.18 | `2f43057c9d718aee5904da12564d54ac697bf8b4` | 同样仅版本和checksum；用户第三张决策卡追加授权 |

rustls实际由reqwest0.12.28/hyper-rustls0.27.9/tokio-rustls0.26.4引入，
约束分别 ^0.23.4/^0.23/^0.23.27。启用feature仍ring/std/tls12，未打开brotli/ECH/PQ新路径。
四份当前公告 [0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html)、
[0347](https://rustsec.org/advisories/RUSTSEC-2026-0347.html)、
[0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html)、
[0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html) 均给出最低修复0.23.46。
注册源对应官方release SHA=`f04804fa13341a9b1dc02b5d1dd8a746f2c4b7fb`。
约束、完整feature树及重复依赖图实际比较；rustls补丁不改变重复警告的package/consumer图。

首次新版 Full 的 Dependency policy PASS，但 RustSec 首次执行暴露quinn-proto五份公告。
当前all-features/all-targets活跃图没有启用该optional链，而audit仍扫描完整锁文件。
五份公告为 RUSTSEC-2026-0338/0339/0340/0341/0342；其中两份要求≥0.11.18，
其余≥0.11.17。故最低共同修复为0.11.18，quinn0.11.11的^0.11.15容纳它，MSRV仍1.85。
先提供两行锁补丁与隔离audit通过证据，收到用户明确授权后才应用。
没有升级reqwest/quinn或其他包，没有新增ignore、依赖、feature或门禁放宽。

| Full epoch | 结果 | Rust / supply chain |
|---|---|---|
| 上一轮原Full | **FAIL，永久保留** | 43 PASS，Dependency policy FAIL，audit NOT_RUN；1515/0/13 |
| 本轮rustls-only | **FAIL，永久保留** | 44 PASS，Dependency policy PASS，audit FAIL：quinn-proto 5公告；1515/0/13 |
| 最终双补丁 source `2f43057c…` | **PASS 45/45** | 1515 PASS / 0 FAIL / 13 IGNORED；deny及audit均PASS |

最终 Full 在该提交的全新干净隔离检出执行，开始/结束均clean，锁文件与交付源码相同。
包含格式/Clippy、Prism依赖与build检查、Rust测试、本地进程/公共协议/多轮回归、源/契约/
文档/秘密/whitespace以及固定质量工具、deny、audit全部必需步骤。
audit/deny数据库实际HEAD相同：`64f92cdc64401fab47baa10e2e11fe382aa57155`。
重复依赖warning仍是已评估的原图；没有改成静默忽略。

证据：[dependency change](assets/cpar-field-contract-dependency-20261017/dependency-change.json)、
[rustls-only FAIL](assets/cpar-field-contract-dependency-20261017/full-rustls-only.json)、
[原日志](assets/cpar-field-contract-dependency-20261017/full-rustls-only.log.gz)、
[final Full](assets/cpar-field-contract-dependency-20261017/full-final.json)、
[45步回执](assets/cpar-field-contract-dependency-20261017/full-final-steps.md)、
[无损日志](assets/cpar-field-contract-dependency-20261017/full-final.log.gz)。

## Local TLS/HTTP and consumer evidence

额外TLS测试调用实际 `UpstreamClientPool::send` 的DNS固定Standard reqwest路径。
TLS1.2/TLS1.3均经合成CA验证并完成HTTP往返；wrong-host与untrusted-CA失败，HTTP接收数0；
成功场景独立接收数1。没有关闭验证、写Keychain/全局根、使用凭据或真实Provider。
probe库源码固定于rustls-only提交；相关源码SHA与最终提交一致，quinn optional链未参与该路径。
[TLS receipt](assets/cpar-field-contract-dependency-20261017/tls-receipt.json)及
[source/binary binding](assets/cpar-field-contract-dependency-20261017/tls-binding.json)记录4/4 PASS。
初始系统Python3.9.6/LibreSSL2.8.3不支持TLS1.3的工具错误独立保留；TLS1.3用现有OpenSSL3.6.3完成。
不是删除负例、放宽TLS或新安装全局工具。

固定OpenAI Node7.36.0与Anthropic Node0.135.0只在独立临时目录安装，禁用安装脚本。
七个内存fetch场景证明：JSON未知字段可访问，但Chat扩展片段覆盖、Messages末态新增Usage丢失，
Responses自定义事件拒绝（无自定义事件的对照通过）。普通JSON大整数解析也有精度反例。
这些事实要求profile专用消费者；不能据“JSON保留扩展”宣布完整标准兼容。
[验收矩阵/消费者证据](cpar-field-mapping-acceptance-20261017.md)列出复现步骤和范围。
产品浏览器检查不适用本批依赖/契约切片；Prism UI行为未改，实际浏览器/其他客户端均未宣称通过。

## Confirmed semantic direction and remaining UNKNOWN

用户三张决策卡已全部回复，见 [decisions](assets/cpar-field-contract-dependency-20261017/decisions.json)。
[ADR-0106](../adr/ADR-0106-native-fidelity-profile.md)的架构方向因此为Accepted；
[字段schema](assets/cpar-field-contract-dependency-20261017/native-fidelity-v1.schema.json)和
[验收矩阵](cpar-field-mapping-acceptance-20261017.md)是下一批实施契约草稿，不是已实现能力。

- opt-in native-v1根封套＋专用消费者：summary/body分层、item/part身份和顺序独立保持。
  默认严格路径保留，标准客户端矩阵继续独立验收；不把summary写入完整body。
- 完整native Usage以精确raw_json和presence保留0/null/[]/未知值，并按确切Attempt加密保存。
  六计数/证据/账本关系维持；原生费用状值不是已证明charge。
- public→actual upstream→native使用三段断言；字面同名或预先冻结的适用别名证据。
  原样本实际native别名关系仍UNKNOWN；原等值oracle与原始失败未改。

方案定义JSON/SSE事务、唯一终态、字段/aggregate/UTF-8/frame/depth边界、已知与未知值规则、
root闭合准入、冲突拒绝、加密证据/reader迁移和历史回放。当前durable UsageSummary不含raw，
native root也未全量保留，这些是必须实施的具体seam，不能仅加编码字段冒称完整保真。
本批产品源码除锁文件外没有协议行为变化；当前rich Chat/Messages及额外Usage的严格拒绝保持。
Build未知字段意义、独立费用、模型别名、专用产品消费者及新profile各模式都是UNKNOWN/NOT_RUN。

## Review and GitHub reconciliation

最终固定base/head/文件集的Standards与Spec双轴审查均已完成，以下分别记录结论。
依赖门禁PASS不替代协议或真实验收PASS。

最终固定审查 **`312584184e7680594878460499c39af0a244d440 → a0aafba8e67b4881bc68802693f8ea63e50c266f`，55文件**：
两路增量复核结合首轮完整覆盖，均complete。**Standards PASS（硬发现0、建议0）；
本批Spec一致性PASS（新增违约0、scope creep0），初P2 closed；原#9仍PARTIAL/BLOCKED。**
见 [最终双轴回执](assets/cpar-field-contract-dependency-20261017/review-final.md)及
[固定文件摘要](assets/cpar-field-contract-dependency-20261017/review-final.json)。主代理点验55份
commit/working bytes、Full原日志、SDK9包/7结果、ci红绿归档、39份GitHub绑定及AGENTS保留。
记录审查本身的后续提交只增加结果/保留记账和本段，不改受审schema、源码、执行证据或已同步正文。

首次固定 `31258418→a09611e2` 的49文件审查为Standards PASS（0/0）、Spec本批一致性FAIL
（P2 1项，scope creep0）：SDK包锁使用外部临时相对路径，报告的clean npm ci不能可靠复现。
[首轮审查](assets/cpar-field-contract-dependency-20261017/review-initial.md)永久保留。实际clean ci
先复现RED，之后仅修portable锁metadata/路径键，9包版本/URL/integrity与7项结果不变；
另一全新目录clean ci＋probe GREEN，见 [replay](assets/cpar-field-contract-dependency-20261017/consumer-clean-replay.json)。
此修复只涉及消费者验收材料，不改产品/Cargo依赖/协议；修正范围已重新固定并独立审查。

本批只允许追加既有progress #47/comment6081215242、#57/comment6081234278、#9/comment6081235490。
同步采用body-only PATCH，旧前缀保留；各次独立GET评论，之后重读全部评论/工单及双向分页依赖。
实际预检三票均OPEN，依赖8/23/0，#57仍11个OPEN依赖；回读确认这些状态保持。

实际同步现已 **PASS 3/3**：对既有 [#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、
[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、
[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)各一次body-only PATCH。
每票评论等于旧body＋3050字节追加正文；独立GET及全部评论/工单/双向分页依赖核对通过，
历史前缀、原body/AC、state/labels/assignees/milestone和依赖边/状态保持，updated_at变化绑定
各自PATCH时间窗口。created comments0、closed issues0、issue body/metadata writes0、push=false。
首个只读预检中断时PATCH0，原件保留；独立GET恢复后重新完整预检，没有重试任何可能已应用的PATCH。
见 [同步回执](assets/cpar-field-contract-dependency-20261017/github-sync-summary.json)及
[39份实际响应/载荷绑定](assets/cpar-field-contract-dependency-20261017/github-readback-bindings.json)。

新增材料静态检查的两个初始失败亦保留：合成SDK占位符碰到16字符扫描阈值，缩短后七项
probe结果不变；合法unified diff的空白context行碰到whitespace门禁，原patch本地保留并以
gzip逐字节归档。未放宽扫描/whitespace或加例外；秘密扫描、Markdown链接、191项
契约引用、JSON/Python语法和diff静态检查均通过。最终审查回执的文档提交另作同类静态核对，
见 [交付静态回执](assets/cpar-field-contract-dependency-20261017/delivery-static.json)。

## Next batch and candidate prerequisites

下一批可直接实施已确认方向：先固定字段schema/新BC和消费者接入，再补native片段/root证据、
Canonical typed保留、三codec/projector、SSE reducer、按Attempt加密记录与reader/migration、
模型关系oracle；按配套矩阵从实际公共入口验收，保留所有标准/扩展、成功/拒绝的分层证据。
存储/续接/compact/WS每项仍需声明及单独实现资格，不凭JSON/SSE成功升级。

新正式候选资格还需：

1. 当前默认严格契约与profile契约实现及local positive/negative矩阵全部满足；模型关系有适用证据。
2. 最终不可变实现提交上的适用Task/Phase/Full和双轴审查完成；本次依赖Full不能覆盖后续行为修改。
3. 新aarch64-unknown-linux-gnu候选的source/binary/build metadata/manifest绑定，reader、egress、
   Linux启动/退出等同候选门槛重新确认；旧材料不能自动继承。
4. 真实复测目标、账号/渠道/Key/route/model/config/actual lease/egress/原请求hash与新预算另行冻结和授权。
   若沿用先前建议，fresh Pin≤1＋原body Chat≤1也是新epoch的新授权，不转移旧stopped预算。
   首个FAIL/UNKNOWN/身份或证据矛盾永久停止，不retry/fallback/换账号模型/refresh/修复后追加。

在这些条件达成前，候选资格和真实渠道/总验收继续BLOCKED；本批不创建正式候选，不发起Pin/Chat。
