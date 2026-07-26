# CR-FE-001 附件 · G1/G7/G3 契约形状提案(前端起草)

| 项目 | 值 |
|---|---|
| 状态 | Proposal — 前端会话起草(2026-07-26),供后端会话实现时采用/修订;形状变更以最终落入 `docs/openapi/management-v1.json` 的为准,前端经 `sync-contract` 跟随 |
| 隶属 | [CR-FE-001](CR-FE-001-management-frontend.md) §2 契约缺口包 |
| 设计原则 | ① 全部复用契约既有 component schemas($ref,零新实体定义);② 遵循既有惯例:统一错误信封、缺失版本→409、ETag `"rev-N"`、redacted 视图、闭集枚举;③ G3 附带 rollup 纪律要求(源自 CPAMP 生态审计,docs/07 §3.3) |

---

## 1. G1 · `GET /admin/config-versions/{config_version_id}/graph`

一次调用返回一个 Config Version 的完整 redacted 配置图。**扁平集合而非嵌套树**:全部实体已含外键(endpoint.upstream_id、candidate.route_id…),前端按 ID 归一化;每个数组元素 `$ref` 契约现有 schema,不新增实体定义。

- operationId: `getConfigVersionGraph`;tag: `configuration`
- 路径参数携带版本(与 validate/publish 一致),**无需** X-Config-Version 头
- 响应头 `ETag: "rev-N"`(与资源 list 语义一致,作为后续变更的 If-Match 起点)
- 版本不存在 → 409(沿用 getConfigVersion 惯例);任何状态(draft/active/archived)均可读

```jsonc
// 200 GraphResponse
{
  "config_version": { /* $ref ConfigVersion */ },
  "egress_policies":     [ /* $ref EgressPolicy */ ],
  "upstreams":           [ /* $ref Upstream */ ],
  "endpoints":           [ /* $ref Endpoint（含 upstream_id）*/ ],
  "credentials":         [ /* $ref Credential（redacted:secret_present,无明文）*/ ],
  "bindings":            [ /* $ref Binding（endpoint_id+credential_id+upstream_id）*/ ],
  "public_models":       [ /* $ref PublicModel */ ],
  "aliases":             [ /* $ref Alias（alias+public_model_id）*/ ],
  "routes":              [ /* $ref Route(含 public_model_id) */ ],
  "candidates":          [ /* $ref Candidate(含 route_id) */ ],
  "access_groups":       [ /* $ref AccessGroup */ ],
  "access_group_routes": [ /* $ref AccessGroupRoute */ ],
  "client_keys":         [ /* $ref ClientKey（redacted:仅 prefix）*/ ]
}
```

OpenAPI 片段(paths):

```json
"/admin/config-versions/{config_version_id}/graph": {
  "get": {
    "operationId": "getConfigVersionGraph",
    "tags": ["configuration"],
    "parameters": [{ "$ref": "#/components/parameters/ConfigVersionId" }],
    "responses": {
      "200": {
        "description": "Complete redacted configuration graph",
        "headers": { "ETag": { "$ref": "#/components/headers/RevisionEtag" } },
        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ConfigVersionGraph" } } }
      },
      "409": { "$ref": "#/components/responses/Conflict" }
    }
  }
}
```

`ConfigVersionGraph` schema = object,13 个字段全 required(空集合用 `[]`,不省略字段),各字段 items `$ref` 既有 schema。规模上限继承各实体的契约上限(单运营者规模,无分页必要;若后端坚持防御性上限,建议整图 ≤ 4 MiB 时 200,超限 409 `graph_too_large`)。

**前端消费方式**(供后端理解读路径热度):解锁后每版本读一次 + 每次成功变更后按需重读;不轮询。

## 2. G7 · `GET /admin/capabilities`

一次探测替代逐端点试探。管理鉴权后访问,无版本头。

