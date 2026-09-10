# Prism V4 本地交付报告

日期：2026-09-10。正式仓库 `cpa-rust-gateway`，分支 `codex/prism-v4-delivery`。

**更正（2026-09-10用户操作验收）：尚不能判定全部完成。**
后续实际操作发现运行矩阵返回占位空数组、失败到诊断深链缺失，属于原V4范围的未完成项。
预览脚本提前保持运行导致运营数据为空的问题已修复。详见 [实际操作验收](prism-v4-user-acceptance.md)。
下述已通过门禁/链路证据保留，但不覆盖这两项功能缺口。

## 交付范围与设计

正式 React/Vite/TanStack Query/Zustand 应用采用用户确认的 V4 银白/石墨灰、Apple 蓝、
低饱和背景、三面 chrome 玻璃、实底数据内容。保留 GlassSurface、PrismLens、同源管理客户端、
CSP 和内存秘密。右侧对象详情约520px，表单及确认居中，手机两侧12px。

视觉基准是 [V4](../design/prism-liquid-glass-v4.html)，新增有效模型与候选流程由本会话设计，
通过既有 OpenDesign MCP 项目 `prism-gateway-console-redesign-a735` 保存和检查；
见 [补充设计](../handoffs/prism-v4-resource-flows.md)。没有调用内置生成器或外部模型CLI。

## 15个入口逐项核对

以下路径均为正式 `web/prism/src/features/` 中的实现，非独立HTML原型。
“全页截图”指真实gateway嵌入应用的84个页面视图（14×3尺寸×2主题），另有6张解锁视图。
E2E文件位于 `web/prism/e2e/`；它们覆盖可控错误/长内容等分支，不能替代真实网关证据。

| 栏目 / 路由 | 实现与保留能力 | 验收证据 |
|---|---|---|
| 总览 `#/` | overview：累计attempts、观测范围、待处理事项、计费处理状态和异常链接 | 全页截图；billing-processing、observability、contrast E2E；真实事件/状态HTTP |
| 请求与失败 `#/monitoring` | monitoring：失败/账本独立筛选分页，Request/attempt/target导航，安全JSONL导出 | monitoring、v4-workspaces E2E；真实失败/账本；大样本窄窗 |
| 用量分析 `#/usage` | usage：现有分组与时间窗，六类token独立置信度、缺失和snapshot | usage E2E、model单测；真实请求用量和四组大样本 |
| 计费与价格 `#/billing` | billing：全局价格目录、版本策略、导入/回退、真实账本及处理状态 | billing、billing-processing E2E；真实16 microunits partial及unpriced、重启幂等 |
| 账号池 `#/accounts` | accounts：过滤、分页、权益/目录证据、绑定/失败/诊断；认证/调度/quota独立 | v4-workspaces、account-actions、provider-pools E2E；全页截图 |
| 上游 `#/upstreams` | upstreams：Upstream/Endpoint/Credential/Binding既有CRUD，安全秘密输入和账号关联 | subresource-crud、credential、provider-pools E2E；真实合成资源创建/重启 |
| 模型目录 `#/catalog` | catalog：target证据、新鲜度；Access Group或Key ID授权投影；exact ID到草稿接续 | effective-models E2E；真实模型→草稿；真实硬过期 |
| 模型与路由 `#/models` | models：Public Model/Route、完整Route/Candidate/Alias分页、候选增改删/重读 | route-candidates、subresource-crud E2E；真实候选CRUD、发布和资源审计 |
| 访问控制 `#/access` | access：组/路由授权、Key签发/启停/撤销、reveal-once | access-groups、flows E2E；真实grant及禁用/撤销身份请求拒绝 |
| 运行诊断 `#/runtime` | runtime：target矩阵、Explain、既有Channel Pin及深链 | account-actions、batch-d、v4-workspaces E2E；真实页读取；Rust Explain回归 |
| 出口策略 `#/egress` | egress：配置策略、兼容代理、Provider出口分域 | compatible-proxy、provider-egress E2E；真实loopback策略及非活动资源装配 |
| 配置版本 `#/versions` | config-versions：草稿/校验、16类配置差异、发布/回滚确认、revision和活动生命周期保护 | configuration-diff、flows E2E；真实差异分页/409、取消/发布/回滚、ABA旧确认拒绝 |
| 审计与备份 `#/audit` | audit：生命周期和资源修改审计、详情、备份预检及原受支持操作 | object-inspectors、batch-d E2E；真实候选修改审计分页与详情焦点恢复 |
| 设置 `#/settings` | settings：浅深色、辅助偏好、既有语言机制、搜索和锁定 | settings、i18n、contrast E2E；真实移动偏好与锁定 |
| 解锁 `#/unlock` | unlock：秘密规范化、真实鉴权、失效清理与错误 | session-ownership E2E、normalizeSecret单测；真实access_denied后清除秘密/停止读取 |

