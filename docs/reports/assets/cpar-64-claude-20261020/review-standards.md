# Standards review — #64

Independent read-only review of `0d13e485fca189fefd2801b0d2c72c7a991a45df...06f779f24434f7de95426b05f33484a1c4e33a15`.
The reviewer did not run tests or clients. Existing documentation overlays are excluded from the product diff.

Standards **PASS**。硬违规 **0 项**；Fowler 判断项 **0 项**。

- OS 隔离具备实际执行边界。`scripts/acceptance/cpar64-claude.py:38-52` 使用 Seatbelt `deny network*`、
  精确 listener 端口放行、任务外读取/写入拒绝及 securityd lookup 拒绝；`:60-97` 的独立探针要求
  IPv4/IPv6/其他 loopback、文件 open 返回 EPERM，另查 receiver accepts=0、sentinel 不变和 shell
  子进程拒绝。不是仅依赖环境变量。
- 观察器保持生产 handler 的原始 body。`apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/cli/http.rs:88-108`
  保存多值头字节，认证仅记录数量，并将原 bytes 恢复为 payload；没有 JSON 重编码。
  `messages/claude/controls.rs:121-150` 以重复 JSON 字段、完整 query、多值 beta 及认证排除断言覆盖这一点。
- 观测与兼容成功分开。`messages/claude.rs:90-107` 要求非空实际 HTTP、精确 400、完整响应和独立
  accepts=0；`:146-167` 检查失败计量及无 Attempt/Usage/ledger/summary。Python `:204-221` 同时检查
  subtype/is_error，默认即使观测成功仍失败退出；`--diagnostic` 明示仅观测通过。报告未升级成工具或取消闭环。
- #62 影响限证据格式。共享 headers 从字符串变为多值字节数组；对 `scripts/acceptance/cpar62*`、
  `cli/oracles.rs` 的范围检索未发现依赖旧头值类型的判定，既有判定使用 body/status/chunks。
  Rust 增量位于 `runtime.rs:505-506` 的 `#[cfg(test)] mod batch_c`，没有生产依赖边。

限制：静态审查，没有重新验证历史运行回执。独立 Full 与当前实际运行身份由主代理补齐。

## Supplemental identity delta

另一位干净探子只读审查 `06f779f24434f7de95426b05f33484a1c4e33a15...6edb8085f9e850edc55e68d382ed97acbc54a0c5`
的两文件小增量：**Standards PASS，硬违规 0，判断项 0**。`cpar64-claude.py:40-43` 在 port=None 时
不生成出站 allow，`:117-121` 的版本/帮助检查共用此禁网 fence；`:141-150` 仍验证任务 numeric loopback
并只放行其端口。报告 runtime 声明值和两个阶段说明与代码一致。未引入依赖/边界/质量门槛变化，
没有重复逻辑或闲置扩展点。此补审没有执行客户端或 Seatbelt，动态证明由主代理另行绑定。
