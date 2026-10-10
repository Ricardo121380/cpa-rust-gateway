# 首轮独立审查：固定7fb544b6

绑定 `git diff 27f8349fff2e98e8d10700ae78c856f27503d52c...7fb544b6f211e3b43a4d5a6c9625e5fe9cfb20cf`，40个变更路径。提交为 `7fb544b6 fix(acceptance): preserve real Build tool results and archive stopped batch (#47)`。两个干净只读探子独立执行，结果由主代理归档。用户未提交的 AGENTS.md 只作规则来源。

## Standards

PASS（固定 diff 范围）。硬规范违例0；判断型气味0。工作区对应字节全部等于固定提交。

- 范围符合 AGENTS.md:7 的 Backend 所有权；未触及AGENTS.md:19-26共享边界。issue-tracker.md:24-25：“远程变更，仍须由当前任务的明确授权覆盖。”探子未进行远程操作。
- cpar_new_build_acceptance.py:635-644保留legacy.followup结果，另加入user指令；:1266严格期待token+":1"，与操作契约的“Instructions never replace tool outputs.”一致。
- test-cpar-new-build-acceptance.py:683-706读取公共请求的实际工具结果／指令；:746-751断言错误结果第五次停止。RED／GREEN解压SHA和测试文件SHA均匹配local-verification.json，GREEN38项通过；探子未重新运行整套测试。
- 历史工具按tools/README.md作为执行字节归档，v1／后继关系及三个公开SHA核验相符，无需为重复代码抽象。真实FAIL／停止／累计9保留。六本地回执均LOCAL_SIMULATED、基础PASS、整体NOT_RUN。

额外交接：探子用基线conversation_step在内存生成无版本号v2历史bundle，三个协议旧逻辑basic_status=PASS，新逻辑因incomplete_or_reordered_history变FAIL。现有historical测试用新函数重建历史，未覆盖原字节。操作契约明确保留2／2／3计数；是否保留旧历史重审语义由主代理结合规格判断。这是兼容性提示，未计入上述硬标准／气味finding。

未核验远端原始材料、Provider、GitHub回读和生产状态；局部Standards不代表#47整票通过。

## Spec

修复及归档切片PASS；0个已证实Spec偏离。#47整票PARTIAL。核对了修复diff、两报告、归档控制、全部JSON状态／计数和RED／GREEN日志。

- #9正文:217要求“对应结果、错误标记与次序要保持”。工具结果1、普通user指令、完整历史控制及严格token:1保留，错误:2第五次停止；未见旧FAIL改PASS。
- 主报告:3记录Pin PASS／文本3轮PASS／工具FAIL／新增6累计9／其他未发送；:78补采仅证明归属和Usage；:100限定本地38项、真实rerun0。三测试文件SHA匹配固定提交，产品路径相对1b6023f5无diff。
- 扩展清单全17类NOT_RUN／BLOCKED，executor／schema／参数待qualification，保留Chat／Messages reasoning缺口、真实30天TTL和不安全故障／取消UNKNOWN；不将17类换17请求或沿用基础费用授权。
- #47正文:22“单个组合可先记录通过，不能据此关闭其余未完成承诺”。工具闭环、连续工具、其余五模式及扩展仍缺，强制停止不构成要求违规继续发送的finding。

未执行测试／服务／真实请求，未核验远端私有wire、数据库和Provider独立接收数；GitHub同步及最终审查收口由主代理后续完成。

汇总：Standards 0项，最严重问题无；Spec 0项，最严重偏离无。兼容性提示另列并由主代理后继复现修复，不把首轮审查自动扩大到后继diff。

## 主代理对兼容性提示的处理

按已经存在的历史版本契约，复现六模式原v2历史encoding经公开audit CLI变FAIL（1 test／6 subtest FAIL）。后继最小修复只让read-only version2 audit严格比较其原历史编码；live producer固定version3，工具结果1与user指令继续分开，7请求历史不能晋升为9请求真实资格。未重写任何真实旧回执／wire／时间戳。后继38/38通过，详见local-verification-v2.json；需以新的固定提交重新审查。
