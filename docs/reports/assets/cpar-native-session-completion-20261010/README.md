# CPAR 原生账号批次补验（2026-10-10）

本目录接续此前批次的不可改写原始执行回执。用户已批准新的 EgoLite space39，旧 space37 的许可前结论保留在原目录。

- `direct-navigation-behavior-red.json`：clean c1e8ab66 的实际嵌入页面，先确认 SQLite 保存，再丢成功响应；直接 Back/Forward 丢失恢复入口。这是有意义的行为 RED。
- `direct-navigation-behavior-green.json`：clean d05d4418 的同一实际路径恢复，只 GET 原 session 一次，grant/exchange/account各1，cancel0。
- `close-navigation-green.json`：clean 36079c1f 的显式关闭、离开、返回路径，同样只读取原会话；对应清晰截图已目视核对。
- `lifecycle-lifecycle-page.json`：clean 36079c1f 的实际产品控件、开发 fixture 后端，共30项PASS。19项原矩阵、缺失身份1项、native pending revision/enabled/auth/missing×前/后共8项、普通binding变化×前/后2项。native首项前DELETE0，首项成功后DELETE1，已确认结果保留。普通前置配置变化报待核对，已有任务的影响变化明确拒绝；未知写入不重放由原批次丢响应场景证明。
- `execution-index.json`：唯一原始执行路径、命令、来源、环境、时间、退出码和SHA；运行中的执行仅在终态后纳入。原始日志在output中保持不变。
- `manifest.json`：归档文件hash以及日志尾部空白规范化的原始/归档hash。SQLite、凭据、TLS私钥、密码与token不归档。

初始登录被macOS密码浮层拦截、未启用fixture的登录失败、普通binding验收分类预期错误和Full新目录缺失SPA文件的失败均保留；它们是harness/前置条件失败，不作为产品行为RED。Full准备按正式SPA构建补齐，保持原门槛。

原目录的HTTP12、OAuth fixture18、shared fixture14、两类真DELETE及在途证据、非空历史保留、E-05真实生产worker/持久化/公共处理/重启、compact/WS410、#56本家族6项分别按原source保留。此补验没有产生真实Provider、远端CI、发布、部署或生产证据。

最终本地门槛绑定clean36079c1f；[attestation](attestation.json)记录Full45/Rust1499/前端513/types/doublebuild。

[两轴评审](review-final.md)保留独立Standards PASS/hard0/heuristic1与Spec独立覆盖PARTIAL/hard0；[主代理静态复用补核](final-reuse-verification.json)另记PASS，不冒称独立覆盖或动态重跑。

[GitHub最终绑定](github-final-attestation.json)与[独立读回](github-final-readback-summary.json)证明16原评论更新、两正文有据AC、十票新增关闭，48票35CLOSED/13OPEN；父9/57正文与原生依赖保留。源码未push，真实Provider0，冻结新候选NOT_MET。

最后文档门槛[7步](closeout-docs-steps.md)/[clean source绑定](closeout-docs-attestation.json)PASS，source057970c7，执行前后clean且代码/契约同360。space39已finish一次；后续仅归档该事实。
