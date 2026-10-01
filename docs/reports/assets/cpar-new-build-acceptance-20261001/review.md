# 新构建验收准备：Standards / Spec 分轴记录

基线 499896e6；首轮固定 64e5cf9a，修复及关闭固定 ff4b59ed。四个代理均一次性、只读、未复制主
线程历史；终态已收敛，未复用或让代理修改代码。主代理负责复现、修复及最终验证。

## Standards

首轮 `/root/newbuild_standards`：**FAIL**，1 个判断性 Fowler smell，0 个确定硬规则违反。

**P2 possible Duplicated Code**：64e5cf9a 的 `cpar_new_build_acceptance.py` audit :677–690 与
execute :959–981 重复校验已漂移；审计 :679 核对 response_model，在线未检查，progress 只写
NOT_RUN 回执，首轮错误模型可能继续发后续推理。规则为 code-review Skill 的重复逻辑启发式，
非硬性仓库违反。最小修复是共享每轮校验，验证首错后续发送 0。

复现使用固定 64e5cf9a 代码、ff4b59ed 最终测试：[red receipt](review-red-receipt.json) 与
[日志](review-red.log) 保留版本、校验值及失败。修复引入 `basic_answer`，在线/audit 共用模型、
参数、工具与 Usage 校验；后续发送只在校验及归属通过后构造。

关闭 `/root/newbuild_standards_closure`：**PASS**，仅 64e5cf9a→ff4b59ed 三文件修复范围；
核实原问题关闭，没有新增硬规则/实质 smell。代理报告 6 项定向测试及 36 个内存停止探针通过，
涉及 collector/保护目录、三协议/两模式、错误模型/版本/缺观察，发送函数均调用一次。
这些额外探针仅是评审辅助，未归集完整脚本；正式可复现绿色证据以 ff4b59ed Full 的 24 测试为准。
shared check.sh 的边界 FYI/trailer 和未提交 AGENTS 保留在首轮已核实。

## Spec

首轮 `/root/newbuild_spec`：**FAIL**，4 项实际内存反例；其他全部边界/扩展审阅为 **PARTIAL**，
未穷尽。该限制保留，不因关闭复核改称全 C 规格审查通过。

1. **native 输入 Usage 假通过**：64e5cf9a :370 仅比 output；三协议 native input=999、source=3、
   total 同步合法仍 basic PASS。违反 task-scope 原生 Usage/账本精确要求。修复按 core Usage::input_for
   的 inclusive/exclusive 规则比较输入、缓存/思考可表达细项；必要转换字段缺失 BLOCKED，不补零。
2. **保护目录逐请求版本漏验**：:975 只读归属，顶层新构建/每轮旧 runtime 仍 LIVE_NEW_BUILD/basic PASS。
   违反 actual source/artifact/process 观察要求。修复两采集路径及 audit 共用 verify_turn_runtime；
   缺失/陈旧 BLOCKED，版本/PID/instance/checksum 矛盾 FAIL。
3. **公开失败输入泄漏**：:650 验证前复制无效身份合成邮箱；:659 完整复制 execution_failure.wire。
   违反不得输出账号/秘密/正文。修复 public_identity/public_failure 投影、限制公共路径，回归合成邮箱/
   PRIVATE_BODY/PRIVATE_KEY 不在 receipt。
4. **无关 404 冒充所有权拒绝**：:558 只要求 operation 非空，unrelated_endpoint 仍 PASS。
   违反父规格 exact ClientKey ownership。修复 corresponding operation、原 owner read200、同资源 ID、
   不同 owner、compact previous_response_id 绑定；401/非认证/发生上游发送仍拒绝。

全部先在 64e5cf9a 复现再修复，最终测试可从固定 red receipt 和 Full 核对。
关闭 `/root/newbuild_spec_closure`：**PASS**，限定四项和修改的直接回归，没有新增实质缺陷；
代理报告 6 定向 unittest 和 108 内存状态/入口/协议组合通过。探针无真实 Provider/文件/网络副作用；
同上，正式绿色证据以已归集的 ff4b59ed Full 为准。

Spec PASS 是本轮确定问题的限定关闭结论。真实声明/controlled boundary/extended sampling 尚未执行，
整体 C/#57 仍 BLOCKED。没有把缺部署/未授权推理列为代码缺陷，也没有新增 accepted restriction。

## 主代理最终证据

ff4b59ed：Full 45/45、工具 unittest 24/24 PASS；6 组 loopback basics PASS，6 种停止 fault ×6 组合
均仅一次发送，保护目录缺 runtime 为 BLOCKED。Native accounting 合法转换正例通过，输入/细项
差异反例拒绝，未知转换保持 BLOCKED；泄漏/资源操作反例拒绝。

初始发现数：Standards 1（判断性）；Spec 4。已确认未修复项：两个轴各 0。
全规格/真实扩展覆盖限制属于未完成验收，保持 PARTIAL/NOT_RUN/BLOCKED，不由此关闭 C/#57/#9。
