# Prism V4 实施 Goal Prompt

请创建并持续执行一个 Goal：在 `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway` 中，将已确认的 V4 Apple Liquid Glass 设计落实到正式 Prism 前端，完成全部 14 个管理栏目与解锁页、有效模型目录、完整路由/候选维护、真实计费链路和必要后端修复，通过本地真实网关验收并交付可审查的代码、测试与报告。若当前已经在执行这个 Goal，直接继续剩余工作，不重复创建。本任务未指定 token 预算。

## 用户已确认的决策

本轮由当前会话的 Codex 统一负责前后端，包括 `web/prism/**`。V4 视觉方向、完整栏目范围和后端补齐均已确认；按里程碑持续实施，不在日常实现选择或每个里程碑结束时重复索要许可。

本轮终点是本地真实 gateway 联调与验收通过。请求趋势、延迟分析和新增请求统计放到下一期；生产部署、远端状态变更和真实 Provider 调用不包含在本 Goal 中。

## 开始前读取

1. `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/AGENTS.md`、`/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/CLAUDE.md` 及 `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/cross-boundary-log.md` 最新记录。
2. `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/handoffs/prism-v4-execution-plan.md`：本轮正式范围、M0–M4、栏目清单与验收标准。
3. `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/design/prism-liquid-glass-v4.html` 及 `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/handoffs/prism-opendesign-v4.md`：用户确认的视觉与交互基准。
4. `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/handoffs/prism-opendesign-v3.md`：完整栏目与原组件映射；其中的设计样本不是生产逻辑。
5. `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/reports/project-review-2026-09-09.md` 及 `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/handoffs/prism-development-plan-2026-09-09.md`：问题证据与前期依据。
6. `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/web/prism/DESIGN.md`、`/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/08-management-frontend-development-plan.md`、`/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/handoffs/claude-code-oracle-singapore-vps.md`：既有交付约束和服务边界。读取环境交接不表示本轮需要访问远端。

先检查实际 HEAD、工作树与接口。参考基线是 `1ba4340b56e9f4214c98912f78dc477fdc817ea2`，不要回滚当前工作树。保留用户已有变更和无关未跟踪文件，尤其设计/计划产物；不要使用外层旧前端仓库。当前用户指令决定任务范围；本轮实施分工和范围以正式执行计划为准，视觉以 V4 为准，接口事实以当前代码及权威 OpenAPI 为准。

## 按 M0–M4 实施

**M0：先修正确性。** 同步权威契约，接入三个变化 schema 对应的 DTO/fixtures；修复旧版本/旧 session 晚到响应污染、revision 倒退、会话失效未锁定和缓存/轮询未清理。增加针对性回归，409 不自动重放写请求。记录已有路由/子视图/功能，保护现有能力。

**M1：落实全部 V4 页面。** 在现有 React/Vite/TanStack Query/Zustand 结构上合并 token、导航、实底数据面板、右侧对象详情及居中表单/确认。保留 `GlassSurface`、`PrismLens` 和三面 chrome 玻璃预算。完整覆盖：总览、请求与失败、用量分析、计费与价格、账号池、上游、模型目录、模型与路由、访问控制、运行诊断、出口策略、配置版本、审计与备份、设置，以及解锁页。总览保留 `#/`，旧链接与 query 继续可用。

**M2：补齐资源管理闭环。** 实现 BE-FE-01 管理鉴权下的有效模型/provenance 投影，复用数据面的授权与 serving snapshot；支持既有 Access Group 或 Key ID 上下文，不让浏览器提交 Client Key secret。完成 BE-FE-02：版本一致、有界的 Route/Candidate/Alias 完整枚举及候选修改/删除，涵盖未绑定草稿，复用已有 Route CRUD。实现对应前端交互、账号权益/目录证据和失败到诊断的深链。修复 B3 目录硬过期的新选择/租约准入，保留在途快照语义。

**M3：接通计费与运营。** 完成 B1：将现有物化能力接入 `serve` 的有界、可停止 worker，验证 checkpoint、重放幂等、坏记录处理和 unpriced；提供最小安全处理状态。完成 B2：修复全局 100,000 条上限导致窄窗也失败的读取路径，将筛选/snapshot/cursor 下推到存储，账本整行批读，保持聚合完整性和非阻塞边界。完成 B4 的限定范围：接入已定义 TTL 的有界维护，不扩大历史删除策略。把这些能力接入账本、用量、失败和总览。

