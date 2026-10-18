# #60 — Grok Build 模型身份三段核对与准确拒绝

日期：2026-10-18（Asia/Shanghai）。状态：本地实施、公共身份矩阵、Full 及固定候选双轴评审通过。

## 基线与授权

只实施 [#60](https://github.com/Ricardo121380/cpa-rust-gateway/issues/60)，父规格
[#58](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58) 的 T02/T09。
实际分支 `codex/prism-v4-delivery`，开工 HEAD `be6cf15b11a0834fe98fd5e07277390efe15e424`，
暂存区为空。已有 9 个跟踪文件改动，初始脏文件清单共 49,041 项；SHA 不是回退指令。
本轮只提交自己的增量，保留全部既有内容、未跟踪历史材料和原始失败。

已读取适用 AGENTS、implement/TDD/code-review、工单规则、CONTEXT、质量门禁、相关 ADR，
以及 #59 报告、ADR-0109 和 BC-OBS-006。gh 重新读取 #60/#58 正文与评论：开工时均无评论；
#60 OPEN、ready-for-agent、无原生 blocker、无人分配；#59 CLOSED。认领 #60 后独立读回确认。
仅 #60 的认领、结果评论与符合条件后的关闭获授权；未修改其他工单或执行 push/部署/发布。

## 实际改变

1. 公共 handler 将已准入 public model 与原始 requested alias 分开传给生产 driver。
   原有 RequestEvent 保留两者；实际 candidate、lease、配置及渠道仍是原来的身份边界。
2. Build 发送前核对 public → selected upstream；只有字面等值或预先冻结、独立批准且范围匹配的
   proof 才能执行。实际选中名继续同时进入 JSON body 和 model-override header。
3. 冻结 proof 校验源字节 SHA256，并绑定三个具体名、源依据、有效期、配置、provider/upstream/
   endpoint/route/candidate、实际凭据与修订、owner、渠道、协议、JSON/SSE 模式和 Build profile。
   最终 binding 还绑定这次 Request ID。哈希不能代替来源可信审查。生产 composition 接受显式
   immutable proof 输入；当前 stock composition 传空集合，没有安装真实证明或读取 inventory。
4. 实际 Build adapter 必须给共享 JSON/SSE 解码状态传预期原生名。原生快照缺失、非法类型/形状、
   未证明名称、阶段变化分别使用封闭安全原因；JSON terminal 与 SSE created/in_progress/
   completed/incomplete/failed 均检查。别名不允许阶段内改名。
5. 发送前不兼容使用现有 ClientRequestError；响应身份错误使用现有 UpstreamProtocolError。
   两者均不触发重试、账号/模型切换或 fallback。晚发现 SSE 矛盾只发一次合法失败，没有成功终态，
   拒绝后不读取后续测试帧。
6. #59 模型合法的合成夹具仍经过原 Usage 拒绝并验证完整加密留存。旧合成 model 缺失单列为
   身份负例，保留原 Usage 字节；未读取/未验证的用量不造记录或零费用。原真实样本、旧 oracle、
   历史 FAIL 与 stopped epoch 均未改变。结构解码 helper 不单独授予模型身份资格。

对应 [ADR-0110](../adr/ADR-0110-build-model-identity.md) 与
[BC-PROVIDER-024](../contracts/BC-PROVIDER-024-build-model-identity.md)。没有新 schema、依赖、
普通元数据放行、reasoning/客户端交付策略、native-v1 或 #61 实现。

## 公共复现与验证

证据目录：[assets/cpar-60-20261018](assets/cpar-60-20261018/)。测试通过生产 compiler/factory/auth/
leased Build 装配，只替换外部传输为合成 loopback HTTP。独立接收器读完请求 body 后记录请求，
另核对收到的 model-override header；它不使用 adapter 的调用次数作为接收证明。
系统 sandbox 已实际证明 loopback 可通，`192.0.2.1:443` 被 EPERM 拒绝。

| 检查 | 当前结果 / 证据 |
| --- | --- |
| 原生模型错误的公共 RED | `public-native-mismatch-red.log`：JSON 与 SSE 都错误成功，用 public 名掩盖错误 native；各接收一次 |
| 已知 public/upstream 不兼容的 RED | `public-request-mismatch-red-receiver-confirmed.log`：错误发送且成功，实际接收一次，预期零 |
| 接收器夹具早期反馈 | 初版非阻塞 accepted socket 继承导致计数竞态，已显式设回 blocking 并重新取得上述 RED；早期日志独立保留 |
| 公共身份矩阵 | `public-identity-final.log`：6 个测试组通过；字面/合成别名成功，17 类范围或时效/冲突负例，缺失/类型/形状/名称失败，以及四种后续阶段冲突 |
| 零/一次接收与外发限制 | `network-denial.json`；发送前拒绝零，响应后错误一次；保留备用 candidate 与三次 route attempt 配置，晚失败未多发或读 tail |
| #59 公共持久化回归 | `cpar59-public-regression-initial.log`：完整切片加密确认、读回、幂等与已知计数通过；缺 key/实际锁超时仍 RecordingUnavailable |
| #59 storage / CLI | `cpar59-storage-isolation-green.log`：8/8 通过。原始 AlreadyExists 与未修复重查日志保留；仅给临时目录名增加进程内原子序号，不降低断言 |
| 编译 / Clippy | workspace check 与最终 Clippy 已通过；中间编译和静态反馈保留 |
| 受影响 Provider 测试 | 7 个 adapter 与 4 个 native-egress 测试通过；原 continuation fixture 名称不一致的 FAIL 已保留并修正其合成名字，不降断言 |
| 仓库 Full | `full.log` / `full-steps.md` / `full-receipt.json`：45/45 PASS；Rust 1532 通过、0 失败、13 忽略；deny/audit 通过 |
| Standards / Spec | 固定候选 `540e24fbffa25b06857469ae7d4fd59a773d505a`：Standards PASS（硬违规 0，P3 判断项 1）；Spec PASS（全部优先级发现 0）；见 `review-final.md` |

所有身份及证明均为明确标记的合成样本，无法解除原真实样本的 UNKNOWN/BLOCKED。
真实账号、Provider、三个实际客户端、远端 CI、发布候选、部署及生产均 NOT_RUN。

## #60 验收逐项对照

| #60 验收条件 | 核对 |
| --- | --- |
| 公共 Responses JSON/SSE 三段核对、字面或冻结有范围别名 | 定向 PASS；实际收到 body/header 与 public 及原生 oracle 分别核对 |
| 缺失、类型错误、阶段矛盾、不适用别名安全可辨 | 定向 PASS；封闭原因不带原生值或私有证据 |
| 发送前拒绝零接收；响应后错误无附加推理 | 定向 PASS；独立 socket 计数与唯一流失败、未读 tail |
| Request/Attempt、实际凭据/租约/配置/渠道边界，无 inventory 补史 | 定向 PASS；身份范围正反例与实际事件归属核对 |
| 明确合成正反例、原真实别名 UNKNOWN、旧 oracle 不改 | 定向 PASS；历史材料保留，未执行真实调用 |
| 仅模型行为和契约，不放开元数据/Usage/reasoning/重试/计费 | PASS；#59 公共和独立存储回归、Full 与双轴评审均通过 |

## 评审与交付边界

Full、既有内容保存核对及固定候选 Standards/Spec 已完成，评审全文与最终判断见
[review-final.md](assets/cpar-60-20261018/review-final.md)。唯一 P3 为两条公共入口回归中的装配代码重复，
不影响行为或断言；本轮保留这些独立 RED 回归的直接装配，未为维护性建议扩大重构范围。
最后核对 staged diff、只提交本轮增量，向 #60 评论结果并独立读回；全部完成条件满足后才关闭并再次独立读回。
最终提交 SHA 与 GitHub 回执由最终交付提供，避免文档自引用提交。

原始成功、失败与准备阶段日志均保留，并以 `validation-logs.tar.gz` 和逐文件 SHA256 的
`log-manifest.json` 交付。两个评审没有修改源文件；最终交付只补充评审、验证与收尾记录，
25 个产品/测试文件继续与 Full、固定评审候选逐字节一致。

本轮 Full 绑定不可变本地候选 `a903dea84afaab3f81daca51d16f10def2488a0c`。25 个产品文件与评审候选逐字节一致；后续仅将冲突的新契约编号改为 BC-PROVIDER-024 并补齐报告/回执，文档链接与契约测试引用已重新检查。首轮 Full 在额外网络包装下启动系统 ps 被 macOS 拒绝，独立复现为 exit 71；`full-initial.log` 与 `full-sandbox-ps-reproduction.json` 保留。改为仅给 workspace 全套 Rust 测试应用 loopback-only 限制，纯本地流/RSS guard 使用原有装配，未改业务代码或降低断言。

`final-preservation.json` 复核初始 49,041 项：48,979 个文件的哈希/模式未变，10 个符号链接的目标字串不变且链接元数据早于基线，2 个共享文档原字节保留并仅追加，50 个嵌套仓库/目录占位保留。符号链接目标目录和目录占位没有递归哈希证明。较早 `preservation.json` 的 48,989 将 10 个符号链接也计入未变记录，不能解读为全部是普通文件哈希；最终收据已明确计数，原收据保留。初步 verifier 将目录占位误分类的结果单独保留并标注，不作为实际内容变动。最终暂存前，原暂存区仍为空。
