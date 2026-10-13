### 2026-10-14 最终 Linux 系统验收与条件式 Chat JSON 诊断收口

记录时间（CST）：2026-10-14T00:50:22.813397+08:00。本批新报告 `docs/reports/cpar-build-linux-final-20261013.md` 与同名 assets 仅在本地提交，未 push；此处不声称 GitHub 能读取未发布文件。旧 progress 全文、历史 FAIL/PARTIAL、旧停止预算与实际冻结载荷保留，本段只追加。

**Linux 系统门槛 PASS（4/4）；前置独立 Standards/Spec PASS/PASS。正式材料、准备、启动 PASS，但 Pin 入口在预留和发送之前 BLOCKED，Chat NOT_RUN，整个正式诊断 PARTIAL/BLOCKED。**

| 本批隔离系统槽 | 实测结果 | 启动 / 资源退出 |
| --- | --- | --- |
| A | 五门通过，真实静默65.450398秒；完整 adapted remote-exit 成功分支实际执行，freeze/start/argv/guard/零预算及 stop intent-PID/start 复查绑定 | PASS / PASS |
| B | 存在且0700的无效格式文件实际到达内核exec；原生 loaded/failed、203/EXEC 与 Exec format error 在 reset-failed 前持久化 | 预期FAIL(exec) / PASS |
| C | 首次且唯一错误管理Key请求按产品契约404；立即停止，无第二次请求 | 预期FAIL(application) / PASS |
| D | 窗口末端65.419759秒追加明确synthetic marker，原最终检查 quiet_audit_not_flat 拒绝；159-byte原audit保留，synthetic1、实际外连0 | 预期FAIL(quiet) / PASS |

A/B/C/D各一次，4/4新槽已用尽并停止；均为合成账号/钥匙、PrivateNetwork隔离与真实systemd/proc/监听/等待，不证明真实账号资格。独立原件readback PASS，生产身份/配置摘要不变，没有基础设施安装或全局网络改动。

新正式 epoch `chat-json-linux-final-20261013`：一次同授权只读观察余效16395.674秒，实际prepare余效16231.104秒（严格>3600），无主动refresh/重新授权/账号替换；正式启动一次，真实65.472122秒静默、实际身份/监听/候选池通过。旧source DTO可选revision=null、candidate revision147与actual lease严格区分，fresh lease未建立（null）。

唯一 Pin 入口的首个不成立断言是 frozen `remote-pin.py:91`，`capability_observations=[]`，len0要求1，发生在 reserve_pin（125行）之前。声明为空的原因 UNKNOWN；不绕过身份/能力断言，不从材料revision推算lease。实际新Pin意图0、Chat意图0、总确认发送0；Request/Attempt/Finished/Usage/ledger/native capture新增均0。Provider独立接收数及费用仍 UNKNOWN，旧502精确根因 UNKNOWN。原请求 SHA `245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062`、grok-4.5、stream=false、max_completion_tokens=4096不变；未执行Chat，不能标NOT_REPRODUCED。

首次 BLOCKED 后 epoch 永久停止，未用2个新意图封存，历史35意图/34确认发送与旧预算不恢复/转移；没有 retry/fallback/换账号模型/refresh 或修复后追加真实调用。正式资源退出和独立最终读回 PASS；系统4次+正式1次资源退出共5/5，unit/PID/cgroup/监听无遗留，原startup/stop/audit证据保留。

最终本地合成全流37/37、系统设计7/7、历史回执离线重放3/3；旧34/36、原RED/FAIL/PARTIAL全部保留。产品source仍 `bf822b14a62c21cf582a7539e3635f69bd8b7677`，binary SHA `cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e`，构建Task42/Phase45按实际摘要复用，无新build/Full。system freeze `f67a5091ba93741ecfd1e0e1608a8b25df708aeb605714f91bab384b8b960d4f`；实际payload freeze `d00c82797519a31c5973717c0eb5c628d70a9078baa089248c75ac63fe3d5f61`。最终文案修正固定本地HEAD `14f694ee9483dce95b6a5305a9ece863c027578d`；运行前两轴固定9d670ba7，实际执行源/17payload未变。

**最终 Standards PASS；Spec PARTIAL（执行契约/修正材料PASS，正式资格缺口1）**。初始D截止调度Spec FAIL、只读helper初次FAIL及报告UNKNOWN文字初审FAIL均原样保留，分别有最小修正和独立窄范围核验，不把旧PASS迁给新执行字节。

**下一轮协议矩阵资格 BLOCKED**：仍缺唯一完整serving声明、实际fresh Pin lease及原Chat真实保真诊断证据。本批不启动六组合矩阵；Remote CI NOT_RUN（固定执行审查点Actions匹配run0、无push/dispatch），发布/部署/生产验收未执行。#47/#57/#9均保持OPEN，票面AC/元数据/依赖不更改；Linux通过或单诊断通过都不能关闭整票。
