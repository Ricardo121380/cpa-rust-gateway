# A-01 Codex 授权回调身份保护修复

日期：2026-10-07，Asia/Shanghai。当前为修复源码检查点；最终不可变提交验证及两轴审查将在本报告补齐后交付。发布仍 BLOCKED。

范围是 2026-10-05 发布复核 A-01/P2，来源 CPAR-05/#14、父规格 #9、ADR-0099；与 CPAR-01/#10、AC-01 不同。固定起点与验收边界见 [scope](assets/cpar-a01-20261007/scope.md)。

原触发路径：注册的 POST /admin/credentials/{id}/oauth/callback → complete_credential_oauth → persist_oauth_credential_if_revision。有效 exchange 可提交不同或无法比较身份，原实现只检查 CAS，没有 old/new identity guard。

[RED](assets/cpar-a01-20261007/red-foreign.md) 以旧源码602874b9配合第一条回归确认：不同稳定账号 ID、相同邮箱收到202，而契约要求409。编译错误和测试错误形状修正不计作RED；它们仅留在本地运行日志。

修复把既有 same_observed_account 原规则迁到 AccountIdentity::same_observed_oauth_account，enrollment 和 legacy callback 共用。新的 reauthorize_oauth_credential_if_revision 读取精确version/id/credential revision，解密该记录并拒绝冲突，随后通过现有持久化 CAS。正常 renewal 的 persist_oauth_credential_if_revision 入口和后台 durable claim 保持各自语义。

原稳定account ID必须相同，旧已观察email也必须匹配；原契约允许的观察email比较保持，但不新增显示名或email替代旧稳定ID的降级。old/new都无可靠观察时拒绝。检查失败发生在AEAD重新seal与库写入之前；同身份保存保留Disabled意图和原拓扑。

开发工作区定向结果：5条callback/38组合PASS；管理HTTP集成49/49 PASS；定向Clippy、format/source/crate/contract/secret scanner PASS。这些开发结果尚不替代后续固定提交的Full与独立review。

Standards/Spec：PENDING。A-01归集最终状态：PENDING验证收口。其他12项P2、S-01及E-01–05保留原状态；#14/#57/#9仍OPEN，A批与项目不宣称完成。source80的Full、签名制品与迁移演练只作历史证据，不绑定本修复提交。
