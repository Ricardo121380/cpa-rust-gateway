# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

单一自托管运营者(项目所有者本人):技术背景强,中文为主,通过 loopback 或私网访问自己部署的 Rust AI 网关。使用情境两类:日常巡检(多次/天,扫健康与用量)与深度运维(诊断失败、发布配置变更、管理凭据)。无多用户、无角色体系(确认:H22 = Later)。

## Product Purpose

Prism 是 cpa-rust-gateway 的管理与观测面板:让运营者在一个界面里完成 (1) 版本化配置管理(上游/端点/凭据/模型/路由/访问控制,草稿→验证→原子发布→一步回滚)与 (2) 持久化可观测(请求历史、用量分析、配额与凭据健康、失败诊断)。成功 = 两站聚合可纯 UI 配置发布(G10 验收),且面板数字与 SQLite 事件日志直查一致。

## Positioning

与参照产品 CPAMP(seakee/CPA-Manager-Plus)不同,本网关把事件持久化做进了本体 —— Prism 因此是**零 sidecar 的双平面单面板**:无第二服务、无第二端口、无第二把钥匙。配置模型是版本化图 + 乐观并发(ETag/If-Match),不是文件编辑;观测信号是 value-free 闭集枚举(17 错误码 × 10 域 × 8 阶段),不是自由文本日志。

## Operating Context

- 面板经编译期 `include_bytes!` 嵌入网关二进制,由管理监听器同源服务(loopback/私网 + CSRF);
- 运营者以 `mgmt_` 前缀 Management Key 解锁;认证失败一律 404 不可探测;
- 所有变更以 Config Version 为作用域,仅 draft 可变;每个变更携带 `X-Config-Version` + `If-Match: rev-N`;
- 参照工作流(来自 CPAMP 生态验证):巡检看板 → 过滤下钻 → 逐请求尝试时间线 → 路由解释(Route Explain)→ 凭据处置。

## Capabilities and Constraints

- 管理契约 41 操作已实现(docs/openapi/management-v1.json,1:1 对齐);
- 已批准缺口包(CR-FE-001):G1 全图读取、G2 事件管道接线、G3 组合分析端点(P0);G4-G7(P1);G9 成本估算(已批准,FE-5);
- 硬约束 C1-C12(docs/07 §4):CSP 'self' 无内联无 CDN、秘密零浏览器存储、生成客户端唯一 fetch 通道、双构建字节一致、PATCH 整体替换;
- 未定/不做:实时推送 H20(Later,先轮询);巡检自动化 v1 只读;多用户 RBAC 不做;客户计费不做(BL-22;G9 仅运营者估算);
- 术语表:Upstream/Endpoint/Credential/Binding、Public Model/Alias/Route/Candidate、Access Group/Client Key(`rgw_` 前缀,签发仅显示一次)、Egress Policy、Config Version(draft/active/archived)。

## Brand Commitments

用户明确锁定:视觉方向为 Apple Liquid Glass(官方 HIG 规则约束下),代号 Prism(棱镜 = 请求光路折射隐喻)。设计方向契约与设计系统见 docs/07 §8(THESIS/OWN-WORLD 已锁定);DESIGN.md 将于首个视觉面建成后由实际构建产出(impeccable 纪律)。

## Evidence on Hand

- 后端真实枚举与 schema:docs/openapi/management-v1.json、crates/gateway-store(徽章词汇)、crates/gateway-router(健康/配额/Explain 投影);
- 观测数据形状:crates/gateway-core gateway_event.rs(Request/Attempt/Usage/Health 事件,6 类 token 字段);
- 参照产品全量勘察记录:docs/07 §3(CPAMP 页面清单、复制与差异决策);
- 缺席声明:成本数据在 G9 落地前不存在 —— 界面不得渲染伪造的 $0 成本;事件数据在 G2 接线前不存在 —— 观测卡显示「管道未接线」专属空态,不伪装为健康。

## Product Principles

1. **不可探测与不可回读是特性**:认证失败不区分原因;秘密只在签发瞬间可见 —— 界面把这些讲清楚,而不是绕过;
2. **两层真相分开呈现**:编译期资格(模型是否上架)与运行时可用性(此刻能否服务)永不混淆;
3. **空态诚实**:真空 / 过滤后空 / 投影不可用 / 管道未接线,四种状态四种文案,绝不伪装;
4. **URL 即状态**:时间范围与全部过滤器可编码可分享,任何「查看详情」都是链接;
5. **界面消失于任务**(Operate 模式):熟悉的表格与表单词汇,克制的动效只表达状态变化,炫技只保留在 Route Prism 一处。

## Accessibility & Inclusion

必须尊重三个系统偏好并逐一验收:prefers-reduced-transparency(玻璃→近实底磨砂)、prefers-contrast: more(实底+高对比边框、徽章黑白化)、prefers-reduced-motion(退火与光束动画禁用)。玻璃面上文字按 WCAG AA 验收,含最坏滚动背景情况;全键盘可达;reveal-once Sheet 有焦点陷阱。