**M4：完成实际验收与交付。** 根据正式计划逐页核对视觉和功能，完成关键 E2E、相关 Rust 回归、权威契约/嵌入门禁和本地真实 gateway 联调。不能只用模拟后端或独立 HTML 证明正式应用完成。

## 设计、契约和数据约束

- V4 的银白/石墨灰、Apple 蓝、低饱和环境、玻璃导航、组合指标和实底数据面板保持统一。桌面对象详情约 520px，手机左右各留 12px；保留浅/深色、辅助偏好、键盘和焦点行为。不要把独立原型内联脚本和样式直接复制进生产入口。
- 新增模型/候选等细节需要设计时，继续在 OpenDesign MCP 的既有项目 `prism-gateway-console-redesign-a735` 中补充，由本会话模型设计，通过 MCP 保存与检查；不调用 OpenDesign 内置生成器、外部模型 CLI 或自行切换设计风格。
- 为新增能力补充契约变更说明/CR，由本轮 Codex 完成后端定稿与实现，再运行 `npm --prefix web/prism run sync-contract` 并接前端；不手改 `web/prism/contracts/management-v1.json` 或 `web/prism/src/generated/`，不造只在 fixture 中成立的 API。已纳入本轮的接口缺失属于待实施工作，不作为等待另一位实现者的外部阻碍。
- 保留同源 `/admin/*`、统一客户端、CSP、会话内秘密和四文件构建：`index.html`、`assets/main.js`、`assets/vendor.js`、`assets/index.css`。不新增无人服务的 chunk、浏览器秘密持久化或跨 listener 绕行。
- 认证、调度、quota、权益和目录分别表达；null 保留未观测。模型使用真实 exact ID，套餐不生成模型白名单。目录模型数不冒充授权可调用数，累计 attempts 不冒充请求成功率，空账本不等于零消费，六类 token 保留各自置信度。
- 前后端由 Codex 统一执行是本轮授权；仍按仓库要求在实际跨边界提交内记录 `docs/cross-boundary-log.md` 与 `Cross-Boundary` trailer。不要永久改写通用分工规则，按合理功能批次保留本地提交，不提交无关文件。

## 验收与停止条件

关键路径必须包括：会话失效清理；跨版本/同版本乱序；账号 → 权益/目录/失败 → target/Explain；授权模型选择 → 草稿路由/候选编辑 → 校验/发布 → 重读与审计；本地受控请求 → 持久事件 → 计费物化 → 账本；大样本窄窗查询；目录硬过期；既有 TTL 维护。

检查实际应用在 1440×900、1280×720、390×844 下的全部入口与主要页面，并覆盖深色、辅助偏好、长内容、空态/错误态、键盘与对比度。复用现有回退机制，记录实际验证过的浏览器。

运行适用的前端类型、单测、E2E、构建与 `node scripts/check-management-spa.mjs`，完成相关 Rust/契约/存储/运行装配/边界检查及最终适用的仓库门禁。每批按改动风险验证；没有新的变化或失败时不反复扩大测试。使用合成配置、临时状态目录及 loopback mock Provider 验证真实 gateway，禁止拿历史测试数量当本次通过证据。

维护清楚的 M0–M4 进度与未完成项，里程碑完成后汇报结果并继续。遇到真实阻碍时继续不依赖它的工作，记录原因与最小缺失输入；不凭等待时长推定授权，不把本轮已承诺的后端功能自动移到下一期。

最终生成 `/Users/huangrui/Documents/CPA-Rust/cpa-rust-gateway/docs/reports/prism-v4-delivery.md`，逐项列出 14 个栏目＋解锁页、BE-FE-01、BE-FE-02、B1–B4 限定范围的实现与测试证据，提供提交、真实本地应用查看方式及明确的下一期事项。仅在全部必需功能与验收完成后，将 Goal 标为完成。

全量英文、Autoreg 控制面、所有渠道 discovery 扩展、在线备份恢复、秘密导出、未支持媒体协议、请求趋势/延迟分析及真实 Provider 自动巡检不在本轮。不要执行生产部署、SSH 运维、DNS/Caddy/流量变更或生产数据清理；这些不作为当前 Goal 的完成条件。
