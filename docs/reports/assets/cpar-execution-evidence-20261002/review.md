# 本轮两轴评审与收敛

## 基线及范围

基线 `09e70e2aa23a8541e8e20726edb15bdc96a6c2af`；初始实现 `ff77a5f05dc3145035cf14c4c2833f24833cafd2`；行为修复 `c94b35edae148445c3c3cd44edc784807fe45762`。最终验证及候选源码 `80f5838902eb56be6dccc3ce047eb227d6c18e6c`。

Standards 依据当前项目 AGENTS、契约与跨边界纪律、ADR/CONTRACT-TESTS、持久化及秘密规则。Spec 依据本次用户实施要求、#9、#39–#50、#57 及上一轮正式验收工具的事实缺口；覆盖本切片的软件行为，不覆盖全部 C 或父规格。

## 独立评审

| 轴 / 轮次 | 精确范围 | 结果 | 结论 |
| --- | --- | --- | --- |
| Standards 初评 | `09e70e2a..ff77a5f0`，30 个实现/契约/文档文件 | PASS | 无确定 findings；服务边界、同提交契约镜像/跨边界记录、原事件兼容、原子 sidecar、有界观察符合规则 |
| Spec 初评 | 同上 | FAIL | 两项 P2：Web relay 错用备用 profile 证明实际出口；Python 采集器重复键 last-wins 与 Rust 严格解码不一致 |
| Standards 复评 | `ff77a5f0..c94b35ed`，8 个修复文件 | PASS | 无新 findings；实际 transport getter、严格 canonical/8192 字节限制、回归、ADR 澄清符合本切片规则 |
| Spec 复评 | 固定修复 diff，另核对 `09e70e2a..c94b35ed` 相关归属/能力/持久化/正式入口 | PASS FOR THIS SLICE | 两项 P2 已关闭，无新确定缺陷；不是 C 全批或父规格完成结论 |

以上使用四个相互独立、只读、单轮子代理。代码选择、修改与最终验证由主代理负责。

## 先复现、再关闭

- Web relay 原实现的定向新增回归先 RED，再修复为 relay 未知；实际自有 transport 的 Direct/ProcessProxy 保留实际事实。[RED](review-web-red.log)、[GREEN](review-web-green.log)。
- 采集器重复键样本原实现先 RED，再启用递归重复键拒绝、闭合字段、数值范围及与 Rust 一致的 canonical 字节核对。[RED](review-collector-red.log)、[GREEN](review-collector-green.log)。
- 真实 gateway + 隔离 SQLite + TLS 上游的初始归属样本能成功返回六协议/模式响应，但缺少执行 sidecar。[基线 RED](red-http/receipt.json)。仅 HTTP 成功未构成通过。

## Full 后的机械补齐与最终范围

`179cf325ee9f9c0531a6d343467021928c1076c0` 只在现有“所有已知表”测试清单中登记 `gateway_attempt_execution`。`80f5838902eb56be6dccc3ce047eb227d6c18e6c` 只登记 composition root 已使用的工作区 `sha2`，并解释 ADR-0102 的实际 process proxy 指纹用途。没有更改运行时、契约、能力声明或权限。

主代理完整阅读这两处即将修改的代码并复核最终 diff；表清单与 migration 34 一致，依赖图仅新增已审查原语，不扩大任何下层 crate 边界。两处补齐的 Standards/Spec 均 PASS。最终 Full 与 ARM64 实测绑定最终 SHA，不能以 `c94b35ed` 的评审替代最终门槛记录。

尚未完成：真实 Provider、生产部署、生产状态副本演练、签名远程发布、#57 全部边界/扩展及 C 全批验收。本文件没有关闭任何 GitHub issue。
