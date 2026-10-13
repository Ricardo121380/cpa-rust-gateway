# systemd exec 边界与失败状态保存

只读实机预检为 systemd255.4、aarch64、kernel6.17.0-1018-oracle；旧三unit均PID0/not-found。
候选binary及guard摘要与复用原件一致，历史停止预算仍为35意图/34确认发送。
没有在预检启动unit、读取真实凭据、观察账号余量或发送Provider请求。

systemd v255 的 [service 文档](https://raw.githubusercontent.com/systemd/systemd/v255/man/systemd.service.xml)
说明 Type=exec 等待 fork 和 execve 成功；这不代替应用、身份、监听或审计检查。
v255 [unit 文档](https://raw.githubusercontent.com/systemd/systemd/v255/man/systemd.unit.xml)
说明 CollectMode=inactive 保留 failed unit，reset-failed 后才能释放；卸载会丢执行状态。
[systemd-run 源码](https://raw.githubusercontent.com/systemd/systemd/v255/src/run/run.c)
显示 aggressive collection 才设 inactive-or-failed。本批显式设 inactive，不使用 --collect。
以上解释的是设计语义。B 仍须同时实测 loaded/failed、ExecMainPID、原生203及
unit journal 的 Exec format error；客户端拒绝、not-found 或只有非零退出都不够。
首次 startup 观测保持；原生 systemctl/journal 在 cleanup/reset-failed 之前另存并fsync。
原始stdout/stderr和journal仅在私有目录，公开回执只保留退出值、结构、摘要与闭合判断。

D 不伪造时间。末次sleep仍调用真实sleep，在真实间隔达到65秒时append/fsync合成标记，
再返回给原 StartupProbe 的最终身份/listener/audit检查。只有 quiet_audit_not_flat、无成功
start receipt、标记原件保留及安全退出齐全才通过。不得把标记算成Provider外连；任何其他
审计事件阻断，并保留原件。
