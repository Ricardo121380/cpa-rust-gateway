# 本批证据读取与复现

行为／驱动源码固定为 `c1b304d02e61fe1c75653a32fba920093462add6`，初始基线为 `d3d2e3c5bf0d9521f3fb0d830344b34053f62155`。报告提交及最终评审归属另列，不将报告HEAD替换成已执行的代码HEAD。

- [源码绑定](source-binding.json)：四个owned实现／契约路径、两份历史二进制来源及SHA，OpenAPI／ADR／UI的差异范围。
- [完整执行索引](execution-index.json)：每条原argv、cwd、执行前后HEAD及dirty状态、UTC、退出码、原日志SHA／bytes、归档gzip及原receipt；失败原样保留。日志gzip使用mtime=0，解压字节须与raw_log_sha256、raw_log_bytes一致。
- [最终T12](clean-t12/receipt.json)及[具体状态比较](clean-t12/t12.json)：最终干净驱动，非空集合、两份binary哈希、旧读者、key版本、coherent restore和实际接收断言。
- [Full步骤报告](full-check.md)：最终代码上的45步结果；Rust测试总数由execution-index的final-full-network-retry原log统计。前一次611来源Full、错误release环境Full及最终源码网络失败Full分别保留在full-check-611949b6.md、full-check-failed-release-revision.md及full-check-failed-network.md，不用于最终产品修复的成功门槛。
- [独立历史Spec补审](review-historical-supplement.md)：184份原回执的定位与覆盖边界；旧来源／时间保留。
- [后继台账](issue57-successor.json)：198基础行、49扩展未知声明行和所有故事／F／T／AC pointer；没有真实执行行。
- [一手来源索引](primary-source-index.json)：公开冻结原始文件版本、URL及实算hash；不提交下载的完整第三方源码。
- [用户材料保存核对](preservation-check.json)：AGENTS原SHA，以及5817个原未跟踪条目的lstat元数据，未触碰原符号链接目标。

复現环境为macOS arm64、既有Rust/Node/OpenSSL和sandbox-exec。先在两个独立干净检出分别构建历史binary：schema32 `24142a9008e58fbc87d0b5791bbb166b45234850`，schema34/pre-Attempt-Usage `08812db2373583b8a725b80071efe75b69f9d7ae`。按原receipt执行npm ci和cargo build，复制各binary到私有测试目录。历史target不能与当前target共用；不改变依赖锁。

在c1b干净检出中，Full使用默认development编译身份，当前普通binary随后绑定同一完整源码SHA：

```sh
env -u GATEWAY_RELEASE_REVISION CARGO_TARGET_DIR="$CPAR_CURRENT_TARGET" scripts/check.sh full
env GATEWAY_RELEASE_REVISION=c1b304d02e61fe1c75653a32fba920093462add6 CARGO_TARGET_DIR="$CPAR_CURRENT_TARGET" cargo build --locked -p gateway --bin gateway
env CPAR_T12_ONLY=1 CPAR_GATEWAY_BINARY="$CPAR_SCHEMA32_BINARY" CPAR_T12_OLD_READER_BINARY="$CPAR_OLD_USAGE_BINARY" CPAR_T12_CURRENT_BINARY="$CPAR_CURRENT_TARGET/debug/gateway" python3 scripts/acceptance/cpar-batch-b-http.py "$CPAR_SAFE_RECEIPT_DIRECTORY"
```

上述CPAR变量是复现者自行指定的私有绝对路径，不是凭据。确切已执行路径和参数以原receipt为准。T12的进程只允许loopback；driver首先实际验证localhost可达和非loopback TEST-NET出站EPERM，未满足时不能称隔离验收通过。

T12通过真实公共HTTP创建旧非空状态，两个冲突Usage仅由合成种子插入；旧账本读者探测只在独立复制状态上重置派生ledger/checkpoint。原source事件及旧DB在活动恢复前后均保留。harness只停止自己启动的进程，私有临时合成DB、credentials和master-key目录保留在本机，没有加入此归档。归档不是可部署的恢复套件。

真实Provider=0，remote CI／候选发布制品／部署／生产均NOT_RUN。当前代码未push；没有以GitHub blob链接暗示其已上传。
