# #57 本地证据归集：执行观测与 ARM64 候选切片

目标 [CPAR-48 / #57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57)，父规格 [#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)，C [#39–#50](https://github.com/Ricardo121380/cpa-rust-gateway/issues/39)。本轮没有修改 GitHub 状态；读取时全部仍OPEN。

正式源码 `80f5838902eb56be6dccc3ce047eb227d6c18e6c`；[主报告](../../cpar-execution-evidence-and-arm64-candidate-20261002.md)、[两轴评审](review.md)、[Full45/45](full-gate.md)、[源码绑定](full-run-receipt.json)、[候选manifest](artifact-manifest.json)、[候选410/410](native-http/receipt.json)、[兼容/恢复](native-compatibility.json)、[发布与首批方案](release-and-first-acceptance-plan.md)。

| #57 相关要求 | 本轮事实 | 仍需完成 |
| --- | --- | --- |
| 新构建与实际attempt精确绑定 | actuallease/config/transport捕获；原子sidecar/hash/unknown；正式collector exactRequest/Usage/ledger；8条实际fixture例子 | 获准部署后重新pin并冻结每个真实组合；历史旧记录保持未知 |
| 三协议JSON/SSE归属和边界 | Linux ARM64候选真实gateway隔离410checks；错误/取消/超时、候选重试、材料/config/node切换、在途隔离 | 真实三协议JSON/SSE及实际Provider边界发送/取消/释放样本 NOT_RUN |
| 按有效声明完整验收 | scoped serving_snapshot 六项compiled声明；unknown停止、contradiction失败；30正式工具回归 | 生产旧runtime声明仍未知；新build下真实扩展样本/重启/存储故障等 NOT_RUN |
| 评审和质量门槛 | 两轴本切片PASS，两个P2先复现后关闭；固定源码Full45/45 | 远程CI及签名release NOT_RUN；不外推为全C/父规格PASS |
| Linux ARM64候选与兼容 | binary/OCI/SBOM/manifest完成；actualbuild信息与hash一致；31→34、原14表保持、31完整副本恢复PASS | 生产native Grok数据副本演练、旧ELF在恢复副本启动和目标动态库检查 NOT_RUN |
| 真实环境/具体发布方案 | new-vps仍b0cf36e3/schema31；完整备份、单服务切换、状态回退、停止条件；Krill后Build两个具体草案 | 新授权覆盖部署/启停/回退；实际推理预算；不含配置发布、账号恢复或权限修改 |
| 原Provider独立限制 | Kiro cap、Grok Web原生多轮/cap、未知relay onward仍显式阻断 | 独立解决及验收；没有新增例外或降声明 |

本轮软件观测与本地候选准备PASS；**#57/C BLOCKED于未部署、未真实验收与既有独立缺口；父#9未完成。** 本地PASS、远程CI、签名release、生产部署与真实Provider是不同证据层。旧C/D及上一轮工具包继续按原SHA/范围有效；原包未改。

剩余顺序：批准并取得签名候选 → 目标动态库/生产native状态副本和完整恢复演练 → 有界部署/版本核验 → fresh Client Key/账号/配置/出口/能力 → 授权pin+freeze → Krill7basics → Build7basics → 其他协议/SSE → 声明扩展/受控边界 → 按各工单验收条款归集。首批每格最多8次是方案预算，当前真实调用0，不是已有授权。

## 后续固定签名候选演练

2026-10-02后续执行已完成远端CI/密码学签名、最终ARM64目标及410回归、一致生产副本迁移和旧完整状态恢复启动；[本轮归集](../cpar-signed-state-rehearsal-20261002/issue-57-evidence.md)是最新部署就绪证据，BLOCKED于升级前就有的归档凭据AEAD失败。本文件前文和JSON保留原交付时点NOT_RUN事实，未关C/#57/#9。
