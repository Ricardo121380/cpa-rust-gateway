# 首批 Grok 真实验收授权清单（2026-10-10）

状态：**PREPARED / BLOCKED_FOR_LIVE_EXECUTION**。用户已确认 Grok 4.5／4.6／4.7 均可用于准备；没有确认 Official／Build／Console 入口、环境、账号范围、真实请求或费用。当前真实 Provider 请求为 0。此文件是可审核的具体候选批次，不是实际可用账号声明或请求授权。

## 候选范围及实际输入

| 项目 | 候选方案 | 已核实程度／停止点 |
| --- | --- | --- |
| 主批渠道 | `grok.official`，代表模型 `grok-4.7` | 官方公开模型资料存在；用户允许该模型。入口和账号 **待用户选择** |
| 可选第二渠道 | `grok.build`，同一代表模型；独立 6 个组合 | 公开指南将 4.7 列为 Build 默认模型；具体账号目录、模型名称、套餐和价格 **UNKNOWN**。不因主批授权自动加入 |
| 备选模型 | `grok-4.5`／`grok-4.6`，替换代表模型 | 用户允许；变更冻结 target 和费用材料后重新绑定授权。不扩张为 3 模型并跑 |
| Grok Console | 若用户实际可用入口是 Console，则重新选定该渠道 | 页面导入通过不证明目录、套餐或推理可用；不能沿用 Official API 价格 |
| 运行位置 | 专用隔离 Linux + systemd 实例，具体 host／service／data origin 待填 | 当前正式 `inspect`／`collect` 使用 Linux `/proc` 和 systemd；尚无拟用真实 host。现有 macOS stock 页面服务已经停止。不能将旧共享域名作为本批目标 |
| 源码／二进制 | 产品基线 `c1b304d02e61fe1c75653a32fba920093462add6`；目标运行进程的实际嵌入 SHA、SHA256 和 manifest 待取得 | 本轮 macOS 页面 binary SHA256 `4648f651c58d1a8e1e0ff63bf720b7ad7546711859ce60478bb173ff10dc1f7e` 仅用于页面验收，不能充当 Linux 真实制品证明 |
| 推理出口 | Official 候选 `https://api.x.ai/v1`；无共享代理、无 fallback | 实际 egress 类型／标识／revision 待只读冻结；账户地区限制未知。US 区域端点是另一项选择，不能自动替换 |
| 配置对象 | 拟新建隔离 `preacc-grok-official` endpoint、route、candidate、public model；Build 使用独立命名 | 这些只是建议 ID，未写入任何真实配置。实际 config version/revision、ID 和模型须由 serving snapshot、Pin 与运行观察取得 |
| 账号范围 | 每渠道一个明确允许使用的既有账号 | 目前没有已授权材料来源或账号标识。将实际 CPAR account ID 转换为 SHA256(`cpar-evidence:` + ID)，材料留在受保护环境；不粘贴到聊天或 GitHub |
| 能力声明 | reasoning／parallel／stored／continuation／compact／websocket 六项 | 全部保持 null；必须 GET 同一 Client Key 的 `/admin/models/effective`，冻结实际 serving candidate 的声明及来源。公开模型卡不能替代实际声明 |

机器草案位于 [首批配置和计数](assets/cpar-preacceptance-20261010/first-grok-batch.json) 及其 `plans` 目录。每个 target 的实际身份、revision、能力和费用证明均保留 null；可直接用正式入口做无推理预检，当前应 BLOCKED。

## 协议、模式、场景和请求数

| 每渠道／每代表模型的组合 | 文本多轮 | 工具回传 | 连续工具交互 | 每组合推理数 |
| --- | --- | --- | --- | --- |
| Chat JSON | 3 | 2 | 4 | 9 |
| Chat SSE | 3 | 2 | 4 | 9 |
| Messages JSON | 3 | 2 | 4 | 9 |
| Messages SSE | 3 | 2 | 4 | 9 |
| Responses JSON | 3 | 2 | 4 | 9 |
| Responses SSE | 3 | 2 | 4 | 9 |

单渠道基本场景共 18 行、54 次推理；两渠道共 36 行、108 次。连续工具交互是初始文本、工具 1、工具 2、最终文本四次请求，包含两次真实 call_id 的本地确定性结果配对；不得用三次场景替代。执行器保留完整、有序原生 Responses items，包括 encrypted reasoning，且检查第二工具周期 ID 新鲜、结果归属、最终记忆 token、JSON/SSE 终态、Usage 和账本。

`freeze` 需要既有、已授权且 protocol/mode 匹配的成功 Channel Pin：若没有可复用回执，六个组合各需一次 Pin，**另计 6 次推理**，不是免费预检。主批拟批准总推理上限为 **60 = 6 Pin + 54 基础**；如已有合格 Pin，则减少实际新增 Pin，不能将余额挪给重跑。第二渠道另需 60，总计 120，须独立批准。

