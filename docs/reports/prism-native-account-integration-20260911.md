# 原生 Grok 账号接入与验收

本轮已把 Grok Build／Console／Web 接到原生加密账号池。九类渠道均有实际凭据接入路径；
Build 另有首次 Device OAuth 和保持原账号 ID 的重新授权交互。实现提交为 `21afa41`。

- 导入后立即在账号页显示，不依赖运行绑定。
- Build 授权等待实际 grant；取消、过期、身份不一致或 revision 过期不覆盖账号。
- 原生库存分页与账号变更绑定，不因普通请求增长失效。
- schema 24 添加库存 generation 和重授权审计。本轮仅本地运行，未部署生产。

本次 7 项 SQLite/HTTP、2 项授权工作流、7 项既有 native migration、7 项浏览器检查通过；
迁移往返、Clippy 和 122 操作契约／四文件嵌入门禁通过。首次测试中的日期字段和原生
provider 存储枚举问题已修正；失败没有计作通过。

随后在 Chrome 的真实本地应用完成了首次授权、同一账号重新授权和 gateway 重启重读。
账号保持 1 个，ID 不变，revision 0→1，密文更新并有审计。具体事实及收据见
[真实授权验收](prism-grok-authorization-acceptance-20260911.md)。

本地入口：`http://127.0.0.1:61700/admin-ui/#/accounts`。管理员已初始化并改密，密码只在
该预览本人可读的临时文件中。最终重启已加载包括终态清理和分组空态修订的新构建。
Chrome 页面保留，停止本地进程不删除其临时加密数据库。

其余渠道首次 OAuth、批量操作、提供商向导和运行配置热切换仍属于待完成工作，详见
[进度](prism-functional-redesign-progress.md)及
[CR-PRISM-NATIVE-ACCOUNT-001](../change-requests/CR-PRISM-NATIVE-ACCOUNT-001.md)。
