# 08 · 管理前端开发计划 — Prism(合并后)

| 项目 | 值 |
|---|---|
| 状态 | `v0.3 — 前端已并入本仓 web/prism;与 docs/06 的 P13 共用一条路线` |
| 日期 | 2026-08-11 |
| 取代 | v0.1(2026-07-26,独立仓库时期)。旧版整篇描述的是一套未落地的选型与阶段划分,差异见 §8;原文在 git 历史中 |
| 位置 | `web/prism`,由 `cargo build` 构建并嵌入 |
| 协作边界 | [AGENTS.md](../AGENTS.md) / [CLAUDE.md](../CLAUDE.md);越界留痕见 [cross-boundary-log](cross-boundary-log.md) |
| 设计侧 | [07 · 设计文档](07-management-frontend-design.md);实现决策与踩坑记录见 `web/prism/DESIGN.md` |

---

## 1. 实际技术栈(以 `web/prism/package.json` 为准)

合并时把 v0.1 的选型表与仓库现状逐行核对,**四项纸面选型从未落地**。记录在此,
避免后来者把它们误读为"还没做"。

运行时依赖只有:`react` `react-dom` `react-router-dom` `@tanstack/react-query` `zustand`。

| 决策 | v0.1 计划 | 实际 | 说明 |
|---|---|---|---|
| 框架 / 构建 / 路由 / 服务器态 / 客户端态 | React 19 + TS strict、Vite 固定文件名、HashRouter、TanStack Query v5、Zustand 仅内存 | ✅ 全部落地 | 产物恰好四个文件与嵌入路由一一对应(C3);C6 禁一切浏览器存储 |
| **图表** | ECharts 显式注册 | ❌ **未引入** | 微图表族全部自绘 SVG(StatTile / SparkLine / HealthStrip / TokenMixBar / MiniTimeline / LineChart / MultiLineChart / RankTable / ZoomBrush)。ECharts 是为 dataZoom 与热力图准备的,而**服务端时间桶至今不存在**(见 §3)。在那之前引入是空转 |
| **表单** | react-hook-form + zod | ❌ **未引入** | 原生 `FormData` + 纯模型解析器(`parseLimits` / `parseOAuthCallback` / `parseLimits` 等),每个都有单测。zod 的价值是镜像后端校验,但契约规则分散在 maxLength/enum/minimum,手写解析器同样能表达且零运行时体积 |
| **样式** | CSS Modules(SCSS) | ❌ **未引入** | 单一 `app.css` + design token 三层双主题。玻璃材质跨组件共享大量变量,模块作用域反成阻力 |
| **API 层** | 升级 `generate-management-client.mjs` 产出 zod + typed hooks | ❌ **该生成器已删除** | prism 自带 `scripts/generate-client.mjs`,只产出 fetch 包装;类型在调用点由 `call<T>()` 显式标注 |
| i18n | zh/en 双语 | ⚠️ **部分** | 框架就位、英文包完整性由类型强制,但**页面正文全是硬编码中文**(`zh.ts` 仅 96 行)。切英文后大部分界面仍是中文 —— 已知缺口,非已完成项 |
| 测试 | Vitest + Playwright + axe | ⚠️ **部分** | 157 单测 + 44 E2E;**axe-core 未引入**,a11y 靠对比度实测与人工审查 |

**优化结论:** 这四项不再作为待办占位。要引入必须由具体需求触发(时间桶数据到位 / 校验重复到痛),
不由计划表推动。

## 2. 唯一耦合:契约

`docs/openapi/management-v1.json` 后端拥有,前端单向跟随:

```bash
npm --prefix web/prism run sync-contract   # 拷贝 + 重新生成客户端
npm --prefix web/prism run check           # 失步则机械失败
```

**前端不得手改契约,也不得手改生成物。** 需要契约没有的形状时,写 `docs/change-requests/`
并在 cross-boundary-log 记一条,由后端决定。

历史教训:独立仓时期曾**失步 14 天**才发现契约已变。并仓后 `sync-contract` 读仓内路径,
失步窗口从"没人通知"缩短为"没跑命令"。

## 3. 路线图:前端项挂在哪个 P13 任务下

前端不再有独立阶段编号(FE-0…FE-5 已废除 —— 它们是独立仓时期的产物,与 P13 编号打架)。
**每一项要么现在能做,要么明确挂在某个后端任务下。**

### 现在能做(无后端依赖)

| 项 | 内容 | 为什么要紧 |
|---|---|---|
| 子资源 CRUD | 端点 / 凭据 / 绑定的建改删(7 个算子) | 枚举已由 P13-04A 解锁,面板目前只读 |
| 路由候选与校验 | `createRouteCandidate` + `validateRoute` | **没有候选的路由永远验不过**。面板现在能把配置改成验不过的状态,却修不回来 |
| Client Key 编辑 | `updateClientKey` | 现在只有签发与吊销两个极端,没有停用/改过期 |
| 质量门收尾 | 390 窄屏 E2E;`--ink-3` 门禁跨块继承漏洞 | 各半天 |
| i18n 页面正文 | 3–4 天 | 取决于是否真要英文界面 |