旧 `#/` 和原有query/Runtime定位保留。实际已有功能清单和有意未接操作见
[进度记录](prism-v4-progress.md)的“受保护的旧功能基线”。无新增趋势、延迟或请求成功率假指标。

## 后端能力与正确性

| 项目 | 实现 | 定向及真实证据 |
|---|---|---|
| M0 | 会话/选择generation、响应体与错误所有权、BigInt revision单调、切换取消和缓存清理；同步3个变化schema | client.ownership单测覆盖A→B→A、同版本乱序、body晚到、旧错误、普通404、无409写重放；session-ownership E2E；真实鉴权清理 |
| BE-FE-01 | 同一scheduler serving snapshot的有效模型授权/provenance投影；只接组/Key ID；hash绑定分页 | 权限/版本/分页回归，真实HTTP模型及浏览器handoff；目录expiry后立即不可新选，无浏览器Client Key secret |
| BE-FE-02 | 版本一致有界完整Route/Candidate/Alias枚举；候选PATCH/DELETE草稿+If-Match原子revision/审计 | store/control/HTTP回归；真实草稿候选增改删、重读/发布/审计；旧cursor409停页 |
| B1 | `apps/gateway/src/billing_worker.rs` 有界可停止物化；坏记录持久隔离/重试，checkpoint和安全处理状态 | 真实持久事件→priced/unpriced账本、重启不重复；坏记录修复及幂等Rust回归；不阻塞请求等待物化 |
| B2 | SQL下推filter/snapshot/cursor，账本整行批读，usage流式关联及完整聚合；HTTP四槽blocking边界 | 99,999/100,000/100,001/100,005样本真实HTTP窄窗，账本完整摘要；snapshot/游标/六类置信度Rust回归 |
| B3 | 快照携带目录deadline，新选择和租约按当前时间拒绝硬过期；已有持久目录在无自动discovery时仍装配 | 真实刷新失败+合法6/24/72h历史窗口；越过硬过期拒绝新Provider调用，既有在途请求仍200 |
| B4 | `apps/gateway/src/maintenance_worker.rs` 单owner、每表256/批、独立连接、可停止；只维护既有Stored Response/compaction TTL | 存储有效continuation/到期/分页回归，文件worker停止测试；真实到期删除/未到期保留与重启 |
| 运行补齐 | Endpoint能力表、模型能力、路由参数契约、有效snapshot接线；非活动资源保留但不创建transport/native pool或占活动容量 | 真实新建配置发布后serve；禁用模型/上游/endpoint/credential/binding/原生渠道共存；禁用/撤销Key及禁用组均未调用Provider |
| 差异与确认 | 16类资源独立只读SQL比较，双revision游标；不投影秘密值；活动ID+生命周期事件防ABA | 存储并发snapshot、208条分页、秘密不投影；真实HTTP旧游标409，浏览器确认取消/回滚链路 |

新增接口均先更新权威 `docs/openapi/management-v1.json`，再用sync-contract生成，共108个operation。
契约说明见 [CR-PRISM-V4-001](../change-requests/CR-PRISM-V4-001.md)。未手改generated或以fixture API替代实现。

## 真实验收证据索引

证据根均是本次创建的本机临时目录，不是生产状态；目录可能被系统后续清理。脚本及断言已提交，可复现。
公共前缀：`/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/`。

| 目录（前缀之后） | 内容 |
|---|---|
| `prism-v4-acceptance-ogdd9sjg/` | 修复后的priced+inactive+preview；evidence.json、preview.json；真实Provider成功/失败、账本、TTL、幂等和不可用身份隔离。此前xxmtlm0a预览提前停留，不能作为这些断言的证据 |
| `prism-v4-acceptance-uxkgt8rn/` | 最终构建：84页+6解锁及8阶段真实浏览器写操作；browser/audit.json、browser-flow/flow.json，桌面/手机差异截图 |
| `prism-v4-acceptance-_52jcm8u/` | browser/audit.json：84页+6解锁及资源审计详情；三尺寸浅深色，无文档横向溢出/页面JS错误 |
| `prism-v4-acceptance-0wa0qlr6/` | 四个大型历史样本的窄窗与完整摘要验证 |
| `prism-v4-acceptance-v_s_i3a3/` | 真实目录硬过期、新准入拒绝与旧租约成功 |
| `prism-v4-acceptance-23hu06iq/` | 禁用资源运行装配回归，包括禁用原生渠道和兼容出口 |

浏览器实际为 **Chromium 151.0.7922.34，macOS**。覆盖1440×900、1280×720、390×844，
深色、减少动态/透明度、增强对比、键盘/Escape/焦点恢复、长内容、空/错误态。
SVG lens正常路径及三种不支持探测分支已有E2E；后者在Chromium中模拟能力探测，
验证回到blur及高对比度关闭滤镜，**不宣称实际测试过Safari/Firefox**。

