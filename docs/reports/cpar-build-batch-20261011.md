# CPAR #47：20261011 基础真实批次、SSE 验收修复及退出

本批真实基础验收 **FAIL，已停止**。Responses JSON 新 Pin 与全部 9 个基础请求通过：文本 3/3、单工具 2/2、连续两轮工具 4/4，取得修复后真实工具结果 `1` 的闭环证据。Responses SSE 新 Pin 通过，但首个基础响应触发验收器 `changed_part_snapshot`，后续请求停止。完整场景 **3/18**、完整组合 **1/6**。新增真实推理 **12（Pin2＋基础10）**，历史9保留，累计 **21/69**。剩余额度不续用，修复后真实重跑0，扩展真实请求0。

## 基线与执行身份

开始 HEAD=`2d66f892a0e507becbf9d2bd1f0142a64d16fd4a`，分支 `codex/prism-v4-delivery`，相对用户历史检查点没有新增提交；没有回退。产品源码仍为 `1b6023f52f4c00ea6d8296524d306c141e4edae7`。用户 AGENTS.md 与无关 tracked／untracked 内容保留。本轮只拥有验收脚本、对应测试和本批新报告／资产，不改产品源码、前端或共享契约。

已读取适用规则、跨边界日志、issue tracker、[上批真实与修复](cpar-build-six-combinations-20261010.md)、[17类清单](cpar-build-extensions-next-20261010.md)、[正式入口说明](../../scripts/acceptance/cpar-new-build-acceptance.md)、直接引用的授权及连续性／存储／WebSocket 契约，以及 GitHub #47／#57／#9 正文、原评论、双向完整 dependencies 与相关依赖。历史检查点只用于衔接。

正式基础执行脚本、依赖与远端字节在发送前冻结并核对，见 [freeze](assets/cpar-build-batch-20261011/freeze.json) 和 [远端字节核对](assets/cpar-build-batch-20261011/remote-byte-verification.json)。

| 身份 | 本批实际值 |
| --- | --- |
| 基础工具源提交 | `c046ba2d97304f9c694b5d05e6efae9182df08a4`，起始 HEAD 的脚本字节相同 |
| 真实正式脚本 SHA256 | `b33e551ef018df71a4bb0d3e301249b75eaf603d46590b20529e649ee1ff9d8a` |
| 既有入口 SHA256 | `7d4187759fb86210d5791c37a397f385e4c3254c69bf1f95319bdd55ee5f992f` |
| binary SHA256 | `ea89d5c72628a871f7683f32d0b51a76dc9c588f7172c86345d1c989f1bd9063`，Linux AArch64，19,500,176 bytes |
| manifest 文件／对象 SHA256 | `20ef11961ca40d4dd659ae278c1e7ee629f9f70319f4a4b006d2d8685a5c9777`／`ffaea62e61fa3c974d2e4a212556249ac2db10963657db9d078eb7e151de7d14` |
| 临时 PID／process_start | 2390292／`657900734`，退出后已失效 |
| config | `cpar-build-config-20261010`，revision9／管理 `rev-9` |
| endpoint／route／candidate | `cpar-build-endpoint`／`cpar-build-route`／`cpar-build-candidate` |
| channel／public／upstream／response model | `grok.build`／`grok-4.5`／`grok-4.5`／`grok-4.5` |
| native 材料 revision／实际 lease | 134／135 |
| 账号 opaque 身份 | `5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54` |
| Client Key／projection | `ff7ef703ed7ad99438aa143c89b8f17704d93746574f3e557875440274181025`／`76b46919f61a1174ea8bab32a9ba08174812e6d068e97c80f0eb9b6070802e25` |
| egress | direct，`69aa38e91dc761ae26632216b5fc04e7362ce1437b2116cc15340c6c77e830b6`，revision=`not_applicable` |
| serving 声明 | reasoning／stored／continuation／compact／websocket=true；parallel=false |