```jsonc
// 200 CapabilitiesResponse
{
  "features": {
    "endpoint_test":        { "available": false, "reason": "rejecting_facade" },
    "catalog_discovery":    { "available": false, "reason": "rejecting_facade" },
    "credential_oauth":     { "available": false, "reason": "rejecting_facade" },
    "catalog_status":       { "available": true },
    "runtime_availability": { "available": true },
    "quota_recovery":       { "available": false, "reason": "rejecting_facade" },
    "request_attempts":     { "available": true },
    "route_explain":        { "available": true },
    "analytics":            { "available": false, "reason": "pipeline_unwired" },
    "dashboard_summary":    { "available": false, "reason": "pipeline_unwired" },
    "model_prices":         { "available": false, "reason": "not_in_release" }
  }
}
```

- feature 名 = 闭集(上列 11 个;新增 feature 是兼容性变更,前端对未知名宽容);
- `reason` 闭集:`rejecting_facade`(注入的默认拒绝实现)| `pipeline_unwired`(G2 未接线)| `not_in_release`(范围外/未批准);`available:true` 时省略;
- 语义:capabilities 表达**部署形态**,不表达瞬时健康(健康归 runtime availability);
- operationId: `getCapabilities`;tag: `runtime`。

### 2.1 附:OAuthOperation 的低成本扩展需求(实现 OAuth 向导时发现)

现契约 `OAuthOperation = {credential_id, state, expires_at_ms?}` 缺少设备授权流的**用户可见要素**:`user_code` 与 `verification_uri`(Grok Build 设备流必需 —— 运营者要把 code 输入到 x.ai 页面)。建议在 G1/G7 同批为其增加两个可空字段:

```jsonc
{ "credential_id": "…", "state": "pending",
  "user_code": "ABCD-1234",            // 可空;设备流才有
  "verification_uri": "https://…",     // 可空
  "expires_at_ms": 1785000300000 }
```

均为响应侧新增可空字段,向后兼容;前端向导已按"缺席时只显示生命周期状态"实现,字段出现即自动升级展示。

## 3. G3 · `POST /admin/analytics` + `GET /admin/dashboard/summary`

### 3.1 POST /admin/analytics(组合查询,单端点支撑全部观测页)

```jsonc
// 请求 AnalyticsQuery —— include 全部可选,按需组合;省略 = 不计算不返回
{
  "from_ms": 1785000000000,            // 含
  "to_ms":   1785086400000,            // 不含
  "timezone": "Asia/Shanghai",         // IANA;仅影响 bucket 边界对齐与 heatmap weekday/hour
  "bucket": "auto",                    // auto|hour|day;auto = span≤48h→hour,否则 day
  "filters": {                         // 全部可选;数组语义 = IN;跨字段 AND
    "public_model": ["minimax-m3"],
    "client_key_id": [], "credential_id": [], "endpoint_id": [], "upstream_id": [],
    "protocol": null,                  // "openai_responses"|"anthropic_messages"|null
    "status": "all",                   // all|success|failed
    "error_code": [], "error_scope": [], "stage": []   // 既有闭集枚举(17 码/10 域/8 阶段)
  },
  "include": {
    "summary": true,
    "timeline": true,
    "ranks":   { "by": "public_model", "limit": 10 },   // by: public_model|client_key|credential|endpoint
    "heatmap": { "metric": "requests" },                // requests|tokens|failure_rate
    "options": true,                                    // 范围内各过滤字段的 distinct 值(供下拉)
    "events":  { "cursor": null, "limit": 100 }         // 逐请求分页;limit ≤ 1000
  }
}
```

