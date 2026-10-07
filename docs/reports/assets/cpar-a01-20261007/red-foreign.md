# A-01 RED：旧实现允许不同身份回调

产品源码：`602874b9ed9943c146c8d12f77ab1cc1adbfa167` 加第一条HTTP回归。命令退出码101；预期409，实际202。

```text
   Compiling gateway-http-actix v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/crates/gateway-http-actix)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.33s
     Running tests/managed_resource_inventory.rs (target/debug/deps/managed_resource_inventory-c19e5a8fceda9a3b)

running 1 test
test legacy_codex_callback_rejects_different_identity_without_changing_account ... FAILED

failures:

---- legacy_codex_callback_rejects_different_identity_without_changing_account stdout ----

thread 'legacy_codex_callback_rejects_different_identity_without_changing_account' (41213475) panicked at crates/gateway-http-actix/tests/managed_resource_inventory.rs:2227:13:
assertion `left == right` failed
  left: 202
 right: 409
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    legacy_codex_callback_rejects_different_identity_without_changing_account

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 44 filtered out; finished in 0.09s

error: test failed, to rerun pass `-p gateway-http-actix --test managed_resource_inventory`
```