只读重新核对同一授权来源材料：enabled／active／available、active leases0、worker claim 空，账号及 identity digest 与既有授权来源相同。当时 expiry=`2026-10-11 05:49:42.392 CST`，不是本任务主动刷新。只从原自有隔离状态 backup 和该账号最小材料建立新 epoch，不复制生产业务库，不修改来源。来源 schema31、自有候选 schema34；[readiness](assets/cpar-build-batch-20261011/readiness.json) 的 `source_sqlite_user_version=0` 是 SQLite PRAGMA 标记，schema 版本以 [prepare](assets/cpar-build-batch-20261011/prepare-receipt.json) 的 migrations 为准。原只读工具使用旧字段名 `source_schema` 的字节保留，不能把0解释为 schema0。

新 epoch：`new-vps:/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010/private/six-combinations-20261011/`，没有覆盖旧 epoch、wire、失败或时间戳。保护根归属 `cpar-build-20261010`；启动前 unit 未加载、端口空闲、目录／fd 无其他任务占用。新材料重新绑定本批 Pin 和实际 lease，未套用旧会话证明，E-05 精确连续性例外不扩大。

[启动回执](assets/cpar-build-batch-20261011/start-receipt.json) 证明后台 Provider tasks disabled，OAuth／refresh／自动 catalog 不运行；guard 已映射、权限有效，无代理环境，单账号池／单 enabled candidate／max_attempts1，无 fallback／隐式 retry。连续65.60秒网络审计不增、管理审计不变。每轮新观察保持600秒规则，未修改旧观察或放宽门槛。

## 六组合基础实际结果

| 组合 | 新 Pin | 文本多轮 | 单工具闭环 | 连续两轮工具 | 基础组合 |
| --- | --- | --- | --- | --- | --- |
| Responses JSON | PASS，1 | PASS 3/3 | PASS 2/2 | PASS 4/4 | **PASS 9/9** |
| Responses SSE | PASS，1 | FAIL，发送1/3 | NOT_RUN 0/2 | NOT_RUN 0/4 | **FAIL，发送1/9** |
| Chat JSON | NOT_RUN 0 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Chat SSE | NOT_RUN 0 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Messages JSON | NOT_RUN 0 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Messages SSE | NOT_RUN 0 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

Responses JSON [Pin](assets/cpar-build-batch-20261011/responses-json-pin.json)、[执行摘要](assets/cpar-build-batch-20261011/responses-json-basic-summary.json) 和 [正式基础回执](assets/cpar-build-batch-20261011/responses-json-basic.json) 分别记账。9轮完整 Request／唯一 Attempt／精确运行身份、成功 JSON 终态、有序历史、工具参数／原结果／call_id／原生 items、Usage 及账本 source-event 关联通过。单工具及连续工具最终答案严格包含实际工具结果；下一指令保留为正常 user 消息。Pin 不要求不存在的 Usage／ledger。

Responses SSE [Pin](assets/cpar-build-batch-20261011/responses-sse-pin.json) 通过；[原基础回执](assets/cpar-build-batch-20261011/responses-sse-basic.json) 与 [摘要](assets/cpar-build-batch-20261011/responses-sse-basic-summary.json) 保留首请求后的 FAIL。原响应终态是 completed，但验收器在 part 结束事件发现额外空元数据时拒绝。不能由一次 completed 或修复后的离线解码推导 SSE 三轮／组合通过。

## SSE 离线复现、最小修复及证据范围

原真实 wire 中，part.added 只含 type/text；part.done 在同一文本上补入 `annotations:[]`／`logprobs:[]`。产品 `protocol-openai-responses/src/native_output.rs` 的 append_native／finish_native 就采用这种编码；validate_parts 明确支持未请求 logprobs 的 null／空数组。原 formal 对整个 part 提前作严格 equality，导致确定性误拒绝，详见 [原诊断](assets/cpar-build-batch-20261011/sse-red-diagnostic.json)。

