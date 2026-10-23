# Claude D2首版选择落实与#58/#64/#66本地修订稿（2026-10-23）

状态：**D2产品选择已落实为可使用、可审查的规格与局部配置；首版尚未READY。**
这是一轮文档与示例变更。固定Build资格仍BLOCKED、#66仍OPEN、#67未开始。

## 1. 当前基线与正式选择

工作树为 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`，
分支 `feature/62-client-metadata`；开工HEAD为 `f43a08ece525d76c940f0195c7df3ab822ce779a`。
开工index为空，继承10个tracked修改和8个untracked文件；7466项实际输入已记录hash/type/mode/link/uid/gid，
包括15项0600权限。历史SHA只定位证据，没有回退或改写旧制品。

新增[ADR-0123](../adr/ADR-0123-first-release-claude-d2-restricted-support.md)落实用户已明确确认的选择：
**Claude Code 2.1.302的完整四变量D2配置纳入首版**。更新
[BC-PROTOCOL-016](../contracts/BC-PROTOCOL-016-claude-restricted-candidate.md)、
[现有Messages使用说明](../support/grok-build-messages.md)、
[候选支持清单](../support/first-release-candidates.md)及必要导航。
ADR-0122只追加后续决定链接；实验原文、旧报告NOT_DECIDED及全部RED不改写。

本轮没有改变产品Rust、协议实现、schema、锁文件、能力声明、native-field qualification、
构建入口或交付制品；没有引入controlled-test-support生产路径。
原始Claude adaptive/display、overall effort、block prompt-cache保持#58/#64后续义务，未删除、未标完成。

## 2. 精确工单修订稿

以下是**完整拟议body文本**及相对本轮读取GitHub正文的diff，全部只在本地。
title/state/labels/assignees/comments和原生依赖均不在修改范围，所有既有AC复选框仍未勾选。

| 对象 | 完整拟议正文 | 审查重点 |
| --- | --- | --- |
| [#58](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58) | [issue-58.body.md](assets/cpar64-66-d2-adoption-20261023/issue-58.body.md) | 将首版Claude行限定为D2，加入已确认版本/四变量、各层证据和限制；56个用户故事及T01–T12原文保留；原始控制移为后续义务，不取消。2026-10-18初始状态明确为历史快照。 |
| [#64](https://github.com/Ricardo121380/cpa-rust-gateway/issues/64) | [issue-64.body.md](assets/cpar64-66-d2-adoption-20261023/issue-64.body.md) | 以D2为选定首版配置，保持原六条AC，另列M1/M2/M5未完成后续项；协商不等于effort语义已支持。 |
| [#66](https://github.com/Ricardo121380/cpa-rust-gateway/issues/66) | [issue-66.body.md](assets/cpar64-66-d2-adoption-20261023/issue-66.body.md) | 固定已选D2配置，分列controlled/binary/OCI/fixed/real状态；原同候选、Gate、审查、reader/egress/系统要求不变；不写完成或READY。 |

[统一正文diff](assets/cpar64-66-d2-adoption-20261023/issue-bodies.patch)用于逐项检查精确变动。
[GitHub基线摘要](assets/cpar64-66-d2-adoption-20261023/github-baseline.json)记录读取时的updated_at、
标题/状态/标签、评论数、原body和拟议body的SHA256，以及依赖。
若以后获得远端写入授权，先重读正文和依赖并确认没有漂移，再审查新旧差异；本文不包含自动写入步骤。

本轮GitHub只读核验：#58/#64/#66/#67均OPEN；#64有一条保留的2026-10-20原始配置受阻评论，
其余三票没有评论。原生依赖为#64→#63→#61、#66→#62/#64/#65、#66阻塞#67，涉及blocker均OPEN。
没有写GitHub、关闭工单、解除依赖、push、PR、发布或部署。

## 3. 配置表达与不变的边界

局部示例使用占位CPAR Key/端口、清空的继承环境、独立HOME/config/cache/tmp/work，固定受测入口、
grok-4.5和完整四项：DISABLE_PROMPT_CACHING=1、CLAUDE_CODE_DISABLE_THINKING=1、
CLAUDE_CODE_MAX_RETRIES=0、CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1。
客户端命令保持注释；没有新启动器、全局配置修改或自动请求。

省略thinking不承诺模型停止推理。首次effort请求被网关准确预拒绝，协商后的新Request由客户端发起；
网关不删字段、不伪造上游错误、不透明重发。Messages不展示Grok summary，但完整加密留存、
exact Attempt核对与工具续轮要求保持。四变量不能简化为两项，也不能遗漏非流式fallback。

## 4. 既有证据与本轮验证

| 类别 | 结果与身份 |
| --- | --- |
| 既有三客户端受控证据 | D2报告的4015bd4542dbc52fde8984c14c01b34843ef3c59；Claude D2正常/失败/活PID取消通过，原始配置BLOCKED；Codex/Pi同源码严格回归。不是本轮执行。 |
| 既有binary生命周期 | 737fd040e983bef329be27b5cf48b4f6986a5772+overlay；Linux ARM64 stock reader/serve/restart/isolation/exit 144/144。不是新客户端执行。 |
| 既有原样OCI | 同一旧产品制品，f8d28c6b验收代码；290/290与完整cleanup。不是固定Build或真实Provider资格。 |
| 既有最新Full | f8d28c6b+实际overlay，Darwin独立源/新target；45/45，Rust1659/0/21。本轮文档HEAD不改绑该结果。 |
| 本轮静态核验 | `scripts/check.sh docs` **7/7 PASS**；执行结果见[verification.json](assets/cpar64-66-d2-adoption-20261023/verification.json)，另含配置语法/完整性、工单diff与状态一致性、继承保护及最终所属diff。 |
| 本轮行为测试 | Full、三客户端、binary/OCI与恢复矩阵均NOT_RUN：无行为变化，不机械重跑。真实账号/Provider/#67、远端CI、发布/部署均NOT_RUN。 |

历史来源按[候选清单](../support/first-release-candidates.md)逐项链接；source/binary/OCI/验收代码与文档commit分列。
静态通过只证明本轮规格与配置表达一致，不能补足未执行的运行资格。

## 5. 最小剩余条件

固定Build启用仍需实际字段/profile/capability、三段模型身份和续轮的一手依据及相应受审查生产资格，
不能靠D2采用、环境变量或测试feature解除默认拒绝。#61 cost_in_usd_ticks继续NOT_ADMITTED，
沿用等待新一手材料的决定，没有新材料不重复已有研究。

#66仍须在其适用流程下对齐同一候选及三客户端配置/证据、source/binary/manifest、reader/egress/系统资格
和Gate/Standards/Spec条件；既有受控复用有明确产品输入范围，binary/OCI客户端执行仍NOT_RUN。
远端CI、签名/验签及独立可重复构建的NOT_RUN按实际交付要求处理，不创造新的无条件门槛。

#67另需#66资格满足、明确账号/环境/预算/新epoch及适用fresh Pin，再完成Codex、Claude D2、Pi
全部真实Grok Build闭环和交付材料。首个FAIL/UNKNOWN/矛盾停止该epoch，旧stopped epoch不恢复。
本轮不启动#67、不读取真实凭据、不发送真实Provider请求。#9/#47/#57剩余义务及native-v1独立方向保持。
