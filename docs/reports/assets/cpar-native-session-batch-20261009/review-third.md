# 本批第三次独立评审（终态整理）

固定 base：`85a168da39e79b49c58e83767bdc2bbce0b8a00a`。评审 HEAD：`11d40790e4b578e1d045da34472304c87a5877c1`。两位只读探子未执行测试或浏览器，以下轻量整理原终态。#23 比较层修正发生在这次评审之后，不属于本回执的 PASS 范围。

## Standards

`/root/closure_standards_readonly` 返回 **PASS（静态评审）**；硬发现0、Fowler12项启发式发现0。

- 名称修正满足 `DESIGN.md:2307`“内部主键只供 API、审计、DOM 精确目标及明确的复制入口，不作为正文”。`AccountBatchDialog.tsx:171–184` 普通/native使用ResourceIdentity，`resourceLabels.ts:32–36,51` 关联exact model_name，失败显示历史资源；DOM原ID和明确复制入口保留。
- `GrokDeviceWizard.tsx:71–72` 使用IdentityDetails；`NativeAccountDialog.tsx:59,74` 缺身份明确提示，无import_batch_id回退。DESIGN §42已补交接，Preferences仅内存与实现一致。
- 跨边界代码提交均有同提交日志和trailer，权威/前端契约字节相同；142项归档hash、40项空白转换双hash和原raw匹配；122份唯一回执。509至11代码/契约diff为空，当前clean门槛记录有据。

限制：最新展示回放、直接导航复现NOT_RUN，GitHub尚未写入。直接导航推断属于Spec轴。

## Spec

`/root/closure_spec_readonly` 返回 **PARTIAL**：2项P2源码疑点；另有最新删除展示回放NOT_RUN。在评审当时均未动态复现，不能称行为RED。

- **P2/#16，missing/partial**。用户文本:49：“关闭、离开并返回后，按现有会话合同继续处理”。`GrokDeviceWizard.tsx:30,48,59` poll error进入未知但只在close保存handle，unknown关闭导航阻挡；`Sheet.tsx:99,225` 无相应卸载保存。Back/Forward可能丢任务。实际驱动:42–46先显式关闭，只证明该路径。
- **P2/#23，looks implemented but wrong**。用户文本:76：“先核对、保留已确认结果”；DESIGN:2309只允许已确认删除成员消失。`AccountBatchDialog.tsx:93`却传全部selected ID，`deletionImpact.ts:5`排除它们全部字段比较，尚未执行成员revision/auth/enabled改变或消失也被忽略。新后端CAS绑定新快照，不能证明它等于操作者确认的旧影响。后续目标自身校验虽会拒绝冲突，此前目标可能已删除；评审当时尚无场景回执。
- 固定diff、14票正文/评论及相关实现/归档范围内，未发现scope creep。新删除展示缺口不能把更新驱动当作重跑。

**#14/#36/#37/#38无独立阻碍。** #14的12组真实handler身份矩阵与18项fixture控件证据分别成立。#36的stock worker/实际重启、JSON/SSE、推理4/刷新1/fallback0/root4、证明/CAS负例有据；compact/WS为410 PASS、实际receive223。eab至HEAD后端/契约及OAuth实现fixture驱动无diff，509至HEAD代码无diff，旧证据保持原source。#14/#36前置均CLOSED；#37/#38仍只由OPEN的#36阻塞，必须按实际依赖顺序关闭。
