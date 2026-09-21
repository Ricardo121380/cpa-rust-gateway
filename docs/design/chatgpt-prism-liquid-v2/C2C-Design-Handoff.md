[C2C]
STATE: REVIEW
WORKSPACE: cpa-rust-gateway
SCOPE: PRISM_LIQUID_V2_DESIGN_REFINEMENT
DELIVERABLE: OFFLINE_PROTOTYPE_AND_MOTION_SPEC
REPOSITORY_CHANGES: NONE
PRODUCTION_ACTIONS: NONE
NEXT_EXPECTED_STEP: REVIEW_V2_VISUAL_DIRECTION_BEFORE_BOUNDED_IMPLEMENTATION

## 目标

在已接受的 Prism 工作台方向上细化 Liquid Glass 元素、按钮与动效。八个工作区和已审查的业务语义不变。不重置既有功能任务，不把当前设计原型升级为生产验收证据。

## 已完成的设计交付

`CPAR-Prism-Liquid-v2.html` 是自包含离线原型；顶栏滑杆入口可看材质与动效预览。统一 110/160/210/240ms 时序、渐变高光与薄叠层按钮、导航／分段控件的共享选择底板、单面板切换、状态回执和辅助降级。MP4 为实际原型演示。

当前代码只读基线：HEAD `ca75a44`，工作树 dirty。现有 `glass.css` 已有玻璃层与降级，`Sheet.tsx` 已有强生命周期，不应替换成展示库的较弱 primitive。package.json 未声明 Motion/Tailwind；本轮不要求新增这些依赖。

## 第一批建议：只动共享材质和按钮

为什么：本次用户请求是视觉微调；优先提升出现频率最高的共同界面，降低同时改业务流程的风险。

目标：`web/prism/src/design/tokens.css`、`glass.css` 以及 `app/app.css` 和 v4/v5/v6 样式中受影响的按钮规则。先核对实际导入顺序与选择器，再整理到单一 token/control 层；不要再追加一整层 v7 覆盖。

约束：不修改路由、配置／账号 API、生成客户端、密钥权限、价格目录规则；不搬入原型的内联事件、原生 dialog 或模拟定时器；不新加系统助手、团队或角色功能。

测试：正常/悬停/按压/焦点/禁用/忙碌/成功/错误；1440×900、1280×720、390×844；深浅色；减少动态和透明；对比度；长文案与固定底栏遮挡。复用现有 smoke、glass、contrast、narrow 与 workspace-visual 测试，再验证类型和确定性构建。

## 后续批次必须保持的边界

导航与面板过渡不能影响 Sheet/OperationBoundary 的 dirty、busy、关闭、Back、会话失效和秘密清理。动效不能延迟实际清理。待应用条不变成发布捷径；已完整读取的差异、显式接续、目标与基线修订才构成应用依据。原生运行时操作保持即时，全局目录独立保存。

## 证据

原型的 54 项布局观察与 35 项交互断言通过，无观察到的 JS 运行错误及网络请求；详情见 JSON。浏览器使用离线 DOM 注入，不是产品服务。Safari、屏幕阅读器、实际网关、最终 bundle/performance 和跨版本资源加载仍需要生产实现后的专项验证。

只有用户确认视觉方向后才进入正式实现。届时 Codex 返回 EXECUTED 后，审阅方按连接器 execution_output 的 list→read 顺序取可读取输出；restricted 时以 git 实际变化为证据，不索要粘贴日志。