### 挂在后端任务下

| 前端项 | 依赖 | 状态 |
|---|---|---|
| 运行时页真实数据 · 503 文案拆分 | **P13-06** | 可用性矩阵与目录新鲜度目前返回空数组(`Vec::new()`);前端已能区分 503 的两种含义,但要等有数据才值得做 |
| 用量分析页改造 | P13-05 ✅ 已交付 | 形状已定:`operations/usage`。**注意仍无服务端时间桶**,图表需客户端聚合 |
| 计费与价格目录页 | P13-05 ✅ 已交付 | 目录导入/列出/回滚 + 每请求成本。G9 的控制面 |
| 请求监控真实列表 | P13-05 ✅ 已交付 | `operations/billing` 带 `request_id`,配合 `listRequestAttempts` 可做下钻 |
| 家族配额页 | P13-06 | 尚未开工 |
| 能力自描述 | — | **G7 不在 P13 清单内,应视为可能永不到来**;各页继续靠 503 反推 |

### 已明确不做

- **恢复流程**(`previewRestore` / `restoreBackup`):只能恢复到空库,活面板永远不满足前置条件,以文档指引替代;
- **`exportCredential`**:明文凭据出浏览器,顶在"秘密零浏览器存储"硬约束上,需单独批准。

## 4. 构建与嵌入

```
cargo build
  └─ crates/gateway-http-actix/build.rs
       ├─ scripts/build-management-spa.sh → npm --prefix web/prism run build
       ├─ 断言四个产物存在
       └─ include_bytes! 嵌入
```

嵌入路由固定四条:`/admin-ui/`、`assets/main.js`、`assets/vendor.js`、`assets/index.css`,
未知路径 404。

**必须从管理监听器提供面板。** 网关把唯一允许的浏览器 origin 推导为该监听器自身地址
(`apps/gateway/src/deployment.rs::management_origin`),任何其他来源的写操作一律
`404 management_access_denied`,而 GET 照常成功 —— 极易误判为前端 bug。

## 5. 流程:两个工具一个仓

| | 后端 | 前端 |
|---|---|---|
| 工具 | Codex | Claude Code |
| 拥有 | `web/prism` 以外全部 | `web/prism/**` |
| 分支 | `codex/*` | `claude/*` |
| 门禁 | cargo fmt / clippy / test、`scripts/check.sh`、Delivery Gate | `npm run check:full`、vitest、playwright |

**关键联锁:`cargo build` 会构建前端。前端坏了,后端也构建不了。**
前端提交前必须跑通 `npm --prefix web/prism run check` 与 `npm run build`。

越界改动(改到对方拥有的路径)必须在同一提交追加 cross-boundary-log 条目并加
`Cross-Boundary:` trailer;开工前先读日志尾部,标 **action required** 的是别人改到你这边的东西。

## 6. 完成定义(前端项)

1. 类型检查干净;纯模型分支有单测;用户可见路径有 E2E;
2. `check.mjs` 全绿(含双构建字节一致);
3. **对真网关验证过** —— fixture 不算数,且必须从 `/admin-ui/` 打开(见 §4 的 origin 约束);
4. 涉及后端文件的改动已记入 cross-boundary-log;
5. 设计决策与踩到的坑记入 `web/prism/DESIGN.md`。

## 7. 风险

| 风险 | 现状 |
|---|---|
| 契约失步 | 已机械化;残余风险是"没跑命令" |
| 前端拖垮后端构建 | 真实存在(§5 联锁)。缓解:前端门禁比后端快得多,提交前跑一次成本极低 |
| 两个工具互相覆盖 | 靠 cross-boundary-log,**完全依赖遵守,没有技术强制** |
| 为不存在的数据建 UI | **已发生过**:用量页按提案的 G3 形状建成,后端最终实现了不同形状(`operations/usage`)。对策就是 §3 那张表 —— 不给未定形状排期 |

## 8. 与 v0.1 的差异(供审阅)

| 变化 | 原因 |
|---|---|
| 删除 FE-0…FE-5 阶段编号 | 独立仓产物;真实约束是"依赖哪个 P13 任务" |
| 技术选型表改为"计划 vs 实际" | 四项从未落地,挂着会被误读为待办 |
| 删除「与后端 CR 的接口」一节 | 跨仓协议已由 cross-boundary-log + 仓内契约取代 |
| 删除全部 `generate-management-client.mjs` / `web/admin-ui` 内容 | 二者已随合并删除 |
| 删除按提案 G1/G3 形状写的组件规格(RefSelect 依赖全图、§4.4 观测数据规格) | 后端实现了不同形状;照旧规格建会第二次白干 |
| 新增 §5 流程与 §6 完成定义 | 合并前两侧各有一套,现在必须是一套 |

## 9. 与 docs/06 的关系

`docs/06-development-plan.md` 是后端锁定主计划(P13 任务表、Delivery Gate、证据链)。
本文档**不复制**其内容,只在 §3 声明前端项挂在哪个 P13 任务下。
P13 任务状态以 docs/06 为准,**前端不修改其任务表**。
