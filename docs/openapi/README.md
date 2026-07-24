# OpenAPI contracts

本目录保存版本化、可机器读取的 HTTP 合同。合同先于实现：它定义外部资源、稳定错误、Secret 生命周期和
并发语义，但不声明某个端口已经监听或某条路由已经可调用。

- [Management API v1](management-v1.json) — P10-01 的 `OpenAPI 3.1` 管理面合同。

`management-v1.json` 当前是 `contract_only`。P10-02 才会把独立的 Management Key、仅本机/私网
准入、CSRF/CORS 和 Actix 路由接到这个合同；P10-03 至 P10-08 分别实现标注的实体、运行时诊断、
审计和备份操作。公开推理 `/v1/*` API 不属于本目录的管理合同。
