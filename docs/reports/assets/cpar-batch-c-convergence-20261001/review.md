# C 批次软件收敛两轴评审 — #57

范围是本轮软件切片，C 全批完成仍为 **BLOCKED**，规格整体覆盖仍为 **PARTIAL**。
固定比较基线为 `f027e0bfe880fa175f764fa58694fae79af8771c`；最终实现源码为
`2e43ea4bccd7c1253df7d5ceddfbc3f4e4bc61fb`。评审代理只读，未执行测试或 Provider 请求。
每轮使用两个独立的新代理，未复用历史上下文。主代理负责实际反例、修复和动态验证。

## 评审来源和范围

权威输入为冻结规格 #9 的 379 行正文及本目录 `issue-9.json`、`issue-39.json` 至
`issue-50.json`、`issue-57.json`。主代理已核对当前读取的 #9 正文与本地冻结规格一致。
标准来源包括仓库 AGENTS.md、CONTEXT.md、domain/issue-tracker 规则、quality-gates、
ADR-0100/0101，以及 BC-PROVIDER-008/013/015。文档化约束优先于 Fowler 启发式建议。

| 轮次／不可变版本 | Standards | Spec | 覆盖及后续 |
|---|---|---|---|
| 第一轮 `9499db7806470903a3960cd9d77c10395c7a1f2b` | FAIL，complete；3 项文档化违例 | FAIL，partial；3 项静态反例，未穷尽声明扩展 | 两路检查固定基线起的 22 文件 diff；主代理新增五类实际反例并修复。 |
| 第二轮 `c90656abbe06a69bc5abad845c14365a53ffaa67` | PASS，complete；晚到 done 疑点及启发式 advisory | FAIL，complete；1 项 P2 静态反例 | 两路检查 22 文件生产修改及相关测试、契约、验收条件；主代理实测七条晚到 done 接受路径后补修。 |
| 第三轮 `2e43ea4bccd7c1253df7d5ceddfbc3f4e4bc61fb` | PASS，complete，**仅增量** | PASS，complete，**仅增量**；零新 finding | 两路检查 `c90656ab...2e43ea4b` 四文件及 finish_part 调用方、完成顺序、Console 委托。其余未变化部分沿用第二轮完整检查，未重复通读。 |

最终切片结论：Standards **PASS**（第二轮全 diff＋第三轮增量）；Spec **PASS for correction**
（第二轮唯一补丁缺陷已由第三轮核验闭合）。这些结论不表示全部 C 规格或真实渠道已完成。

## Standards：第一轮结果（轻量整理）

- **P1，完成 part 可重开并覆盖引用。** Official 的 added/done 路径以及 Build 同形路径允许
  `added → done(A) → added → done(B)` 覆盖首次快照，违反 ADR-0101 的矛盾快照拒绝与确认规则。
  当时出处：Official `:857,1094,1114`，Build `:1626,1658,1671`。
- **P2，完成快照逃逸 1 MiB annotation 保留预算。** 原计数仅覆盖增量观察表，part/item
  完成快照可直接保留大引用。17 个 62 KiB title 的反例可累计超过 1 MiB，单帧仍小于
  64 KiB。违反 ADR-0101 `:39–40`；当时共享验证器 `:130`。
- **P2，生命周期事件绕过闭合字段校验。** created/in_progress/completed/failed 落入
  `validate_event` 的默认成功分支，未知语义字段和 event logprobs 可消失。
  违反 ADR-0101 `:32`、BC-PROVIDER-015 `:32`；当时验证器 `:64`。
- **Advisory，possible Duplicated Code。** 两 decoder 重复构造空 TextDelta 承载 annotation；
  后续已移到共享纯事件构造。启发式建议不是硬违例。

Kiro 清队列并封闭源的改动与回归一致。第一轮没有执行反例；随后由主代理实测。

## Spec：第一轮结果（轻量整理）

- **P1，生命周期事件和终态后的 in_progress 绕过校验。** 违反规格 `:216,219` 的
  最早可判定拒绝和非法生命周期不能完成；当时共享验证器 `:64`、Official `:783`、Build `:1313`。
- **P1，text/reasoning done 未封闭对应 part。** `done("a") → delta("b") → item.done("ab")`
  可被接受。违反规格 `:219` 及 ADR-0101 完成确认要求；当时 Official `:1313`、Build `:2073`。
- **P1，初始 item 引用可消失。** added 的 content/summary 清空后未登记引用，done 可删除
  已观察 annotation。违反规格 `:218`；当时 Official `:796`、native_output `:150,172`。

已读路径未见前端范围扩大、新已接受例外或可选 call_id/name 回放缺陷。Kiro 上限、Web
多轮、富元数据跨协议及真实/扩展验收保留未完成；声明扩展未穷尽，第一轮覆盖为 partial。

