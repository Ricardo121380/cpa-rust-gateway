# Quality gates

P0-04 定义所有后续 Task 必须继承的本地质量和供应链门禁。

固定的外部质量工具版本为 `cargo-deny 0.20.2` 和 `cargo-audit 0.22.2`；P0-05 的 CI 安装步骤必须使用相同版本和 `--locked`。

## 快速代码门禁

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
scripts/check-source-policy.rb
scripts/test-secret-scan.sh
scripts/check-contract-tests.rb
```

`check-contract-tests.rb` 解析每份行为契约的 `Corresponding tests` 小节，要求其中列出的每个测试
函数名在源码中确实存在。契约的这一节就是从"要求的行为"到"证明它的测试"的证据链；此前无人校验
这些名字，因此重命名或删除一个测试会让契约继续声称一份已不存在的证据——恰好在审计或阶段 Gate
要依赖它的地方悄悄烂掉。

## 供应链门禁

```text
cargo deny check
cargo audit
```

- `deny.toml` 使用显式许可证 allowlist；未列出的许可证直接失败，因此 AGPL/GPL/SSPL 不会被无意引入。
- 依赖必须来自允许的 crates.io registry；Git 与未知 Registry 默认拒绝。
- Registry wildcard dependency、Yanked crate 和未忽略的 RustSec Advisory 直接失败；本 Workspace 内的 path-only dependency 允许不写版本号。
- 重复依赖版本当前告警但不阻断；每个告警必须在依赖升级任务中评估，不能静默忽略。
- Advisory 例外必须注明 RustSec ID、影响、补偿控制、到期时间和批准人；P0 不设例外。

## 源码纪律

- 每个 Rust Crate 根文件显式 `#![deny(unsafe_code)]`。
- 生产代码禁止 `unwrap()`、`expect()`、`panic!()` 和 crate 级 `allow(unsafe_code)`。
- `TODO`/`FIXME` 不能作为未登记需求留在 Rust 源码；后续工作必须对应 Task/Issue。
- 格式、Clippy 和测试必须使用 `rust-toolchain.toml` 固定的同一工具链。
- Release 产物统一通过 `./scripts/build-release.sh` 构建。macOS linker 会生成非确定性的 Mach-O `LC_UUID` 和关联 ad-hoc signature；脚本先移除 linker signature，按去除 UUID 后的产物内容生成稳定 UUID，再使用固定 identifier 重新 ad-hoc sign。Linux/非 Mach-O 产物保持不变。
- 禁止为 Apple target 全局使用 `-Wl,-no_uuid`：当前 macOS `dyld` 会拒绝运行缺少 `LC_UUID` 的 build script 和最终程序。

P0-05 会把这些命令封装为统一入口并放入 CI；本文件定义的门槛不能在封装时放宽。
