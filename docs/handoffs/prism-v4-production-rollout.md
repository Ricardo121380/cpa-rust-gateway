# Prism V4 新加坡生产发布执行记录

2026-09-10。用户已明确授权完整执行：发布包、隔离验收、单渠道真实小测、生产升级与必要回滚。
目标为 `new-vps`，仅CPAR；不变更Caddy/DNS/防火墙/Autoreg，不停止Jakarta实例。
本文件随实际执行更新，不能把准备状态视为已上线。

## 冻结版本与发布渠道

- 待发布：`92d1d50c61ed5a92ff6c35a56a5739cafd29d28a`。
- 实际现役：`4bb55b147518d32ac0ce6210ce652b4bb1668663`，aarch64；服务和两个loopback listener正常。
- 新版是现役版本后继，未回滚历史功能。已推送同项目`codex/prism-v4-delivery`供既有签名流程构建。
- 发布workflow：https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34438965555
- 正式门禁：https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34438968428
- 仓库公开；源代码提交推送，产物留在workflow artifacts，无额外GitHub Release发布。

## 现役检查与备份（已完成）

SQLite quick_check=ok，schema最大21；一个active配置；管理端口没有Caddy映射，公网健康返回200。
备份位于服务器 `/var/backups/cpa-rust-gateway/prism-v4-20260910-92d1d50`，权限0700：
在线一致数据库副本、旧gateway、配套6个服务凭据文件、启动参数与哈希收据。
密钥/库/配置内容未外发，操作记录只保留状态、数量和哈希。
旧二进制SHA256：`44955dee3abde3e9a40060b19300971e64667309d78243e2143a1d7c9a3bb2af`。
上线前仍需停止CPAR后取最终一致备份，初始在线副本不替代最终恢复点。

## 隔离与回滚演练

预验收使用独立状态目录 `/var/lib/cpa-prism-preflight-92d1d50`。
新network namespace只有lo，无默认路由，服务降权为cpa-gateway；副本内的刷新/目录worker
无法连接Provider，避免与生产账号争用刷新权。预验收端口18280/18281只存在于该network namespace。

已实际验证：旧binary+schema21正常启动；手工应用本次唯一22号迁移后旧binary拒绝启动；
副本撤销22号迁移及对应schema_migrations记录后旧binary恢复，管理页面及6模型读取成功，quick_check=ok。
因此不得沿用旧发布文档“只换二进制即可”的结论。

正式回滚路径：先停止新CPAR并保留失败现场一致副本；在单事务中撤销仅22号新增表及迁移登记，
保留事件、账本、账号/凭据和其他既有表的新写入；切回已核验的旧二进制后启动。
第22号表的失败隔离记录保留在失败现场副本中，不能无备份直接丢弃。
若迁移/完整性异常使定向撤销不成立，则使用停机后最终备份恢复；必须在恢复收据说明恢复点及差异。

## 真渠道与上线条件（待执行）

只选一个已有渠道和一个exact模型，短固定内容，普通/流式合计最多10次请求，无自动批量重试。
真实数据面请求需要已有Client Key文件；标准CPAR配置目录未找到，已向用户询问路径，禁止猜测或打印密钥。
生产Key/配置不会为了绕过缺失认证而被直接SQL改写。管理Channel Pin不能冒充Client Key数据面/账本验收。

ARM64产物必须经独立Cosign身份验证、仓库artifact/SBOM/哈希验证后上传；服务器再验哈希。
新版副本迁移、配置编译、有效模型、运行矩阵、Prism/CSP通过后，才进行真实渠道小测与生产升级。
安装采用版本目录，保留原目录/启动选项/凭据源；短维护窗口内停服务、最终备份、切换、启动。
上线后核验二进制身份、健康、真实请求、持久事件→计费以及管理面私有边界；任一关键项失败按演练路径回滚。

## 发布候选更新（供应链检查修复）

原92d1d50候选的功能fast通过，但供应链门禁发现h2漏洞与chacha20撤回版本；未上线该候选。
仅对两个依赖做补丁更新，见 [依赖修复](../reports/prism-v4-release-dependency-fix.md)，
本地184项HTTP/上游测试通过、4个既有ignored；cargo-deny advisories/cargo-audit通过。
新的候选revision：`18f29a3be34422eb30ea7621bdf61459628b8d5f`。
新签名构建：https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34440246371
新正式门禁：https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34440249222
服务器备份目录中的92d1d50仅为本次操作标识，不代表最后安装的revision。
旧候选上传发生网络中断，未通过服务器哈希验收，已隔离；新版演练要求完整verified-artifact
收据和二进制哈希一致后才启动，禁止使用部分上传。

## 当前停止点：尚未上线

新签名构建已经成功；新正式门禁最后读到fast通过、供应链工具安装中，连接失败后未能确认最终结果。
GitHub产物下载出现EOF/TLS握手超时，GitHub连接器也传输失败；SSH出现Connection closed。
没有使用未完整上传的文件切换生产，没有停机或重启生产服务；真实Provider测试请求为0。
一次旧候选隔离启动因上传未完成提前失败，不计作验收通过；随后加入完整验签收据和SHA前置条件。
测试Client Key文件所在机器和路径仍待用户提供，不要把密钥值发到聊天。

恢复执行顺序：

1. 确认18f29a3完整门禁通过，重新下载ARM64新产物并独立验签。
2. 使用rsync压缩、partial和delay-updates上传；服务器校验SHA并保存verified-artifact收据。
3. 同步最新offline.py，在只有lo的network namespace执行new和rollback-new演练。
   验证新版启动、迁移、有效模型和矩阵，以及撤销22号迁移后既有表逻辑指纹保持一致。
4. 使用用户指定的Key文件执行canary.py before；固定一个近期成功的exact模型和单渠道，
   公网响应ID必须对应到这台主机的持久事件，不能仅用healthz推断流量归属。
5. 同步cutover.py，NEW必须为18f29a3完整hash；确保当前active配置ID/revision匹配演练副本。
   所有前置条件通过后才能停机、取得最终备份、切换和启动。
6. 执行canary.py after并核对账本/用量；关键失败按已经演练的down22路径恢复旧版。
   管理18181保持loopback，不改变Caddy/DNS/Autoreg/Jakarta。

本机恢复脚本保存在output/prism-v4-rollout-20260910，只有操作逻辑和已知路径，不含密钥。
服务端操作目录中的92d1d50是本次操作标识，不是最终待安装revision。
本次四步尚未整体完成，也没有安排后台自动继续；既有部署和测试授权持续有效。
