# CPAR 下一轮真实验收计划（未执行）

本计划是本轮交付的可审查准备材料；**没有真实请求授权，没有真实账号材料读取，Provider 调用数为0**。审计源码为 `c1b304d02e61fe1c75653a32fba920093462add6`；实际下一轮 runtime/source/artifact/config/model/account/声明尚未冻结。不能把本计划或本地 Full 当成真实验收。

完整行清单见 [后继台账](assets/cpar-channel-delivery-20261010/issue57-successor.json)。原198行台账和原 source/time/status保持不变；本后继记录将审计归属与实际执行归属分开。

## 组合与可独立准备部分

11渠道 × Chat/Messages/Responses × JSON/SSE × 文本多轮/单次工具回传/连续多轮工具 = 198行。Grok Web 的12个工具行是已接受不适用，剩余186行需验收。

| 部分 | 行数 | 当前准备状态 |
| --- | ---: | --- |
| openai-compatible、anthropic-compatible、Codex、Claude、Grok Official、Kimi Coding、Kimi API、Grok Build、Grok Console | 162 | 可独立冻结模型、配置、账号证据及已声明扩展；真实执行 BLOCKED，需具体授权与预算 |
| Kiro | 18 | IDE/CLI、凭据、地区、profile和原生硬上限语义未齐；不可用 uncapped 成功或提示词预算进入收费验收 |
| Grok Web 文本多轮 | 6 | 当前生产续接/parent及 Messages cap 未齐；软件 BLOCKED |
| Grok Web 工具 | 12 | NOT_APPLICABLE_ACCEPTED；单列明确拒绝，不计成功组合 |

49项扩展准备行的 actual_declared 全为 null，表示声明未知。下一轮从实际 serving snapshot冻结每个 endpoint/candidate/model 的 reasoning、parallel、stored、continuity、compact、count-token、WS 等适用声明，展开为协议×mode×模型×参数行。未声明项不扩张，已声明项必须完成；不得关掉声明规避失败。历史24个 Kiro cap行和12个丰富metadata条件行单独保留，不能计入基础组合成功。跨协议无法等价表示的 metadata 需要明确拒绝，不将拒绝改成成功。

## 冻结前置与执行顺序

1. 对将实际使用的源码、制品 SHA256、依赖锁、runtime版本和配置 active revision 做独立核对；当前本地提交尚未 push，不提供不存在的 GitHub commit/blob 链接。
2. 用户具体授权环境、账号范围、Provider 请求与费用上限；使用既有认证机制。报告只存不可逆脱敏的账号证据标识，不存令牌、cookie、profile原材料或Client Key。当前计划的这些值均为 null。
3. 冻结精确 provider/channel/endpoint/route/candidate/model/account revision、egress种类和revision、参数及实际能力声明；验证账号有效期、health/quota/capacity及权限。目录/登录/健康/额度读取只作前置，不能替代推理结果。新的授权、目录或额度调用也需要纳入明确请求清单和预算。
4. 先完成9渠道的162个可独立准备行；每轮记录真实运行归属，不因Kiro/Web受阻而全部停下。Kiro/Web只在原生语义及软件闭环完成后进入各自验收。
5. 基础正例按精确单candidate、max_attempts=1、无隐式retry/fallback执行。受控故障已有证据按原来源复用；需要真实故障探测时另列行和费用，不藏在正例里。
6. 每行结束核对实际 Request/Attempt/Usage/ledger/存储根、资源释放及上游请求次数。失败或取消的根不可成为成功续接根；同组foreign Client Key、改变模型/账号/revision/egress和伪造历史均有相应拒绝断言。工具没有真实副作用：使用本地确定性echo执行器并保留原call_id。

## 场景与额度估算方法

| 场景 | 每行计划的推理次数 | 必需证据 |
| --- | ---: | --- |
| text_multi_turn | 3 | 初始文本、两次续接；原生有效历史或conversation/parent、owner、精确执行身份、JSON/SSE终态 |
| tool_roundtrip | 2 | 原生工具调用、真实call_id的结果回传、最终文本；不得提示词模拟 |
| tool_multi_turn | 4 | 两轮工具调用与对应结果回传；保持多项交错/顺序、参数及连续性 |

基础186行的计划上界为 `66×3 + 60×2 + 60×4 = 558` 次推理（单Attempt、无额外重试）。9渠道独立部分为486次；Kiro待补54次；Web文本待补18次。扩展、授权、目录、额度、失败探测和验证重跑**另计**，不是558次内的隐含免费调用。

候选硬输出预算可从每请求64单位开始，但须先证明当前模型的原生字段、计数单位、reasoning是否包含以及命中终态。只有单位相同且覆盖所有计费输出时，基础上界才可算为 `558×64=35,712` 个输出计数单位；本计划不把这个条件数字当成已确认账单。输入、隐藏reasoning、缓存、工具上下文及定价需由实际模型/价格来源与固定payload测算，再形成用户可批准的货币上限。当前实际budget、price和output_limit为null，Kiro/Web原生上限未知时禁止开始。

## 证据格式与停止条件

每个 JSON 回执至少包含：case ID、UTC起止、source/runtime/artifact哈希、配置id/revision、精确provider/channel/endpoint/route/candidate、模型、脱敏credential证据ID/revision、egress/revision、实际能力声明及来源、发送参数中非秘密部分、Request ID、每次Attempt ID/结果/重试决定、原生conversation/parent的脱敏映射、实际终态、Usage六项nullable计数/provenance/input accounting、ledger source-event关联、存储owner和资源释放结果、原始日志哈希。

没有任意新增TTL。凭据expiry、配置/路径/归属变化按现有契约失效；Pin必须精确资格、显式一次、无fallback。触达授权或费用上限、身份/出口变化、计数不明、缺终态、失败根、未知/冲突历史或声明漂移时停止该行并标FAIL/BLOCKED，保留原始回执。未知历史不补归属、不补造账本；real Provider、远端CI、候选制品、部署及生产层分别给结论。

冻结候选当前 **NOT_MET**：软件/语义阻断、独立历史页面覆盖缺口、186个真实基础行及实际声明扩展未完成，且当前源码尚未上传、无对应远端CI/发布制品。T12合成升级恢复和本地Full不能替代这些门槛。
