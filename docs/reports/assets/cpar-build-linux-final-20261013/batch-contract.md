# CPAR-38 最终 Linux 门槛与条件式单次 Chat JSON 诊断

本批 base 为 51281f0a7255e178ddc770034a8141b197e67d6f；历史检查点不作回退。
仅拥有本目录、同名报告及 output/cpar-build-linux-final-20261013。
保留用户 AGENTS、无关改动、全部历史报告、实际载荷、FAIL/PARTIAL 和 stopped 预算。
产品 bf822b14 与 binary/manifest/metadata、Task42/Phase45 已按完整摘要复核；不重建或重跑 Full。

| 场景 | 实际入口 | 预期证据 | 退出路径 |
| --- | --- | --- | --- |
| A normal | system-control A → linux-system-validation → StartupProbe.launch/wait_listeners/quiet_audit | Type=exec；实际 /proc 身份、唯一 env、guard/cap4、socket/PID、管理 GET200；真实 ≥65秒静默 | 成功 start receipt/freeze/launch/argv/budget 绑定后执行完整 remote-exit.py 成功分支；停止意图 fsync、PID/start 复查、实际 stop；后5秒审计不变 |
| B exec_failure | 同一启动入口，存在且0700的无效格式文件；CollectMode=inactive | 非零 launch；清理前 loaded/failed、ExecMainPID>0、ExecMainCode1/Status203、Result exit-code；原生 journal 的 Exec format error；首次观测和原生输出保留 | 确认无活 PID、停止预算/意图持久化；失败状态保存后才 reset-failed；unit/cgroup/PID 均不存在 |
| C management_auth_failure | 完整 exec/identity/listener 后一次错误合成管理Key GET | 固定产品契约404，第一次失败即停止，无第二GET；启动/application失败独立记录 | 完整归属绑定、预算/停止意图先持久化、PID/start复查后停止；退出PASS不升级启动 |
| D audit_tail_injection | 真实65秒审计首次观测到窗口结束时，在该真实时间读数返回给审计循环前追加明确 synthetic marker | 原审计文件追加/fsync；注入时间 ≥65秒，发生在最终身份/listener/audit检查前；quiet_audit_not_flat；无 startup PASS | 安全绑定退出；保留原始非空文件、后5秒摘要不增长；synthetic事件与真实网络事件分别计数 |

最多4个新的隔离槽，各场景一次。每槽 durable UNKNOWN 先于 SSH/launch；新目录、unit、身份和零Provider预算。
意外失败、身份未知、非预期外连或退出未知立即锁停后续；不重试槽位、不恢复旧预算、不追加第五次。
预设负例仅在拒绝原因精确命中且资源退出可验证时 PASS。
PrivateNetwork=yes、独立namespace仅lo，IPv4/IPv6 TEST-NET ENETUNREACH；不改变全局/生产网络，不安装基础设施。
合成disabled账号/钥匙绝不读取或复制真实凭据。所有适配及字节摘要单列，不授予正式账号资格。
保留 startup失败、identity失败、resource_exit和scenario测试四种独立结论；晚到身份仅清理。

A 的 synthetic exit adapter 仅替换epoch/unit路径、生产凭据HTTP读取为生产元数据摘要核对、
正式预算形状为零Provider预算及其保存函数。完整成功退出分支的freeze/start/argv/guard绑定、
StartupProbe身份/持久化停止/复查、物理停止、审计、SQL对账和文件权限保留并真实执行。
原正式程序与适配程序的完整diff和摘要冻结；适配不是生产配置/真实账号证明。
D 的 clock adapter 始终调用真实 monotonic/sleep，在首次读到 elapsed≥65 时先追加/fsync，再返回该读数。
这同时覆盖末次sleep返回和下一次循环检查才跨过截止时间的调度边界；不缩短65秒、不替换最终审计检查。

Linux四槽门槛与独立Standards/Spec诊断前置审查全PASS才可观察正式材料。
全新epoch chat-json-linux-final-20261013；fresh Pin≤1、原Chat JSON≤1，合计≤2；历史35意图/34确认发送独立。
准备时同授权连续性及材料严格>3600秒，只读一次；不足/未知BLOCKED，无主动刷新、重新授权、换账号或等待重试。
新冻结driver/payload/binary/manifest、账号源、route/model/Client Key/egress及运行身份；正式进程、监听、候选池和65秒静默必须全部PASS。
旧source DTO可选revision摘要与候选、fresh Pin执行lease严格校验分离；实际lease不可由材料revision推算。
保持POST /v1/chat/completions、grok-4.5、stream=false、max_completion_tokens=4096及原请求SHA
245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062。
首个正式失败、未知或矛盾永久停止epoch；无retry/fallback/账号模型替换/预算恢复/修复后真实追加。
HTTP200仍需保真契约；原生capture仅私有0700/0600，公开只脱敏结构/enum/计数/摘要。
缺独立Provider接收/费用证据保持UNKNOWN。未执行Chat不标NOT_REPRODUCED，旧502根因不推断补齐。

分别交付本地合成、回执重放、真实Linux、正式启动、Pin/Chat、两轴审查、退出、GitHub、remote CI。
既有progress body-only追加且完整前缀保留，独立GET评论/工单/分页依赖；全票未满足保持OPEN。
只提交本批拥有文件；无push、发布、部署、生产服务修改或六组合矩阵。