## Standards：第二轮结果（轻量整理）

**PASS**，已审范围未发现确定文档违例。检查全部生产 diff、回归与契约/ADR/条件文档。
保留疑点：Official text/reasoning done 和 Build text done 未在同值确认前检查 item 完成。
`finished_parts` 同值分支可能把晚到 done 吞掉。当时 Official `:964–979,1010–1033,1387–1394`，
Build `:1753–1769,2168–2175`；该疑点交由主代理对照生命周期规格并实测。

**Advisory：possible Duplicated Code / Data Clumps。** 两侧 finish_part 的前缀确认与
`(String,String,usize)` part identity 形状重复。可考虑纯状态辅助，但不得合并 Provider owner/profile。
这是启发式观察，未要求本轮额外重构。

## Spec：第二轮结果（轻量整理）

**FAIL，1 项 P2 静态反例**；整个 C **BLOCKED**。
规格 `:219`：“非法生命周期、截断、错误、取消和超时都不能成为完成响应。”

`created → item.added → delta("answer") → item.done → text.done("answer") → completed`
可成功。Official/Console 的 reasoning/summary done 也有相同绕过；Build reasoning 路径已有检查。
现有 done 后 delta 回归没有覆盖 item.done 后同值 done。上述源码出处与 Standards 疑点相同。

其余明确保留的 Kiro/Web、扩展、真实证据缺口不计为本切片新缺陷。22 文件 diff 未见
前端、部署、账号或权限改动。Full 和最终台账当时待主代理归集。

## 主代理实际反例与修复

| 反例 | 原实现实际结果 | 修复与最终回归 |
|---|---|---|
| 完成 part 重开并替换引用 | 第一轮新增反例 FAIL | 禁止重开，首次快照必须确认。 |
| text/reasoning done 后再追加 | 第一轮新增反例 FAIL | 单独封闭 part；同值确认限于 item 尚开放。 |
| 生命周期外层字段／响应终态后事件 | 第一轮新增反例 FAIL | 所有已支持事件闭合 schema；终态后仅允许审定 keepalive。 |
| 初始 item 引用被最终快照删除 | 第一轮新增反例 FAIL | 初始文本/引用登记；最终完整确认。 |
| annotation 只出现在完成快照逃逸预算 | 第一轮新增反例 FAIL | 观察表及 part/item 保留副本统一计费。 |
| item.done 后同值 text/reasoning/summary done | 第二轮新反例 FAIL，实际列出 7 条接受路径 | 两 decoder 在同值 shortcut 前检查已完成 item；九条合法确认和九条晚到确认均验证。 |

[review-red.log](review-red.log) 是 `9499db78` 生产实现加新增测试覆盖，五类测试实际失败；
测试在首个失败断言停止，不能宣称每个三渠道分支都曾红灯。
[review-late-red.log](review-late-red.log) 是 `c90656ab` 生产实现加新增测试，累计列出七条实际接受路径。
[review-green.log](review-green.log) 是最终候选代码的 12/12 通过。

## Standards：第三轮增量结果（轻量整理）

**PASS（仅增量）**。检查四文件和相关调用，不重复未变化的 22 文件全 diff。
Official `:1387–1389`、Build `:2168–2170` 的 guard 位于同值确认之前；Console 使用 Official。
Official/Build 完成标记分别到 `:1245`、`:1585` 才写入，晚于各 part 收尾。
新增回归 `cpar_c_native_metadata.rs:538–580` 覆盖三类文本乘三渠道的九个合法、九个非法确认。

合法 `text.done → part.done → item.done` 的同值确认仍成立。符合 BC-PROVIDER-015 `:32`
以及 ADR-0101 `:28–30` 的独立 owner 约束。既存 possible Duplicated Code/Data Clumps
未由本增量扩大，仍是 advisory；未执行测试或 lint。

## Spec：第三轮增量结果（轻量整理）

**PASS，零新 finding（仅增量）**。覆盖四文件、finish_part 调用、完成顺序和 Console 委托。
两 guard 在同值 shortcut 前拒绝完成 item，覆盖第二轮全部七条接受路径。
合法确认在 item 开放时仍能通过；item 完成时先收尾 parts 后写入完成标记。
未发现增量缺项或范围扩大，未增加限制或更改运行环境。

**C 全批仍 BLOCKED，规格完成仍 PARTIAL。** Kiro 硬上限、Web 原生连续性/必填上限、
实际声明扩展以及新构建真实组合保持未完成。评审未执行测试。

两轴最终剩余硬 finding：Standards 0、Spec 增量 0；各轴启发式建议和整体完成边界独立保留。
最终动态门禁另见 [Full](full-check.md) 和 [版本回执](full-run-receipt.json)。