## 最终验证结果

最终命令结果（本轮重新执行）：

- `/tmp/prism-v4-delivery-unit.log`：251项单测通过。
- `/tmp/prism-v4-delivery-e2e-rerun.log`：127项E2E通过（125 Chromium + 2 narrow-390）。
- `/tmp/prism-v4-delivery-gates.log`、`output/prism-v4-delivery-gates.md`：check.sh fast整体通过；完整Rust为1186通过、9个既有ignored（115个结果组），全目标全feature Clippy通过。
- `/tmp/prism-v4-fallback.log`：6项玻璃测试通过；本批type-check通过。

第一次E2E与check.sh中的npm ci重叠，Playwright worker文件暂缺导致启动失败；这是执行安排错误，
保留 `/tmp/prism-v4-delivery-e2e.log`，不计为通过。依赖重建后完整重跑。
此前门禁发现的测试/Clippy/serve夹具问题和修复均记录在progress，没有弱化门禁。
完整门禁逐步结果已留档：[本次检查记录](prism-v4-checks.md)。

fast包括权威契约/generated/CSP/四文件双构建、Rust format、Clippy、全workspace测试、
serve envelope、源码/边界/文档/secret/whitespace及仓库离线脚本。没有依赖更新，未另跑
full模式额外的在线依赖审计。前端type-check及build由SPA门禁执行通过。

四文件未压缩体积；基线从参考提交只读导出，在临时目录用同一工具链构建，没有回滚工作树：

| 文件 | 基线 bytes | 当前 bytes | 增量 |
|---|---:|---:|---:|
| index.html | 693 | 874 | +181 |
| assets/main.js | 457449 | 514050 | +56601 |
| assets/vendor.js | 141007 | 141061 | +54 |
| assets/index.css | 64783 | 67188 | +2405 |

新增管理能力主要进入main.js；没有新增chunk，vendor微小差异未伴随依赖清单升级。

## 本地查看与重现

当前预览：`http://127.0.0.1:57444/admin-ui/#/`。这是本次真实gateway，Provider仅为loopback mock。
新预览以独立后台进程保持；旧终端预览已停止。解锁采用临时合成凭据：

`/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/prism-v4-acceptance-ogdd9sjg/credentials/`

在本机分别将 `management-key` 与 `management-csrf` 文件内容粘贴进对应输入，不需要真实生产凭据。
可用 `pbcopy < 文件完整路径`，避免在终端打印。秘密不写入报告、URL、截图或浏览器持久存储。

可复现启动（项目根目录）：

```sh
npm --prefix web/prism run build
cargo build -p gateway --bin gateway
python3 scripts/prism-v4-local-acceptance.py --priced --inactive --preview
```

脚本输出新随机loopback URL和credentials目录，验收后保持运行；Ctrl-C停止gateway及Provider，
保留临时合成状态用于审查。默认不带preview时验收后自动停止。要重跑专项可分别使用
`--priced --browser-flow`、`--priced --browser`、`--priced --large`、`--catalog-expiry`。
不要把已停止的历史验收URL当作持续实例。发布后数据面需重启serve使用新配置，确认页明确说明。

## 可审查提交与边界

参考基线 `1ba4340b56e9f4214c98912f78dc477fdc817ea2`，全部功能按本地批次提交，未推送。
重点：`190b57c` M0；`986e9a7`/`b149ec6`/`9366493` V4页面；`e0ff96a` OpenDesign补充；
`14d7371`/`43afecb`候选维护；`57101fe`/`45d5b51`完整枚举；`5f5025a`资源审计；
`39b932f`发布/回滚确认；`eb4be56`/`de22dcc`/`d0867bc`配置差异；
`b58b4df`禁用资源装配；`f31680b`回退及持续预览。完整提交和跨边界记录由git log及
[跨边界日志](../cross-boundary-log.md)审查，未覆盖或提交用户无关未跟踪文件。

明确保留的范围：旧round_robin/priority_failover配置可完整枚举及检查，既有写契约和活动运行
仍限制为受支持策略；不新增未承诺调度算法。active group limits及运行容量仍受既有runtime约束。
Usage沿用现有成本DTO口径，真实金额在账本中展示；partial不升级exact、unpriced不当零费用。
缺失目录/权益为unknown/null，目录数量不等于授权数量。

下一期：请求趋势、延迟/P95、RPM/TPM、窗口请求成功率；全量英文、Autoreg控制面、全部渠道
discovery扩展、在线恢复、秘密导出、未支持媒体协议和真实Provider自动巡检不在本轮。
未执行生产部署、SSH、DNS/Caddy/流量修改、远端状态变更或生产数据清理。
