

## 2026-10-17 三协议字段决策确认＋最小依赖门禁修复

本批从实际HEAD `312584184e7680594878460499c39af0a244d440`继续，未回退，保留用户AGENTS、原材料/FAIL/停止预算。重新核对13份旧日志归档、27份旧GitHub原件绑定；没有重新捕获或重证首个summary拒绝点。

依赖交付：`28568553ce5d8754f4063884c1c9c37821f68c9f`只把rustls0.23.45升到最小修复0.23.46；首次新版Full为44 PASS＋audit FAIL，发现quinn-proto五份公告，此失败独立永久保留。经用户第三张决策卡明确授权，`2f43057c9d718aee5904da12564d54ac697bf8b4`只把quinn-proto0.11.16升到共同最低修复0.11.18。两次都仅改Cargo.lock的对应版本和checksum；无其他升级、feature/ignore/门禁放宽。

最终source `2f43057c…`干净隔离检出的Full **45/45 PASS**，Rust **1515 PASS / 0 FAIL / 13 IGNORED**，Dependency policy和RustSec audit均PASS。最终日志原SHA256 `1398ed8c9821d0a363e8c7084963f2d7035fb51223cce8b3741139fd4f422f38`，Cargo.lock SHA256 `ee3d735f0f7aa364f3f4675e75a87751c8ad3784f0788ac244c4d391ce479cce`。原Full 43PASS/dependencyFAIL/auditNOT_RUN保持原结论。

实际生产UpstreamClientPool本地TLS路径4/4：验证CA的TLS1.2/1.3 HTTP往返成功，wrong-host/untrusted-CA拒绝；不关闭验证，不写全局信任。固定OpenAI Node7.36.0/Anthropic Node0.135.0七个内存fetch消费者探测表明：JSON未知字段保留不等于SSE/helper消费，存在覆盖/丢失/拒绝及大整数精度反例。均为合成本地证据，Provider调用0。

用户已明确选择：(1)opt-in native-v1＋专用消费者、summary/body分层、完整Usage按exact Attempt加密并作为非结算证据；(2)字面等值或预先冻结的适用可信别名证明＋public/actual upstream/native三段断言。ADR-0106方向因此Accepted；字段schema/JSON-SSE事务/界限/未知及冲突/存储回放/验收矩阵已形成实施契约草稿，安排下一批。本批没有修改协议行为。原样本native模型等价、未知字段解释和独立真实费用仍UNKNOWN，旧等值oracle不删除。

本地材料：`docs/reports/cpar-field-contract-dependency-20261017.md`、`docs/adr/ADR-0106-native-fidelity-profile.md`、`docs/reports/cpar-field-mapping-acceptance-20261017.md`与同名assets；本批不push。

依赖门禁通过不等于rich三协议/标准客户端/真实渠道通过。当前严格summary及额外Usage拒绝保持；标准矩阵、专用消费者、profile实现/reader兼容、实际模型证据及新候选门槛仍待完成。原工单正文/AC/metadata/依赖和开放状态保持，本进展不关闭#47/#57/#9。

本批真实Provider、fresh Pin、真实Chat、远端验收实例、正式候选、远端CI、部署、发布均0/NOT_RUN。只有语义实现、公共正反例、后续不可变Task/Phase/Full、双轴审查和新ARM64 source/binary/manifest/reader/egress/Linux系统门槛满足后，才能另行冻结新epoch并申请具体真实预算。旧stopped预算不恢复、不转移。
