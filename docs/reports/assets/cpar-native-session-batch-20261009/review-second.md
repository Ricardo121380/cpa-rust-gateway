# 本批第二次独立评审（主代理整理终态结果）

固定 base：`85a168da39e79b49c58e83767bdc2bbce0b8a00a`。评审 HEAD：`184f9daaa32c90cf60f817659ac64c349217a128`，与已完成 clean Full 的 `3b4f78c8` 之间仅有报告及证据归档差异。
两位只读子代理分别核验 Standards 与 Spec；以下是主代理对终态发现的整理，不是原话转录。

## Standards

**FAIL**。`/root/post_standards_review` 返回一项文档标准违反，Fowler smell 发现 0。

`AccountBatchDialog.tsx:171` 的普通删除预览仍把 credential、endpoint、remaining credential、route 和 client key 的内部 ID 展示为正文名称；`cpar-account-lifecycle-page.mjs:78` 又要求页面包含这些原始 ID。普通预览的旧实现已经存在于 base，但本批修改该处周边并新增了依赖原始 ID 的验收断言，属于本批相关修正范围。它违反 `web/prism/DESIGN.md:2215–2225` 对普通展示名称与精确机器引用的分工。

此前新增原生展示、缺失身份回退、DESIGN §42 交接和 Preferences 陈述均已修正。权威契约与生成客户端相同，跨边界修改具备同提交日志及 trailer，121 项归档 hash 匹配。旧页面证据没有被写成新页面回放。

## Spec

**PARTIAL**。`/root/post_spec_review` 返回一项 P2 未满足项，仍是源码推断，未动态复现。

用户要求“关闭、离开并返回后，按现有会话合同继续处理”。`GrokDeviceWizard` 在 poll error 后置为未知，但恢复 handle 仍只在 `close()` 登记；未知时不阻止导航，直接浏览器 Back/Forward 可能绕过登记。既有实际页面验收先显式关闭，不能证明直接导航。新的原生展示回放亦为 NOT_RUN，这是证据缺口，不能把脚本更新当作执行记录。

评审明确未发现 #14 的独立阻塞项；HTTP 身份矩阵与18项 fixture 页面证据按各自边界有效。#36/#37/#38 亦无独立阻塞项：保留已批准的严格 Build 刷新例外，证明/CAS 负例、实际 stock worker 重启和 public JSON/SSE 的推理4、刷新1、fallback0、成功根4，以及 compact/WS 的410项和实际 receive223分别有据。

两轴各保留自己的判定；普通名称修正后的规范结论和本批最终源码仍须另行复评。GitHub 在本次评审时尚未写入。
