# P12 生产渠道加入与公网验收记录

日期：2026-08-10

## 结论

本次只更新 CPAR 的 successor Config Version；旧 CPA、Caddy/DNS、CC Switch、生产
grok2api 进程和既有公开入口均未被替换或改写。当前 Oracle Singapore 上的 CPAR 服务
保持 `active`，生产 SQLite `quick_check=ok` 且外键检查为空。

| 渠道 | 生产图 | 真实 CPAR 公网证据 | 当前结论 |
|---|---|---|---|
| ChatGPT Go（Sub2API 格式 OAuth） | 已加入独立官方 Codex OAuth upstream、credential pool、三协议 routes | Chat/Responses/Messages 的 JSON 与 SSE 受控请求均完成；本次 Go route 的 JSON/SSE 为 `2/2` | **PASS** |
| Grok Console | 已加入 native Console account pool 与独立 route | 真实 CPAR JSON/SSE `2/2`；请求从生产 CPAR 数据面发出并完成终态 | **PASS** |
| Grok Build | 已加入 native Build account pool 与独立 route | 当前生产池唯一 Build credential 在本次导入时已过期，JSON/SSE 均在凭证选择阶段以 `CredentialUnavailable/credential` 终止；没有发出上游推理 | **BLOCKED_WITH_EVIDENCE** |
| Krill | 独立 upstream、bearer credential、egress policy 和三组 route；不与 Go/Grok 共享凭证或回退 | 修正 Responses/Messages 使用兼容的 Chat endpoint + canonical bridge 后，Chat/Responses/Messages JSON/SSE `6/6` | **PASS** |

## 配置图边界

当前 active version 为 `p12-09-codex-official-oauth-v6`，其 successor parent 为前一版
Codex 图。图中包含六个公开模型/route：一个 ChatGPT Go、两个 native Grok（Build、
Console）以及三个 Krill 协议入口。现有生产 client key 仍绑定到新 access group；没有保留
已撤销的临时 helper key，因为运行时会对 active graph 中任何 revoked key fail closed。

Krill 的三个入口虽然在协议桥接层复用同一个兼容的上游 Chat endpoint，但仍属于独立
Krill upstream：credential、egress policy、candidate、Health/Quota 和失败域均不与
ChatGPT Go、Grok Build、Grok Console 共享，也不存在跨渠道 fallback。该桥接只是修复
Responses/Messages 的线协议兼容，不改变渠道归属。

## 公网验收回执

回执均由远端 root-only 目录保存，内容为 value-free 计数、协议、HTTP 类别、终态和
脱敏失败类别，不含 endpoint、client key、OAuth/SSO、Cookie、请求正文或响应正文。

- 初始 12-call 回执：`public-acceptance-20260810.json`
  - ChatGPT Go JSON/SSE：`2/2`
  - Grok Console JSON/SSE：`2/2`
  - Krill Chat JSON/SSE：`2/2`
  - Grok Build JSON/SSE：`0/2`，固定类别 `CredentialUnavailable/credential`
  - 初始 Krill Responses/Messages 直连轮廓分别出现 `StreamTruncated`/上游过载类别；这两项
    不是最终结论，因为后续只调整了 Krill candidate 的 endpoint/bridge 组合。
- Krill 修正后的最终回执：`public-acceptance-krill-final.json`
  - Krill Chat、Responses、Messages 各自 JSON/SSE 均通过，总计 `6/6`。

## Grok Build 阻断原因

生产 native Build pool 中当前只有一个 active/enabled account。该账号曾在早期隔离
验证中通过，但本次生产导入使用的运行时 token 已超过其 expiry；CPAR 的
`GrokBuildCredential::import_runtime_json` 因此在凭证解包/有效性边界返回
`CredentialUnavailable`，而不是把无效 token 发给 Grok。服务器侧现有 Autoreg/grok2api
源池中没有可直接替换的、已验证未过期 Build OAuth；对现有 refresh/SSO 记录的只读复核
也没有产生新的有效 token。

因此 Build **已进入生产配置图，但尚未达到“生产可用”**。获得新的有效 Build OAuth/refresh
后，只需在不触碰已通过渠道的前提下重跑 Build-only 的真实 CPAR JSON/SSE 短矩阵；不应
把本回执的 `0/2` 改写为成功，也不应因此回滚 ChatGPT Go、Grok Console 或 Krill。

## 运行与回滚证据

- 服务：`cpa-rust-gateway.service=active`；数据/管理 listener 仅绑定 loopback。
- 数据库：`quick_check=ok`，`foreign_key_check` 无行。
- 回滚包：远端 root-only 目录 `p12-channel-add-20260810T160316Z`，保留 v5 一致性快照、
  v6 fixed 数据库、当前 binary/unit/current-link、哈希和 value-free receipts。
- 收尾：临时生产脚本、临时 debug 进程、grok2api loopback 容器和明文 OAuth export 均已清理。
- 按既定执行规则，本轮未触发 GitHub CI，也未 push；日常构建继续使用 Oracle 本机构建，
  P 级正式收口再单独安排一次远端 Delivery Gate。

## 后续

1. 获取一个当前有效的 Grok Build OAuth/refresh，登记新的 Build-only 验证边界。
2. 通过同一个真实 CPAR Base URL + client key 发送 Build JSON/SSE；成功后再把 Build
   状态从 `BLOCKED_WITH_EVIDENCE` 更新为 `PASS`。
3. 不重复已通过的 ChatGPT Go、Grok Console、Krill tuple，不改变 Krill 的独立渠道边界。
