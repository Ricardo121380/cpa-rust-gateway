### 2026-10-13 CPAR-38：Linux 启动修复与三次零Provider系统验证
<!-- cpar-linux-start-20261013 -->

本批PARTIAL，正式诊断BLOCKED；没有本次Chat结果。base9aa08db6，产品bf822b14及binary/metadata/manifest、Task42/42/Phase45/45完整日志和适用审查摘要核对后复用，build/install/Full重跑均0。AGENTS及无关工作区、历史回执/停止预算保持。

新增驱动先RED后最小修复：Type=exec同步execve、20s systemd启动期限和25s launch timeout；首次身份断言前fsync脱敏systemd/PID/start/exe/argv-state/guard/唯一环境/时间/闭合原因。原生launch输出仅私有0700/0600，公开只摘要/长度。清理先完整归属、持久化stopped预算/意图、复查PID/start后只停本批unit；晚到绑定不升级失败identity或startup资格。正式transport绑定系统门槛摘要，PARTIAL拒绝；材料不足整小时不等待/刷新。完整实际控制器/阶段34/34 LOCAL_SIMULATED；脱敏真实回执的离线oracle3/3，不能冒充Linux复测。

真实new-vps为systemd255.4/aarch64。实际3/3个独立零Provider系统验证槽位、3次systemd-run、观测到2个unit/exec。run1正常启动身份/监听/应用通过，真实65.4558秒静默及退出PASS；run2预设缺失binary返回1，首次unit不存在，未取得203，原FAIL及具体原因UNKNOWN保留；run3错误管理Key首GET返回产品契约404，身份/监听/系统隔离/安全退出通过，原401 oracle FAIL保留，修正后的离线重放单列。namespace仅lo、IPv4/IPv6 TEST-NET ENETUNREACH，无全局网络/生产改动，无真实账号凭据读取或复制。

3次资源退出及新鲜只读复核均PASS：PID0/not-found、旧PID消失、临时host端口无监听、5秒审计平坦，合成DB事件/账本均0，旧/新预算stopped、私有权限保持；生产进程/启动时刻、unit/current/Caddy/binary摘要一致。修正后的exec负例及最终观测delta未再次Linux执行，完整system gate仍PARTIAL，不将退出PASS或离线oracle修正改写成启动/系统全通过。独立Standards首审FAIL揭露成功启动后退出分支遗漏，先保存6个RED断言，再复用完整归属/意图/复查流程；最终退出补修33/33合成通过；第二轮Spec再揭露65秒审计末端事件可错误PASS，先RED再补末端audit复查，最终34/34通过；两项补修均未Linux执行。保留首审FAIL与未完整Spec；后续固定base/head终态另有本地回执，不能升级系统门槛。

计划全新chat-json-linux-20261013本地停止，远端正式epoch/上传NOT_CREATED；prepared-only新17文件freeze不等于实际执行/账号/lease资格。材料连续性/整小时、正式launch/候选池/65秒门槛NOT_RUN。fresh Pin0、Chat0、新意图0，新增网关/Pin确认发送0；历史35/34独立，旧及本次停止余额不恢复/转移。正式Request/Attempt/Finished/Usage/ledger/nativecapture及真实lease均未建立，Provider独立接收数/费用UNKNOWN。原Chat字节SHA、grok-4.5、stream=false、max_completion_tokens4096保持；无Chat结果，旧502根因UNKNOWN，不标NOT_REPRODUCED。

本地报告docs/reports/cpar-build-linux-start-20261013.md及同名assets（未push，不宣称远端可读取）。下一道门槛是另行授权有界零Provider复测修正exec负例和最终代码，取得完整system PASS后再建立新诊断资格/新freeze/整小时材料及真实Pin lease。六组合、声明扩展、remote CI、发布/部署/生产验收仍未完成，本批不执行矩阵。保留三票正文/AC/元数据/依赖/OPEN；局部启动/单诊断不能关闭整票。
