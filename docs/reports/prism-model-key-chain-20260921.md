# 模型开放与受限密钥链路 — 2026-09-21

## 结论

当前 ce70125 产品代码的真实本地 gateway + loopback TLS mock 链路通过。使用新建隔离数据库、临时凭据和进程专用 CA；没有使用生产账号、改变系统信任或调用真实 Provider。本批没有产品代码变更或部署。

## 实际操作与证据

1. 脚本建立一个已开放模型 Exact/Model-v2 及旧密钥。真实目录刷新读取两页共3个模型，客户端仍只见1个模型，刷新不自动开放。
2. EgoLite登录正式嵌入前端，完成合成管理员首次改密；从目录选择 Model-Second 并确认接入。回执明确已保存并应用，原始模型名保留。
3. 通过API密钥页面创建 Local model-only client：先点“全选当前已开放模型”，实际选中2个；清空后只选 Model-Second，再创建并应用。一次性合成密钥仅保存到0600临时文件，没有写入截图、聊天或报告，完成后确认DOM秘密已清除。
4. 数据面验证旧密钥仍仅见 Exact/Model-v2，新密钥仅见 Model-Second；新密钥请求旧模型返回404，请求选定模型返回200。
5. 新请求终态 succeeded，账本记录出现；缺价保留 unpriced，cost_microunits=null。
6. 原模型另验证JSON与SSE成功、上游503、流式截断、TCP取消：5个外部请求、5次attempt、2成功、2失败、1取消；两条成功记录完成计费物化。截断含response.failed，不含response.completed；有实际耗时观测。

机器证据：[请求链](evidence/prism-model-key-chain-20260921/request-chain-acceptance.json)、[界面模型与密钥](evidence/prism-model-key-chain-20260921/ui-model-key-acceptance.json)。密钥回执中的全选数量同时由本次浏览器读取验证为2，不仅依赖脚本常量。

使用现有 scripts/acceptance 下的 prism-local-gateway.py、prism-request-chain.py、prism-ui-model-key.py。控制器、mock、网关及浏览器任务均已正常关闭；未清理或重启其他服务。浏览器有可用更新提示，本轮未升级。

## 边界与后续

这是正式前端加真实网关的本地业务链验证，推理与目录来源为mock，不证明真实渠道登录或真实目录完整性。不替代Safari、跨版本旧标签刷新或全站视觉验收。下一批核对请求日志与用量费用页面如何展示这些终态、缺价、重试及筛选结果；之后完成设置与新候选发布前检查。
