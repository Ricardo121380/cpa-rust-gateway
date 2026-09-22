# V2 账号密度复核与预览恢复

2026-09-23，基线 `3a748bc`。

## 环境恢复

原 EgoLite TaskSpace 9 已不存在，62992 预览端口停止。取得用户明确同意后建立 TaskSpace 1，重新启动原临时状态目录的 gateway，未重建或清空账号/请求/账本。原稿服务器恢复至 63008。

管理端：`http://127.0.0.1:62992/admin-ui/#/accounts`。原稿：`http://127.0.0.1:63008/CPAR-Prism-Liquid-v2.html#accounts`。本轮未恢复 62991 mock Provider，不能把管理只读预览视为推理链路验收。

## 对照与修改

- 实际应用与原稿正文的计算样式一致：13px，`-apple-system, system-ui, Segoe UI, Noto Sans CJK SC, PingFang SC, sans-serif`。本轮没有再次更换字体。
- 账号运行摘要由“状态、额度、快照、证据链接”四行改为三行：状态即证据入口。缺快照时仍为两行，不制造观测值。
- 保留渠道分组、额外筛选、真实身份缺失提示和独立授权语义，因此列表并非把原型的合成账号表直接替换进来。

## 本次验证

- 类型检查通过；AccountRuntimeSummary 3 项分页/快照/边界回归通过。
- management-spa 的 152 操作契约、CSP、四文件及确定性双构建门禁通过；gateway 嵌入构建通过。
- 最终实际网关三尺寸 × 深浅主题共 6 组，无画布横向溢出；每组 6 个状态证据链接。点击可用状态，进入 `/accounts?view=runtime`。
- 主题切换动画中的早期截图不作为结论；最终截图等待所有动画停止，见 `assets/prism-v2-density-20260923/final-*` 与 `final-layout.json`。
- 亲自查看了桌面对照及最终浅色图；未把几何检查称作全站像素级签收。

未上线，未调用真实 Provider，未改变生产。复杂弹窗矩阵仍以前一报告为准；本次只修正并验收一个明确的视觉密度差异。
