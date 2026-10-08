# GitHub #57 本地证据补充草稿：B-04 / P2

仅本地草稿，未发布、未关闭工单。本轮授权不包含 GitHub 写入。

Release-readiness **B-04 RESOLVED_LOCAL**，不是 CPAR-04，也不是整个用量 #32 完成。
固定 base `08812db2373583b8a725b80071efe75b69f9d7ae`，最终源码
`062f8281279b9bc6a0dc36b2859120e26e5f85dd`；实现 `42fae997`，Standards 登记修复 `640f4a18`。

Chat/Responses 在首个实际 read 已校验 Usage 后 bootstrap 失败，原来观察数为 0。修复保留原错误，
先确认准确失败 Attempt，再确认 Usage；缓存/思考细项、provenance、input accounting 与 unknown
保持。新明确 Usage 按 Attempt 幂等标识，失败与合法备用成功分别归属，无补采重放。Messages
合法 message_start 已先 handoff，不属同一缺口，已有输入/缓存观察保护通过测试。

公开 gateway + TCP loopback 验证真实首读 516/259 bytes，使用实际 read 字节断言。
真实 BoundedEventQueue/AsyncSqliteEventWriter/BillingWorker 和 Store/账本/protected read 保留相同事实，
重复确认不重复记账。管理 HTTP 是调用生产只读 reader 的测试 facade，人工历史 fixture 只证明
Store 表示。没有带 B-04 新记录的真实进程重启证据，没有最终完整上游 Usage 的保证。

本地验证：

- RED：Chat/Responses 各 exit 101、0 passed / 1 failed，保存真实输出/fixture/hash；旧 harness 失败不算行为 RED。
- 三协议与负例/快照/备用/protected read：6 passed；B-01/B-02/B-03 相邻回归：32 passed。
- 八包定向：949 passed / 0 failed / 8 ignored，实际 dirty HEAD 08812db2，文件哈希绑定 42fae997。
- Spec 修复后 Store：9 passed / 0 failed，实际 dirty HEAD 640f4a18，文件哈希绑定最终源码。
- 最终源码干净隔离 Full：45/45，exit 0；1487 passed / 0 failed / 12 ignored；未强制运行授权型测试。
- Standards PASS / Spec PASS：固定 base 至最终源码全部 26 文件，独立 scope + 最终 delta + blob 证明。两项 finding 已关闭，原 FAIL/中断保留。

Full UTC 2026-10-08T11:22:01.085006Z–11:27:20.214834Z，319.095 s；输出 SHA-256
`0abdc8fa13ef67b07f444958187751c834bf2bb646a2452d5a5423413af4a547`。
完整 [完成报告](../../cpar-b04-bootstrap-usage-20261008.md)、[结构化验证](verification.json)、
[Full 源码绑定](full-source-manifest.json)、[评审覆盖](review-coverage.json)、[评审记录](review-rounds.md)。

真实 Provider 请求 0；无生产凭据、生产改动、部署、发布、push 或 GitHub 写入。
新源码远端 CI/制品/真实渠道/生产验收均 NOT_RUN；release BLOCKED，不能继承旧候选结果。
探索前等级和数值预算未记录的过程偏差保留，不追溯改记合规。

本轮只解决 B-04。A-01/B-01/B-02/B-03 已有 RESOLVED_LOCAL 不扩展；原始 P2 剩余由 9 降至
8：A-02～A-08、B-05。S/E/O 独立保留；#32、Batch B、#57 与发布均未完成。
建议下一项 B-05，另行界定外部版本化 master-key ring/真实进程重启，本文不授权相应实施。
