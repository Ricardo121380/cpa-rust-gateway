# Grok P-256 依赖链安全维护

日期：2026-10-25（Asia/Shanghai）。用户授权在 #68 当前工作区修复
RUSTSEC-2026-0387，并要求独立提交、保留原始验收记录。

## 源码与修复范围

工作区 `worktrees/cpar-68-count-tokens-20261025`，分支 `codex/68-count-tokens`；
修复前 HEAD 为 `e94ebd042572bff02bfe7a92d28abe5cab783b93`，开始时工作树干净。
本记录对应该 HEAD 加本次安全修复；受测源码清单与日志位于本工作区
`output/rustsec-2026-0387/`，没有复用其他 worktree 的 target 或数据库。

[RustSec 公告](https://rustsec.org/advisories/RUSTSEC-2026-0387.html) 指出
crypto-bigint 多项原本应为恒定时间的操作存在时间差异，修复范围为 `>=0.7.6`。
本地原链为 `provider-grok → p256 0.13.2 → elliptic-curve 0.13.8 → crypto-bigint 0.5.5`。
旧 elliptic-curve 要求 crypto-bigint `0.5`，因此不能在原兼容版本约束内直接解析到修复版。

- [根清单](../../Cargo.toml) 仅升级 P256 至稳定版 `0.14.0`，保留关闭默认 features，
  增加其系统随机源 `getrandom`；删除不再使用的直接 `rand_core` 依赖及边界声明。
- 用 `cargo update -p p256 --precise 0.14.0` 更新锁文件。新主链为
  `p256 0.14.0 → elliptic-curve 0.14.2 → crypto-bigint 0.7.8`；同一 bigint 还被
  `primefield 0.14.0` 和 `rfc6979 0.6.0` 使用，锁文件没有残留受影响版本。
- 锁文件删除13个 package/version、新增19个：12个版本升级、7个新增依赖，
  另移除旧 `const-oid 0.9.6`。这些版本变化全部属于新旧 P256 传递依赖闭包；
  未升级其他依赖线。其他变化是 features 引起的边变化或 hmac/sha2 同名版本消歧。
- [DPoP 模块](../../crates/provider-grok/src/console_dpop.rs) 使用 `SigningKey::generate()`
  和 `to_sec1_point(false)`。系统 CSPRNG、P-256、ES256、未压缩公钥坐标、64字节签名、
  JWK/thumbprint、访问令牌绑定与有效期规则保持。系统熵源失败时的既有 infallible API
  行为没有扩展为新的错误模型。
- 没有改变 HTTP、Router、executor、凭据持久化或跨 crate 的业务接口签名。
  Rust 内部暴露的 `SigningKey` 类型随 P256 版本升级；仓内调用者使用类型推导，统一重编译验证。
- 新依赖均来自 crates.io，许可证为 MIT/Apache-2.0；最高声明 MSRV 为 sha2 0.11.1 的1.88，
  低于项目固定1.97.1。没有启用 P256 的 pem、pkcs8、ecdh、std 或预计算表默认集。
  `deny.toml` 未改动，未添加 advisory ignore、skip 或策略豁免。

## 重复依赖告警评估

依照 [质量门禁](../quality-gates.md)，逐项核对本次 deny 的15组 warning：

| 依赖与版本 | 变化及保留依据 |
|---|---|
| block-buffer 0.10.4 / 0.12.1 | 已有；对应两条 digest 线，新 P256 共用已有的新线 |
| chacha20 0.9.1 / 0.10.2 | 已有；存储 chacha20poly1305 与 rand/HTTP 栈分别要求 |
| cpufeatures 0.2.17 / 0.3.0 | 已有；存储/旧 sha2 与 rand/新 sha1/sha2 分属两线 |
| crypto-common 0.1.7 / 0.2.2 | 已有；aead/cipher/旧 digest 与新 digest/P256 分属两线 |
| digest 0.10.7 / 0.11.3 | 已有；直接 hmac/sha2 与 sha1/新 ECDSA 分属两线 |
| getrandom 0.2.17 / 0.3.4 / 0.4.3 | 已有；ring、项目直接使用、rand/uuid/新 P256 分别要求 |
| hmac 0.12.1 / 0.13.0 | 新增重复；原 auth/Grok 直接依赖保留0.12，新 rfc6979 要求0.13；统一版本会扩大为直接 HMAC 迁移 |
| http 0.2.12 / 1.4.2 | 已有；Actix 与 hyper/reqwest/wreq 分别要求 |
| rand_core 0.6.4 / 0.9.5 / 0.10.1 | 已有；password-hash、测试 rand、HTTP rand/新 P256 分别要求；已移除 Grok 旧直接边 |
| sha2 0.10.9 / 0.11.1 | 新增重复；项目直接使用0.10，新 P256 要求0.11；统一版本会扩大为跨 crate SHA-2 迁移 |
| shlex 1.3.0 / 2.0.1 | 已有；bindgen 与 cc 的构建依赖分别要求 |
| socket2 0.5.10 / 0.6.5 | 已有；actix-server 与 Tokio/其他 HTTP 栈分别要求 |
| syn 2.0.119 / 3.0.2 | 已有；不同 proc-macro 依赖分别要求 |
| thiserror 1.0.69 / 2.0.19 | 已有；tokio-socks 与 OpenTelemetry/tungstenite 分别要求 |
| thiserror-impl 1.0.69 / 2.0.19 | 已有；配套两条 thiserror 宏依赖 |

本次新增 hmac、sha2 两组重复，移除 const-oid 重复；其余没有增加版本数。
不同版本约束仍存在，合并会扩大本次范围，故保留并记录，不以忽略配置隐藏告警。

## 验证与审查

| 检查 | 结果 |
|---|---|
| 修改前 cargo audit --no-fetch | FAIL，退出1，复现唯一 RUSTSEC-2026-0387 |
| 升级前增强 DPoP 测试 | PASS，4/4，旧依赖先建立 wire 行为基准 |
| 升级后相同 DPoP 测试 | PASS，4/4，含 wire JWK 验签、篡改拒绝、固定坐标、绑定/过期拒绝、缓存 successor 保留 |
| Grok Console/Web 与 Build OAuth 回归 | PASS，13 + 10；本地受控 transport 与合成凭据 |
| scripts/check.sh supply-chain | PASS，3/3；deny 四类策略与 audit 均通过 |
| scripts/check.sh full | PASS，44/44，退出0；含格式、Clippy、全部 Rust 测试、进程/多轮回归、源码与文档检查、密钥扫描及最终供应链门禁 |
| Workspace Rust tests | PASS，132个测试集合，1683 passed、0 failed、21 ignored；忽略项不计为通过 |

Full 回执为 `full-check.md` 和 `logs/full.log`，于 `2026-10-25T05:49:41Z` 完成。
`tested-source.json` 绑定修复前 HEAD 和5个变更的源码/清单/边界文件，其 SHA-256 为
`0355851816a7fc65b236da757597f735c14f43adfda6a06f43c993f0fb66025c`。
门禁前后逐项哈希一致；`tested.patch` 保存门禁开始时的暂存补丁。
Full 完成后仅补记本报告及工程路线图，文档静态检查、暂存密钥扫描和差异检查另行验证。
本次 #68 的4项生产 HTTP 计数测试也在新依赖下通过，包含不产生推理 Usage/账本的断言。

独立静态代码与依赖审查未发现可行动缺陷；依赖审查确认版本变化限于 P256 闭包，
并评估上述15组重复告警。代码审查的附加 docs.rs 请求失败，主代理已通过下载的官方
crate 源码确认 Generate 的系统 RNG、SEC1 未压缩编码语义，并完成升级后编译及回归。
这些验证不等价于证明整个密码学实现不存在其他缺陷。

初次新增测试曾因旧版 ECDSA error 没有实现标准 Error 而编译失败；仅将测试错误映射为
无值静态错误后重新通过，原失败保留在 `logs/dpop-before.log`，成功记录为
`logs/dpop-before-second.log`。供应链前后及全部后续日志位于独立输出目录。

## 历史证据与边界

[原 #68 实施记录](issue-68-count-tokens-20261025.md) 及 `output/issue-68/` 原始记录保持；
它们继续记载当时的42 PASS / 2 FAIL，不能用本次结果追溯改写。
新目录中的 `issue-68-evidence-before.json` 对原报告及日志共26个文件保存 SHA-256，
交付前逐项验证，26/26哈希保持一致。本次没有修改 #72/#74 工作区，没有 push、合并、关闭工单、部署或真实
Provider 调用；远端 CI、发布制品、实际客户端和真实 Provider 资格均 NOT_RUN。