先用保留的旧脚本和新回归测试复现 `changed_part_snapshot`；随后仅允许 output_text 终态补齐缺失的空 annotations、空／null logprobs，并保存其原 representation 继续核对 item.done 和 response.completed。文本变化、非空新增元数据、未审查字段、字段消失及终态矛盾继续 FAIL。未删参数、丢历史或放宽答案／归属检查。

最小修复版 formal SHA256=`9b186dc0140292c3767b2b6064b45f67841dabca7ab9d0ef9f200079b69e8802`，[39/39 测试](assets/cpar-build-batch-20261011/sse-fix-tests.json) 包括六协议模式 HTTP 9请求、实际 SSE sparse metadata 正例和严格矛盾负例；全部本地真实 Provider0。后增扩展版另取来源与验证，不能用这项回执覆盖新代码。

[捕获 wire 离线解码](assets/cpar-build-batch-20261011/sse-offline-green.json) 只证明新 decoder 可读取原 completed 编码。停止后用同一正式 REMOTE_COLLECT 只读补采 [归属](assets/cpar-build-batch-20261011/sse-poststop-attribution.json)：唯一 Attempt succeeded、Request succeeded、Usage1／ledger1，目标、Client Key、source-event 与原终态精确匹配；input224、output150、reasoning149、cached128，cache_read／creation 为 null，measured／inclusive。该观察层为 **POST_STOP_READ_ONLY_ATTRIBUTION**，没有运行进程证明，未塞回原 wire 或抹除 execution_failure。

## 计数、费用和退出

| 类别 | 本批新增 | 历史保留 | 累计 |
| --- | ---: | ---: | ---: |
| Pin 实际上游发送 | 2/6 | 3 | 5 |
| 公共基础 Attempt | 10/54 | 6 | 16 |
| 推理总数 | **12/60** | **9** | **21/69** |
| 目录／quota／OAuth／主动refresh／重新授权 | 0 | 原历史保留 | 本批0新增 |
| 扩展真实／故障探测／retry／fallback／失败后重跑 | 0 | 原历史保留 | 本批0新增 |

事件增量 Request／Attempt／Usage／request_finished 各10，ledger10，与10个基础请求一致；继承各1／ledger1另列。网络 DNS／TCP 计数只是 allow 审计，**Provider 独立接收数 UNKNOWN**。账本金额 null／unpriced，实际费用 UNKNOWN，未填0；用户本批授权金额不限仍不取消请求上限。

[退出核对](assets/cpar-build-batch-20261011/final-receipt.json) PASS：临时 unit MainPID0、原 PID 不存在、19180／19181 无 listener，停止后5秒审计不增，budget.stopped=true，历史文件 hash 全保留。保护目录0700／文件0600，无全局清理。原 wire、候选、旧失败和材料留在保护根；本机原始工作底稿留在 `output/cpar-build-batch-20261011/private/`，不提交秘密／会话内容。

生产前后 PID499070、NRestarts0、active/running；unit SHA256=`fe8e5fb9329a478c17fac3b23a8a3c29bc6048aab7f6af6db82e14589bc49213`，生产 binary=`0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b`，Caddy=`bfede78bae3c2bb4d120bcbd08924c0ba564fa3a1a02d8edda37a9004f9b945e`；current 保持 `b0cf36e387b774439acc23830221389eb3ac74d0`，active config=`kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75/rev-1`。无生产 service／配置／current／Caddy 改动、无 push／发布／切换。

## 分层状态

产品 source 未变，按已核对来源复用上批产品 Full45/45、Rust1501/0/12、Build19项与既有 ARM64 候选制品。新验收器修复独立取得39/39本地证据。真实部分一组合PASS、一组合FAIL、四组合NOT_RUN；#47整票及#57／#9继续未完成。扩展工具、本地公共边界验证、独立 Standards／Spec、正式 docs gate 与 GitHub 写后读回由本批后继收口资产分别绑定，不能由基础部分局部PASS扩大结论。远端 CI、正式发布、生产切换本轮 NOT_RUN。