| 外部动作 | 主批拟计数 | 当前授权／行为 |
| --- | --- | --- |
| 凭据只读、配置和运行身份检查 | 选定账号一次；本地／管理 GET 单独记录 | **待授权账号范围及运行位置**；不触发登录、刷新或配置发布 |
| 认证网络／OAuth | 0 | 不调用授权、refresh 或真实浏览器会话 |
| 模型目录 | 最多 1 次 authenticated GET；仅已确认的 Official 目录入口 | **待授权**；无模型或无权限则停止、推理 0。目录结果不冒充推理成功 |
| 额度查询 | 0 | 未证明适用的 quota 入口，不猜测、不隐式查询；需操作者提供可审核套餐／余额及费用控制证据 |
| Channel Pin | 最多 6 次推理 | **待授权**；每次 max_attempts=1、精确候选和单账号；任一次失败后停止 |
| 3／2／4 基础 | 最多 54 次推理 | **待授权**；每组合 9 次、六组合串行，下一组合前核对剩余额度 |
| 扩展、故障探测、重试、重跑 | 0／0／0／0 | 不隐含在基础预算中；实际声明为 true 后另列逐项计划和请求数。不能删除声明以规避义务 |
| Build 后台刷新／额度行为 | UNKNOWN | 未冻结背景请求、套餐及账号前不加入第二渠道；软件正常刷新例外 E-05 的既有批准不构成真实网络授权 |

基本场景之外的错误／取消／timeout／retry 及已声明扩展继续 NOT_RUN；基本场景 PASS 也不能将组合或渠道整体标为 PASS。

## 参数、价格和费用边界

草案为 Responses `max_output_tokens=128`、Chat `max_completion_tokens=128`、Messages 必填 `max_tokens=128`；不传 temperature、stop、reasoning 或 parallel 控制，保留未知的实际默认行为。这是待核对的控制输入，不能声称三个下游字段有相同原生／计费范围。Messages 合法性和 native cap 等价性必须分别核实。

新查得官方 [Responses Markdown 定义](https://docs.x.ai/developers/rest-api-reference/inference/responses.md) 明确 `max_output_tokens` 只覆盖可见输出，排除 reasoning 和 function calls。因此它 **不是总输出／费用硬上界**。官方 [reasoning 文档](https://docs.x.ai/developers/model-capabilities/text/reasoning) 说明 4.5／4.6／4.7 默认 high 且不能关闭 reasoning；降 effort 不能证明费用有界。模型指南没有提供可用的全部计费输出 hard cap。本批保持 `cost_bound.status=NOT_PROVEN`。

公开 [价格表](https://docs.x.ai/developers/pricing) 的 global 短上下文价格为 USD／百万 tokens：4.5 input 2、cached 0.30、output 6；4.6／4.7 input 2、cached 0.50、output 6。达到 200k 输入阈值采用 long-context 全请求费率：input 4，output 12；cached 分别 0.60／1。US 区域有 10% 溢价。Build Fast、Build 套餐及 Console SSO 不能直接使用这些 Official 标准费率。来源的观察 UTC、原文下载 hash 和路径见 [xAI 索引](assets/cpar-preacceptance-20261010/xai-source-index.json)。

费用公式必须使用实际完整 billable output，结合 input/cache/region/套餐/额外请求费用。仅作敏感性示例：假设全部 60 次各有 2000 个未缓存 input、全计费 output 分别为 128／1128／10128，global 短上下文费用分别为 **USD 0.28608／0.64608／3.88608**。这些是假设，不是请求的已证明 token 量或上界；工具、历史增长、Pin 参数、reasoning 和后台网络可改变费用。

主批可提议 **USD 1.00** 总额作为待批准货币上限，实际尚未批准。仅当全部 60 次的输入／可见输出／reasoning／工具输出／缓存及请求费均有可审核上界，且总和不超过该上限，才将证明填入授权。每个 9 次命令分配预算并绑定真实 source/process/artifact/target/base URL；Pin 预算另分配。若没有完整证明，**在第一个 Pin 前停止**，不会先付费探测再声称可控。第二渠道独立填写套餐及预算，不占用主批余额。

## 可复用执行顺序与停止条件

1. 用户填写：入口、专用 Linux host/service/data origin、单账号范围／脱敏 ID、代表模型、真实只读和目录范围、6 Pin + 54 基础请求上限、货币上限。此阶段不索取密钥文本。
2. 在已获授权位置读取现有保护材料，获取实际进程字节 SHA／manifest、配置、Client Key 声明、精确 candidate 和完整池目录；未证明价格、计费上界或实际可用模型时停止。
3. 先证明 route max_attempts=1、无其他 enabled candidate、单账号池和同配置；仅 `all_active` scope 不是精确账号绑定。正式执行器在发送前检查上述 GET 观察；不能靠事后 Attempt 检查代替事前隔离。
4. 在上述条件及 Pin 授权满足后获取／复用逐 protocol/mode Pin；任何未知结果不盲目重放。冻结实际 lease revision、配置和出口，不使用 inventory 当前 revision 代替已执行 lease。
5. 用 `cpar-batch-c-live.py --new-build run` 对每个冻结组合先不加 `--execute` 预检，再按对应 9 次授权执行。真实 secret 只通过环境变量及保护 collector 使用；每轮独立确认 actual Attempt、Request owner、协议、模式、runtime、Usage、账本和 source event。第一处缺失、矛盾或失败立即停止。
6. 保存 private request/wire、真实 call_id／结果配对、per-turn runtime、Attempt execution sidecar、Usage／账本原文及 hash；公开回执仅含 allowlisted identity／计数／结果。native session/ciphertext 不出保护区。Grok Build 若产生会话，额外保留 owner、授权谱系、credential revision、config／egress；严格同授权正常刷新例外沿用 E-05，其他 revision 变化拒绝。

授权状态 PENDING、目标未填、实际声明 unknown、配置观察不完整／过期、额外 candidate／账号、max_attempts≠1、模型／region／revision／价格变化、总费用未有界、collector 缺失、terminal／history／tool／Usage 失配，均有明确停止点。源码、账号、环境或计费发生变化后不得复用旧批准绑定。
