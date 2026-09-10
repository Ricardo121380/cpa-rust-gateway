# Prism V4 实际操作验收（2026-09-10）

**结论：部分通过，不能判定完整操作验收通过。** 本次按用户要求代操作正式应用，
发现此前自动门禁未覆盖的运行矩阵实现缺口和失败深链缺口。此前交付报告的
“全部必需功能完成”结论过早，应以本报告的未解决项为准。

环境：当前正式gateway二进制，Chromium 151.0.7922.34，1440×900；本地合成状态和TLS
loopback mock Provider。未使用生产数据/秘密，未调用真实Provider，未进行远端操作。
模型ID为实际本地投影返回的 `local-exact-model`，不是对外部商业模型可用性的认证。

## 本次结果

| 操作 | 实际结果 |
|---|---|
| 解锁/版本选择 | 成功；使用临时management-key/csrf，未输出秘密 |
| 账号搜索→详情→权益 | 搜索写入URL；返回保留搜索。真实账号显示0/4并发、认证/调度可用，权益明确未观测，不捏造套餐 |
| 账号→失败 | 跳转携带provider/channel/account筛选；修正预览数据后可读取真实ProviderTransient失败 |
| 有效模型→来源→Explain | Access Group local-group返回exact模型；来源含真实route/candidate/endpoint；深链自动填入route/model，Explain选中1、排除0 |
| 模型→草稿→候选维护 | 独立新状态中的真实浏览器执行创建/修改权重3/重读/删除/重建，通过 |
| 授权→校验→发布→审计 | 真实管理listener持久写入；取消发布无效应，确认发布成功；新候选重读及增改删审计存在 |
| 版本差异与回滚 | 基线切换、同版本空差异、手机12px边距、取消回滚、确认恢复原版本通过；旧生命周期确认被拒绝 |
| 真实请求→账本→attempt | 1条账本，输入10、输出3、已知成本16 microunits、partial；点击请求能看到succeeded及endpoint/credential |
| 失败归因 | 真实ProviderTransient/provider/retry_eligible一条；不是用账本倒推失败率 |
| 用量 | 1个产生用量的请求、输入10/输出3精确，其他4类token为未知而不是0 |
| 会话拒绝 | 独立真实浏览器流程确认锁定、清空秘密、停止读取，无秘密持久化 |
| 运行矩阵/失败到诊断 | **未通过，见下表** |

## 发现与处理

| 编号 | 级别 / 状态 | 复现及原因 | 建议验收条件 |
|---|---|---|---|
| UAT-01 | P1，未修复 | 有效绑定local-endpoint/local-credential存在，账号池正常、Explain选中local-candidate；但GET /admin/runtime/availability返回200 `[]`，页面声称无绑定。`apps/gateway/src/runtime.rs:7454`的runtime_availability只校验snapshot后返回Vec::new() | 接入真实可用性投影，区分健康/冷却/quota/禁用等现有状态；真实进程验证非空有效绑定及状态变化，不以fixture替代 |
| UAT-02 | P2，未修复 | 失败表的request/attempt/provider/channel/account是纯文本，无法直接进入request尝试或对应target/Explain。`web/prism/src/features/monitoring/MonitoringPage.tsx:509` | 增加有真实身份上下文的深链/详情，返回保留筛选；真实失败行点击验收 |
| UAT-03 | P2，已修复并复验 | preview启动逻辑被重复插到目录硬过期分支后，在生成受控请求前提前保持运行。故旧预览有模型但账本/失败为空，不是数据丢失或时间窗问题 | 已删除提前preview块，保留唯一的末尾保持运行；priced+inactive+preview先完成全断言/写evidence，再输出URL；独立catalog-expiry也通过 |
| UAT-04 | P3，未修改 | 请求详情将“裸数组、游标、闭集”等实现说明直接作为主要文案；虽不影响读取，但不利于日常管理 | 保留必要数据范围提示，把接口说明移入开发文档或折叠说明 |

本次代码只修正本任务预览脚本的启动顺序。其余功能缺口如实报告，没有借验收自动重构产品。
UAT-01/02属于原V4范围，不能自动移到请求趋势/延迟分析等下一期事项中。

## 证据与复现

- 本次真实写流程：`/tmp/prism-v4-user-acceptance.log`；
  `/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-yxgnw475/evidence.json`，
  同目录browser-flow/flow.json记录8阶段通过及截图。
- 修复后带运营数据的持续预览：
  `/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-ogdd9sjg/evidence.json`。
- 目录硬过期提前返回回归：
  `/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-zcm_da7z/evidence.json`。
- 人工逐步CLI操作截图：`output/playwright/prism-user-acceptance/`，包含request-attempt.png、
  failure-list.png、runtime-explain.png；页面控制台0错误/0警告。
- Playwright CLI曾因默认查找另一版本浏览器而启动失败，改用已安装Chromium1234路径后完成；
  此为工具启动问题，不计作应用错误。未安装新浏览器/升级项目依赖。

实际命令：`python3 scripts/prism-v4-local-acceptance.py --priced --browser-flow`、
`--catalog-expiry`、`--priced --inactive --preview`，以及Playwright CLI逐步操作。
未将历史251/127/1186测试数量重复计成本次运行结果。脚本只删11行重复分支，
通过上述实际执行及git diff/doc link检查，不额外重复全仓测试。

当前新预览：`http://127.0.0.1:57444/admin-ui/#/`。凭据目录为上述ogdd9sjg下的credentials。
独立后台父进程PID保存在 `/tmp/prism-v4-uat-preview.pid`，用SIGINT停止该父进程会执行
清理并停止它拥有的gateway与mock Provider；此处无生产进程。旧56839地址不再作为交付地址。