```jsonc
// 200 AnalyticsResponse —— 仅含请求的 include 段
{
  "range": { "from_ms":…, "to_ms":…, "bucket": "hour", "bucket_count": 24 },
  "summary": {
    "requests":…, "failures":…, "attempts":…,
    "tokens": { "input":…, "output":…, "reasoning":…, "cache_read":…, "cache_creation":…, "cached":… },  // UsageSummary 六字段,u64 计数
    "latency_ms": { "avg":…, "p50":…, "p95":…, "p99":… }        // 来源:AttemptEvent started/ended_at_ms(G2 必须入库时间戳)
  },
  "timeline": [ { "bucket_start_ms":…, "requests":…, "failures":…, "tokens_total":…, "latency_p95_ms":… } ],
  "ranks":    [ { "key": "minimax-m3", "requests":…, "failures":…, "tokens_total":…, "last_seen_ms":… } ],
  "heatmap":  [ { "weekday": 0, "hour": 8, "value":… } ],        // weekday 0=周一(ISO)
  "options":  { "public_model": […], "client_key_id": […], "credential_id": […], "endpoint_id": […] },
  "events":   { "items": [ /* RequestEventView */ ], "next_cursor": "opaque-or-null" }
}

// RequestEventView(value-free 逐请求行 = Request+末次 Attempt+Usage 三事件连接)
{
  "request_id":…, "occurred_at_ms":…, "protocol":…, "public_model":…, "streaming": true,
  "outcome": "success",                 // success|failed
  "error_code": null, "error_scope": null, "stage": null, "retry_decision": "Completed",
  "attempt_count": 1, "latency_ms": 21400,
  "tokens": { /* UsageSummary,可空 */ },
  "client_key_id":…, "credential_id":…, "endpoint_id":…
}
```

约定:成本字段**整体缺席**直至 G9 落地(缺席 ≠ 0);全部标识符为 ID/前缀,无 URL/头/正文(value-free 与既有事件模型一致)。

### 3.2 GET /admin/dashboard/summary

轻量首屏端点,避免总览页发大查询:

```text
GET /admin/dashboard/summary?today_start_ms=…&now_ms=…&top_models=5&recent_failures=5
→ { "kpi": {requests,failures,success_rate,tokens_total,latency_p95_ms},
    "health_strip": [ {bucket_start_ms, state} ],          // 10 分钟桶;state: empty|ok|warn|bad(闭集)
    "token_mix": {input,output,reasoning,cache_read,cache_creation,cached},
    "top_models": [ {public_model, requests, tokens_total} ],
    "recent_failures": [ {request_id, occurred_at_ms, error_code, error_scope, stage} ] }
```

### 3.3 实现纪律要求(随契约一并采纳,源自 CPAMP 生态实测)

1. **rollup**:UTC 小时聚合表(键:hour+public_model[+endpoint_id]),checkpoint 增量(wake+ticker,每批 1000),`format_version` 键不匹配→清派生表全量重建;**仅"严格无过滤"查询走 rollup**,读法 = raw 前沿 + 完整小时 + raw 后沿;带任意 filter 的查询直查 `gateway_event_log`;
2. **事件哈希幂等已有**(`(event_type,event_id)` UNIQUE),rollup 侧用事件 ordinal 做 checkpoint 即可;
3. **P95/TTFT 类指标永远走 raw**(不进 rollup);
4. `events` 游标 = 不透明的 `event_ordinal` 编码;排序恒为 ordinal DESC;
5. 附属(可后置):`GET /admin/events/export`(JSONL 流式,同 filters)。

## 4. 对 P12 的相关性(供后端排期参考)

依 2026-07-26 计划审计:G2+最小 G3(summary/timeline/events)是 **P12-08 Canary 回滚判据的数据来源**(状态码/TTFT/P95/Usage 分布对比),建议在 P12-06 前落地;G1 是 P12-06 重新布线时"写后可读回"的基线能力。G7 成本最低(读部署形态常量),可与 G1 同批。

## 5. 前端侧同步承诺

契约落入 `management-v1.json` 后:prism 仓库 `npm run sync-contract` 一条命令完成客户端再生成;fixture(`src/dev/fixtures.ts`)与本提案形状的差异由前端会话对齐;本提案的 TS 镜像见 prism 仓库 `src/api/proposed-types.ts`(实现落地后删除,以生成客户端为准)。
