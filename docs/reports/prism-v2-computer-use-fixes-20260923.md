# V2 Computer Use 四项修复 · 2026-09-23

## 交付结论

已处理 `prism-v2-computer-use-acceptance-20260923.md` 的 CU-01～CU-04。范围为总览、提供商详情、手机账号首屏和创建密钥的模型选择反馈；本地真实 gateway 复验通过。此结论不扩大为全站逐像素复刻、生产发布或真实渠道授权完成。

## 改动与复验

| 编号 | 实际改动 | 本轮 Computer Use 证据 |
|---|---|---|
| CU-01 | 总览关注事项增加语义图标与行层级；成功率/P95 的数值和单位分级；费用完整性改为主比例、构成条和四类明细，保留 unknown；资源和处理信息收进披露区 | 1440×900 总览及下方区域；合成账本 3 条，其中 partial 2、unpriced 1，精确占比为真实的 0%，未复制原型百分比 |
| CU-02 | 移除详情中的额外卡片壳；接口和账号使用连续分区；模型目录/连接账号/详情为主要操作，测试、核对、编辑和删除放入更多操作 | 1440×900 浅色和 1280×720 深色；展开更多后“编辑接口”可打开、取消返回；未触发测试、删除或保存 |
| CU-03 | 手机标题与授权主按钮同行；渠道/套餐/排序和批量入口收进筛选面板；显示生效筛选数量；账号主要操作紧随身份 | 390×844 身份位置从旧截图约 720px 提前到约 439px，主要动作约 496px；选择 API 后折叠仍保留 `category=api` 和“筛选 · 1”；无横向溢出 |
| CU-04 | 区分加载中、读取失败、无开放模型、无搜索匹配；增加清除搜索；独立列出已选原始模型 ID 并允许移除 | 选择 Exact/Model-v2 后搜索不存在模型，已选摘要保留；清除搜索后选择仍在；移除后提交禁用；390×844 深色弹窗为 366×820、四边 12px，取消/未保存保护有效 |

手机账号操作放在身份单元格内，只在手机显示；桌面继续使用末列操作，避免单纯 CSS 重排造成键盘顺序与视觉顺序不同。权限集、会话、版本所有权、写请求、后端契约和生产状态均未改变。

## 截图

- [总览](assets/prism-v2-cu-fixes-20260923/overview-1440.png)
- [费用完整性](assets/prism-v2-cu-fixes-20260923/overview-billing.png)
- [提供商详情](assets/prism-v2-cu-fixes-20260923/provider-detail-1440.png)
- [提供商窄桌面深色](assets/prism-v2-cu-fixes-20260923/provider-detail-1280-dark.png)
- [手机账号首屏](assets/prism-v2-cu-fixes-20260923/accounts-390.png)
- [密钥无匹配与已选摘要](assets/prism-v2-cu-fixes-20260923/key-empty-selected-1440.png)
- [手机深色密钥表单](assets/prism-v2-cu-fixes-20260923/key-empty-selected-390-dark.png)

## 自动检查与运行

- TypeScript 检查通过。
- 6 个受影响的现有测试文件、38 项测试通过：请求筛选/计价模型、账号身份与动作目标、密钥权限摘要、接口展示模型。
- 生产构建及 `node scripts/check-management-spa.mjs` 通过，包含 152 操作的契约核验和双构建检查；仍为四文件产物。
- `cargo build -p gateway --bin gateway` 通过；重启既有隔离本地 gateway，使正式嵌入资产生效。
- [四资产逐字节核对](assets/prism-v2-cu-fixes-20260923/assets-verification.json)：本地 HTTP 返回与 `web/prism/dist` 一致。
- Vite 提示既有主包超过 500 kB；本轮未为此改变四文件构建约束或加入额外 chunk。

本地入口：`http://127.0.0.1:62992/admin-ui/#/`。本轮 UI 复验使用 EgoLite 的 Computer Use，未另起 Safari 或独立 Playwright 测试浏览器。

## 保留的验收边界

本轮没有提交资源变更、创建密钥、真实授权或推理；账号仍是原隔离数据，未用假邮箱冒充真实身份。未重新执行全站主题矩阵、屏幕阅读器认证、所有错误注入或生产验收。四项具体问题的修复证据不替代用户最终视觉签收。
