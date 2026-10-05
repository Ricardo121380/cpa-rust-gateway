# 发布范围审查覆盖表（2026-10-05）

候选源码 `80f5838902eb56be6dccc3ce047eb227d6c18e6c`；报告输入 HEAD `98230abc9ccf0b7478a1e4e7d042828c506d4bdf`。精确文件分组、区间、提交清单与 diff hash 见 [review-scope-manifest.json](review-scope-manifest.json)。当前用户的 AGENTS.md 修改保留。

| 批次／工单 | 原始固定起点、实现与修复链 | 已有完整覆盖 | 原 PARTIAL 遗漏 | 后续修复是否补齐审查 | 本轮精确补齐范围 |
| --- | --- | --- | --- | --- | --- |
| A / CPAR-01–17 / #10–26 | `9fc4ceb4… → 24142a90…`，96 文件；报告列出的13类修复已包含在同一交付提交，不能凭空拆成后续提交；然后核对 A 文件至候选80的相关变化 | 原 Standards complete；A 本地44/44、HTTP与30个EgoLite场景各自保留原范围 | Spec 对CPAR-01–07与08–17关键流程两轮均partial，没有逐hunk覆盖完整manifest；修复seam复核不等于完整原实现 | D修复覆盖其明确的运行回执／模型状态／UI范围；执行身份切片覆盖其30+8文件。均不能替代A导入、授权、去重、删除、权限等完整Spec | 两个干净只读Spec探子分别覆盖A全部UI／生成契约／fixture／浏览器脚本和后端／权威契约／HTTP脚本；主代理核对ADR、报告、回执与跨界证据。每组读取原实现完整diff，并核对相关文件后续变化和候选行为 |
| B / CPAR-18–29 / #27–38 | `24142a90… → b8cba0f4…`初评；`b8cba0f4… → f6c37ef0…`修复源码检查点；`f6c37ef0… → 45a315a6…`交付；再至候选80核对相关改变；73个交付文件 | 已知三处P2及pending-tool-history边界有RED/GREEN；Full44/44与322项HTTP的执行证据完整，但不是完整审查 | Standards大型diff截断，测试模块、fixtures、生成contract/receipts未逐hunk；Spec #27–32未穷尽codec分支，#33–38错误／隔离矩阵和进程级密钥轮换审查未穷尽 | C原生metadata修复、正式验收工具和execution证据审查只证明各自切片。没有覆盖B原manifest及全部要求，不能将其PASS扩展为B全批 | Standards分代码／证据两组；Spec分协议／Usage #27–32和执行／隔离／Responses #33–38两组。覆盖原实现、修复和当前候选；具体文件见分组JSON，交叉接缝按各轴核对 |
| D / #51–56 | `ff23f2d8… → 7478d276… → d837056b…` | 两轴完整D diff及四文件文案增量PASS；结构化review-summary绑定范围 | 历史partial不改写，终态覆盖链已补齐D限定范围 | 已有充分证据；候选后续没有Prism业务改动 | 复用，除A／B相关接缝外不重复D审查 |
| C／#57执行证据切片 | C `f027e0bf… → 9499db78… → c90656ab… → 2e43ea4b…`；工具64e5／ff4b；execution `09e70e2a… → ff77a5f0… → c94b35ed… → 179cf325… → 80f58389…` | 各限定切片两轴PASS；候选Full45/45、签名ARM64410回归及副本迁移／恢复证据 | C真实组合、Kiro cap、Web原生多轮与cap等仍未完成；#57整体核对不能用切片PASS关闭 | AC-01只修复历史材料门槛，未改变产品代码／渠道能力 | 只核对发布门槛及A／B文件生存链；保持C／#57／#9未完成及真实证据的独立状态 |

本表保留派发前的范围设计；终态已完成：A96/96、B73/73静态覆盖收口，A Spec FAIL、B Standards FAIL、B Spec FAIL。A原50文件探子中断未计入覆盖，四个新轻组覆盖41文件、主代理9契约文件补齐；B证据组错误后的少量静态检查由主代理补齐，B五个相邻遗漏由新单轮组补齐。测试／合同缺口仍PARTIAL，真实及部署NOT_RUN。精确终态、发现与主代理点验记录在 [review-results.md](review-results.md) 和结构化回执中记录。只读审查不运行Full、不调用Provider、不修改产品代码、不写GitHub。

B 的 `b8cba0f4`、`f6c37ef0` 是可解析的历史检查点，但与最终交付提交不构成祖先链：相互 merge-base 均为 A 提交。因此保留 Skill 的原始固定起点三点比较，同时修复链必须使用两个精确端点的 `git diff <lo> <hi>`，不能把三点比较误当补丁差异。`b8→f6` 实际端点差异11文件；`f6→45a` 实际仅报告及两份回执3文件，产品树相同。所有三点／端点命令与 hash 已分别保存。
