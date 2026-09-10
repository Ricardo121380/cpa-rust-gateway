# Prism V4 发布门禁依赖修复

2026-09-10，用户授权完整生产发布流程时发现。原候选`92d1d50`的签名构建成功，
但正式delivery-gate的供应链步骤拒绝该锁文件；该候选没有切换到生产。

- `h2 0.4.15` → `0.4.16`：修复空DATA帧无界排队问题。
  来源：[RustSec RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258.html)。
- `chacha20 0.10.1` → `0.10.2`：原版本已yanked，改用同次版本可用补丁。
  来源：[RustCrypto crate发布页](https://docs.rs/crate/chacha20/0.10.2)。
  此实例由rand 0.10.2引入；现有SecretStore使用的chacha20 0.9.1实例未改变。

仅更新这两个版本/校验和及rand的对应引用。cargo update附带的无关socket2/windows-sys
解析变动已排除，cargo check --locked确认精简锁文件有效；没有添加ignore/exception或弱化门禁。
本机cargo-deny advisories及cargo-audit通过。受影响HTTP/上游回归与新的CI正式门禁结果
记录于生产发布执行记录。需重新构建和验签新revision，不能复用旧候选二进制。

原失败记录：https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34438968428
