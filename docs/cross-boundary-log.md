# Cross-boundary change log

Every change one side makes to a file the **other** side owns is recorded here,
newest last. See `AGENTS.md` (Codex) / `CLAUDE.md` (Claude Code) for the
boundary and the protocol.

Read the tail before starting work. Entries marked **action required** are
changes made to your side that you did not make.

Format:

```
## <date> · <tool> · <one-line summary>
**Touched:** path(s) the other side owns
**Why:** the need that forced it
**Other side:** FYI | action required — what to do
```

---

## 2026-08-11 · Claude Code · Frontend merged in at `web/prism`, embed repointed

**Touched:**
- `crates/gateway-http-actix/build.rs`
- `crates/gateway-http-actix/src/management_ui_resources.rs`
- `crates/gateway-http-actix/tests/p10_09_embedded_management_ui.rs`
- `scripts/build-management-spa.sh`
- `scripts/check.sh`, `scripts/check-ci-workflow.rb`
- `.github/workflows/ci.yml`, `.github/workflows/release-artifact.yml`
- `README.md`
- deleted `web/admin-ui/**` and `scripts/generate-management-client.mjs`

**Why:** the Prism SPA was developed in a separate repository and is now the
management UI. The embed pointed at the old hand-written `web/admin-ui`, whose
asset names differ (`styles.css` / `generated/management-client.js` vs Prism's
`index.css` / `vendor.js`). The old SPA and its generator are dead code now;
Prism generates its own client from the same contract and `npm run check`
fails on drift.

The build stays hermetic: `cargo build` still builds the SPA and asserts its
four outputs before `include_bytes!`. What changed is that the build script now
runs `npm run build` (tsc + Vite) instead of `tsc` plus two `cp`s.

**Other side:** action required — after pulling, run
`npm ci --ignore-scripts --no-audit --no-fund --prefix web/prism` once, or
`cargo build` will fail with *dependencies are missing*. CI already does this;
only local checkouts need it.

---

## 2026-08-11 · Claude Code · `img-src 'self' data:` in the embedded UI CSP

**Touched:** `crates/gateway-http-actix/src/management_ui_resources.rs`,
`crates/gateway-http-actix/tests/p10_09_embedded_management_ui.rs`

**Why:** the panel's glass lens generates its Snell displacement maps at runtime
with `canvas.toDataURL("image/png")`
(`web/prism/src/components/glass/PrismLens.tsx:113`) and feeds them to an SVG
`feImage`. Under `img-src 'self'` the browser blocked all three and the lens
silently degraded — caught only by running the embedded build, since the data
URIs do not exist in the bundle for a static scan to find.

`data:` is admitted **for images only**. A data: image cannot execute; the
dangerous data: sinks are `script-src` and `object-src`, both still `'none'` /
`'self'`. The SPA's own meta CSP already declared `img-src 'self' data:`, so the
two policies now agree instead of the header silently overriding the meta.

**Other side:** FYI — no action. If you tighten this back, the lens breaks and
the failure is silent in the console, not in any test.

---

## 2026-08-11 · Claude Code · Contract re-synced down to `main`'s version

**Touched:** nothing backend-owned — recorded because it is easy to misread.

**Why:** Prism's vendored contract copy was taken from a Codex feature branch
that carries P13-05 (76 operations, 81 KB). `main` has P13-04 (72 operations,
68 KB). The merge re-synced Prism to `main`, which is correct: Prism calls no
P13-05 operation, so nothing was lost.

**Other side:** FYI — when P13-05/P13-06 merge to `main`, the frontend will pick
them up with `sync-contract`. No coordination needed.

---

## 2026-08-11 · Claude Code · 计划与流程合并

**Touched:**
- `docs/06-development-plan.md` — 新增 §19.2b(前端挂接点),**未改动 P13 任务表**
- `docs/adr/ADR-0071-management-spa-generated-client-build.md` — 顶部加"部分被取代"注记
- `docs/contracts/BC-MGMT-004-management-spa-generated-client.md` — 同上
- `docs/08-management-frontend-development-plan.md`(前端侧,整篇重写)

**Why:** 合并后两侧各有一套计划,而它们互相不知道对方的存在。docs/08 旧版整篇描述的是
独立仓时期的一套**未落地**选型(ECharts / zod / CSS Modules / 升级后端生成器)与按
**提案版 G1/G3 形状**写的组件规格 —— 后端最终实现了不同形状,照旧规格建会第二次白干。

ADR-0071 与 BC-MGMT-004 是 P10-03 的 Accepted 记录,描述的机制(`generate-management-client.mjs`、
`web/admin-ui`)已随合并删除。**没有改写这两份记录的决策内容** —— 决策仍然成立(生成客户端是
唯一 API 通道,唯一输入是契约),只是实现位置变了,所以只加注记不改正文。

**Other side:** FYI — §19.2b 只是指针与解锁关系表,P13 任务表、状态、Gate 全部未动。
若你认为前端计划应当反过来并入 docs/06,请在此日志回一条,我来搬。

---

## 2026-08-12 · Claude Code · 后端 bug:重复 id 建端点返回 500 而非 409

**Touched:** 无 —— 未改后��代码,只报告。

**Why:** 接子资源 CRUD 时,对真网关(`serve`,本地库)用**已存在的 id** 再建一次端点:

```
POST /admin/upstreams/up-grok/endpoints   {"id":"ep-t1", ...}
→ 500 {"error":{"code":"management_internal_error","message":"Management operation failed"}}
```

首次创建同一载荷返回 201,`models_path` 取 null / 字符串 / 省略三种写法也都 201 ——
**唯一变量是 id 重复**。重复 id 是客户端错误,其他资源(如 access-groups、config-versions)
在同样情况下返回 `409 management_lifecycle_conflict`。

面板因此只能显示 "Management operation failed",无法告诉运维"这个 id 已经存在"。

**Other side:** action required(低优先级)—— 期望与既有惯例一致返回 409。
凭据/绑定的重复路径未逐一验证,可能是同一处的问题。
前端已按 409 语义写好文案,后端改过来即可自动生效。

---

## 2026-08-15 · Codex · P13-05/P13-06 management contract synced into Prism

**Touched:**
- `docs/openapi/management-v1.json` — the authoritative P13-05/P13-06A backend contract was
  brought into this integration branch; P13-06B also aligns the Provider account-pool numeric
  bounds with the existing runtime scheduler domain. This documentation closeout did not
  hand-edit the contract.
- `web/prism/contracts/management-v1.json` — updated only by
  `npm --prefix web/prism run sync-contract` from the authoritative backend contract.

**Why:** the integration branch combines the P13-05/P13-06A backend work with the current Prism
tree. Keeping Prism's vendored contract byte-aligned prevents the frontend check from consuming
the older P13-04 snapshot while the backend exposes the protected billing and Provider account-pool
operations.

**Other side:** FYI — no manual action required. Prism's generated client is already current; do
not hand-edit `web/prism/contracts/management-v1.json` or `web/prism/src/generated/`.

---

## 2026-08-15 · Codex · Prism management-key loader made an explicit closure

**Touched:** `web/prism/src/api/client.ts`

**Why:** the existing client passed `readManagementKey` directly as a callback, which is valid at
runtime, but the repository's value-free tracked-secret scanner conservatively interpreted the
long bare identifier after `managementKey:` as a possible literal. The equivalent zero-argument
closure keeps the generated-client fetch seam and session ownership unchanged while allowing the
scanner to distinguish executable code from a credential value. No credential, default, fixture,
or request behavior was added.

**Other side:** FYI — no action required. This is a one-line semantic no-op required to restore the
existing documentation/secret gate without weakening its detection pattern.

---

## 2026-08-15 · Codex · P13-06C operator actions and failure-feedback contract synced

**Touched:**
- `docs/openapi/management-v1.json` — added the authoritative P13-06C action and
  failure-feedback operations.
- `web/prism/contracts/management-v1.json` — refreshed only through
  `npm --prefix web/prism run sync-contract`.
- `web/prism/src/generated/management-client.ts` — regenerated by that same command; not hand-edited.

**Why:** P13-06B exposes the live Provider account-pool inventory but operators cannot yet apply a
bounded exact-account cooldown/recovery action or inspect safe durable failure attribution. P13-06C
adds those backend operations without changing the serving route graph or Provider executors.

**Other side:** action required — contract sync and generated API methods are complete. Claude Code
should add the Prism state/UI integration for `applyProviderAccountPoolAction` and
`listProviderAccountFailures`, including explicit confirmation for cooldown/recovery, safe display
of the closed error classification, pagination, and 409 stale-target handling. Do not hand-edit the
generated contract/client. No formal UI is part of this backend slice.

---

## 2026-08-15 · Codex · P13 phase gate closeout and P13-07A kickoff

**Status:** P13-04/P13-05/P13-06A/B/C passed the single formal Delivery Gate run
`31858904767` for exact revision `a22f312`; these backend slices are now `DONE_WITH_BOUNDARY`.

**Next backend slice:** P13-07A starts a Provider-scoped deterministic routing selector and Route
Explain projection. No Prism source, OpenAPI contract, generated client, or frontend route was
changed in this kickoff. If the selector changes the Route Explain response shape, Codex must first
sync `docs/openapi/management-v1.json` into Prism and add a new entry here; Claude Code should not
hand-edit generated client output.

---

## 2026-08-15 · Codex · P13 phase-gate compatibility checks aligned with Prism

**Touched:**
- `scripts/check-management-spa.mjs`
- `scripts/test-p12-02-serve.sh`

**Why:** the repository-wide gate still invoked the deleted `web/admin-ui` checker and asserted
the pre-Prism document title. The root compatibility command now validates the authoritative
OpenAPI contract against Prism's vendored copy and delegates source/CSP/generated-client and
reproducible-build checks to `web/prism/scripts/check.mjs`. The serve envelope test now checks the
current `Prism · Gateway Management` title without a `curl | rg -q` pipefail/SIGPIPE false
failure.

**Other side:** FYI — no `web/prism/**` source, generated client, or contract file was edited;
no frontend action is required. The existing P13-06C wrappers remain available for the planned
Prism runtime-page/operator-action integration.

---

## 2026-08-15 · Codex · P13-07A routing policy seam completed without API drift

**Touched:** `crates/gateway-router/src/provider_scoped_selector.rs` and backend-only routing
documentation. No `docs/openapi/management-v1.json` or `web/prism/**` file changed.

**Why:** P13-07A freezes a side-effect-free Provider-scoped cost/quota/load ranking policy before
it is connected to the serving scheduler or management Route Explain surface.

**Other side:** FYI — no Claude Code action is required for P13-07A. P13-07B will perform the
composition/scheduler/Route Explain integration; if that changes the management response shape,
Codex must update the authoritative OpenAPI contract and sync Prism before requesting UI work.

---

## 2026-08-15 · Codex · P13-07B Provider-scoped Route Explain composition

**Touched:**
- `crates/gateway-router/src/route_explain.rs`
- `crates/gateway-router/src/credential_scheduler.rs`
- `apps/gateway/src/runtime.rs`
- `crates/gateway-http-actix/src/management_resources.rs`
- `docs/openapi/management-v1.json`
- `web/prism/contracts/management-v1.json`
- `web/prism/src/generated/management-client.ts`

**Why:** P13-07B now projects the existing, shared Route/Credential Health/Quota observations
through the P13-07A Provider-scoped deterministic selector. The management facade receives the
same read-only scheduler/pool assembly used by serving; no second lease owner, cursor, Provider
request, cost inference, or cross-Provider fallback is introduced. Route Explain accepts an
optional exact `provider_id`; omission is inferred only for a single-Provider Route, while a
multi-Provider Route fails closed with `provider_scope_required`.

**Other side:** action required — the response shape is unchanged, but Prism's generated contract
now includes the optional `provider_id` query parameter. Claude Code should expose an explicit
Provider selector when a Route contains multiple Providers and render the new safe reason values
`provider_scope_required` and `provider_mismatch`; do not hand-edit the generated contract/client.
No formal UI implementation is part of this backend slice.

---

## 2026-08-15 · Codex · P13-07D Config-Version-bound routing price evidence

**Touched:**
- `crates/gateway-http-actix/src/management_resources.rs`
- `docs/openapi/management-v1.json`
- `web/prism/contracts/management-v1.json` and generated client (synced from the authoritative contract)
- `web/prism/scripts/generate-client.mjs` and `web/prism/scripts/check.mjs` (PUT support and drift guard)
- `scripts/check-management-spa.mjs` (root compatibility checker now includes PUT)

**Why:** P13-07D binds an immutable billing catalog and the closed `rate_dominance_v1`
comparison to the selected Config Version. Serving and Route Explain now share six-dimensional,
secret-free price evidence; no token estimate or scalar request-cost guess is exposed. The
protected management surface adds read/set/clear policy operations and Route Explain returns a
required nullable policy lineage (`null` means disabled) plus one closed candidate evidence value
per candidate. The authoritative OpenAPI and generated Prism contract/client are synchronized.

**Other side:** action required — Claude Code should sync Prism's generated contract/client and
add only a display/control surface for catalog lineage and the closed evidence values
(`dominant`, `equal`, `dominated`, `incomparable`, `unpriced`, `not_evaluated`, `disabled`). Do not
calculate prices in Prism, edit generated files by hand, or change public inference protocols. Keep
the PUT support and regression guard in the two listed Prism scripts aligned with the authoritative
OpenAPI.

---

## 2026-08-15 · Codex · P13-08 protected Channel Pin contract

**Touched:**
- `crates/gateway-http-actix/src/management_resources.rs`
- `crates/gateway-http-actix/tests/p10_01_management_openapi_contract.rs`
- `docs/openapi/management-v1.json`
- `web/prism/contracts/management-v1.json` and generated client (synced from the authoritative contract)

**Why:** P13-08 adds a management-only `POST /admin/operations/channel-pin` seam for one exact
Provider/Channel/Route/Credential diagnostic in JSON or SSE mode. The handler validates the selected
Config Version graph, requires `X-Config-Version` plus `If-Match`, and records value-free request and
pre-execution `channel_pin_started` audit actions before any Provider call. The returned receipt is
the terminal source; no post-send audit append is performed. The executor is an injected fail-closed seam; this first slice admits only
generic OpenAI Chat/Responses and Anthropic Messages Canonical/bridge candidates. `NativeExact` and
native Grok Console/Web adapters with hidden bootstrap/refresh HTTP are rejected before lease/network.
Admitted candidates may send at most once, with no retry, quota-recovery fallback, or cross-Provider
fallback. The operation is not part of the public inference API and does not change Config Version
revision; at most two pins are in flight and the bounded drain is 45 seconds idle/total with 4096
events.

**Other side:** action required — Claude Code should sync the generated Prism contract/client and,
if a management control is later exposed, render only the bounded target fields and receipt state;
the UI must collect the existing Config Version and revision preconditions, never construct a
provider request or expose native-adapter controls in this slice.
Do not add arbitrary prompt/body controls, echo upstream response data, calculate retry decisions in
the frontend, or hand-edit generated files. No formal UI implementation is included in this slice.

---

## 2026-08-16 · Codex · P13-10A public Responses WebSocket client surface

**Touched:**
- `README.md`
- `apps/gateway/src/runtime.rs`
- `crates/gateway-http-actix/src/lib.rs`
- `crates/gateway-http-actix/src/stored_response_continuity.rs`
- `crates/protocol-openai-responses/src/lib.rs`
- `crates/gateway-router/src/lib.rs`
- `crates/gateway-router/src/attempt_orchestrator.rs`
- `crates/gateway-router/src/execution_lineage.rs`
- `crates/gateway-router/src/protocol_transform.rs`
- `crates/gateway-catalog/src/lib.rs`
- `docs/01-feature-selection-matrix.md`
- `docs/02-behavior-contracts.md`
- `docs/adr/ADR-0092-public-responses-websocket.md`
- `docs/contracts/BC-RESP-004-public-responses-websocket.md`

No `docs/openapi/management-v1.json` or `web/prism/**` file changed.

**Why:** P13-10A adds the public native-client `GET /v1/responses` WebSocket upgrade. It accepts
strict text-only flat `response.create` events and emits the existing OpenAI Responses lifecycle as
JSON text messages while reusing Client Key auth, Canonical execution, runtime lease, usage,
stored-response durability and exact continuation. It is not the Realtime API. Downstream
WebSocket is independent of upstream Provider transport and requires an explicit runtime
`responses_websocket` capability.

**Other side:** action required for documentation/client integration only — Claude Code should
recognize that the same public base URL now supports WebSocket upgrade on `GET /v1/responses`, use a
native client without `Origin`, and send `response.create` rather than Realtime events. Do not add
a Prism management control, edit management generated clients, assume browser support, or expose
`response.append`, Chat/Messages WebSocket, binary/media, or Provider-native upstream WebSocket in
this slice. Management OpenAPI and the existing Prism API contract remain unchanged.

---

## 2026-08-17 · Codex · P13-11D2 compatible egress management API

**Touched:**
- `crates/gateway-control/src/management_mutation_service.rs`
- `crates/gateway-http-actix/src/management_resources.rs`
- `crates/gateway-http-actix/tests/p10_01_management_openapi_contract.rs`
- `crates/gateway-http-actix/tests/p10_04_management_resources.rs`
- `crates/gateway-store/src/control_plane.rs`
- `docs/openapi/management-v1.json`
- `web/prism/contracts/management-v1.json` and generated client (synced from the authoritative contract)

**Why:** P13-11D2 adds protected, revision-guarded management CRUD for compatible proxy pools,
compatible proxy nodes, and exact Endpoint-Credential egress bindings. Node endpoints are accepted
only for immediate local-DNS SOCKS5 validation and AEAD sealing; responses and audit rows expose
only bounded identities, policy, capacity, and `proxy_configured`, never endpoint URLs, ciphertext,
key versions, credentials, or request bodies. Mutations remain draft-only, If-Match guarded, and
audited atomically; no Provider, proxy, DNS, server, or production request is made.

**Other side:** action required — Claude Code should sync the generated Prism contract/client and
may add only management controls that select existing Upstream/Pool/Node/Endpoint-Credential IDs,
show revision/ETag conflicts, and render the secret-free response fields. Never echo or persist the
write-only `proxy_endpoint`, construct transport requests in the browser, or hand-edit generated
files. No public inference protocol changed in this slice.

---

## 2026-08-18 · Codex · P13-11E4 provider-specific egress status projection

**Touched:**
- `crates/gateway-http-actix/src/management_resources.rs`
- `crates/gateway-http-actix/tests/p13_11e4_management_egress_status.rs`
- `docs/openapi/management-v1.json`
- `web/prism/contracts/management-v1.json` (synchronized from the authoritative contract)
- `web/prism/src/generated/management-client.ts` (regenerated from the synchronized contract)
- `crates/gateway-http-actix/tests/p10_01_management_openapi_contract.rs`

**Why:** P13-11E4 defines the protected read-only `GET /admin/operations/provider-egress-status`
projection. The contract binds exactly one `X-Config-Version`, bounded filters and cursor
pagination, and a closed `oneOf` for independent `egress`, `session`, and `clearance` rows. It
contains only provider/channel identities, opaque target IDs, closed states, safe revisions, and
bounded timestamps; it has no request body, `If-Match`, audit action, endpoint URL, proxy detail,
credential value, cookie, or recovery/refresh action.

**Other side:** action required — Claude Code should consume the generated
`listProviderEgressStatus` client method and render the three domains as separate views. The UI
must send the selected `X-Config-Version`, preserve the opaque cursor and handle a 409 snapshot
conflict by restarting the read. Do not infer a combined health value, fixed proxy/pool semantics,
or add action buttons; do not hand-edit `web/prism/contracts/management-v1.json` or
`web/prism/src/generated/management-client.ts`. Empty Web or clearance rows mean the exact source
is absent; they do not mean healthy, available, fresh, tested, or production-ready.

---

## 2026-08-18 · Claude Code · 前端计划重排为 v0.4(后端 P13 收口后)

**Touched:** `docs/08-management-frontend-development-plan.md`(仅本文件;无 `web/prism/**` 之外的源码改动)

**Why:** P13-04…P13-10 全部 `DONE`、P13-11A–E4 `DONE_WITH_BOUNDARY` 之后,v0.3 的"挂在哪个后端任务下"排序依据失效 —— 没有一项前端工作在等后端。重排后的两条依据来自本次核实:

1. 契约 99 个算子中 54 个已接线,45 个未接;
2. `src/api/proposed.ts` 的 `analyticsAvailable()` 在生产构建恒为 `false`,依赖它的用量页 / 监控页 / Overview 分析半区 / 六个图表组件共 **3479 行(占 src 20.6%)在真网关上只渲染空态**。后端最终实现的是 `operations/usage` + `operations/billing`,形状与当初提案的 G3 不同。

**同时记录一处正在失效的接线(前端自己的欠账,非后端问题):** P13-07B/D 之后 `explainRoute` 响应新增必填 `price_policy` 与每候选必填 `price_evidence`,并新增可选 `provider_id`;多 Provider 路由在不传 `provider_id` 时 fail closed(`provider_scope_required`)。`web/prism/src/features/runtime/RuntimePage.tsx:608` 仍只传 `requested_model` 与 `protocol`,且全仓无 `price_evidence` / `price_policy` 渲染。已排为批 A 首项修复。

**Other side:** FYI — 无需 Codex 行动。docs/06 的 P13 任务表未改动,前端不修改其任务状态。

一条可能对后端有用的观察:`sync-contract` + `check.mjs` 的漂移门禁保证 `contracts/` 与 `src/generated/` 跟契约一致,但**响应体新增必填字段时,调用点不渲染它,类型检查与门禁全部照过**。上面那处失效就是从这个盲区漏出来的。目前只能靠读本日志的 action-required 条目补。

---

## 2026-08-18 · Claude Code · 批 A 交付:路由候选/校验 + Route Explain 补齐

**Touched:** 仅 `web/prism/**` 与 `docs/08-management-frontend-development-plan.md`。
**未改动任何后端源码、契约或生成物。**

**Why:** 消化两条 action required —— P13-07B(`provider_id` 与
`provider_scope_required` / `provider_mismatch`)与 P13-07D(`price_policy` 血缘、
七值 `price_evidence`);并补上配置链断口:面板能建路由却加不了候选,而零候选路由被
`management_mutation_service.rs:2074` 判为 `route_missing_active_candidate`,
草稿因此进入面板自己修不回来的状态。

新接线算子:`createRouteCandidate` `validateRoute` `getRoute` `updateRoute`
`deleteRoute`,以及 `explainRoute` 的 `provider_id` 参数与两个新响应字段。
契约接线率 54/99 → 59/99。

**Other side:** FYI — 无需 Codex 行动。以下三条是实施中对后端行为的观察,供参考:

1. **`explainRoute` 在草稿版本上必然 503。** `apps/gateway/src/runtime.rs::explain_route`
   第一步 `snapshot_for(config_version_id)`,而编译快照只在发布后存在。这与
   "本部署未接线"在协议层完全无法区分,面板此前把两者都说成后者。现已按当前版本
   status 分开文案。**若后端认为这两种情况值得在错误码上分开,那会是一处真正的改进;
   不分开的话前端这样处理也能工作。**

2. **`explainRoute` 的 protocol 枚举有三个值,前端此前只列了两个。**
   `openai_chat_completions` 随 P12-08 进契约后前端从未跟进,Chat Completions
   路径在面板里一直无法解释。已补齐。

3. **漂移门禁的盲区(前端侧问题,记在这里是因为它决定了 action-required 条目的价值):**
   `sync-contract` + `check.mjs` 保证 vendored 契约与生成客户端一致,但
   **响应体新增必填字段时,只读的调用点不渲染它、类型检查照过**;
   **页面漏掉一个 enum 字面量**同理不可见。上面第 2 条与 P13-07D 的两个字段都是从
   这个缝里漏出来的。目前唯一的补救是读本日志的 action required 条目 ——
   所以那些条目请继续写,它们不是形式。

**门禁:** 176 单测 · 55 E2E · `check:full`(含双构建字节一致)·
`cargo build -p gateway` · 嵌入测试 3/3 · 真网关闭环(全新 state-dir,
从 `/admin-ui/` 打开):建路由 → validate `route_missing_active_candidate`
→ 加候选 → validate 通过,零失败请求。

---

## 2026-08-18 · Claude Code · 批 B1 交付:用量分析页接到 operations/usage

**Touched:** 仅 `web/prism/**` 与 `docs/08-management-frontend-development-plan.md`。
**未改动任何后端源码、契约或生成物。**

**Why:** 旧用量页按提案的 G3 分析形状建成,数据源在生产构建里恒为不可用 ——
它从未在真网关上渲染过数据。现改接 `listOperationalUsage`(P13-04B)。
连带删除六个只为服务端时间桶而存在的图表组件:净 -2797 / +1196 行。

**Other side:** FYI — 无需 Codex 行动。两条对后端行为的核对结果,供参考:

1. **运营面算子的版本作用域是分裂的**,而且这个区分很容易被前端搞反:

   | 声明 `X-Config-Version` | 不声明 |
   |---|---|
   | `listOperationalAccountPools` | `listOperationalUsage` |
   | `listProviderEgressStatus` | `listOperationalBilling` |
   | `listBillingCatalogs` | `listProviderAccountPools` |
   | | `listRequestAttempts` |

   语义上讲得通(观测跨版本、配置绑版本),这里只是记下来,免得下次又搞反。

2. **`listOperationalUsage` 没有服务端时间桶。** 一行是整个窗口的聚合。
   前端要画趋势只能发 K 个窗口的查询,且每个窗口都得跟游标才不少算 ——
   代价是 K×页 次请求。**目前的决定是不画,并在页面上写明原因。**
   若后端将来考虑加时间桶参数,那会显著改变这一页能提供的东西;
   在此之前前端不打算用拼接近似它。

**门禁:** 160 单测 · 57 E2E · `check:full`(含双构建字节一致)·
真网关验证(不选配置版本,`GET /admin/operations/usage` 200,空态与水位正确,零控制台错误)。
非空渲染因离线部署无法产生真实用量,仅 fixture 下验证。

---

## 2026-08-18 · Claude Code · 批 B2 交付:请求监控页按真实契约重设计

**Touched:** 仅 `web/prism/**`。**未改动任何后端源码、契约或生成物。**

**Why:** 旧监控页的 KPI 是 P50/P95 延迟与成功率,契约里一样都没有,所以不能接线,
只能重设计。现基于三条真实来源:`listOperationalBilling`(账本)、
`listProviderAccountFailures`(失败归因)、`listRequestAttempts`(单请求尝试轨迹)。

**Other side:** FYI — 无需 Codex 行动。三条核实结果记录在此:

1. **`listOperationalBilling` 的 `summary` 覆盖整个筛选集,不是当前页。**
   已在 `management_operations_service.rs` 确认:累加循环跑在游标 `retain` 与
   `truncate(limit)` 之前,并由 `snapshot_ledger_id` 钉住快照。前端因此敢在只加载
   第一页时展示整窗口的计价可信度 KPI。**这条性质很有用,请勿在后续重构中改掉;
   若必须改,请在本日志标 action required。**

2. **`listOperationalBilling` 的 `status` 参数取值是 `exact|partial|unknown|unpriced`
   (计价置信度),而不是请求成败。** 参数名极易被读成后者 —— 前端界面上已改称
   「计价置信度」并加了回归断言。**这只是记录,不是改名请求**:契约既已冻结,
   改名的代价大于收益。

3. **`cost_microunits` 与目录费率字段都没有币种声明。** 前端因此只显示 microunits,
   不折算、不加货币符号,导出文件同理。若将来引入币种,那是一处需要 action required
   的契约变更。

一个仍然存在的观测缺口(不是请求,只是说明前端为什么不画某些东西):
契约没有任何延迟字段,也没有"请求成败"清单 —— 账本与失败流不是同一总体的两半,
前端不会用它们相除产出成功率。

**门禁:** 180 单测 · 63 E2E · `check:full`(含双构建字节一致)·
真网关验证(账本无版本 200、失败归因正确索要版本后 200、零控制台错误)。

---

## 2026-08-19 · Codex · 公共源码镜像的 Prism 构建根目录解析

**Touched:** `scripts/build-management-spa.sh`、`.dockerignore`、
`deploy/docker/Dockerfile`、`deploy/docker/compose.yml`、双语 README/部署指南。
未修改 `docs/openapi/management-v1.json`、Prism vendored contract、generated client 或任何
`web/prism/**` 源码。

**Why:** 公共源码镜像有意从 Docker build context 排除 `.git`，原脚本使用
`git rev-parse --show-toplevel` 会使 Cargo build-script 在镜像 builder 中失败。脚本现在只根据
自身的绝对目录定位仓库根；仍执行同一个 `npm --prefix web/prism run build`，仍由
`crates/gateway-http-actix/build.rs` 校验同样四个嵌入资产。

**Other side:** FYI — Claude Code 无需改前端。没有 API/schema/runtime UI 行为变化；当前未提交的
Prism 工作保持原样。最终 integration review 应继续运行 `npm --prefix web/prism run check` 与嵌入
资源测试。对应提交应带 trailer：

`Cross-Boundary: scripts/build-management-spa.sh`

---

## 2026-08-18 · Claude Code · 批 B3 交付:计费与价格目录页(全新)

**Touched:** 仅 `web/prism/**`。**未改动任何后端源码、契约或生成物。**

**Why:** P13-05C 的目录导入/列出/回滚与 P13-07D 的路由价格策略此前完全没有控制面。
新增 `/billing` 页,接入全部六个算子:`listBillingCatalogs` `importBillingCatalog`
`rollbackBillingCatalog` `getRoutingPricePolicy` `setRoutingPricePolicy`
`clearRoutingPricePolicy`。契约接线率 66/99。

**Other side:** FYI — 无需 Codex 行动。四条核实结果:

1. **目录是全局的,策略是按版本的。** `list_billing_catalogs_bounded()` 不接版本参数,
   `X-Config-Version` 只用于回传 revision;而 `upsert_routing_price_policy` 写在版本上。
   前端把"在草稿里导入目录会影响所有版本"明写在页面上 —— 这是本页最容易被误读的一点。

2. **`getRoutingPricePolicy` 未配置时返回 `404 management_resource_not_found`。**
   前端按状态而非错误渲染,并**同时匹配错误码**:因为
   `404 management_access_denied`(不被允许的浏览器 origin)会触发会话重置,
   只看状态码会把它吞掉。**这两个 404 语义不同、都走 404,是一处值得注意的重叠。**

3. **`entries` 的 `minItems` 是 1** —— 空目录非法,所以"清空价格"不可表达;
   改价只能导入新目录。前端已按此设计,不提供"删除目录"入口(契约也没有)。

4. **`set_routing_price_policy` 拒绝未生效目录**(`RoutingPriceCatalogNotEffective`)。
   前端的绑定选择器只列已生效目录,未生效的在列表里标注但不可选。

**门禁:** 200 单测 · 71 E2E · `check:full`(含双构建字节一致)·
**真网关完整写循环**:404 未配置 → 201 导入 → 200 绑定 → 204 清除 → 404,零控制台错误。
(这是批 B 第一个不依赖流量、因而能在离线部署上完整验证写路径的页面。)

---

## 2026-08-18 · Claude Code · 批 B4+B5 交付:Overview 收口,提案通道删除并加门禁

**Touched:** 仅 `web/prism/**`。**未改动任何后端源码、契约或生成物。**

**Why:** Overview 的分析半区是最后一处按提案 G3 形状建成、在生产里只渲染
"尚未接线"卡片的代码。改为:计价可信度 KPI(取自 `listOperationalBilling` 的
whole-window summary,一次请求即可准确)+ 一张说明"为什么没有趋势线"并指向
用量分析 / 失败归因的卡片。

随后删除 `api/proposed*` 与其全部消费者,并在 `web/prism/scripts/check.mjs`
新增一条门禁:**`src/**` 不得 import 提案端点通道**。契约是端点的唯一来源;
形状缺失时的正确路径是 `docs/change-requests/` 加诚实空态,而不是一条只在 dev 应答的第二客户端。

清理总账:生产死代码 3479 行 → 0;`components/data` 1243 行 → 141 行。

**Other side:** FYI — 无需 Codex 行动。一条与后端相关的说明:

Prism 现在**不再为任何未交付的契约形状保留占位实现**。若将来后端新增服务端时间桶、
延迟观测或请求成败清单,前端不会"恢复"旧代码(已删除),而是按当时的真实形状重建 ——
这正是当初照提案建 UI 所付出的代价想要避免的重演。有此类新增时请照常在本日志留痕。

**门禁:** 196 单测 · 71 E2E · `check:full`(含双构建字节一致)·
新门禁已用临时违规文件验证过确实会失败(FAILED),移除后恢复 OK ——
未验证过会失败的门禁等于没有门禁 ·
真网关验证:Overview 整页无任何"等待未来契约"的卡片,零控制台错误。

---

## 2026-08-20 · Claude Code · 批 C1 交付:Provider 账号池实时视图与操作

**Touched:** 仅 `web/prism/**`。**未改动任何后端源码、契约或生成物。**

**Why:** 消化 P13-06C 的 action required(`applyProviderAccountPoolAction` 与
`listProviderAccountFailures` 的 UI 集成)。失败归因已在批 B2 接入监控页;
本次补上实时账号池表与两个精确到账号的操作,含确认、闭集回执与 409 陈旧目标处理。
契约接线率 68/99。

**Other side:** **action required(低优先级,后端可自行决定是否处理)** ——
一处与本仓其余投影不一致的错误映射:

```rust
// crates/gateway-http-actix/src/management_resources.rs:8071
ProviderAccountPoolError::InvalidSnapshot | ProviderAccountPoolError::SourceUnavailable => {
    internal_error()   // 500
}
```

`listProviderAccountPools` 在**来源未接线**时返回 **500 management_internal_error**,
而本网关其余注入式投影(`getRuntimeAvailability`、`getCatalogStatus`、`explainRoute`)
在同样情形下返回 **503**。前端据 503 判定「此部署未启用该投影」,因此账号池未接线时
会被显示成一个普通内部错误,运维会去排查一个并不存在的故障。

前端不做猜测(500 也可能是真错误),已在错误文案里同时写出两种可能。
**若后端认为 `SourceUnavailable` 应与其他投影一致改为 503,前端无需改动即可自动正确分类;
若维持现状也可工作。** 请按你们的判断处理,改动时在此留痕即可。

另记录一条已核实的作用域事实(非请求):`listProviderAccountPools` 不带
`X-Config-Version` 而 `applyProviderAccountPoolAction` 带,且后者**没有 If-Match**
(作用于运行时而非配置)。前端已按此设计:未选版本时表可读、操作按钮禁用并说明原因。

**门禁:** 202 单测 · 76 E2E · `check:full` · 真网关验证(未选版本时池卡片可读、
作用域说明与 500 歧义提示均正确呈现,零 pageerror)。

---

## 2026-08-21 · Claude Code · 本轮收尾:计划文档进度追记与接线率订正

**Touched:** `docs/08-management-frontend-development-plan.md`(前端自己的计划文档,
位于 `web/prism/**` 之外故留痕)。**无代码改动,无契约改动,无生成物改动。**

**Why:** 批 A / B / C1 已交付并推送,计划文档仍停在制定日的状态。新增 §3.0 记录
批次进度、提交号,以及**实施中发现但计划原文没有的八条契约事实**(版本作用域逐算子
split、`404` 需连 code 一起判、`null` 不得当 0 求和、billing `summary` 在截断前算好、
`status` 是计价置信度、原生约束校验先于自写校验等),使下一轮可冷启动接手 C2。

**订正一处此前记错的数字:契约接线率是 71/99,不是 2026-08-20 那条记的 68/99。**
统计脚本只匹配单行 `call<T>("op")`,漏掉跨行调用与 `callText` 路径,
`createRouteCandidate` / `refreshCredentialOAuth` / `getObservabilityMetrics`
三个其实均已接线。正确口径(带引号的字面量)已写进计划文档 §1.2。
剩余 28 个未接线恰好等于 C2(1)+ C3(15)+ 批 D(10)+ 明确不做(3),无游离项。

**Other side:** **无需任何动作。** 2026-08-20 那条 500 vs 503 的 action required
仍然有效,状态不变。

**门禁:** 文档变更,代码门禁状态维持上一提交(202 单测 · 76 E2E · `check:full` ·
真网关验证)。

---

## 2026-08-21 · Claude Code · 批 C2 交付:Provider egress 状态三分区只读视图

**Touched:** `web/prism/**` 与本仓两份文档(`docs/08-…`、本文件)。
**未改动任何后端源码、契约或生成物。**

**Why:** 消化 P13-11E4 的 handoff。按 `egress` / `session` / `clearance` 分区展示,
传 exact `X-Config-Version`,opaque cursor 原样回传,不合成 overall health,
不加任何 action,空的 Web / clearance 明确写成"该来源不存在,不等于健康"。
契约接线率 71 → 72/99。

**一处对 handoff 的技术性偏离(更严格,不是放松):** handoff 说"分区展示",
我实现成**三次独立读取**(每次带 `domain=`)而不是一次混读再切分。原因是三个域共用一个
分页流:一台有 100+ 条 egress 行的部署,混读第一页里会一条 session 行都没有,
而空态那句"该来源不存在"就变成了假话。代价是三个快照可能不同步,已在每个分区标注各自的
`snapshot_id` 与采样时刻,并在卡头写明。

**另记录一条已核实的契约事实(非请求):** 本投影的两个 `409` 语义不同 ——
`..._cursor_conflict` 是运行时快照轮换(从头重读有效),`..._config_conflict` 是所选版本
不是该快照来源(从头重读无效)。前端已分别渲染。顺带说明:本投影的
`SourceUnavailable → 503` 与本仓其余投影一致,**没有** 2026-08-20 记的账号池那种不一致。

**实测边界(前端侧,供后端知情):** 一台刚启动、建了配置版本但未导入任何 Provider 凭据的
网关,三个域全部返回 `503 management_runtime_unavailable`。这与"投影未接线"在协议层无法
区分,前端不猜,把两种可能都写在卡片上。因此**行渲染、翻页与游标冲突恢复只在 fixture 下
验证过** —— 离线部署导不了 Provider 凭据,无法产生真实行。

**Other side:** **无需任何动作。** 2026-08-20 那条账号池 500 vs 503 的 action required
仍然有效,状态不变。

**门禁:** 211 单测 · 82 E2E · `check:full` · 真网关验证(三分区渲染、503 走"投影未启用"
状态、无 action、无 overall health、外壳未误弹配置冲突横幅,零 pageerror)。

---

## 2026-08-21 · Claude Code · 批 C3 交付:兼容出口代理池 / 节点 / 绑定 CRUD

**Touched:** `web/prism/**` 与本仓两份文档(`docs/08-…`、本文件)。
**未改动任何后端源码、契约或生成物。**

**Why:** 消化 P13-11 A–D 的 15 个算子,挂在出口策略页下方(策略说"能去哪",
这三层说"从哪条线出去")。接线率 72 → 84/99。

**三条已核实的契约事实(非请求,仅留痕):**

1. **`CompatibleProxyNodeUpdateInput.proxy_endpoint` 的更新语义与 `CredentialInput.secret`
   相反。** 前者"省略或 null 保留现有封存值",后者 PATCH 必填。两者都只写不回显,
   但把凭据表单那句"必须重新输入"抄过来,就是让运维重打一个正在工作的代理地址。
   前端已按各自真实语义分别措辞。
2. **`proxy_configured` 在 `management_mutation_service.rs:378` 硬编码为 `true`。**
   它是常量不是观测,永远不会返回 false。前端保留 boolean 渲染(契约声明的是 boolean),
   但文案不让它读起来像"面板验证过这个代理可达" —— E5 真实网络仍未授权。
3. **`target_id` 按 `target_kind` 取自两个不同命名空间**,后端按整对匹配,
   `direct` + 任意 id 与 `proxy_pool` + 无 id 同样是 400。前端按 kind 切换候选列表,
   `direct` 不渲染该字段。

**三个 `getCompatibleProxyPool` / `ProxyNode` / `EgressBinding` 故意未接:**
三个读模型都已完整(池与绑定的读模型等于输入模型;节点缺的 `proxy_endpoint` 是只写的,
GET 同样不返回),逐行再拉一次拿不到新东西。归入批 D3 的详情抽屉。

**Other side:** **无需任何动作。** 2026-08-20 那条账号池 500 vs 503 的 action required
仍然有效,状态不变。

**门禁:** 221 单测 · 90 E2E · `check:full` · **真网关全链路验证**(建池 → 空池仍可见 →
非 socks5 被表单拦下 → 合法地址被网关接受并封存 → 全页面任何位置都搜不到该地址 →
留空编辑后网关保留了封存地址 → 被引用的池删除被网关拒绝且面板提前预测)。
本批是 C 批里**唯一能在离线部署上端到端验证的**,因为它是普通配置资源,不依赖任何 Provider 来源。

---

## 2026-08-21 · Claude Code · 批 D 收尾:Client Key 编辑、Channel Pin、绑定核对、质量门

**Touched:** `web/prism/**` 与本仓两份文档(`docs/08-…`、本文件)。
**未改动任何后端源码、契约或生成物。**

**Why:** 消化批 D 的 D1 / D2 / D4 / D5。接线率 84 → **87/99,剩余 12 个全部是明确不做的**。

**一条对后端行为的核实,前端已据此加了警告(非请求):**
`update_client_key`(`management_mutation_service.rs:2440`)**无条件写入 `status`,没有任何
状态迁移检查**;而 `revoke_client_key` 的文档写明 "retaining its redacted record"。
两者合起来:把一把 `revoked` 的 Client Key 改回 `active`,**当初发出去的那串密钥会重新通过认证**。
**已在真网关上实测确认这确实会成功。** 前端不阻止它(契约允许),但在选中那一刻就明确告知
"这会让已吊销的密钥再次可用;若当初是因泄露吊销,请改为签发新的"。
如果后端认为吊销应当是终态,那是后端的决定 —— 前端现在如实呈现当前行为。

**八个单资源 GET 明确不接(`getUpstream` / `getPublicModel` / `getAccessGroup` /
`getEgressPolicy` / `getConfigVersion` / `getClientKey` / 三个 `getCompatible*`):**
核对契约后,**每一个 list 返回的 schema 与它对应的 get 完全相同**,列表行即完整记录。
`getEndpoint` 是唯一真正的反例(没有 `listEndpoints`,且运营库存不含 `base_url`),它早已接线。
接这八个会把接线率推到 95,但不会多渲染任何一个字段 —— 那是为计数器接线,不做。

**前端自身质量门补洞(与后端无关,仅留痕):**
- `.canvas` 是 `overflow-x: hidden`,因此"文档横向滚动"这类断言**永远不会失败** ——
  超宽内容不是滚出去而是**被裁掉**。改成检测"越界且无可横向滚动祖先"后,在 390px 抓出
  三页被裁(计费 +533px、出口策略 +211px、运行时 +625px),已修并加 `narrow-390` project。
- `--ink-3` 门禁此前只在同块内查字号,继承来的字号查不到;现要求本块声明 <12px 字号
  或写明 `/* ink-3: … */` 理由。全仓唯一命中处(`.secret-toggle`)确实合法,补注解而非放宽门禁。

**Other side:** **无需任何动作。** 2026-08-20 那条账号池 500 vs 503 的 action required
仍然有效,状态不变。

**门禁:** 223 单测 · 97 E2E(含新增 `narrow-390` project 2 条)· `check:full` ·
真网关验证(Client Key 从列表行直接预填并保存、revoked→active 在真网关上成功且警告正确、
Channel Pin 面板文案与"无自由输入框"成立,零 pageerror)。
**Channel Pin 未在真网关上实际触发** —— 它会真的调用上游 Provider,离线部署无凭据,
且那属于 E5 未授权的真实网络范围。它的回执渲染只在 fixture 下验证过。

---

## 2026-08-21 · Claude Code · D6 骨架层:枚举词汇英文 + 诚实的覆盖声明

**Touched:** `web/prism/**` 与本仓两份文档。**未改动任何后端源码、契约或生成物。**
**契约接线率不变(87/99)** —— 本次是文案与本地化,不新增算子。

**Why:** 批 D 的 D6。先实测体量:**9,776 个用户可见中文字符 / 约 1,461 个片段 / 32 个文件**,
是计划估算(3.5 天)的数倍,且长句与 JSX 标记交织。据此与使用方确认后收敛到骨架层:
导航、设置、解锁、版本选择,加上全部十个闭集状态词汇(52 条 label + detail)。
页面说明性正文保持中文,并在语言开关处如实写明覆盖范围。

**一处设计决定,值得后端知情(因为它关系到契约词汇的呈现):**
枚举词汇的英文放在**枚举旁边**(`StateMeta.en`),不放进扁平的 i18n pack。原因是契约的
状态词汇**故意重叠**:`disabled` 在 auth 轴、price evidence、egress 域是三个不同的东西,
`active` / `expired` / `fresh` / `available` 同样碰撞。一张按原值索引的扁平表会把这些区分合并掉,
而"不合并不同词汇表"正是前端对这些闭集的既定纪律。已加单测钉住二者的英文释义必须不同。

**呈现不变的部分:** 徽章仍然同时显示本地化标签与**契约原值**(`<span className="visually-hidden">`),
glyph 与 tone 不随语言变。后端返回的标识符一律不翻译。

**Other side:** **无需任何动作。** 2026-08-20 那条账号池 500 vs 503 的 action required
仍然有效,状态不变。

**门禁:** 227 单测 · 100 E2E · `check:full` · 真网关验证(导航整条切英文零中文残留、
覆盖声明在屏、旧的过度承诺文案已不在包里、正文如声明所述仍是中文,零 pageerror)。
**徽章英文在真网关上验不到** —— 离线部署三个投影全 503,`.rt-chip` 计数为 0,
没有数据就没有徽章;只在 fixture 下验证过(并用临时回退确认过 E2E 确实会失败)。

---

## 2026-08-26 · Codex · Oracle Singapore VPS 安全连接交接

**Touched:** `CLAUDE.md`、`docs/handoffs/claude-code-oracle-singapore-vps.md`、本文件。
**未改动** `web/prism/**`、管理 OpenAPI、生成客户端、后端源码或远程服务。

**Why:** 用户要求确认 CPAR 是否部署在 Oracle 新加坡 VPS，并给 Claude Code 一份连接方案。
Codex 通过本机既有 SSH alias 做了 value-free 只读核对：`new-vps` 是 Oracle
`ap-singapore-1` / Ubuntu 24.04 / ARM64，CPAR、Autoreg wrapper、Caddy 与三个 loopback
listener 均存在；旧 `jakarta-vps` 的 CPAR 进程也仍 active，因此不得把“Oracle 已运行”
解释为“Jakarta CPAR 可停”或“当前公网流量位置已重新证明”。零远程 mutation、零 Provider 请求、
零 Secret 输出。

**Other side:** **Action required.** Claude Code 在任何 Oracle 真机 UI 验证前必须完整阅读新
handoff，只使用本机 `new-vps` alias 与 `18181:127.0.0.1:18181` SSH tunnel；不得展开/提交
真实 IP、私钥路径或管理凭据。Prism 嵌入 gateway binary，禁止直接覆盖远程 `dist`。本交接只
授权连接和只读检查，不授权部署、restart、Docker/SQLite/Caddy/DNS/Provider/Autoreg mutation。

---

## 2026-08-29 · Claude Code · P13 全量发布方案(第 1–4 步已完成,未装机)

**Touched:** `docs/handoffs/p13-production-release-plan.md`(新增)、本文件。
**未改动** `web/prism/**`、契约、生成物、后端源码。**未连接任何远程主机。**

**Why:** operator 要求把本轮前端成果跑在真机上。核查后确认**"只发前端"结构上不可能** ——
`c02a689`(当前生产血缘)的契约里 `listProviderEgressStatus` / `listOperationalUsage` /
`createCompatibleProxyPool` / `executeChannelPin` / `listOperationalBilling` 全部为 0,
Prism 批 A–D 就是消费 P13 契约的那部分工作。operator 据此选择 **P13 全量后端上生产**。

**已完成 handoff §6 的第 1–4 步:**
干净提交 `d75ab21` → `sync-contract`(契约未变、生成物零漂移)+ `check:full` →
经既有 release workflow 在 `ubuntu-24.04-arm` 原生 runner 构建 aarch64 artifact
([run 33091046905](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/33091046905))→
`p12-release-artifact.rb verify --require-signature --require-receipt` 通过,
并在本机独立 `cosign verify-blob` 验签 **Verified OK**。
二进制 `714faebe…9e9cbc`,SBOM CycloneDX 1.5 / 290 组件。

**给后端知情的两条实测结论(本次发布的运维要害):**

1. **迁移在网关启动时自动施加**(`control_plane.rs:1384` `from_connection` → `migrate`),
   没有单独命令、没有确认。`systemctl restart` 即在生产库上建 `0014`–`0019` 六张表。
2. **六个迁移全部只新建表,没有任何 `ALTER TABLE` / `DROP` / `RENAME`** ——
   既有 P12 表不变,因此**回滚只需换回旧二进制,不需要 down 迁移**。
   但每个迁移各自一个事务(非六个原子),中途失败会停在中间态,只能走库恢复。

**方案的头号风险,已写进文件 §8:** P13-07 改过路由/候选编译路径,
现役 P12 时代 active Config Version 在 P13 下能否编译出运行图,**只能靠用生产库快照做 preflight**
(独立端口 18280/18281、独立目录)来回答;preflight 第 3 项 `/v1/models` 非空是发布的硬闸。

**Other side:** **无需 Codex 动作。** 但若后端认为 P13 上生产还需要额外的 Delivery Gate 证据
(文件 §8 第 2 条:36 个后端提交含协议与调度改动,本文件不能代替 P13 自己的门禁),请在此留痕。

**未做:** 未装机、未重启、未碰 Caddy/DNS/防火墙/Docker/Autoreg,零 Provider 请求,零 Secret 输出。
handoff §6 明确装机与重启由 operator 执行。

---

## 2026-09-01 · Codex · P13-12 Provider/channel-scoped account entitlement

**Touched:** 后端 Rust、migration `0020`、authoritative
`docs/openapi/management-v1.json`、开发计划/CR 与本文件。**未改动** `web/prism/**`。

**Why:** operator 要求识别反代账号等级，并明确纠正 Grok
`free/supergrok/heavy` 只属于 **Grok Build**；Grok Web 与 Grok Console 是单独渠道。
同时，Autoreg 是独立项目，不再占用 CPAR 的 P13-12。后端新增 nullable
`ProviderAccountPoolItem.entitlement`，原子字段为 `domain/tier/source/confidence/observed_at_ms`。
无证据时是 `null`，不得根据 quota、请求成功或同一 Grok 身份猜测。

**Action required for Claude Code:** authoritative OpenAPI 已变，`web/prism/contracts` 与 generated
client/fixture 目前会故意显示 drift。请在你当前前端分支执行 `npm --prefix web/prism run
sync-contract`，然后只在账号池详情中消费新字段：

- 必须把 domain 与 tier 一起显示，例如 `Grok Build · SuperGrok`；
- `entitlement: null` 显示“未观测/Not observed”，不能显示 Free 或 Unknown；
- 不得把 Grok Build 的值复制给 Web/Console，也不得合成 overall health；
- source/confidence/observed time 可作为证据详情；tier 不能替代 auth/runtime/quota 状态；
- fixture 至少覆盖 Build `supergrok`、Web 独立域、Console/null、ChatGPT 与 Claude 重名 `free`
  仍按 domain 区分。

**Resolved prior action:** 2026-08-20 留下的账号池 `500` vs `503` 已由后端修复并回归：
`InvalidSnapshot`/`SourceUnavailable` 现在统一返回 `503 management_runtime_unavailable`。

**Other side:** Claude Code 不需要也不得修改 migration、Rust tier parser、生产数据库或执行 live
subscription sync。后端完成前此项保持 `IN_PROGRESS`；OpenAPI 字段形状已冻结，可先完成前端接线。

**Existing frontend gate issue:** 本次后端验证运行 `./scripts/check.sh docs` 时，文档链接、契约引用、
计划状态、canary/Caddy 边界与 secret scan 均通过，但仓库的全历史 whitespace 步骤发现已提交的
`web/prism/src/features/models/model.ts` 在 EOF 有一个额外空行。普通后端工作区 `git diff --check`
通过，说明这不是本次后端 diff。Claude Code 同步契约时请一并删除该 EOF 空行并重跑前端/文档门禁；
Codex 按所有权边界没有修改该文件。

---

## 2026-09-01 · Codex · Pi/Grok Build Responses 接线纠正与全渠道模型目录边界

**Touched:** 后端 Request Context、Grok Build Responses adapter、systemd/deployment 文档与本文件。
**未改动** `web/prism/**`、Management OpenAPI 或 generated client。

**Why:** operator 明确纠正三项接线原则：

1. Grok Build 给 Pi 的协议必须是 `openai-responses`，不能把 Anthropic Messages 诊断路径当成最终配置；
2. OpenAI Responses SDK 的 provider `baseUrl` 应停在 CPAR 的 `/v1`，由 SDK 追加 `/responses`；
3. Client-facing model catalog 必须来自各渠道上游发现，保留上游 model id，不能靠 CPAR 人工增加
   `grok-cpar-*` 一类别名冒充透传。第三条适用于 Grok Build/Web/Console、OpenAI/Anthropic-compatible
   端点以及任意 `baseURL + API key` 渠道，并且必须经过 Client Key 授权过滤，不能跨渠道泄漏。

本次后端修复了 Pi Responses 请求携带 `prompt_cache_key` 时的租户隔离边界：入口只把已认证
`ClientKeyId`（不含 presented secret）带到 Grok Build adapter；adapter 使用 deployment-scoped
32-byte key、Client Key identity、exact upstream model 和原始 cache key 派生 opaque identity，
原始值不会发给上游。该部署 key 是第六个独立 systemd credential，不复用 master key 或
Client Key pepper。

**Action required for Claude Code:** P13-15A 后端工作树已经把 `/v1/models` 改为当前 Client Key
有权访问且 hard-eligible 的 Route Candidate `upstream_model` 原值，并用同一 exact ID 约束请求
路由；部署前的生产 binary 仍返回旧静态 alias。P13-15A 只修正 serving boundary，
**尚未完成 P13-15B-E 的全渠道真实 discovery、freshness/removal 与 Credential-scoped route
materialization**。因此：

- 不要在 Prism、fixture、Pi/CC Switch 交接里把 `grok-cpar-build` 或其他 CPAR alias 写成上游真实模型；
- Provider/API 类型保持 OpenAI Responses；不要恢复 Anthropic Messages workaround；
- 模型选择器必须消费授权后的 gateway catalog，并显示 exact upstream model id 与 channel/domain，
  不应在前端维护渠道模型白名单；
- Grok Build、Grok Web、Grok Console 即使返回同名 model id，也必须保留各自 channel identity，
  generic compatible endpoint 也必须保留 endpoint identity。

**Other side:** 后端已按 `CR-P13-UPSTREAM-MODEL-CATALOG-001` 完成 A 片，B-E 将继续实现
provider/channel/endpoint/credential-scoped discovery、staleness/removal、授权 union 与 exact route
materialization；在真实上游目录和隔离回归完成前不得把该能力标为 DONE。OAuth 注册仍不属于
CPAR，但 CPAR 对已导入 refresh token 的 runtime refresh/过期处理需要单独补齐，不得把“本次凭据
探针成功”表述成“长期自动续期已完成”。

---

## 2026-09-02 · Codex · P13-15A 生产上线、Pi Responses 实测与下一片模型发现

**Touched:** 后端 Grok native runtime revision、通用协议转换、回归测试、P13 报告与计划。
**未改动:** `web/prism/**`、Management OpenAPI、generated client。

生产已运行 revision `ac23f387bb71d2494d6b2d999096e2bd1c9c5d1e`。认证后的 `/v1/models`
返回当前授权 Route Candidate 的 exact ID：`gpt-5.6-terra`、`grok-4.20-0309`、`grok-4.5`；
`grok-cpar-build` 不再广告，跨 Route 歧义 ID 仍 fail closed。真实 public
`POST /v1/responses` 与 Pi 0.84.3 streaming 调用均以 `grok-4.5` 成功；服务事件明确记录
`protocol=openai_responses`。Pi 和 CC Switch 保存的 provider 均为 CPAR `/v1` +
`openai-responses` + exact `grok-4.5`，不是 Messages。旧 Messages event 只属于历史诊断探针。

当前导入的官方 Grok Build 凭据无需重新 OAuth；CPAR 的 entitlement projection 已受控记录为
`grok_build / supergrok / provider_subscription / authoritative`。本次还修复 durable revision `0`
到 runtime revision `1` 的 one-based 投影，否则一个 revision-zero Console row 会让完整 Build/Console
egress source fail closed；同时只允许同协议 OpenAI Responses 保留 typed prompt-cache controls，
跨协议转换仍拒绝，Build adapter 继续负责 tenant-isolated opaque derivation。

**Action required for Claude Code:**

- 模型选择器只消费 gateway 返回的 exact ID，不重新引入 `grok-cpar-*`、Provider 前缀别名或前端白名单；
- 不要把当前三项列表硬编码。P13-15A 仍是 compiler-retained Candidate 投影，下一片 P13-15B
  才会为 Build、Web、Console、Official、Kiro、OpenAI/Anthropic-compatible 以及任意新增渠道组合
  Endpoint/Credential-scoped discovery source；上游不支持 list API 的渠道必须展示显式 configured
  exact source/freshness，不能猜测模型；
- 同名 ID 可能因多个 channel/Endpoint 而被后端省略或判歧义；UI 需要呈现受保护 provenance，不能
  靠改名消除冲突；
- Grok Build 的 `free/supergrok/heavy` 只属于 Build entitlement；Web 和 Console 继续独立。

后端下一任务是 P13-15B。P13-15C/D 与 P13-15E 剩余 generic/multi-Credential/channel isolation、
正式 Gate 仍未完成，不能把全量动态模型透传标为 DONE。

---

## 2026-09-02 · Codex · Build/Codex 真实目录源与 CPAR runtime OAuth 自动续期

**Touched:** 后端 Provider catalog adapters、Credential pool/runtime composition、双语 README/部署
指南、计划/CR/report 与本文件。**未改动** `web/prism/**`、Management OpenAPI 或 generated client。

**Why:** operator 纠正了两个长期运行语义：进入 CPAR 的 refreshable OAuth 应由 CPAR 日常自动
续期，而不是依赖 Autoreg；模型列表必须来自 exact upstream catalog，不能把当前三项 production
Candidate 列表当作完整模型集合。

后端本地实现已增加：

- 启动前 catch-up + listener 启动后的每分钟 bounded refresh owner；当前 active production scope 是
  Grok Build 与 official Codex OAuth。刷新后通过 encrypted revision CAS 持久化，并原子替换后续
  lease 的 runtime material；在途 lease 继续固定旧 revision。API key、Console/Web SSO cookie 不伪
  refresh；注册、首次 OAuth 与 revoked grant 的交互 reauth 仍属于 Autoreg/operator。
- Grok Build exact-Credential catalog source，真实上游当前返回 `grok-4.6`、`grok-4.5`。
- official Codex exact-Credential catalog source，真实上游 visible + API-supported 当前返回
  `gpt-5.6-terra`、`gpt-5.6-luna`、`gpt-5.5`、`gpt-5.4-mini`；hidden/non-API 项不公开。

**Action required for Claude Code:**

- 不要把上述集合写进前端常量、套餐映射或 fixture 当作永久清单；它只是 2026-09-02 exact
  Credential 的上游观测。模型选择仍消费 gateway 的授权后 `/v1/models`。
- ChatGPT `free/go/plus/pro` 与 Grok Build `free/supergrok/heavy` 是 entitlement 展示域，不是前端
  生成 model ID 的规则；同一套餐以后出现/移除模型时以上游目录为准。
- 当前没有 Management OpenAPI shape 变化，不需要重新生成 client。P13-15C/D 接入 durable
  freshness 与 route materialization 后若新增 protected provenance/status API，后端会另留 action。
- UI 若显示 credential expiry/health，不要把 `reauth_required` 表述为“CPAR 没有自动刷新”；它表示
  自动 refresh grant 已失败或撤销，需要外部交互授权。普通 `refresh_due` 由 CPAR 自己处理。

**Other side:** FYI + action required。P13-15B 目前只完成 Build/Codex source；其他渠道、P13-15C/D
和正式 Gate 未完成。P13-16A 本地通过，仍等待 Oracle deployment/post-expiry continuity 证据。

---

## 2026-09-02 · Codex · P13-16A 生产刷新验收与 P13-15C/D 前端边界

**Touched:** 后端 credential refresh worker、部署指南、计划/report/traceability。
**未改动:** `web/prism/**`、Management OpenAPI、generated client、Pi provider 模型数组。

Oracle Singapore 已部署 exact commit `388e156cdf8c4c0693ecaed02ddd772f89e03962`，ARM64
artifact SHA-256 为 `4691d6b34dd09f8eff101f7e65f3ad5094bc54b8e1ae39faee8b0da6b095b5fc`。
CPAR 在启动 catch-up 中自动刷新了一个 Build 凭据，durable/runtime revision
`0 -> 1`，真实公网 `grok-4.5` Responses 仍返回 `200/completed/CPAR_OK`。一个 Build grant
已 `reauth_required`；两个生产 Codex Credential 都是 `free`而非 Go，且它们的旧 refresh grant
均被上游拒绝，需要 operator 为目标账号重新 OAuth。Codex 失败已使用
`1/2/4/.../60` 分钟有界退避，不再对无效 grant 每分钟实际发送。

**Action required for Claude Code:**

- 生产 `/v1/models` 当前仍只有 `gpt-5.6-terra`、`grok-4.20-0309`、`grok-4.5`。
  这是 P13-15C/D 尚未物化的后端缺口，不是前端应该用常量补齐的缺口。
- 不要根据 ChatGPT `free/go/plus/pro` 或 Grok Build `free/supergrok/heavy` 生成模型列表；
  `gpt-5.6-luna` 与 `grok-4.6` 必须由 exact production Credential 的 catalog 证据进入后端
  immutable authorized snapshot 后才显示。
- 重新 OAuth 后，UI 只需继续消费 gateway catalog 和 credential status；不得把
  `free` 凭据显示为 Go，也不得把 refresh-grant 失效误写成“CPAR 不会自动刷新”。

**Other side:** action required，但目前没有 OpenAPI 形状变更，不需要重生 generated client。

---

## 2026-09-02 · Codex · P13-15C/D 动态目录状态与 exact Credential 路由物化

**Touched:** `docs/openapi/management-v1.json`、后端 Catalog persistence/runtime/router、双语
README/部署指南。**未改动:** `web/prism/**`、Prism generated client 与 vendored contract。

**Why:** P13-15B 已能从 exact Grok Build/Codex Credential 读取真实上游模型，但 discovery 结果尚未
进入 durable freshness/removal 与 serving snapshot，导致生产 `/v1/models` 仍只显示静态 Candidate。
P13-15C/D 现在按 Config Version + Endpoint + Credential 持久化 last success，并原子物化只允许实际
列出模型的 Credential 租约。

后端变化：

- `GET /admin/catalog/status` 的每一项仍保留原有必填字段，新增 optional
  `snapshot_version`、`refresh_due`、`model_count`、`last_failure_at_ms`、
  `last_failure_class`；failure class 是 `authentication|authorization|rate_limit|transport|upstream|internal`；
- failure-only、尚无成功目录的 target 使用 `freshness=missing`，version/refresh/model count 缺席；
- `/v1/models` 仍是普通 OpenAI list shape，但 Build/Codex worker 成功后集合可以随 immutable Catalog
  publication 动态增减；UI 不能把当前观测到的 `grok-4.6`、Luna 等写成常量；
- account tier 与 model catalog 分离：Grok Build `free/supergrok/heavy` 或 ChatGPT
  `free/go/plus/pro5x/pro20x` 只供权益展示，不能生成 model ID；
- 同名模型的 channel/Endpoint/Credential provenance 继续由后端保护和消歧；公开客户端只发送 exact ID。

**Action required for Claude Code:** 在合并本提交后运行
`npm --prefix web/prism run sync-contract`，提交生成的 Prism contract/client 变化，并让目录状态 UI
兼容上述 optional 字段。`missing` 不能显示成空目录成功；`stale` 仍可服务但应提示；`expired` 不可路由；
safe failure class 只能用于诊断标签，不能据此自动 OAuth、修账号或合成 overall account health。模型选择器
继续实时消费授权后的 gateway `/v1/models`，不得增加前端模型/套餐白名单。

**Other side:** action required。P13-15C/D 当前是本地通过，Oracle `grok-4.6` production acceptance、
remaining channel sources 与 P13-15E formal Gate 尚未完成；不要提前把 P13-15 标成 DONE。

---

## 2026-09-03 · Codex · P13-15C/D Grok Build 已通过 Oracle 生产验收

**Touched:** 后端生产 release、Build exact Credential import/entitlement sync、计划与 value-free
receipt。**未改动:** `web/prism/**`、Prism generated client、Management OpenAPI shape、Autoreg、
Caddy 配置、Cloudflare、DNS 与其他服务。

**Why:** exact implementation `058805e556d5b22a00bd56b846f75ed8b81696fd` 已部署 Oracle
Singapore，授权 `/v1/models` 现返回四个 exact upstream ID 并通过 durable catalog 动态包含
`grok-4.6`；唯一一次真实 non-streaming `grok-4.6` Responses canary 返回 `200/completed`。这取代
上一条记录中“生产仍只有三个模型”的时点事实，但不改变 account tier 与 model catalog 分离原则。

当前 protected `GET /admin/catalog/status` 有四个 target：一个 Fresh Build target（两个模型）与三个
Missing target（仅保留 safe failure class）。这代表 Build 生产路径通过，不代表所有 target 或整个
P13-15 已健康/完成。

**Action required for Claude Code:** 若上一条 P13-15C/D 的 contract/client sync 尚未完成，继续运行
`npm --prefix web/prism run sync-contract` 并实现 optional catalog operational fields。目录页必须逐
target 展示 `fresh|stale|expired|missing` 与 safe failure；不得把 1 Fresh 合成为 overall healthy，也
不得把 3 Missing 合成为 CPAR 整体失败。模型选择器继续实时消费授权 `/v1/models`；不要硬编码
`grok-4.6`、Luna 或套餐到模型映射。P13-15 在 remaining channel sources、target Go/Codex evidence、
P13-15E isolation 与正式 Delivery Gate 完成前仍是 `IN_PROGRESS`。

**Other side:** action required；本次 production acceptance 没有新增 OpenAPI shape，需消费的仍是
上一条已提交的 P13-15C/D 管理契约。

---

## 2026-09-08 · Codex · Grok Build 自动刷新生产修复

**Touched:** `crates/provider-grok/src/oauth.rs`、`apps/gateway/src/grok_admin.rs`、相关回归测试及
`docs/reports/evidence/grok-build-refresh-scope-recovery-20260908.md`；Oracle CPAR 最终运行
`928e971eb7d6f520b50ded677803e259d4549cb1`。没有修改前端文件或管理 HTTP 契约。

**Why:** 上游 OAuth 成功响应返回扩展后的 scope，后端原先按字符串完全相等比较，误判刷新失败。
已改为权限集合包含校验，保留上游 scope 并拒绝必需权限丢失；原生套餐同步/诊断命令也已支持
自动刷新后的内部凭据格式。

**Claude Code FYI:** 恢复的 Build 账号已三次刷新成功，运行时 `active / available`，套餐为权威
`grok_build / supergrok`。公网 `grok-4.6`、`grok-4.5` 均通过 Responses 验收。此次无须重新生成
API client。模型选择器仍应读取授权 `/v1/models`，不要硬编码；其他 Missing/Expired target
未因这次单账号恢复而变成健康，P13-15 整体状态不变。本机 Pi 的旧配置仍只列 `grok-4.5`。

**Other side:** FYI，无新增前端接线要求；此前未完成的逐 target 状态展示要求仍有效。

---

## 2026-09-08 · Codex · Grok Build 流式工具调用与结果回传修复

**Touched:** `crates/provider-grok/src/build_responses.rs`、
`crates/gateway-router/src/protocol_transform.rs`、对应测试、
`scripts/verify-pi-responses-tools.mjs` 与脱敏验收报告。Oracle 运行签名版本
`fb93c489bbbc6e2d5aa35b41b3da413b7ab5851e`。

**Why:** 上游工具参数事件仅带 item_id 时不应要求重复 call_id；Pi 回传的合法 function_call.id
也不应被路由当作未知扩展而返回 CredentialUnavailable。已修复两处，保持身份校验与跨协议隔离。

**Claude Code FYI:** 无 Management OpenAPI 或前端文件修改，无需重生成客户端。公网 Grok Build
的 grok-4.6 / grok-4.5 均已通过实际 Pi Responses 客户端的两轮流式工具闭环，不需要切 Messages。
后续模型验收应区分目录可见、文本调用、流式工具调用及结果回传；仓库新增显式执行的验收脚本。
本次仍不代表其他渠道或整个 P13-15 完成。

**Other side:** FYI，无新增前端接线要求。

---

## 2026-09-08 · Codex · OMP 多轮 Responses 文本历史回传修复

**Touched:** `crates/gateway-router/src/protocol_transform.rs`、
`scripts/verify-pi-responses-tools.mjs`、
`docs/reports/evidence/grok-build-omp-text-replay-20260908.md`。
Oracle 已部署签名版本 `4bb55b147518d32ac0ce6210ce652b4bb1668663`。

**Why:** 此前两轮工具验收未覆盖助手文本再次进入历史；Pi 回传的合法助手消息
id/status/phase 与空 annotations 被候选路由误拒绝，外显为 503 CredentialUnavailable。
现在同协议保留这些已验证字段，未知字段与跨协议转换仍拒绝。

**Claude Code FYI:** 无 Management OpenAPI 或前端文件修改，无需重新生成客户端。
OMP 受控 Pi AI 0.84.3 对 grok-4.6、grok-4.5 均完成四轮真实闭环：工具、文本、
带完整历史的工具、文本。后续模型验收需包含助手文本历史重放，不能仅以两轮工具成功判断。
此结果支持继续 OMP 验收，但不等于 OMP C12、其他渠道或 P13-15 整体完成；费用仍未核实。

**Other side:** FYI，无新增前端接线要求。

---

## 2026-09-09 · Codex · Prism V4 M0 契约与会话/版本正确性

**Touched:** `web/prism/contracts/management-v1.json`（sync-contract 生成）、
`web/prism/scripts/check.mjs`、`web/prism/src/App.tsx`、
`web/prism/src/api/client.ts`、`web/prism/src/api/queryClient.ts`、
`web/prism/src/api/client.ownership.test.ts`、`web/prism/src/api/client.fixtures.test.ts`、
`web/prism/src/session/sessionStore.ts`、`web/prism/src/utils/revision.ts`、
`web/prism/src/utils/revision.test.ts`、`web/prism/src/features/config-versions/versionStore.ts`、
`web/prism/src/app/AppShell.tsx`、`web/prism/src/app/DraftDock.tsx`、
`web/prism/src/components/Sheet.tsx`、`web/prism/src/dev/fixtures.ts`、
`web/prism/src/features/runtime/model.ts`、`web/prism/src/features/runtime/RuntimePage.tsx`、
`web/prism/src/features/runtime/EntitlementEvidence.tsx`、
`web/prism/e2e/session-ownership.spec.ts`。

**Why:** 用户本轮明确授权 Codex 统一实现前后端 V4，不永久更改默认分工。
补齐最新 entitlement/catalog 字段；阻止晚到响应污染其他版本或新会话、revision 倒退，
并在鉴权失效时真正锁定、取消请求、清缓存/秘密/旧表单。锁定不保留含秘密的退出动画节点。

**Other side:** FYI，无需等待另一位实现者。权威 OpenAPI 本批未变，generated client
经同步后内容不变，不能手改生成物。日常 check 现会校验权威契约。
B1–B4 与 BE-FE-01/02 仍属于后续 M2/M3 待实施，见 CR-PRISM-V4-001；
本批不是 V4 全界面或真实网关验收完成。类型、245 项单测、Chromium 定向 E2E
及权威 SPA 双构建门禁通过；未访问远端或真实 Provider。

---

## 2026-09-09 - Codex - Prism V4 M1 shell and resource workspaces

**Touched:**

- `web/prism/DESIGN.md`

- `web/prism/e2e/batch-d.spec.ts`
- `web/prism/e2e/contrast.spec.ts`
- `web/prism/e2e/credential.spec.ts`
- `web/prism/e2e/flows.spec.ts`
- `web/prism/e2e/glass.spec.ts`
- `web/prism/e2e/helpers.ts`
- `web/prism/e2e/monitoring.spec.ts`
- `web/prism/e2e/narrow.spec.ts`
- `web/prism/e2e/observability.spec.ts`
- `web/prism/e2e/provider-egress.spec.ts`
- `web/prism/e2e/provider-pools.spec.ts`
- `web/prism/e2e/route-candidates.spec.ts`
- `web/prism/e2e/v4-workspaces.spec.ts`
- `web/prism/index.html`
- `web/prism/src/App.tsx`
- `web/prism/src/api/client.ownership.test.ts`
- `web/prism/src/api/queryClient.ts`
- `web/prism/src/app/AppShell.tsx`
- `web/prism/src/app/navigation.ts`
- `web/prism/src/app/themeStore.ts`
- `web/prism/src/app/v4.css`
- `web/prism/src/components/Sheet.tsx`
- `web/prism/src/components/StatusBadge.tsx`
- `web/prism/src/components/glass/GlassSurface.tsx`
- `web/prism/src/design/modal.css`
- `web/prism/src/design/tokens.css`
- `web/prism/src/dev/fixtures.ts`
- `web/prism/src/features/accounts/AccountsPage.tsx`
- `web/prism/src/features/catalog/CatalogPage.tsx`
- `web/prism/src/features/monitoring/MonitoringPage.tsx`
- `web/prism/src/features/overview/OverviewPage.tsx`
- `web/prism/src/features/runtime/EntitlementEvidence.tsx`
- `web/prism/src/features/runtime/RuntimePage.tsx`
- `web/prism/src/features/runtime/entitlements.test.ts`
- `web/prism/src/features/runtime/entitlements.ts`
- `web/prism/src/features/settings/SettingsPage.tsx`
- `web/prism/src/features/unlock/UnlockPage.tsx`
- `web/prism/src/features/upstreams/CredentialSheet.tsx`
- `web/prism/src/features/upstreams/SubresourcePanel.tsx`
- `web/prism/src/features/usage/UsagePage.tsx`
- `web/prism/src/i18n/en.ts`
- `web/prism/src/i18n/zh.ts`
- `web/prism/src/main.tsx`
- `web/prism/vite.config.ts`

**Why:** The user authorized Codex to implement both sides for this Goal. Apply the
approved OpenDesign V4 palette and 14-section navigation to the real React app,
with solid data panels, centered forms, right-side inspectors, responsive accounts
and catalog workspaces, in-memory accessibility preferences and section search.
Existing operations remain wired. Fix focus restoration under StrictMode and clear
legacy identity-only query caches when the selected configuration changes.

**Other side:** FYI. No backend contract change in this batch. The OpenDesign MCP
readback matches the approved V4 SHA-256. The current conversation authored the code;
no model CLI or OpenDesign internal generator was used. M1 object-detail work remains,
and BE-FE-01/02 plus B1-B4 and real local gateway acceptance remain pending. This is
not a completed Goal or a production deployment. See docs/reports/prism-v4-progress.md
for the tested boundaries and remaining work.

Validation: type-check, 251 unit tests, 109 Chromium E2E tests and the authoritative
SPA gate passed; narrow-screen checks passed separately. No real Provider or remote operation.

---

## 2026-09-09 - Codex - V4 object inspectors

**Touched:**

- `web/prism/e2e/object-inspectors.spec.ts`
- `web/prism/src/app/v4.css`
- `web/prism/src/components/ObjectInspector.tsx`
- `web/prism/src/features/access/AccessPage.tsx`
- `web/prism/src/features/audit/AuditBackupPage.tsx`
- `web/prism/src/features/billing/BillingPage.tsx`
- `web/prism/src/features/egress/EgressPage.tsx`
- `web/prism/src/features/models/ModelsPage.tsx`
- `web/prism/src/features/models/RouteWorkbench.tsx`
- `web/prism/src/features/runtime/RuntimePage.tsx`
- `web/prism/src/features/upstreams/UpstreamsPage.tsx`
- `web/prism/src/i18n/en.ts`
- `web/prism/src/i18n/zh.ts`

**Why:** Complete the approved V4 read-only object inspection layout for upstreams,
models, routes, price catalogs/rates, access groups, Client Key metadata, egress
policies and audit records. Reuse existing loaded safe fields; do not issue duplicate
single-resource reads or serialize secrets. Editing remains in centered forms.
The egress workspace now also exposes the existing three-domain Provider projection.

**Other side:** FYI. User-authorized frontend implementation by Codex; no backend
API/schema change. Type-check and 16 affected browser checks passed, including four
new inspector flows. Remaining M1-M4 work stays explicit in the progress report.
No remote state or real Provider was accessed.

---

## 2026-09-09 - Codex - M1 account actions, diagnostics and read states

**Touched:**

- `web/prism/e2e/account-actions.spec.ts`
- `web/prism/e2e/i18n.spec.ts`
- `web/prism/e2e/object-inspectors.spec.ts`
- `web/prism/e2e/provider-egress.spec.ts`
- `web/prism/e2e/provider-pools.spec.ts`
- `web/prism/e2e/read-status.spec.ts`
- `web/prism/src/app/navigation.ts`
- `web/prism/src/app/v4.css`
- `web/prism/src/components/ObjectInspector.tsx`
- `web/prism/src/components/ReadStatus.tsx`
- `web/prism/src/design/modal.css`
- `web/prism/src/dev/fixtures.ts`
- `web/prism/src/features/access/AccessPage.tsx`
- `web/prism/src/features/accounts/AccountsPage.tsx`
- `web/prism/src/features/audit/AuditBackupPage.tsx`
- `web/prism/src/features/catalog/CatalogPage.tsx`
- `web/prism/src/features/config-versions/VersionsPage.tsx`
- `web/prism/src/features/egress/EgressPage.tsx`
- `web/prism/src/features/models/ModelsPage.tsx`
- `web/prism/src/features/overview/OverviewPage.tsx`
- `web/prism/src/features/runtime/PoolActionSheet.tsx`
- `web/prism/src/features/runtime/RuntimePage.tsx`
- `web/prism/src/features/upstreams/UpstreamsPage.tsx`
- `web/prism/src/i18n/en.ts`
- `web/prism/src/i18n/zh.ts`

**Why:** Move the existing exact-account cooldown/recovery flow into the account
inspector using the shared PoolActionSheet. Runtime defaults to diagnostics while
preserving its resource views behind an on-demand expansion and existing account
links. Failed reads now label retained results and offer read-only retry; no write
is replayed. Fix the long-title flex cascade and use the backend's exact runtime
conflict code in fixtures. Format newly authored components for review.

**Other side:** FYI under the user's full-stack authorization. No new backend API
has been introduced yet. M0/M1 are complete; BE-FE-01/02, B1-B4 and real local gateway
acceptance remain mandatory work. Type-check, 251 unit tests, all 119 browser tests
and the authoritative four-file SPA gate passed. After formatting the new components,
the build and all 119 browser tests passed again; no project dependency or lock file changed.
No remote or real Provider action was performed.


## 2026-09-10 — Codex / V4 M2 candidate HTTP contract

**What:** Added candidate PATCH/DELETE to
`crates/gateway-http-actix/src/management_resources.rs` and the authority
`docs/openapi/management-v1.json`; regenerated
`web/prism/contracts/management-v1.json` and
`web/prism/src/generated/management-client.ts` using sync-contract.

**Why:** BE-FE-02 requires independent candidate maintenance with exact draft
revision, immutable owner and atomic audit. Existing Route CRUD remains intact.

**Other side:** FYI under this Goal's full-stack authorization. New operations are
`updateRouteCandidate` and `deleteRouteCandidate`, under the existing same-origin
management listener. CandidateInput is reused; no new response schema or fixture
shape is needed. UI integration and complete resource enumeration remain pending.


## 2026-09-10 — Codex / V4 M2 routing enumeration contract

**What:** Added protected GET `/admin/routes`, `/admin/route-candidates` and
`/admin/model-aliases` in `crates/gateway-http-actix/src/management_resources.rs`;
updated `docs/openapi/management-v1.json`, then ran sync-contract to regenerate
`web/prism/contracts/management-v1.json` and
`web/prism/src/generated/management-client.ts`.

**Why:** Complete draft maintenance must include resources without Access Group
grants. Pages use bounded SQL keyset reads with configuration/revision consistency.

**Other side:** FYI under this Goal's full-stack authorization. New operations:
listRoutes, listRouteCandidates, listModelAliases. Each returns a page envelope,
not an array. Restart enumeration after cursor/revision 409; do not replay writes.
RouteListItem preserves legacy policy labels; the existing write contract is unchanged.
UI DTOs, fixtures and resource maintenance integration remain work in this Goal.


## 2026-09-10 — Codex / V4 M2 Prism resource inventory

**What:** Added `web/prism/src/features/models/RoutingInventory.tsx`; updated
`web/prism/src/features/models/{model.ts,RouteWorkbench.tsx,ModelsPage.tsx}`,
`web/prism/src/dev/fixtures.ts` and `web/prism/e2e/route-candidates.spec.ts`.

**Why:** Use the new authoritative Route/Candidate/Alias pages, including unbound
drafts, instead of operational inventory hints. Add explicit loaded counts,
manual pagination and restart-from-first-page after errors; refresh after local
resource mutations. Remove obsolete claims that enumeration is unavailable.

**Other side:** FYI under full-stack authorization. No contract edits in this batch.
Typed page DTOs and synthetic fixture handlers mirror the new endpoints. Candidate
edit/delete UI, access grant route selection and full M4 acceptance remain pending.
Type-check and six focused Chromium browser tests passed, including the new unbound
route and candidate enumeration assertions. No real Provider/remote action occurred.


## 2026-09-10 — Codex / V4 M2 candidate maintenance UI

**What:** Updated `web/prism/src/features/models/RouteWorkbench.tsx` and
`web/prism/src/features/models/RoutingInventory.tsx`, with matching handlers in
`web/prism/src/dev/fixtures.ts` and regression coverage in
`web/prism/e2e/route-candidates.spec.ts`.

**Why:** Complete independent Candidate editing/deletion using the approved centered
form and confirmation pattern. Editing preserves field values and immutable ID/Route;
success restarts inventory reads and validates topology. Deletion retains its Route.

**Other side:** FYI under full-stack authorization. Uses the already authoritative
updateRouteCandidate/deleteRouteCandidate operations through the shared client;
no generated edits or automatic write retry. Type-check, six focused Chromium tests
and four-file/CSP/double-build gate passed. The browser test edits weight, priority,
transform and capability overrides, reopens to verify values, deletes the Candidate
and checks the remaining Route and invalid topology. Real gateway validation is still pending.


## 2026-09-10 — Codex / V4 M2 Access Group route enumeration

**What:** Added `web/prism/src/features/models/useRoutingPages.ts`, shared by
`web/prism/src/features/models/RoutingInventory.tsx` and
`web/prism/src/features/access/AccessPage.tsx`; updated
`web/prism/src/dev/fixtures.ts` and `web/prism/e2e/access-groups.spec.ts`.

**Why:** Access Group route suggestions must include unbound drafts, with bounded
pagination and explicit restart on conflicts. The old operational inventory hints
could not provide this. Manual exact-ID entry remains available and is validated
by the real gateway; fixture grants now also reject absent Route references.

**Other side:** FYI under full-stack authorization. Existing authoritative listRoutes
and grantAccessGroupRoute operations are reused. Eleven focused Chromium tests
passed, including creating a draft Route, seeing it in the grant suggestions and
successfully granting it. No live Provider or remote action occurred.


## 2026-09-10 — Codex / BE-FE-01 effective model HTTP contract

**What:** Added GET `/admin/models/effective` to
`crates/gateway-http-actix/src/management_resources.rs` and
`docs/openapi/management-v1.json`; ran sync-contract to update
`web/prism/contracts/management-v1.json` and
`web/prism/src/generated/management-client.ts`.

**Why:** Expose serving-only authorization/provenance through management authentication,
using an existing Access Group ID or Key ID, never a Client Key secret. Bounded pages
bind the entire safe projection and reject changed context/content with 409.

**Other side:** FYI under full-stack authorization. New operation listEffectiveModels
and three closed schemas. No config revision ETag. Projection fingerprint is not a
catalog revision. Frontend DTO/fixtures and catalog snapshot evidence remain pending.
Four runtime HTTP tests and thirteen contract tests passed; no real Provider was called.


## 2026-09-10 — Codex / BE-FE-01 Prism effective model directory

**What:** Added `web/prism/src/features/catalog/EffectiveModels.tsx`, integrated it
in `web/prism/src/features/catalog/CatalogPage.tsx`; updated source-to-Explain
prefill in `web/prism/src/features/runtime/RuntimePage.tsx`, safe synthetic identity/
model data in `web/prism/src/dev/fixtures.ts`, and
`web/prism/e2e/effective-models.spec.ts`.

**Why:** Show serving exact models by existing Access Group or Key ID with scoped
paging, safe source inspection and diagnostic deep links. Identity selection uses
existing list results; no Client Key secret input exists. The projection fingerprint
is labelled distinctly from catalog observations.

**Other side:** FYI under full-stack authorization. Uses listEffectiveModels through
the shared client, with matching DTOs. Six focused Chromium tests, type-check and
four-file/CSP/double-build gate passed. Group switch clears prior model results;
revoked Key errors do not retain the previous identity's models. Real gateway and
catalog expiry/snapshot evidence remain unfinished work in this Goal.


## 2026-09-10 — Codex / B3 catalog completion timestamps and source evidence

**What:** Updated `apps/gateway/src/runtime.rs` to resample time before per-account
admission, after discovery completion and before publication. Added pinned catalog
evidence to `crates/gateway-http-actix/src/management_resources.rs` and authority
`docs/openapi/management-v1.json`; ran sync-contract. Updated
`web/prism/contracts/management-v1.json`,
`web/prism/src/features/catalog/EffectiveModels.tsx`,
`web/prism/src/dev/fixtures.ts` and `web/prism/e2e/effective-models.spec.ts`.

**Why:** Avoid using a slow pass's start time for completed observations. Display
per-Credential durable version and deadlines from the exact serving snapshot,
with current catalog admission and an explicit unobserved state.

**Other side:** FYI under full-stack authorization. Source.catalog_evidence is now
required; no secrets are included. Contract/HTTP tests, gateway Clippy, type-check,
source-evidence Chromium flow and four-file/CSP gate passed. Delayed real-loopback
refresh and final gateway acceptance remain pending; no real Provider was contacted.


## 2026-09-10 — Codex / B1 protected processing status

**What:** Added `crates/gateway-control/src/billing_processing.rs`, wired the shared
monitor through `apps/gateway/src/{billing_worker.rs,deployment.rs}` and
`crates/gateway-http-actix/src/management_resources.rs`. Updated authority
`docs/openapi/management-v1.json` and ran sync-contract for
`web/prism/contracts/management-v1.json` and
`web/prism/src/generated/management-client.ts`.

**Why:** Make absent/starting/current/lagging/repair/failed/stopped worker states
observable without blocking management reads on storage. Failure keeps last
successful observation metadata and exposes only a closed failure code.

**Other side:** FYI under full-stack authorization. New operation
getBillingProcessingStatus is unscoped and read-only. Frontend DTO/fixture/display
remain the next step. Monitor lifecycle, real SQLite worker restart, five runtime
HTTP tests, thirteen contract tests, Clippy and SPA gate passed. This is not M4
listener-to-Provider acceptance.


## 2026-09-10 — Codex / B1 Prism processing state

**What:** Added `web/prism/src/features/billing/ProcessingStatus.tsx`, integrated
it into `web/prism/src/features/{billing/BillingPage.tsx,overview/OverviewPage.tsx,
usage/UsagePage.tsx,monitoring/MonitoringPage.tsx}`, and updated fixture/E2E support
in `web/prism/src/dev/fixtures.ts` and `web/prism/e2e/billing-processing.spec.ts`.

**Why:** Show global processing/repair state alongside financial data and keep
empty ledgers distinct from zero spend. Reads share one query cache, stop polling
on read failure and use session cleanup already enforced by the shared client.

**Other side:** FYI under full-stack authorization. No contract edits. Type-check,
new cross-page Chromium flow, 23 existing billing/usage/monitoring tests and SPA
gate passed. The billing no-version early return now also shows this unscoped
status; unknown values remain unobserved rather than zero.


## 2026-09-10 — Codex / B2 streaming usage and snapshot cursor

**What:** Updated production usage wiring in `apps/gateway/src/deployment.rs`,
streaming aggregation/cursor in `crates/gateway-control/src/management_operations_service.rs`,
opaque cursor transport in `crates/gateway-http-actix/src/management_resources.rs`,
and authority `docs/openapi/management-v1.json`. Ran sync-contract, updating
`web/prism/contracts/management-v1.json`; generated client bytes are unchanged.

**Why:** Remove global-history loading while retaining complete per-group token
observations and bounded application memory. New cursors pin event ordinals;
legacy cursor transport stays accepted.

**Other side:** FYI under full-stack authorization. No public response DTO changed.
Aggregation equivalence, partial token confidence, late-event exclusion, new/legacy
cursor roundtrip, 3 operational HTTP tests, 13 contract tests, Clippy and SPA gate
passed. M4 real gateway acceptance remains required.

## 2026-09-10 — Codex / legacy route inspector compatibility

**What:** `web/prism/src/features/models/RoutingInventory.tsx` opens old-policy
route records in the existing ObjectInspector, with loaded safe fields and a
candidate inventory entry. `ModelsPage.tsx` and `runtime/RuntimePage.tsx` remove
obsolete missing-enumeration statements. Added regression in
`web/prism/e2e/route-candidates.spec.ts`.

**Why:** Complete route enumeration includes round_robin/priority_failover, while
the existing single-route reader only supports smooth_weighted_round_robin.
Opening known legacy records must not call that incompatible reader or change policy.

**Other side:** FYI under current full-stack authorization. Type check, 7 Chromium
route E2E tests and the four-file/CSP/contract double-build gate passed. The legacy
inspector uses the approved object-details surface; no new generation or API.

## 2026-09-10 — Codex / effective model to draft candidate handoff

**What:** `web/prism/src/features/catalog/EffectiveModels.tsx` links each selected
source into ModelsPage with safe exact model/Endpoint/source-version query values.
`features/models/ModelsPage.tsx` preserves selection across draft switching and
can seed a new public-model form; `RouteWorkbench.tsx` seeds new candidate forms
only. Existing edits retain their own values; add-form errors now stay visible
inside the form. Added `e2e/effective-models.spec.ts` coverage.

**Why:** Implement the approved M2 OpenDesign “用于草稿候选” flow without copying
serving configuration into drafts or sending Client Key secrets. Users still select
or create a draft, choose/create a route, and explicitly save. Backend validation
checks the target draft's Endpoint and topology.

**Other side:** FYI under full-stack authorization. Two effective-model E2E tests
passed (context isolation plus serving→draft→new model/route→candidate prefill and
clear), seven existing route E2E tests passed, type check and SPA double-build gate
passed. This UI fixture evidence does not replace M4 real gateway validation/publish.

## 2026-09-10 — Codex / production browser audit and V4 controls

**What:** Added `web/prism/e2e/real-gateway-audit.mjs`, invoked with ephemeral
stdin credentials by `scripts/prism-v4-local-acceptance.py --browser`. Updated
`web/prism/src/app/v4.css` and padded panels in `features/billing/ProcessingStatus.tsx`,
`features/catalog/EffectiveModels.tsx`, `features/models/ModelsPage.tsx`.

**Why:** Real embedded screenshots exposed flush panel contents and native white,
short inputs in dark mode. Shared control styling now uses V4 surface/ink/border
and 36px minimum height. Padded surfaces use 16px on every side; selector specificity
beats the legacy card-shift rule that previously zeroed the left padding.

**Other side:** FYI under full-stack authorization. Final real Chromium 151.0.7922.34
run captured 84 page views + 6 unlock views across 1440×900 / 1280×720 / 390×844,
light/dark with reduced motion; no page overflow, JS errors, or bad panel padding.
251 unit tests, all 123 fixture E2E, SPA double-build gate and actual gateway build
passed. Screenshots are evidence of entry/layout coverage, not a claim that every
interactive flow has been exercised against the real gateway.

## 2026-09-10 — Codex / resource mutation audit read surface

**What:** Added bounded per-version append-ID resource audit reads in store/control,
`GET /admin/resource-audit-events` in `management_resources.rs`, authority schema and
CR supplement. Ran sync-contract (107 operations), updating the vendored contract and
generated client. Added `web/prism/src/features/audit/ResourceAudit.tsx`, mounted by
AuditBackupPage, and a valid empty fixture response. Extended real gateway and browser
acceptance to inspect actual route_candidate_updated metadata and restore focus.

**Why:** Existing /admin/audit-events deliberately reads lifecycle events only, while
candidate/resource writes atomically append to a separate table that had no HTTP read.
The new page preserves both streams and exposes no secrets or mutation payloads.

**Other side:** FYI under current full-stack authorization. Config/ID filtering and
limit+1 run in storage using the existing config/ID index; page size <=100, newest first.
205-row pagination/version/late-append regression, 13 contract tests, gateway Clippy,
11 relevant fixture E2E, type check and SPA gate passed. Real HTTP rejects invalid bounds
and missing versions; actual Chromium verified details/Escape/focus at all three sizes
in both themes. The new inspector reuses approved V4 ObjectInspector geometry.

## 2026-09-10 — Codex / real browser write and lock acceptance

**What:** Added `web/prism/e2e/real-gateway-flow.mjs` and the local harness's
`--browser-flow` option. The harness prepares only dependency resources in a new
empty draft through real management APIs; the browser selects an authorized model,
creates the model/route/candidate, edits/rereads/deletes/recreates the candidate,
grants the route, validates and publishes, then rereads configuration and audit.

**Why:** Fixture behavior and API-only writes do not prove that the embedded UI
can complete the full authorized workflow. Input secrets use stdin and remain out
of screenshots/reports. The new draft parent is lineage only; no direct DB cloning.

**Other side:** FYI under full-stack authorization. All six real Chromium flow
stages passed, including mobile dark/a11y controls and an actual gateway access
denial. To exercise denial, request interception replaces only the management key
with a random invalid test key; no HTTP response is mocked. Actual 404 plus
management_access_denied locks the UI, clears both secret fields, prevents protected
navigation and stops management reads for the observation window. Initial test's
401 expectation was corrected to the repository's deliberate 404 masking policy.

## 2026-09-10 — Codex / lifecycle confirmation and conditional activation

**What:** Added shared `web/prism/src/features/config-versions/LifecycleConfirmation.tsx`
for DraftDock and VersionsPage publish/rollback. Authority and
`management_lifecycle_resources.rs` add optional X-Expected-Active-Version and
X-Expected-Lifecycle-Event conditions, checked under the lifecycle lock before writes.
Ran sync-contract and updated relevant fixture/real browser flows.

**Why:** Approved V4 requires confirmation before publication and rollback. The
modal shows observed active/target/revision, requires fresh reads, stops after
failure, and does not replay writes. Active identity alone cannot catch ABA;
the latest publication/rollback append ID prevents a switch-away-and-back race.
Both optional headers are independently enforced; legacy callers may omit them.

**Other side:** FYI under full-stack authorization. Type check, 14 relevant E2E,
13 contract tests, 2 lifecycle HTTP tests, Clippy, SPA gate and gateway build passed.
Real browser verifies cancel/confirm publication and rollback, restores the prior
version, and the gateway rejects stale confirmation after the same active ID and
revision return. Initial authority insertion attached a header to the wrong path;
generated-client rejection caught it, it was corrected and checks rerun. Modal uses
approved V4 centered confirmation and states the current serve restart boundary.

## 2026-09-10 — Codex / configuration comparison HTTP contract

**What:** Added independent read-only ConfigurationDiffReader in gateway-store,
source accessor in management_mutation_service, and bounded blocking
`management_resources/configuration_diff.rs` handler. Authority adds
compareConfigVersions with base_id/limit/cursor and safe change metadata; ran
sync-contract, updating `web/prism/contracts/management-v1.json` and generated client.
Extended the real local harness to verify comparison pages and stale cursors.

**Why:** Complete the actual configuration comparison API needed by the approved
V4 versions workspace. No secret/field values are returned, and a full SQL comparison
does not retain the management mutation mutex or connection.

**Other side:** FYI under full-stack authorization. Three store tests (including a
read while a write transaction remains uncommitted), 13 contract tests, Clippy and
SPA gate passed. Real gateway compares versions across pages, excludes test secrets,
returns 409 after draft changes and 404 for absent versions. Frontend consumer is
still pending; do not describe configuration diff as fully delivered yet.

## 2026-09-10 — Codex / production configuration difference panel

**What:** Added ConfigurationDiff.tsx and its scoped table CSS; VersionsPage exposes
“查看差异”. The panel selects baseline, pages actual compareConfigVersions responses,
shows loaded revisions/count, stops on conflicts and explicitly resets comparison.
Fixtures compute differences from their resource maps. Added pagination/conflict/empty
state E2E and expanded the real browser flow. Mobile modal padding now uses 12px for
both supported layouts, with specificity that preserves the inspector rule.

**Why:** Complete the approved V4 centered difference table without fixed samples or
secret values. Baseline/target IDs remain fully visible outside the truncated selector;
three-column mobile data wraps within its cells instead of hiding field names.

**Other side:** FYI under full-stack authorization. Seven relevant E2E, type/build/SPA
gate and the real 8-stage browser write flow passed. Actual gateway differences, baseline
switch, same-version empty state, mobile 12px margin, cell overflow checks and focus
restore passed. Early mobile CSS specificity/nowrap issues were corrected and rerun.
No internal generator or external design model was used; approved V4 diff layout reused.

## 2026-09-10 — Codex — V4 lens fallback acceptance

**Files:** `web/prism/e2e/glass.spec.ts`.

**What / why:** Exercise the production capability probe with unsupported URL filters
and Firefox/Safari probe responses; verify layered blur across all three chrome panes,
usable settings navigation and contrast preference disabling the effect. This is
Chromium branch coverage, not certification of Firefox or Safari. Six glass tests and
frontend type checking passed. Full-stack authorization remains in effect; FYI only.

## 2026-09-10 — Codex — V4 final design handoff

**Files:** `web/prism/DESIGN.md`.

**What / why:** Mark earlier V4 batches as historical and document the completed resource,
diff/confirmation, mobile and fallback implementation, actual browser coverage and the
final delivery report. Documentation only; no generic ownership rule changed. Full-stack
authorization applies, FYI. Validate links and diff; application checks already passed.

## 2026-09-10 — Codex — UAT runtime matrix and failure diagnosis fixes

**Files:** `web/prism/contracts/management-v1.json` (sync-contract),
`web/prism/src/features/runtime/RuntimePage.tsx`, `model.ts`, `model.test.ts`, `runtime.css`,
`web/prism/src/features/monitoring/MonitoringPage.tsx`,
`web/prism/e2e/monitoring.spec.ts`, `web/prism/e2e/real-gateway-flow.mjs`.

**What / why:** Connect the matrix to the serving scheduler's live, secret-free binding
observations. Authority adds credential_unauthorized/expired instead of reporting these as
available. Add failure request inspection and exact binding links; focus the matrix and
recovery controls without guessing missing route/model fields. Replace the request inspector's
implementation jargon with a data-scope explanation. Reuse V4 solid cards, existing inspector
and Explain; no new design generator, service or secret storage.

**Other side:** User explicitly authorized completing UAT-01/02. FYI, no handoff dependency.
127 gateway binary tests, 5 HTTP runtime tests, 44 model/i18n tests, 9 monitoring E2E plus
2 account-action E2E passed. Real gateway browser flow now has 10 stages, including live
cooldown/expiry, failure→attempt→target→Explain→back, mobile target layout, and the existing
write/audit/rollback/session paths. SPA authority/CSP/four-file double build and Clippy passed.
Early E2E found a misplaced nested button, obsolete copy assertion and a missing URL-settle
wait; fixed and rerun rather than counting those failed runs as acceptance.

## 2026-09-10 — Codex — HTTPS domain acceptance handoff

**Files:** `docs/handoffs/claude-code-oracle-singapore-vps.md`,
`docs/handoffs/prism-domain-access.md`, `docs/handoffs/prism-v4-production-rollout.md`,
`docs/reports/prism-domain-delivery.md`, `docs/reports/prism-v4-production-rollout-status.md`.

**What / why:** The user explicitly requested Prism access from other devices on the existing
cpar domain. Backend commit c7cfd2c adds an optional exact HTTPS management origin; production
Caddy now routes only that site's admin/UI paths to the loopback management listener. Four SPA
asset hashes and HTTP DTOs are unchanged. Document the actual domain entry, key/CSRF behavior,
scoped rollback and remaining project issues instead of presenting the old SSH-only entry as current.

**Other side:** FYI. Use the approved HTTPS origin for browser writes on this instance; the old
HTTP tunnel origin will be denied. No frontend code or generated client changes are required.
Parser/composed admission, existing security regression, local and isolated real gateway writes,
signed release/full gates and public authenticated write/readback passed. The browser check covers
the real HTTPS unlock page, not a claim of completed physical-device manual acceptance.

## 2026-09-10 — Codex — administrator password login and quieter V4 entry

**What:** `docs/openapi/management-v1.json` adds login/password/logout and their closed DTOs;
`web/prism/contracts/management-v1.json` and `src/generated/management-client.ts` are regenerated.
`web/prism/scripts/generate-client.mjs` derives unauthenticated login from OpenAPI security.
`src/api/client.ts`, `src/session/{sessionStore,administrator}.ts`, `src/app/AppShell.tsx`,
`src/features/unlock/{UnlockPage,PasswordField}.tsx`, `src/features/settings/SettingsPage.tsx`,
`src/app/v4.css`, both i18n packs, fixtures and affected tests consume the real session contract.
`web/prism/scripts/check.mjs` keeps the machine-key paste rule on SecretField while permitting
real password fields/autocomplete. Existing V4 real-browser harnesses use account/password.

**Why:** The user explicitly rejected key/token login and excess explanatory text, chose random
initial `admin` credentials in an owner-only local file and mandatory first password change.
Backend includes Argon2id, a separate private singleton account store, bounded expiring/revocable
sessions, exact-Origin login and first-session restrictions. Existing CLI management keys,
control schema 22 and data-plane authorization remain intact. Design was authored by this
session and saved/read through OpenDesign MCP in the existing project, not a model generator.

**Other side:** Full-stack implementation remains authorized; FYI, no separate implementer wait.
Targeted HTTP/contract/security tests, session/store tests, 256 frontend unit tests, initial
10 frontend E2E and the real embedded browser/password/refresh flow passed. Three viewports
1440×900, 1280×720 and 390×844 have real Chromium light/dark captures. Full E2E found one obsolete English-coverage copy assertion; the concise coverage notice
and its three i18n tests now pass (126 other E2E passed). The existing real gateway ten-stage
browser write flow also passes under administrator login. Formal release checks and deployment
receipt are recorded in the delivery report after they finish.


## 2026-09-10 — Codex — administrator login production handoff

**Files:** `docs/handoffs/prism-admin-password-login.md`, `prism-domain-access.md`,
`claude-code-oracle-singapore-vps.md`, `docs/reports/prism-admin-login-delivery.md`.

**What / why:** Record the completed user-authorized CPAR deployment of b30d191 and the
actual account/password entry, replacing historical key-form instructions. The signed ARM64
artifact was independently verified and accepted with an isolated real gateway before cutover.
Both release architectures and the final Fast/full supply-chain gates passed. Public HTTPS
login, initial-session restriction, CSRF rejection, logout, CLI readback and actual in-app browser
UI passed. Initial credentials were delivered through a local owner-only file; no secret is
included in these docs. First password change remains for the user.

**Other side:** FYI under the existing full-stack/deployment authorization. Stop-to-health was
1236 ms. Caddy/DNS/Autoreg and schema 22 were unchanged; the active configuration and 87 historical
billing repair records were preserved. The report distinguishes local Chromium acceptance from
physical Safari/Firefox testing and records the earlier gate/dependency declaration correction.

## 2026-09-10 — Codex — V5 coherent glass workspace and configuration workflow

**What:** `web/prism/src/app/{AppShell,DraftDock}.tsx`, `src/app/v5.css`, `src/main.tsx`,
`src/features/config-versions/{VersionsPage.tsx,versionStore.ts,versionStore.test.ts}`,
`src/features/overview/OverviewPage.tsx`, `src/features/billing/ProcessingStatus.tsx`,
`src/features/accounts/AccountsPage.tsx`, affected configuration-context copy in access, billing,
egress, models, monitoring, runtime, upstreams and usage, both i18n packs, `web/prism/e2e/**`,
and `web/prism/DESIGN.md`. Backend-owned maintenance helper
`scripts/prism-retire-test-drafts.py` and its regression script do not add an API or schema.

**Why:** The user explicitly rejected V4's opaque content slab and confusing topbar version
selector. OpenDesign MCP stores the session-authored V5 design in the existing project.
Three chrome lenses remain; one shared frosted workspace gives content the same material.
Published configuration is selected by default, with deliberate draft/history selection on the
configuration page. Counter semantics, capabilities, same-origin auth, revisions and write
confirmation remain intact. New user confirmation limits cleanup to 11 identified test drafts;
66 resource rows were removed with private backup, audited archive tombstones and an invariant
check over all retained data. Historical events, billing and other configurations are retained.

**Other side:** FYI under the user's continuing full-stack authorization. No contract change or
regeneration drift. Tests use visible configuration-page actions, with no-published-version cases
explicitly arranged. Delivery evidence and verification limits are in
`docs/reports/prism-v5-refinement.md`; production deployment is recorded there only after verification.

## 2026-09-10 — Codex — verified V5 deployment handoff

**Files:** `docs/reports/prism-v5-refinement.md`, `docs/design/prism-v5-evidence/public-login.png`,
`docs/handoffs/{claude-code-oracle-singapore-vps,prism-domain-access}.md`.

**What / why:** Runtime `6489566` is deployed on the existing CPAR domain after both signed
architecture builds, the exact-head full gate and isolated ARM64 real-gateway acceptance.
Public asset hashes match the locally accepted V5 build. Existing admin credentials, published
configuration, request/billing history and the audited test-draft retirement remain intact.
First cutover failed due to a 0700 release directory inherited from backup umask and rolled back;
the script now sets release permissions explicitly and checks execution with the service account
and no extra capabilities. Second cutover passed, stop-to-ready 1227 ms. No database rollback.

**Other side:** FYI. The latest report distinguishes successful final state from the failed
attempt and documents binary-only rollback to b30d191. Caddy, origin settings, DNS and other
services were not changed. Public browser verification is the login page; full authenticated
14-page and write-flow acceptance used the real local gateway and synthetic Provider.


## 2026-09-10 — Codex — readable production resource identities

**What:** `web/prism/src/utils/{resourceNames.ts,resourceNames.test.ts}`,
`src/components/{ResourceIdentity.tsx,resource-identity.css}`, `src/app/DraftDock.tsx`,
`src/features/accounts/AccountsPage.tsx`, `upstreams/{UpstreamsPage,SubresourcePanel,CredentialSheet}.tsx`,
`catalog/{CatalogPage,EffectiveModels}.tsx`, `models/{RouteWorkbench,RoutingInventory}.tsx`,
`access/AccessPage.tsx`, `egress/{EgressPage,CompatibleProxyPanel}.tsx`, `runtime/RuntimePage.tsx`,
`monitoring/MonitoringPage.tsx`, `billing/BillingPage.tsx`, `usage/UsagePage.tsx`,
`config-versions/{VersionsPage,ConfigurationDiff}.tsx`, `audit/{AuditBackupPage,ResourceAudit}.tsx`,
`overview/OverviewPage.tsx`, `web/prism/e2e/{resource-names,compatible-proxy}.spec.ts` and `DESIGN.md`.
Feature paths above are relative to `web/prism/src/features/`.

**Why:** The user rejected P12/test-era IDs and opaque account hashes as the main production
labels. Shared presentation preserves business names, removes historical naming scaffolding and
adds a stable short discriminator. Full IDs stay in technical details/copy and operational values.
Repeated credentials with different bindings remain distinct rows. No database, configuration,
secret, historical event, ledger or canonical model ID is rewritten.

**Other side:** FYI under continuing full-stack authorization. Authoritative sync/check unchanged;
TypeScript and three naming unit tests passed. The 39 focused E2E cases cover account actions,
inspectors, config diff, billing, pools, usage, proxy and effective-model flows. After completing
selector and proxy labels, 16 related cases passed; the case-sensitive ID check was updated to
assert the actual ID child beside its business name and passed on rerun. New E2E verifies that
readable legacy names still copy and send exact original account/provider/channel IDs.
The separately requested OpenDesign Pi/Kimi Coding K3 max design is in progress, not represented
as shipped by this presentation-only batch. No production change in this commit.


## 2026-09-10 — Codex — V6 K3 max layout and coherent production surfaces

**What:** `web/prism/src/app/{AppShell.tsx,v6.css}`, `src/main.tsx`,
`src/features/accounts/AccountsPage.tsx`, `overview/OverviewPage.tsx`,
`billing/ProcessingStatus.tsx`, `config-versions/VersionsPage.tsx`,
`models/{ModelsPage,RouteWorkbench,RoutingInventory}.tsx`, and the configuration-context labels
in `access/AccessPage.tsx`, `audit/ResourceAudit.tsx`, `catalog/CatalogPage.tsx`,
`egress/EgressPage.tsx`, `monitoring/MonitoringPage.tsx`, `runtime/RuntimePage.tsx`,
`upstreams/UpstreamsPage.tsx` (feature paths relative to `web/prism/src/features/`).
Also `src/utils/{resourceNames.ts,resourceNames.test.ts}`, `e2e/monitoring.spec.ts`, `DESIGN.md`,
`docs/design/prism-liquid-glass-v6.html`, `prism-v6-k3-spec.md`, `prism-v6-provenance.json`,
`prism-v6-evidence/`, `docs/handoffs/prism-opendesign-v6.md`, and the V6 delivery report.

**Why:** The user requested real OpenDesign Pi/K3 generation at max, removed test-era names
from primary production labels, and wanted the overall hierarchy and material reconsidered.
The authorized Kimi Coding fallback ran with the actual `:max` Pi selector. Its section rhythm,
asymmetric overview and grouped accounts inform the React implementation. User-required shared
frosted content remains coherent with chrome. Prototype samples, account deduplication assumptions,
fixed model/route pairs and public password initialization are not copied into real behavior.

**Other side:** FYI under continuing full-stack authorization. No API, schema, secret storage,
provider invocation, stable resource ID or historical-data mutation. 262 unit tests, 128 final E2E,
111-operation authority/four-file gate and 3 Rust embed tests passed. Real gateway captured all
14 pages plus login in three sizes/light-dark and passed the 10-stage write/audit/billing/session
flow. A premature tab-test fill was corrected with a committed-tab assertion; prototype table
string/array handling was corrected through MCP. Report records failures and final passes separately.
Signed release and production handoff are recorded only after actual verification.


## 2026-09-11 — Codex — spaced legacy display names found in live readback

**What:** `web/prism/src/utils/{resourceNames.ts,resourceNames.test.ts}` and the V6 report.

**Why:** Read-only production identity metadata exposed three legacy human names such as
`P12-06 official ChatGPT Codex`: their space separator left a leading `06`. The formatter now
recognizes whitespace after the phase number and between legacy words, including staging
scaffolding. Business names without a legacy prefix and all operational IDs remain unchanged.

**Other side:** FYI. Regression cases cover actual observed name formats. All 37 identity/name
references from the read-only inventory now have no residual P12 or numeric phase prefix; the
account inventory has no further cursor. Naming unit tests and resource-identity/inspector E2E
passed. This is a display-only follow-up to deployed V6 6b4e9a7; no production data was renamed.

## 2026-09-11 — Codex — V6 final deployment and handoff receipt

**What:** `docs/reports/prism-v6-refinement.md`,
`docs/reports/evidence/prism-v6-production-20260911.json`,
`docs/design/prism-v6-evidence/public-login.png`, and
`docs/handoffs/{prism-domain-access,claude-code-oracle-singapore-vps,prism-opendesign-v6}.md`.

**Why:** Record the completed authorized rollout of V6 and its final spaced-name fix, so the next
frontend session sees the running revision rather than the preceding V5 deployment. The delivery
report distinguishes full V6 acceptance from the small follow-up's targeted tests.

**Other side:** FYI. Running code is 8a1b537; both signed architectures and the exact-revision
formal gate succeeded. Independent Cosign/artifact verification, isolated real ARM64 auth/assets,
original-service cutover and public readback passed. Stop-to-ready was 1226 ms. The active config,
schema 22, existing administrator store, historical events/ledger and eleven retired draft records
remain intact. No production password access, Provider requests, DNS/Caddy/Autoreg changes or
database restore. Previous V6 binary 6b4e9a7 remains the rollback point. This documents the existing
user authorization only; generic ownership and future deployment permission rules are unchanged.

## 2026-09-11 — Codex — CPAMP task-parity audit and functional redesign proposal

**What:** `docs/reports/prism-cpamp-functional-audit-20260911.md`,
`docs/handoffs/prism-cpamp-functional-redesign.md` and `docs/design/cpamp-parity-20260911/`.

**Why:** The user rejected the internal-object-led navigation and asked for CPAMP-style CPA
management tasks, especially account authorization. Current CPAMP source and its official demo
were compared with fresh local real-gateway captures and current CPAR source/OpenAPI. Findings
include the oauth versus oauth_json UI mismatch, api_key versus bearer runtime mismatch, incomplete
credential inventory, deep account actions and empty-parent draft creation. Previous V6 navigation
and seeded-flow passes do not prove first-account onboarding.

**Other side:** Action required in the next implementation batch: use this functional proposal
instead of treating fourteen top-level columns as a fixed requirement. The suggested eight primary
workspaces are a proposal, not an implemented or newly user-approved navigation. Account lifecycle,
complete inventory, safe current-configuration apply and request-level observability need real
backend contracts. Do not enable every provider's OAuth by changing one string comparison.
This commit changes documentation and fresh synthetic/demo captures only; no production mutation,
provider calls, frontend/backend implementation, contract changes, or data deletion occurred.

## 2026-09-11 — Codex — real credential types in the first functional batch

**What:** `web/prism/src/features/upstreams/{CredentialSheet,SubresourcePanel}.tsx`,
`web/prism/src/dev/fixtures.ts`, `web/prism/e2e/{credential,flows,subresource-crud}.spec.ts`,
and `docs/reports/prism-functional-redesign-progress.md`.

**Why:** User authorized the functional redesign. Show existing Codex OAuth operations for the
actual oauth_json envelope; offer bearer as the new API credential default. Stop using a Grok
fixture to assert Codex authorization. Add a separate normalized Codex fixture and type selection.

**Other side:** FYI under full-stack authorization. Type and contract checks and 20 focused browser
cases passed. Complete account inventory and onboarding remain in progress, not blocked on another
assistant. No backend contract or production state change in this batch.

## 2026-09-11 — Codex — complete managed credential and endpoint inventory

**What:** `crates/gateway-store/src/control_plane{.rs,/resource_inventory.rs}`,
`crates/gateway-http-actix/src/management_resources{.rs,/resource_inventory.rs}`,
`crates/gateway-http-actix/tests/managed_resource_inventory.rs`, authoritative OpenAPI,
`docs/change-requests/CR-PRISM-ACCOUNT-INVENTORY-001.md`, generated Prism contract/client,
`web/prism/src/features/accounts/inventory.ts`, upstream SubresourcePanel, dev fixtures,
API conflict mapping/tests, credential/flows/subresource E2E and the implementation progress report.

**Why:** Runtime binding projections cannot enumerate newly created or unbound resources. The
new protected inventory reads complete credential/endpoint collections with SQL filtering and
bounded pagination. A resource-audit watermark protects continuation across OAuth rotation even
when the graph revision is unchanged. Ciphertext is not selected. Read-only connections execute
under the existing bounded blocking admission. Existing operational bindings remain distinct.

**Other side:** FYI under the authorized functional plan. `listManagedCredentials` and
`listManagedEndpoints` are now real contract operations (113 total); sync-contract generated the
client. The provider child panel displays unbound resources and leaves create actions available
for an empty draft provider. Two real SQLite/HTTP tests, 21 focused E2E, 13 API unit tests and the
authority/four-file gate passed. Initial test fixture compilation/unique-temp-path issues and old
binding-only expectations were corrected before these passes. No production changes. Full account
onboarding and runtime configuration application remain open in the progress report.

## 2026-09-11 — Codex — eight task-oriented primary workspaces

**What:** `web/prism/src/app/{navigation.ts,navigation.test.ts,AppShell.tsx,v6.css}`,
Chinese/English navigation labels, affected E2E navigation helpers and real-gateway entry headings,
`web/prism/DESIGN.md` and the functional progress report.

**Why:** User approved the functional proposal. Remove internal configuration/audit tools from
the primary rail while preserving all existing destinations through workspace subnavigation.
Keep catalog with models and pricing with usage, with correct parent highlighting and old URLs.

**Other side:** FYI. Twelve settings/language/three-size workspace E2E cases passed. Navigation
unit tests verify eight primary entries and reachability of all fourteen old destinations.
This changes information architecture, not the underlying configuration apply semantics. Account
onboarding and live runtime composition work remain tracked separately. No production change.

## 2026-09-11 — Codex — guarded active configuration editing

**What:** Store migration 23 and configuration_edit origin guards; control-plane full graph
fork with credential/proxy AAD resealing; HTTP fork endpoint and version-scoped OAuth workflow
keys; authority/generated 114-operation contract; Prism begin-edit helper, versions action,
fixtures and browser regression; configuration-edit CR and progress report.

**Why:** parent_id did not clone resources. Editing live configuration must preserve unrelated
resources and secrets without returning them to the browser, and reject concurrent rotations
or lifecycle changes before activation. Same credential IDs in copied versions must not share
an OAuth session.

**Other side:** FYI under the approved functional redesign. Three control tests, four SQLite/HTTP
tests, two existing OAuth HTTP tests, migration up/down, browser edit regression, TypeScript,
Clippy and authority/four-file double-build gate passed. Runtime generation switching remains
open; this API starts an edit and does not represent live application. Schema 23 is local only.
No production or historical-data changes.

## 2026-09-11 — Codex — managed account workspace and secret-free status

**What:** control mutation credential_status module; HTTP credential_status module and route;
managed_resource_inventory integration test; authoritative/generated status contract;
`web/prism/src/features/accounts/{AccountsPage,AccountRuntimePanel,AddApiKeyAccount}.tsx`;
upstream provider deep link and inspector comments; dev fixtures and affected account/E2E tests;
CR-PRISM-ACCOUNT-STATUS-001 and functional progress report.

**Why:** A bound runtime record is not a complete account list. Users need to see and manage
unbound credentials and disable them without supplying secrets again. Draft status writes retain
ciphertext, check both revisions and record an audit. Existing runtime controls remain reachable.

**Other side:** FYI under the approved plan. The new API is real, not fixture-only. Browser tests
cover adding an unbound API Key account, secret-free disabling, provider deep links and direct
Codex reauthorization. Initial OAuth/import/bulk flows and actual runtime apply remain pending.
No production, external provider or history mutation occurred in this batch.

Validation for this batch: real SQLite/HTTP ciphertext/CAS regression, four new account browser
checks and nine existing runtime/workspace regressions passed. TypeScript, Clippy and the
115-operation embedded asset gate passed. Screenshots were inspected at desktop and mobile sizes;
mobile account actions use cards. Initial header/type errors were fixed before these passes.

## 2026-09-11 — Codex — channel directory and validated account imports

**What:** `crates/gateway-http-actix/{Cargo.toml,src/management_resources.rs,src/management_resources/account_channels.rs,tests/managed_resource_inventory.rs}`, Cargo.lock; authoritative OpenAPI and generated Prism contract/client; `web/prism/src/features/accounts/{AccountsPage,AddAccountDialog}.tsx` (replaces AddApiKeyAccount), fixtures and managed-account E2E; CR-PRISM-CHANNEL-ENROLLMENT-001 and progress report.

**Why:** User reaffirmed all channel entry points. The first implementation step is an explicit
channel directory and provider-validated imports, replacing the generic bearer-only form. Native
Grok must not be written into the ordinary credential table; incomplete initial OAuth remains
explicitly unavailable rather than misdirected to Codex.

**Other side:** FYI under existing joint implementation authorization. Nine directory entries;
six real ordinary import paths. Two targeted Rust checks and five browser checks passed;
TypeScript and Clippy passed. Native Grok/initial OAuth/full reauthorization remain in-scope work,
not an external implementer's blocker. No production or historical-data changes.

## 2026-09-11 — Codex — native Grok account import and Device OAuth

**What:** `apps/gateway/src/deployment.rs`; HTTP native_accounts/grok_device modules, resource
registration and channel availability; provider-grok account_pool/management module; store
migration 24 and registry; authoritative/generated five native operations; Prism NativeAccounts,
GrokDeviceWizard, AddAccountDialog, AccountsPage, API conflict mapping, fixtures and E2E; native
contract/report and functional progress. Cargo dependencies reuse existing provider/time crates.

**Why:** Native Grok accounts must use their real encrypted global store, not ordinary graph
credential rows. First authorization must persist only a provider grant; reauthorization must
retain identity and reject stale or wrong-account material. Pagination must ignore unrelated
gateway traffic.

**Other side:** FYI under the user's joint implementation authorization. Nine credential channel
entries now connected, including three native Grok imports. Build Device OAuth is available from
the actual deployment composition. Local regression and embedded gates passed; actual local UI
obtained a real device session. User consent is pending, not a completed real grant. Schema 24 is
local only; no production deployment or historical cleanup. Other provider first-OAuth and live
configuration apply work remain required, not implicitly completed by this batch.

## 2026-09-11 — Codex — Chrome live authorization closeout

**What:** AccountsPage group heading/empty-state labels; current native/auth reports, safe live
authorization receipt and functional progress.

**Why:** Actual Chrome enrollment showed a native account while the ordinary-account panel said
no accounts existed. Group-specific labels now make both facts clear. Actual first/repeated Grok
authorization and encrypted restart persistence are verified, replacing the earlier pending status.

**Other side:** FYI. First grant created one encrypted native account; reauthorization retained
its ID and import identity, advanced revision 0→1 and wrote one audit event. The independent
gateway was rebuilt/restarted and API/Chrome readback verified the same record. Type checking and
build passed for the label change; existing native regression evidence remains in the report.
No production deployment/storage change; wider functional-plan work remains incomplete.

## 2026-09-11 — Codex — production release gate corrections

**What:** `scripts/check-crate-boundaries.rb` registers the existing channel import/native
authorization adapter dependencies; `web/prism/e2e/managed-accounts.spec.ts` uses short explicit
fixture token markers. The preceding store test commit adds migrations 23/24 tables to its
expected inventory.

**Why:** Formal deployment checks found stale table/dependency inventories and a synthetic
fixture string matching the credential scanner. No scanner exception or runtime change is added.

**Other side:** FYI under the authorized Oracle release. The provider dependencies reuse actual
credential validators and the injected native store/Device flow. Tests and the final exact-revision
gate must pass before cutover; existing production data and credentials are retained.

## 2026-09-11 — Codex — human account directory presentation

**What:** `gateway-store` account_identity and resource_inventory, control mutation service's
bounded display projector, HTTP managed/native inventory responses and regression; authoritative
OpenAPI plus synced Prism contract; accounts AccountsPage/AccountList/NativeAccounts/inventory/
presentation, V6 styles, fixtures and account E2E. `gateway-store` reuses workspace base64 only
for bounded, display-only JWT identity claims; dependency policy is updated accordingly.

**Why:** User rejects phase labels/digests/import batches as account names, requires six separate
families and consistent Grok Web/Console/Build rows, and needs connection purposes instead of
unexplained endpoint counts. Identity is an allowlisted server-side projection; missing identity
remains null. No token reaches the browser, no provider is contacted and no migration is added.

**Other side:** FYI under joint frontend/backend implementation authorization. Existing identifiers,
revision/CAS/session boundaries, draft writes and native account separation remain. Source labels
such as Autoreg are separate from identities. Identity search is explicitly scoped to loaded rows;
counts do not claim full totals before pagination ends. Further manual identity supplementation is
awaiting user preference. Chrome verification is blocked by an extension popup; fixture Chromium
and actual local gateway API evidence are distinguished in the report. No production deployment.

## 2026-09-12 — Codex — capture provider identity during enrollment

**What:** `apps/gateway/src/credential_refresh.rs`; `crates/provider-grok/src/{oauth.rs,lib.rs,account_pool/management.rs}`;
`crates/gateway-store/src/account_identity.rs`; `crates/gateway-http-actix/src/management_resources/{grok_device.rs,native_accounts.rs}` and
`crates/gateway-http-actix/tests/managed_resource_inventory.rs`; `docs/openapi/management-v1.json` and
`web/prism/contracts/management-v1.json`; `web/prism/src/features/accounts/{AccountList.tsx,AddAccountDialog.tsx,GrokDeviceWizard.tsx,presentation.ts,presentation.test.ts}`;
`web/prism/src/dev/fixtures.ts`, `web/prism/e2e/{native-accounts,managed-accounts,account-presentation}.spec.ts`,
`web/prism/DESIGN.md`; focused report/CR and safe live receipt.

**Why:** User rejected manual identity entry. The grant previously discarded id_token and the
list missed binary credential contents. Identity is now captured automatically and retained inside
AEAD storage; a missing profile uses one fixed issuer userinfo request with sub binding and bounded
transport. No extra scope, token response, provider inference or production change.

**Other side:** FYI under joint implementation authorization. Native authorization name is optional;
views add identity/identity_state. Compact v2 carries profile fields and reads v1; a future production
rollback must account for this format, not just swap an older binary. Chrome's connection still
reports extension occupancy/timeouts despite the user's closed-popup confirmation; no new real
Chrome authorization acceptance is claimed. Manual supplementation is superseded, not pending.

This batch's real local follow-up used the existing durable refresh coordinator for one previously
authorized Build account: one refresh and one issuer profile read, no retry, stable account ID,
revision 1→2, encrypted identity persisted and new gateway API readback passed. This supersedes
the earlier read-only expired-grant finding as the final local status; production remains unchanged.

## 2026-09-12 — Codex — EgoLite account verification and dialog identity

**What:** `web/prism/src/features/accounts/AccountsPage.tsx`,
`web/prism/src/features/upstreams/{CredentialSheet,OAuthWizard}.tsx`,
`web/prism/e2e/{account-presentation,credential}.spec.ts`, `web/prism/DESIGN.md`;
`docs/reports/prism-egolite-20260912.md` and its safe evidence, with the prior identity report linked.

**Why:** The requested EgoLite test reached the actual Codex reauthorization dialog and exposed
an internal phase ID left in its title. Inventory identity/provider now follows details and both
reauthorization entries. Existing metadata supplies identity at other inspector entry points;
missing identity remains explicit. API IDs and OAuth behavior are unchanged.

**Other side:** FYI under the existing joint implementation authorization. EgoLite exercised two
real local gateways, the existing real Grok profile, six groups/eight synthetic accounts, connection
meaning, three sizes and session cleanup. One synthetic draft account was disabled, read back and
restored without publishing. After fixing the title, 13 focused E2E, type check, contract/Prism gates,
gateway build and 122-operation/four-file double build passed; rebuilt gateway UI readback passed.
No new provider requests, authorization grants, production deployment or production state change.

## 2026-09-12 — Codex — account identity production deployment

**What:** Deployed already-reviewed `a243aab` to the existing Oracle Singapore CPAR service/domain;
`docs/reports/prism-identity-production-20260912.md`, its safe evidence JSON and latest Oracle/domain handoffs
record the result. No additional frontend or backend application changes in this receipt commit.

**Why:** User explicitly requested deployment for manual acceptance after the EgoLite verification.

**Other side:** FYI. Both signed architectures and the exact-revision formal gate passed. ARM64
production-copy startup and compact v2→v1 rollback preserving current token bytes passed offline.
The successful cutover took 1245 ms to readiness; existing admin/account/configuration/history data
remain, public assets match, and EgoLite displays the production login page. Some historical grants
still lack human identity; no name is invented. Format-aware rollback is required, not binary-only
rollback. No forced Provider canary, new authorization, password reset, DNS/Caddy/Autoreg change.

## 2026-09-12 — Codex — account identity continuity and duplicate authorization presentation

**What:** `apps/gateway/src/{account_identity.rs,deployment.rs,main.rs,provider_account_pool_adapter.rs,runtime.rs}`;
`crates/gateway-control/src/{account_presentation.rs,lib.rs,provider_account_pool_service.rs}`;
`crates/gateway-store/src/{account_identity.rs,lib.rs}` and
`crates/gateway-store/migrations/0025_native_account_identity.{up,down}.sql`;
`crates/provider-grok/src/{account_pool.rs,account_pool/identity.rs,console_responses.rs,lib.rs,oauth.rs,session_identity.rs}`;
`crates/gateway-http-actix/src/management_resources.rs`, its
`management_resources/{grok_device.rs,native_accounts.rs,resource_inventory.rs}` and
`crates/gateway-http-actix/tests/{managed_resource_inventory.rs,p13_04_management_inventory.rs}`;
`docs/openapi/management-v1.json`, synced `web/prism/contracts/management-v1.json` and
`web/prism/src/generated/management-client.ts`; `web/prism/src/features/accounts/{AccountsPage.tsx,AccountRuntimePanel.tsx,AddAccountDialog.tsx,inventory.ts,inventory.test.ts}`,
`web/prism/src/features/runtime/{PoolActionSheet.tsx,model.ts}`, `web/prism/src/{app/v6.css,dev/fixtures.ts}`,
`web/prism/e2e/{account-actions,account-presentation,provider-pools}.spec.ts` and `web/prism/DESIGN.md`;
`docs/change-requests/CR-PRISM-ACCOUNT-IDENTITY-CONTINUITY-001.md`,
`docs/reports/prism-account-identity-continuity-20260912.md`, its evidence JSON and four synthetic screenshots.

**Why:** User's production acceptance exposed two identical Codex grants, missing SSO email acquisition,
and legacy IDs/upstream labels in runtime status. Directory contacts now group without deleting grants;
runtime connections retain exact targets with human presentation from serving state. Fixed-target SSO
profile reads run automatically on import or explicitly on an existing record, with bounded transport,
encrypted observations, credential/observation CAS and inventory invalidation. Failed profile reads do
not invalidate imported credentials, lock the admin session or automatically replay writes.

**Other side:** FYI under continuing joint frontend/backend authorization. Schema 25 and 123 operations
are candidate-only; production remains `a243aab`/schema 24. Contract was edited at its authority and
synced. Relevant Rust, frontend, contract and four-file checks passed; EgoLite used a real isolated
gateway at three sizes with synthetic identities. Read-only production diagnostics confirmed identical
Codex token/subject values without outputting them. Three Console profile GETs returned 403; one
additional classification within those three confirmed a Cloudflare challenge. This standard-curl
diagnostic does not validate the candidate Chrome transport on Oracle. Real Console emails remain
unretrieved. No production data/configuration changes, inference, new authorization or deployment.
Future release/rollback must account for migration 25; no credential-format change in this batch.

## 2026-09-12 — Codex — identity continuity production deployment

**What:** Released `5b92e15` to the existing Oracle Singapore CPAR service/domain;
`docs/reports/prism-identity-continuity-production-20260912.md`, its evidence JSON/PNG,
`docs/reports/prism-account-identity-continuity-20260912.md`,
`docs/handoffs/claude-code-oracle-singapore-vps.md` and `docs/handoffs/prism-domain-access.md`
record the production state and rollback. No additional application-code changes in this receipt.

**Why:** User explicitly requested deployment for manual acceptance after the local repair.

**Other side:** FYI. Both signed architectures and exact-revision formal checks passed. ARM64
auth/assets and an offline production-copy schema 24→25→24 rehearsal passed, including encrypted
observation readback and retained existing tables/credential bytes. Production now uses schema 25;
cutover took 1236 ms to readiness. Public assets and runtime presentation readback passed; EgoLite
retains the real login entry for the user. Existing admin/configuration/accounts/history remain.
Two Console accounts still have no email; no new Provider profile, refresh, authorization or inference
request was initiated for this release. No DNS/Caddy/Autoreg change. Future rollback requires a fresh
backup and migration 25 downgrade, preserving latest tokens; production rollback was not executed.


## 2026-09-12 — Codex — resource names and exact duplicate grant consolidation

**What:**
- `docs/handoffs/claude-code-oracle-singapore-vps.md`
- `docs/handoffs/prism-domain-access.md`
- `docs/reports/evidence/prism-resource-presentation-20260912-binding-mobile.png`
- `docs/reports/evidence/prism-resource-presentation-20260912-desktop.png`
- `docs/reports/evidence/prism-resource-presentation-20260912-mobile-accessible.png`
- `docs/reports/evidence/prism-resource-presentation-20260912.json`
- `docs/reports/prism-resource-presentation-20260912.md`
- `web/prism/DESIGN.md`
- `web/prism/e2e/access-groups.spec.ts`
- `web/prism/e2e/batch-d.spec.ts`
- `web/prism/e2e/compatible-proxy.spec.ts`
- `web/prism/e2e/configuration-diff.spec.ts`
- `web/prism/e2e/credential.spec.ts`
- `web/prism/e2e/effective-models.spec.ts`
- `web/prism/e2e/flows.spec.ts`
- `web/prism/e2e/i18n.spec.ts`
- `web/prism/e2e/provider-pools.spec.ts`
- `web/prism/e2e/resource-choice-fixtures.ts`
- `web/prism/e2e/resource-names.spec.ts`
- `web/prism/e2e/route-candidates.spec.ts`
- `web/prism/e2e/smoke.spec.ts`
- `web/prism/e2e/subresource-crud.spec.ts`
- `web/prism/e2e/usage.spec.ts`
- `web/prism/src/app/DraftDock.tsx`
- `web/prism/src/components/ChipsInput.tsx`
- `web/prism/src/components/ObjectInspector.tsx`
- `web/prism/src/components/ResourceIdentity.test.ts`
- `web/prism/src/components/ResourceIdentity.tsx`
- `web/prism/src/components/ResourcePicker.tsx`
- `web/prism/src/components/resource-identity.css`
- `web/prism/src/features/access/AccessPage.tsx`
- `web/prism/src/features/accounts/AccountRuntimePanel.tsx`
- `web/prism/src/features/accounts/AccountsPage.tsx`
- `web/prism/src/features/accounts/AddAccountDialog.tsx`
- `web/prism/src/features/billing/BillingPage.tsx`
- `web/prism/src/features/config-versions/ConfigurationDiff.tsx`
- `web/prism/src/features/config-versions/LifecycleConfirmation.tsx`
- `web/prism/src/features/config-versions/VersionsPage.tsx`
- `web/prism/src/features/egress/CompatibleProxyPanel.tsx`
- `web/prism/src/features/egress/EgressPage.tsx`
- `web/prism/src/features/models/ModelsPage.tsx`
- `web/prism/src/features/models/RouteWorkbench.tsx`
- `web/prism/src/features/monitoring/MonitoringPage.tsx`
- `web/prism/src/features/runtime/PoolActionSheet.tsx`
- `web/prism/src/features/runtime/RuntimePage.tsx`
- `web/prism/src/features/upstreams/CredentialSheet.tsx`
- `web/prism/src/features/upstreams/SubresourcePanel.tsx`
- `web/prism/src/features/upstreams/UpstreamsPage.tsx`
- `web/prism/src/features/usage/UsagePage.tsx`
- `web/prism/src/utils/resourceNames.test.ts`
- `web/prism/src/utils/resourceNames.ts`

**Why:** User requested retaining one genuinely duplicate Codex authorization and removing historical
testing IDs/codes across every management surface. Human names replace old phase IDs in ordinary
content, immutable editors, choices and confirmations; exact original values remain in API requests,
URLs, hidden form values and deliberate internal-reference copy/raw exports. Actual identity/model
names are preserved. The narrowly verified identical production grant was removed through a forked,
validated and guarded published configuration, then read back after a controlled service restart.

**Other side:** FYI under continuing joint frontend/backend implementation authorization and the
user's explicit request to consolidate this duplicate. No Rust, schema or OpenAPI/client generation
changes. Production is still binary `5b92e15`/schema 25, active config `production-accounts-20260912`,
Codex credential/connection 1. History, native accounts and admin credentials remain; the old config
is archived. Offline production-copy publish/restart/rollback passed; production rollback was not run.
No Provider/DNS/Caddy/Autoreg action. Frontend candidate is NOT deployed. 274 frontend tests,
TypeScript, 123-operation/four-file double build and real gateway embedding passed. EgoLite checked
14 existing entries at three sizes plus scoped real form read/write and identity/reference behavior;
last dialog fixes were rechecked on the rebuilt gateway. Changed Playwright specs were type-checked,
not run in another browser. Two missing Console emails remain outside this batch's acceptance.


## 2026-09-12 — Codex — resource presentation production deployment

**What:** Released already-reviewed `7432763` to the existing Oracle Singapore CPAR service/domain.
`docs/reports/prism-resource-presentation-production-20260912.md`,
`docs/reports/evidence/prism-resource-presentation-production-20260912.{json,png}`,
`docs/reports/prism-resource-presentation-20260912.md`,
`docs/handoffs/claude-code-oracle-singapore-vps.md` and `docs/handoffs/prism-domain-access.md`
and `web/prism/DESIGN.md` record the production receipt. No new application-code changes in this deployment batch.

**Why:** User explicitly requested deployment for manual acceptance after the resource-name fixes.

**Other side:** FYI. Both signed architectures and exact-revision formal checks passed. ARM64 auth,
four assets and network-isolated production-copy candidate/rollback startup passed. Production remains
schema 25, active config `production-accounts-20260912`, Codex credential/connection 1; stop-to-ready
was 1025 ms. Original administrator store and history remain. Public HTTPS assets/CSP and authenticated
inventory/runtime readback passed; EgoLite retained the production login page without entering an admin
password. No Provider, DNS/Caddy/Autoreg, configuration publication or data deletion in this release.
Rollback to `5b92e15` preserves schema 25 and latest data; do not apply the preceding 25→24 migration
rollback. Production rollback was not executed. Two missing Console emails remain unresolved.

## 2026-09-12 — Codex — complete runtime publication foundation

**What:** `Cargo.lock`, `apps/gateway/Cargo.toml`, `apps/gateway/src/credential_refresh.rs`,
`apps/gateway/src/deployment.rs`, `apps/gateway/src/runtime.rs`, `apps/gateway/src/runtime/reload.rs`,
`crates/gateway-control/src/compatible_egress_runtime_compiler.rs`,
`crates/gateway-control/src/management_service.rs`, `crates/gateway-control/src/snapshot_publisher.rs`,
`crates/gateway-http-actix/src/lib.rs`, `crates/gateway-http-actix/src/management_lifecycle_resources.rs`,
`crates/gateway-router/src/route_snapshot.rs`, `crates/gateway-router/src/runtime_quota.rs`,
`crates/gateway-store/src/control_plane.rs`, `crates/gateway-upstream/src/credential_pool.rs`,
`crates/gateway-upstream/src/lib.rs`, `crates/provider-grok/src/account_pool.rs` prepare and switch complete
serving generations. The new execution record and comparison/runtime reports describe source evidence
and remaining work: `docs/handoffs/prism-cpa-alignment-execution-20260912.md`,
`docs/reports/prism-cpa-alignment-matrix-20260912.md`, `docs/reports/prism-runtime-publication-20260912.md`.

**Why:** User requested real CPA-style frontend workflows and deployment. Save/apply cannot be exposed
as an ordinary action while only the route snapshot updates and execution still requires a restart.

**Other side:** FYI under this session's joint implementation authorization. No OpenAPI/schema or
frontend changes in this commit. The actual frontend simplification and native account mutation sync
remain in progress in this session. Publication prepares before durable activation, checks all relevant
revisions, preserves captured requests and shared concurrency, and switches bounded runtime workers.
Invalid runtime graphs and stale OAuth material fail before activation. Validation and evidence are in
the runtime report; production remains `7432763`, schema 25. No production/Provider/data cleanup action.

## 2026-09-13 — Codex — CPA daily workflows and live account application

**What:** The following exact frontend paths implement normal account/import/batch, provider, model and
client-key workflows, private revisioned save/apply, safe system information, corrected observations,
fixtures and synchronized contracts:

- `web/prism/contracts/management-v1.json`
- `web/prism/e2e/account-presentation.spec.ts`
- `web/prism/src/api/client.ts`
- `web/prism/src/app/v6.css`
- `web/prism/src/components/Sheet.tsx`
- `web/prism/src/dev/fixtures.ts`
- `web/prism/src/features/access/AccessPage.tsx`
- `web/prism/src/features/access/IssueKeyDialog.tsx`
- `web/prism/src/features/access/model.test.ts`
- `web/prism/src/features/access/model.ts`
- `web/prism/src/features/accounts/AccountBatchDialog.tsx`
- `web/prism/src/features/accounts/AccountList.tsx`
- `web/prism/src/features/accounts/AccountsPage.tsx`
- `web/prism/src/features/accounts/AddAccountDialog.tsx`
- `web/prism/src/features/accounts/CredentialUpdateDialog.tsx`
- `web/prism/src/features/accounts/GrokDeviceWizard.tsx`
- `web/prism/src/features/accounts/NativeAccountDialog.tsx`
- `web/prism/src/features/accounts/RuntimeApplyNotice.tsx`
- `web/prism/src/features/accounts/inventory.ts`
- `web/prism/src/features/config-versions/ConfigurationTaskNotice.tsx`
- `web/prism/src/features/config-versions/LifecycleConfirmation.tsx`
- `web/prism/src/features/config-versions/beginEdit.ts`
- `web/prism/src/features/config-versions/configurationTask.test.ts`
- `web/prism/src/features/config-versions/configurationTask.ts`
- `web/prism/src/features/models/ConnectModelDialog.tsx`
- `web/prism/src/features/models/ModelsPage.tsx`
- `web/prism/src/features/models/RouteWorkbench.tsx`
- `web/prism/src/features/models/connectModel.ts`
- `web/prism/src/features/overview/OverviewPage.tsx`
- `web/prism/src/features/settings/SettingsPage.tsx`
- `web/prism/src/features/settings/SystemInformation.tsx`
- `web/prism/src/features/settings/settings.css`
- `web/prism/src/features/upstreams/ProviderDialog.tsx`
- `web/prism/src/features/upstreams/SubresourcePanel.tsx`
- `web/prism/src/features/upstreams/UpstreamsPage.tsx`
- `web/prism/src/features/upstreams/connectionPresets.ts`
- `web/prism/src/generated/management-client.ts`

Backend changes are listed in the same commit and in
`docs/change-requests/CR-PRISM-ACCOUNT-LIFECYCLE-002.md` and
`docs/change-requests/CR-PRISM-SYSTEM-INFORMATION-003.md`. The authority is
`docs/openapi/management-v1.json` (129 operations); the vendored contract and client were generated
with sync-contract. Schema 26 adds append-only native account maintenance audit.

**Why:** User explicitly requested comparison with CPA-Manager-Plus / CLIProxyAPI / Management Center,
removal of the manual identity-read workaround, complete usable daily workflows and deployment.
Identity remains part of authorization/import/replacement. Whole-runtime publication, source CAS,
standby semantics and durable catalog isolation are necessary for truthful save/apply and account
maintenance. Deactivating A must not grant A's discovered models to an unobserved B.

**Other side:** FYI under ongoing joint frontend/backend authorization. No separate implementer action
is required. Eight primary workspaces, fourteen legacy routes, same-origin auth, CSP, in-memory
secrets, exact model IDs and four deterministic assets remain. New interfaces are implemented, not
fixture-only. This commit is not yet deployed. Local test evidence, remaining release checks and
schema 26 rollback constraints are in `docs/reports/prism-cpa-alignment-delivery-20260913.md`.
No production account/history cleanup, DNS/Caddy/Autoreg mutation or new real Provider test request.


## 2026-09-13 — Codex — CPA workflow production acceptance

**What:** `web/prism/DESIGN.md` records the verified live workflow and removes the applicability of
old manual-identity/restart descriptions. `docs/handoffs/prism-cpa-alignment-execution-20260912.md`,
`docs/handoffs/claude-code-oracle-singapore-vps.md`, `docs/handoffs/prism-domain-access.md`,
`docs/reports/prism-cpa-alignment-delivery-20260913.md`,
`docs/reports/evidence/prism-cpa-alignment-local-20260913.json`,
`docs/reports/evidence/prism-cpa-alignment-production-20260913.json` and
`docs/reports/evidence/prism-cpa-alignment-production-20260913.png` contain completed receipts.

**Why:** User explicitly requested deployment after CPA frontends/functionality alignment.
This documentation batch follows the reviewed application commits c7c86b4 and 1b81ce7.

**Other side:** FYI. Production now runs signed 1b81ce7/schema 26, with independent ARM64 signature,
SBOM/assets verification, exact-revision formal gate and network-isolated production-copy 25→26→25
rollback (new audit exported). Public readback and real EgoLite login page passed. Original admin,
accounts/config/history remain; no production data cleanup or manual Provider call. DNS/Caddy/Autoreg
unchanged. Rollback must preserve latest state and downgrade only schema 26 after exporting audit;
new user configuration/native mutations require a fresh compatibility review, not blind rollback.


## 2026-09-13 — Codex — Raw model IDs and provider workspaces

**What:**

- `web/prism/contracts/management-v1.json`
- `web/prism/src/api/errors.ts`
- `web/prism/src/app/AppShell.tsx`
- `web/prism/src/app/v6.css`
- `web/prism/src/dev/fixtures.ts`
- `web/prism/src/features/billing/BillingPage.tsx`
- `web/prism/src/features/billing/ProcessingStatus.tsx`
- `web/prism/src/features/catalog/CatalogPage.tsx`
- `web/prism/src/features/catalog/UpstreamModelBrowser.tsx`
- `web/prism/src/features/egress/CompatibleProxyPanel.tsx`
- `web/prism/src/features/models/ConnectModelDialog.tsx`
- `web/prism/src/features/models/ModelConnectionsDialog.tsx`
- `web/prism/src/features/models/ModelsPage.tsx`
- `web/prism/src/features/models/connectModel.test.ts`
- `web/prism/src/features/models/connectModel.ts`
- `web/prism/src/features/models/models.css`
- `web/prism/src/features/models/useModelConnections.ts`
- `web/prism/src/features/monitoring/MonitoringPage.tsx`
- `web/prism/src/features/settings/SettingsPage.tsx`
- `web/prism/src/features/settings/SystemInformation.tsx`
- `web/prism/src/features/settings/settings.css`
- `web/prism/src/features/upstreams/ProviderDialog.tsx`
- `web/prism/src/features/upstreams/UpstreamsPage.tsx`
- `web/prism/src/features/usage/UsagePage.tsx`
- `web/prism/src/generated/management-client.ts`

The backend implements the authority change in `docs/openapi/management-v1.json` (130 operations);
contracts/client were regenerated through sync-contract. `CR-PRISM-UPSTREAM-MODELS-004.md` explains
saved target-local model enumeration and exact-model multi-source dispatch. No schema change.

**Why:** User requested further alignment with the three CPA projects and deployment. One upstream
can have many models; one exact model can have explicitly configured sources. Model aliases are
optional and do not create duplicate models. Real loopback acceptance exposed and fixed the old
single-Provider inference restriction for authorized exact IDs. Custom mixed mappings, Channel Pin,
stored continuation, credential isolation, protocol semantics and bounded retries retain their scope.

**Other side:** FYI under ongoing joint frontend/backend authorization. Daily provider/model/catalog
workspaces, bulk selection and source editing are implemented; advanced maintenance stays reachable
through existing links. V6 material, eight primary workspaces, fourteen legacy routes, CSP, memory-only
secrets and four deterministic assets remain. Current local evidence: 286 frontend tests, 130-operation
double build, 179 router tests, 71 HTTP library tests, 132 gateway tests, 16 inventory tests and 3 embedded
asset tests. Real UI model/key/alias/source flows and TLS mock requests to ledger passed. No production
mutation in this commit; signed release, production-copy normalization/rollback and public readback
remain required before claiming deployment. Existing history/accounts/admin must be retained.


## 2026-09-13 — Codex — Model workspaces production acceptance

**What:** `web/prism/DESIGN.md` records the verified deployed workspace and V6 precedence.
`docs/handoffs/prism-model-workspace-plan-20260913.md`,
`docs/handoffs/claude-code-oracle-singapore-vps.md`, `docs/handoffs/prism-domain-access.md`,
`docs/reports/prism-model-workspace-delivery-20260913.md` and
`docs/reports/evidence/prism-model-workspace-20260913/acceptance.json` record the completed release;
new screenshots in that same evidence directory cover models, providers, mobile source detail,
settings, auxiliary preferences and public login.

**Why:** Complete the user's explicit request to align functionality/frontends and deploy for manual
acceptance. A production-copy failure exposed duplicate source uniqueness; the existing canonical
bridge already covers the other paths, so normalization retains one equivalent source, not three
invalid duplicate candidates. All six old names remain aliases; original ACL sets/history remain.

**Other side:** FYI. App57e32d1/schema26, signed ARM64 verification, exact-revision formal gate,
isolated production-copy normalization/rollback and HTTPS verification passed. Active configuration
is production-models-20260913, with four original model IDs/four connections. Retained2229 events,
593 ledger records, original accounts/admin and all old links. Same-schema rollback target1b81ce7
must retain current databases and cannot blindly handle later new multi-Provider configurations.
No DNS/Caddy/Autoreg changes, production cleanup or new manual Provider invocation. EgoLite public
login is retained for the user's existing administrator credentials. No separate implementer action.


## 2026-09-13 - Codex - Explicit catalog refresh and manual opening

**What:** `web/prism/contracts/management-v1.json`, `web/prism/src/dev/fixtures.ts`, `web/prism/src/features/access/IssueKeyDialog.tsx`, `web/prism/src/features/accounts/AccountsPage.tsx`, `web/prism/src/features/accounts/NativeAccountDialog.tsx`, `web/prism/src/features/accounts/presentation.test.ts`, `web/prism/src/features/accounts/presentation.ts`, `web/prism/src/features/catalog/CatalogPage.tsx`, `web/prism/src/features/catalog/UpstreamModelBrowser.tsx`, `web/prism/src/features/upstreams/ProviderDialog.tsx`, `web/prism/src/features/upstreams/SubresourcePanel.tsx`, `web/prism/src/features/upstreams/UpstreamsPage.tsx`, `web/prism/src/generated/management-client.ts`, `web/prism/src/utils/resourceNames.test.ts`, `web/prism/src/utils/resourceNames.ts`; authoritative contract `docs/openapi/management-v1.json`,
`docs/change-requests/CR-PRISM-COMPLETE-WORKFLOWS-005.md`,
`docs/handoffs/prism-complete-alignment-execution.md` and
`docs/design/prism-compact-workspace.md`. Generated artifacts updated with sync-contract.

**Why:** Explicit user authorization for joint frontend/backend alignment. Refresh must perform
real metadata I/O without opening models or expanding key permissions. Spaced legacy prefixes and
native account connection protocols need consistent presentation.

**Other side:** FYI. POST /admin/catalog/refresh accepts only endpoint/account IDs, with active
runtime generation, source capability, concurrency, timeout and server credential boundaries.
Catalog pages distinguish complete latest-success count and matching saved count. Unsupported legacy
workflows no longer report fake zero-change success or increment revisions. Frontend distinguishes
refresh from cache reread, and key creation can select all currently opened models explicitly.
This batch is NOT full alignment or deployment. Production service/permission freeze, alias removal,
real source acceptance, unified workflows, request timing and full visual acceptance remain required.


## 2026-09-13 - Codex - Compact provider workspace

**What:** `web/prism/src/app/v6.css`,
`web/prism/src/features/upstreams/UpstreamsPage.tsx`,
`web/prism/src/features/upstreams/SubresourcePanel.tsx`.

**Why:** User requested compact desktop provider rows, a detail workspace and consistent mobile
cards instead of oversized duplicate provider cards. Preserve configured protocols and existing
maintenance actions; move scheduling detail out of the primary view.

**Other side:** FYI under joint authorization. No contract/backend change. EgoLite space16 used
isolated development fixtures: provider creation, detail opening, desktop1440x900, dark1280x720 and
mobile390x844 were inspected. Mobile and desktop detail controls do not overflow viewport width.
This is visual fixture evidence only, not real directory, authorization or full production acceptance.


## 2026-09-13 - Codex - Durable request observations and direct key permissions

**What:** `web/prism/contracts/management-v1.json`, `web/prism/src/components/ResourceIdentity.tsx`, `web/prism/src/dev/fixtures.ts`, `web/prism/src/features/access/AccessPage.tsx`, `web/prism/src/features/access/KeyPermissionsDialog.tsx`, `web/prism/src/features/access/access.css`, `web/prism/src/features/monitoring/MonitoringPage.tsx`, `web/prism/src/features/monitoring/RequestHistory.tsx`, `web/prism/src/features/monitoring/model.ts`, `web/prism/src/features/monitoring/requests.css`, `web/prism/src/features/overview/OverviewPage.tsx`, `web/prism/src/features/usage/UsagePage.tsx`, `web/prism/src/generated/management-client.ts`, `web/prism/src/utils/resourceLabels.ts`. Authoritative request schemas and operations in `docs/openapi/management-v1.json`; generated artifacts produced by sync-contract.

**Why:** Explicit joint implementation authorization. Real request totals and latency require durable terminal evidence at the HTTP delivery boundary; key edits must select models without altering sibling keys.

**Other side:** FYI. Schema27 terminal event, indexed snapshot-bound request history/summary, nullable historical timing, real dashboard/list/detail/export, scoped name resolution and direct key permission editing. Retained model grants do not broaden sources; shared quota groups cannot silently split. Frontend288 tests, HTTP72 tests, large SQLite narrow-window regression, selected all-target clippy and real loopback gateway receipt passed. EgoLite16 verified manual model opening, restricted issuance and editing; zero real-provider inference. Four-file gate is rerun for final CSS. This is a local implementation batch, NOT complete alignment or production deployment. Remaining work is tracked in `docs/reports/prism-complete-alignment-progress.md`; schema26 rollback after terminal writes is intentionally refused pending the production-copy rollback design.


## 2026-09-13 - Codex - Unified searchable account inventory

**What:** `web/prism/contracts/management-v1.json`, `web/prism/src/dev/fixtures.ts`, `web/prism/src/features/accounts/AccountsPage.tsx`, `web/prism/src/features/accounts/useAccountDirectory.ts`, `web/prism/src/generated/management-client.ts`. Backend `crates/gateway-http-actix/src/management_resources/account_inventory.rs`, reused credential projection and authoritative OpenAPI account inventory operation.

**Why:** Explicit user-authorized full workflow alignment: search must cover encrypted identity projections on later source pages, including native accounts, rather than filtering only browser-loaded rows.

**Other side:** FYI. Complete bounded search and counts, shared categories/status/sorting/paging, safe existing operations, snapshot-bound continuation; no upstream call during reads. Two real SQLite/HTTP regression tests and frontend288 tests passed, selected clippy and134-operation four-file gate passed. EgoLite16 actual loopback app mobile filter/empty/recovery/overflow checks passed. Package limits, remaining authorization, price and all-workspace acceptance remain pending; no deployment or production mutations.


## 2026-09-13 - Codex - Price difference confirmation and direct import

**What:** `web/prism/src/features/billing/BillingPage.tsx`, `web/prism/src/features/billing/model.ts`, `web/prism/src/features/billing/model.test.ts`, `web/prism/src/features/billing/billing.css`.

**Why:** User-authorized workflow alignment. Price import needs six-rate differences and explicit confirmation, and daily operations must not require manually selecting a draft.

**Other side:** FYI. Reuses existing immutable global catalog import/rollback and automatic configuration task; preserved catalog save receipts on application failure, added completion view. Local time input labels corrected; mobile rows show all actions. Pure difference tests, frontend290 tests, type/build/134-operation embed gate, and EgoLite16 actual gateway future-price imports passed. Old local catalog retained; no production changes or real inference. Remote price-source sync and the remaining complete-plan work are still pending.


## 2026-09-13 - Codex - Partial workflow production deployment

**What:** `docs/handoffs/claude-code-oracle-singapore-vps.md`,
`docs/reports/prism-workflows-production-20260913.md`,
`docs/reports/prism-complete-alignment-progress.md` and machine receipts/screenshot in
`docs/reports/evidence/prism-workflows-production-20260913/`.

**Why:** User explicitly requested deployment of the completed batches with remaining work tracked.

**Other side:** FYI. Production8578e55/schema27 verified,1022ms stop-to-ready, unchanged active config,
accounts/admin/2229 prior events/593 ledger rows and effective permissions. Exact gate and both signed
architectures passed; ARM64 synthetic auth and isolated production-copy upgrade/fallback/upgrade
passed. Signed915983b is the compatible same-schema fallback; do not use prior schema26 binaries or
restore old databases. Public EgoLite login is user-owned for manual acceptance. Full alignment,
legacy-alias/name migration and missing real-channel acceptance remain unfinished. No DNS/Caddy/
Autoreg edits or real-provider inference in release acceptance.

## 2026-09-14 - Codex - Exact alias deletion and native Build catalog material

**What:** `web/prism/contracts/management-v1.json`, `web/prism/src/generated/management-client.ts`, `web/prism/src/dev/fixtures.ts`, `web/prism/src/features/models/AliasList.tsx`, `web/prism/src/features/models/ModelsPage.tsx`, `web/prism/src/features/models/models.css`; authoritative `docs/openapi/management-v1.json` adds revisioned alias deletion.

**Why:** User-authorized complete workflow alignment and removal of six legacy aliases requires a real exact-alias delete operation while retaining normal custom aliases. Native Grok Build catalog refresh must accept the same persisted credential format as inference.

**Other side:** FYI. Generated via sync-contract (135 operations). Alias delete uses JSON body to preserve slash-containing names, draft/CAS/ownership/audit checks, and automatic configuration lifecycle in Prism. Build catalog now imports active JSON or compact credential material and still rejects expiry. Frontend290 tests, type/build/double-build/embed checks, targeted service/HTTP/Build regressions and workspace all-target/all-feature clippy passed. Actual local synthetic gateway alias create/delete/validate/publish preserved public models and another custom alias; zero Provider calls. EgoLite is paused for user control, so browser acceptance remains pending. Production metadata attempts exposed the Build format bug; actual corrected Build directory and production migration are NOT yet verified or deployed.

## 2026-09-14 - Codex - Visible mobile model actions and batch receipts

**What:** `web/prism/src/features/models/ModelsPage.tsx`, `web/prism/src/features/models/models.css`, `web/prism/src/features/models/connectModelBatch.ts`, `web/prism/src/features/models/connectModelBatch.test.ts`, `web/prism/src/features/catalog/UpstreamModelBrowser.tsx`.

**Why:** Explicit joint implementation authorization. Actual EgoLite mobile acceptance exposed model row actions outside the viewport; directory multi-write failures lacked per-model receipts.

**Other side:** FYI. One model-row DOM adapts to mobile cards with directly visible actions; desktop tables retained. Directory batches stop on the first uncertain write and distinguish saved, existing, uncertain and unattempted models without replay. Frontend292 tests and135-operation double-build/embed gate passed. EgoLite16 actual local gateway verified alias create/read/delete/read, retained custom alias,366px mobile dialog,1440 light layout,1280 dark empty-filter recovery and model details. Full other-workspace and batch browser acceptance, production-copy migration and final release remain pending. Only rebuildable local HTTP target cache was cleaned after disk exhaustion; no credentials/state/history/evidence deleted.

## 2026-09-14 - Codex - First Codex authorization entry

**What:** `web/prism/contracts/management-v1.json`, `web/prism/src/generated/management-client.ts`, `web/prism/src/features/accounts/AddAccountDialog.tsx`, `web/prism/src/features/accounts/CodexEnrollmentDialog.tsx`; authoritative `docs/openapi/management-v1.json` adds first-authorization start/cancel/callback under an exact upstream.

**Why:** Explicit joint implementation authorization; Codex had reauthorization but new accounts required importing an existing export. First authorization must create no placeholder account and must use server-side exchange/persistence.

**Other side:** FYI. Real exchange capability controls channel availability. PKCE scope includes draft/version revision/upstream/proposed ID; start/cancel do not write credentials. Completion runs on a bounded blocking worker, seals/imports with CAS and audit, then Prism connects the chosen interface and applies its owned configuration. Session map bounded128 with expiry cleanup. Secrets never returned; conflicts/uncertain results are not replayed.138-operation double-build/embed gate,292 frontend tests,23 managed-inventory HTTP tests, session-capacity regression and HTTP all-target/all-feature Clippy passed. EgoLite16 actual local gateway verified entry, official-host link generation, invalid callback validation,320px input inside366px mobile dialog, cancellation and unchanged five-account inventory. Real official login/exchange remains NOT verified; HTTP completion uses an injected synthetic exchange. Claude/Kiro first authorization and other recorded plan gaps remain unfinished. No production deployment or real inference.

## 2026-09-14 - Codex - Claude code authorization and OAuth import identity

**What:** `web/prism/contracts/management-v1.json`, `web/prism/src/generated/management-client.ts`, `web/prism/src/features/accounts/AccountsPage.tsx`, `web/prism/src/features/accounts/AddAccountDialog.tsx`, renamed `web/prism/src/features/accounts/CodexEnrollmentDialog.tsx` to `web/prism/src/features/accounts/AuthorizationCodeDialog.tsx`; authoritative `docs/openapi/management-v1.json` adds Claude operations and explicit replacement input.

**Why:** User-authorized full workflow alignment. Claude needs first and repeat authorization through its actual protocol; repeated OAuth imports must not manufacture duplicate accounts or merge distinct organization members.

**Other side:** FYI. Claude-specific fixed URL/client/redirect/scopes, verified-state token exchange and advisory identity read; shared bounded PKCE lifecycle with draft/CAS persistence. Reauthorization preserves ID/disabled state and refuses changed observed account/email. Identical-identity normalized imports rotate one existing account, scoped to upstream/kind.25 management-inventory HTTP tests,17 Anthropic-provider tests,292 frontend tests, selected all-target/all-feature Clippy and141-operation double-build/embed gate passed. Token exchange tests are injected synthetic responses; actual Claude official authorization/browser acceptance is still pending. No production deployment or real inference.

## 2026-09-14 - Codex - Account plan and authentication projection

**What:** `web/prism/contracts/management-v1.json`, `web/prism/src/app/v6.css`, `web/prism/src/dev/fixtures.ts`, `web/prism/src/features/accounts/AccountList.tsx`, `web/prism/src/features/accounts/AccountsPage.tsx`, `web/prism/src/features/accounts/CredentialUpdateDialog.tsx`, `web/prism/src/features/accounts/inventory.ts`, `web/prism/src/features/accounts/useAccountDirectory.ts`, `web/prism/src/features/config-versions/configurationTask.test.ts`, `web/prism/src/features/config-versions/configurationTask.ts`, `web/prism/src/generated/management-client.ts`.

**Why:** Authorized complete workflow alignment requires real plan filters and authentication states across ordinary and native accounts.

**Other side:** FYI. Nullable plan/source never grants model permissions; native entitlement writes invalidate cursors atomically. OAuth/API labels derive from stored credential shape; cooling and unauthorized remain distinct from operator pause. Owned configuration reads recheck ownership after await. Store/HTTP/native regressions,293 frontend tests, selected Clippy and141-operation embed gate passed. EgoLite16 actual synthetic gateway verified free1/max20x1/unknown5/all7 filtering; empty unrelated groups hidden. Real official authorization and production deployment remain pending.

## 2026-09-15 - Codex - Remaining account, key and price workflows

**What:** `web/prism/contracts/management-v1.json`, `web/prism/src/components/Sheet.tsx`, `web/prism/src/features/access/AccessPage.tsx`, `web/prism/src/features/access/KeyPermissionsDialog.tsx`, `web/prism/src/features/access/model.ts`, `web/prism/src/features/accounts/AddAccountDialog.tsx`, `web/prism/src/features/billing/BillingPage.tsx`, `web/prism/src/features/billing/billing.css`, `web/prism/src/features/billing/model.ts`, `web/prism/src/features/config-versions/configurationTask.test.ts`, `web/prism/src/features/config-versions/configurationTask.ts`, `web/prism/src/generated/management-client.ts`, `web/prism/src/features/accounts/KiroDeviceDialog.tsx`, `web/prism/src/features/billing/PriceEntriesEditor.tsx`, `web/prism/src/features/billing/PriceEntriesEditor.test.ts`.

**Why:** Explicit joint alignment implementation authorization. Complete Kiro device entry, real key request metadata, editable six-rate prices with public source preview, and shared unsaved-form protection.

**Other side:** FYI.145 generated operations; schema28 is an index-only migration, requiring updated compatible fallback before deployment. Nullable times/rates are never guessed. Source quote read is separate from catalog import; same-origin client and four embedded files retained. Kiro secrets stay server-side; injected exchange HTTP test proves no placeholder/poll interval/persistence/replay.81 store tests,28 inventory HTTP tests,296 frontend tests, selected Clippy and145-operation double-build/embed gate passed during this batch. Actual EgoLite local gateway read models.dev10-source quotes, selected a quote, manually supplied missing synthetic rates, confirmed one future catalog and reread it; existing prices retained. Unsaved cancel/dismiss retains input. No real inference or production deployment. Shared access-group quota editing is NOT claimed: runtime rejects nonempty limits; the temporary UI extension was withdrawn.

## 2026-09-15 - Codex - Complete narrow-screen actions and isolated catalog acceptance

**What:** `web/prism/src/app/v6.css`, `web/prism/src/features/access/AccessPage.tsx`, `web/prism/src/features/audit/AuditBackupPage.tsx`, `web/prism/src/features/audit/ResourceAudit.tsx`, `web/prism/src/features/catalog/CatalogPage.tsx`, `web/prism/src/features/egress/EgressPage.tsx`, `web/prism/src/features/runtime/RuntimePage.tsx`.

**Why:** Full-size EgoLite acceptance found advanced-table actions outside the mobile viewport. Production-copy metadata verification must not race live refresh grants.

**Other side:** FYI. Static field labels and responsive rows preserve the same desktop data; mobile runtime matrix repeats its real account controls per cell. EgoLite16 captured all14 routes at1440×900,1280×720,390×844 in light/dark; corrected catalog/access/runtime/egress/audit actions have no horizontal overflow. Browser screenshots and receipts are under output/prism-final-qa-20260915.145-operation double-build/embed gate and gateway checks passed. New marked-copy-only catalog-check uses the same runtime discovery without listeners/maintenance/startup renewal; actual local two-page three-model read passed with unchanged credential rows and zero inference. Alias migration fingerprints now exclude observational last-request timestamps. Production migration/deployment and real upstream metadata are still pending.

## 2026-09-15 - Codex - Unified account inspector and Kiro reauthorization

**What:** `web/prism/contracts/management-v1.json`, `web/prism/src/app/v6.css`, `web/prism/src/components/Sheet.tsx`, `web/prism/src/features/accounts/AccountsPage.tsx`, `web/prism/src/features/accounts/AccountEvidenceTabs.tsx`, `web/prism/src/features/accounts/KiroDeviceDialog.tsx`, `web/prism/src/features/accounts/NativeAccountDialog.tsx`, `web/prism/src/features/upstreams/CredentialSheet.tsx`. Authoritative Kiro input adds optional replacement flag.

**Why:** Authorized completion of account overview/quota/configuration/models/diagnostics and repeat authorization.

**Other side:** FYI. Same existing account projection/runtime/catalog APIs; unknown balance remains unknown. Device replacement binds observed credential revision, preserves disabled status and interface bindings, and rejects stale completion. Generated contract synchronized. Expanded injected-exchange HTTP regression and workspace Clippy passed;296 frontend tests and145-operation embedded/double-build checks passed. EgoLite16 actual local gateway verified five inspector tabs and existing-account Kiro entry; no real official login completion or inference claimed. Production release still pending.

## 2026-09-15 - Codex - Complete alignment released

Signed e77bcb5/schema28 is live on existing Oracle CPAR. Exact-revision formal gate and both architectures passed; main/fallback Cosign, SBOM and artifact checks verified. Network-isolated production-copy candidate/migration/fallback/candidate and signed ARM64 authentication passed. Active production-aligned-20260915 uses formal provider names, removes six legacy aliases and sets verified Krill model-list paths. Original model permissions,7 accounts,2229 events,593 ledger rows and administrator retained. Real metadata refresh: Build1/2 models, one expired account; Krill33 per interface. No real inference or DNS/Caddy/Autoreg edits. Fallback b792a99 is schema28/current-credential compatible: retain latest state, never restore stale credentials. Final report: docs/reports/prism-complete-alignment-delivery-20260915.md; official user login checks remain explicitly separate. EgoLite public login retained for human acceptance.

## 2026-09-15 - Codex - Prism daily-workspace alignment

**What:** `web/prism/src/api/{client.ts,client.ownership.test.ts,errors.ts,errors.test.ts}`, `web/prism/src/app/{AppShell.tsx,v6.css}`, `web/prism/src/components/ResourcePicker.tsx`, `web/prism/src/features/accounts/{AccountsPage.tsx,AddAccountDialog.tsx}`, `web/prism/src/features/models/{ModelsPage.tsx,useModelConnections.ts,useModelConnections.test.ts}`, `web/prism/src/features/monitoring/{MonitoringPage.tsx,RequestHistory.tsx,RequestHistory.test.ts,model.ts}`, `web/prism/src/features/overview/{OverviewPage.tsx,metrics.ts}`, and `docs/reports/prism-c2c-alignment-20260915.md`.

**Why:** User-authorized joint frontend ownership and CPAMP/CLIProxyAPI/Management Center workflow alignment. Daily management needs clear account authorization/import, compact provider and model workspaces, truthful request observation, and a coherent Apple Liquid Glass data plane without surfacing internal configuration revisions in normal navigation.

**Other side:** FYI. Configuration bootstrap remains presentation-free on success and gives a retryable warning on failure, so version-scoped account/provider/model/key reads retain their active context without indefinite loading. Bounded management GETs are serialized; read-capacity rejections retry only for reads, while writes and conflicts never replay. The request filter preserves dashboard deep-link ranges unless an operator selects a new preset. No management contract, generated client, secret handling, external provider behavior, production state, DNS/Caddy, or Autoreg configuration changed. Local real-gateway/owned-loopback acceptance and EgoLite desktop/mobile checks are recorded in the report; signed release and public manual acceptance remain next.

## 2026-09-15 - Codex - Prism daily-workspace alignment release

**What:** Signed revision `7988bffd60d4c40cfa45c7cee39126b7d441043b` was built in GitHub Actions run `34946952551`, verified as native ARM64, installed into the existing Oracle CPAR release directory, and atomically selected by the service. Deployment evidence is in `docs/reports/prism-c2c-alignment-production-20260915.md`.

**Why:** The user explicitly authorized deployment after the reviewed daily-workspace alignment implementation and local gateway/EgoLite acceptance.

**Other side:** FYI. The service switched from `e77bcb5` to `7988bff`, reached loopback health in 910 ms, and retained the prior schema28 binary for rollback. Existing administrator, accounts, active configuration, requests and ledger remain in place. Public administrator-login and all four embedded assets/CSP read back successfully. No production Provider request, data cleanup, DNS/Caddy/firewall, Autoreg, credential, alias, or configuration mutation occurred.

## 2026-09-15 - Codex - Prism bounded read recovery hardening

**What:** `web/prism/src/api/{client.ts,client.ownership.test.ts}`, `web/prism/src/features/monitoring/{RequestHistory.tsx,RequestHistory.test.ts}`, and `docs/reports/prism-c2c-alignment-20260915.md`.

**Why:** ChatGPT review and the owned local gateway exposed two correctness edges in the workspace-alignment batch: historical exact intervals were presented as moving presets, and a stalled management GET could retain the shared read slot indefinitely.

**Other side:** FYI. Exact `from_ms`/`to_ms` links now always show a historical-range option until an operator selects a relative preset; filtering preserves the range and selecting any preset replaces it. The bounded management-read scheduler gives active reads a deadline and lets queued work cancel without dispatch. Writes remain outside the queue; conflicts and writes are never replayed.307 frontend tests, type/build/double-build checks and embedded UI Rust regressions passed. No contract, provider, credential, production data, DNS/Caddy, Autoreg, or external request behavior changed in this commit.

## 2026-09-15 - Codex - Prism bounded read recovery release

**What:** Signed revision `0ff3b82e715ce750f05a7116b444f375edfd60dd` was built in GitHub Actions run `34948527229`, independently verified as native ARM64 through its release manifest and Sigstore bundle, installed into the existing Oracle CPAR release directory, and atomically selected by the service. Deployment evidence is in `docs/reports/prism-c2c-alignment-production-20260915.md`.

**Why:** The user explicitly authorized deployment after the final ChatGPT review corrections for truthful historical request ranges and a deadline/cancellation-safe management-read scheduler.

**Other side:** FYI. The service switched from `7988bff` to `0ff3b82`, reached loopback health before the 12-second rollback deadline, and retains `7988bff` as its immediate schema28-compatible rollback. Existing administrator, accounts, active configuration, requests and ledger remain in place. Public administrator-login and all four embedded assets/CSP read back successfully through EgoLite. No production Provider request, data cleanup, DNS/Caddy/firewall, Autoreg, credential, alias, or configuration mutation occurred.

## 2026-09-15 - Codex - Channel-owned account targeting

**What:** `crates/gateway-http-actix/src/management_resources/account_channels.rs` and `web/prism/src/features/accounts/AddAccountDialog.tsx` stop named account channels from borrowing generic compatible upstreams and stop the ordinary account flow from silently selecting the first matching Provider or Endpoint.

**Why:** A Kimi onboarding flow showed Codex/Krill and their connections because protocol compatibility was incorrectly treated as account-channel ownership. That made an authorization/import capable of attaching credentials to an unrelated service.

**Other side:** FYI. Codex, Claude, and Kimi now require an exact channel-owned upstream; only the explicit API-key branches show service and connection controls. Named channel onboarding fails visibly for zero or multiple dedicated targets until the shared target-preparation and Kimi authorization work lands. No credential, model access, Provider request, or production state changed in this batch.

## 2026-09-15 - Codex - Kimi Coding device authorization

**What:** `docs/openapi/management-v1.json`, synchronized Prism contract/client/fixture, `crates/gateway-http-actix/src/management_resources/{account_channels.rs,kimi_device.rs}`, deployment composition, `crates/provider-openai-compatible/src/runtime_credential.rs`, `apps/gateway/src/runtime/{.rs,catalog_refresh.rs}`, `web/prism/src/features/accounts/{AddAccountDialog.tsx,KimiDeviceDialog.tsx}`, and `configurationTask.ts`.

**Why:** The Kimi account path was an API-key import presented as authorization, exposed unrelated Provider/Endpoint choices, and could fall through to the Grok wizard. The channel needs its own target, OAuth device session, runtime credential boundary, and interaction rather than a relabelled generic flow.

**Other side:** FYI. A Kimi Coding selection now resolves or prepares only the canonical `api.kimi.com/coding` Responses target in the owned draft, then opens a dedicated Kimi device-code dialog with automatic bounded polling. Existing Moonshot/Kimi API-key setup remains a separate `Kimi API` channel and is never repurposed. The browser receives only the code, verified Kimi URL and status; compact poll states retain the original challenge until a terminal receipt. Access/refresh/device material is zeroized, held server-side, persisted only after a successful revision-bound exchange, used for Kimi request/catalog headers, and renewed by the bounded serving worker only for a Kimi Coding credential bound to the canonical endpoint. Pending/slow-down, denial, expiry, cancel, malformed data, stale scope and replay are closed. Legacy Codex OAuth handlers reject Kimi credentials at the backend. The generated contract was synchronized rather than edited. Injected Kimi device HTTP integration, Kimi target/API separation, credential tests, frontend type/tests/build and Rust checks passed; no real Kimi login, inference, production configuration, or deployment occurred.

## 2026-09-16 - Codex - Channel-owned Kiro device authorization

**What:** `docs/openapi/management-v1.json`, synchronized `web/prism/contracts/management-v1.json` and `web/prism/src/generated/management-client.ts`, `crates/gateway-http-actix/src/management_resources/{account_channels.rs,account_inventory.rs,kiro_device.rs}`, route composition, inventory HTTP regressions, `web/prism/src/features/accounts/{AddAccountDialog.tsx,KiroDeviceDialog.tsx,AccountsPage.tsx,AccountRuntimePanel.tsx}`, `CredentialSheet.tsx`, device/task tests, and dev fixtures.

**Why:** Built-in Kiro onboarding still depended on a preconfigured Provider/Endpoint and manual polling, while Kimi credentials could be hidden behind generic OAuth maintenance rules. Ordinary account workflows must be channel-owned and must not expose or infer infrastructure choices.

**Other side:** FYI. Kiro first authorization now prepares a region-owned canonical Kiro upstream, HTTPS egress and Messages endpoint inside the same owned draft after the operator starts; the browser only sees Kiro and optional organization settings. Compact polls retain the device challenge, honor the server interval, and end clearly on denial, expiry, cancellation or malformed token data without creating an account. Kimi OAuth accounts now advertise exact reauthorization and inspector actions use server-projected categories rather than the shared `oauth_json` storage label. Kiro/Kimi target and terminal-state tests, frontend tests/build, SPA checks and HTTP Clippy passed. Actual official login, real gateway/EgoLite acceptance and deployment remain pending.

## 2026-09-16 - Codex - Exact renewal owners and direct Codex/Claude setup

**What:** `docs/openapi/management-v1.json`, synchronized Prism contract/client, `account_channels.rs`, management route composition, `codex_enrollment.rs`, legacy OAuth guards, inventory regressions, and `web/prism/src/features/accounts/{AddAccountDialog.tsx,AuthorizationCodeDialog.tsx,KimiDeviceDialog.tsx,KiroDeviceDialog.tsx}` with focused tests.

**Why:** Independent review found renewal paths could invoke first-authorization target preparation, Kiro import could depend on stale provider enumeration, and stricter new-channel ownership could strand verified legacy OAuth accounts.

**Other side:** FYI. Codex and Claude first authorization and import now prepare their fixed channel-owned targets after the explicit action; their exact existing owner is retained during renewal. Kimi renewal requires the selected credential owner and never prepares or changes another Coding target. Kiro imports derive one non-secret declared region from transient material before any draft change, prepare that exact region target, and never infer a binding. Legacy `openai-compatible` Codex and `anthropic-compatible` Claude credentials remain renewable only after their sealed material validates as the correct OAuth family; Kimi remains rejected at Codex boundaries. Contract sync, 317 frontend tests/build/SPA gate, format/Clippy, and all 37 managed-resource HTTP regressions passed. Official login, real gateway/EgoLite acceptance and deployment remain pending.

## 2026-09-16 - Codex - Explicit account and catalog selections

**What:** authoritative `docs/openapi/management-v1.json`, synchronized `web/prism/contracts/management-v1.json` and `web/prism/src/generated/management-client.ts`, `web/prism/src/dev/fixtures.ts`, `crates/gateway-http-actix/src/management_resources/account_channels.rs`, `crates/gateway-http-actix/tests/{managed_resource_inventory.rs,p10_01_management_openapi_contract.rs}`, `web/prism/src/features/accounts/{AddAccountDialog.tsx,AddAccountDialog.test.ts}`, and `web/prism/src/features/catalog/{UpstreamModelBrowser.tsx,UpstreamModelBrowser.test.ts}`.

**Why:** The ordinary account and catalog workflows still inferred an operator choice from a singleton or first inventory row. Kimi Coding also accepted a validated OAuth export server-side but incorrectly hid its import entry.

**Other side:** FYI. API-key setup now requires an explicit service choice even with one configured service. Catalog browsing requires an explicit endpoint and account, preserves explicit deep links, and cannot switch to another resource when a current selection disappears. Kimi Coding now presents both device authorization and strictly validated OAuth JSON import; its import prepares the exact Kimi target inside one revisioned draft and does not bind an observed endpoint. `importChannelAccount` declares both distinct Kimi modes (`kimi-api`, `kimi-coding`), and a contract regression prevents this UI/backend capability drift. 39 Prism test files/320 tests, targeted and complete managed-resource HTTP coverage, format/Clippy, contract/embed gates and the double-build SPA gate passed. EgoLite verified the current embedded React application at 1440×900 and 390×844 without a Provider or Endpoint selector for Codex, Claude, Kimi or Kiro; no official authorization or Provider inference was requested.

## 2026-09-16 - Codex - Remove implicit provider-card endpoint selection

**What:** `web/prism/src/features/upstreams/{UpstreamsPage.tsx,model.ts,model.test.ts}`.

**Why:** The provider card’s “开放模型” link embedded the first observed endpoint in its URL. A multi-protocol upstream could therefore enter the model workflow with an unchosen Responses, Chat Completions or Messages endpoint.

**Other side:** FYI. The link now enters the model workspace without `from_endpoint`; the explicit endpoint selector owns that choice. A unit regression guards the neutral URL, relevant Prism tests/build passed, and a source search confirms no first-item account/catalog/endpoint selection remains.

## 2026-09-16 - Codex - Prism shared Sheet and daily account workflow refinement

**What:** `web/prism/src/{components/Sheet.tsx,components/modalNavigationGuard.ts,components/ChipsInput.tsx,design/{tokens.css,modal.css},app/{app.css,v4.css},dev/fixtures.ts}`, daily account/access/model/provider/egress Sheet callers, and their browser regressions.

**Why:** The management console had visually inconsistent sheets, unstable form actions, browser-confirm discard prompts, focus escape, and a HashRouter Back path that could hide a one-time key receipt during its pending write. Account maintenance also used direct inner “Back” transitions that could lose replacement material.

**Other side:** FYI. The shared Sheet now owns the portal-safe frame, semantic modal tokens, focus isolation, in-panel discard choice, busy route protection and stable footer. Daily forms retain their existing generated-client request, revision, secret-cleanup, channel-owner and one-time-receipt behavior while placing submit controls in form-associated footers. The fixture test control stores method/path counts only; it never records request material. Current evidence: type check, 39 unit files/321 tests, four-file production build, SPA gate, and six serial daily modal/native E2E cases. C2C reviews are recorded in task `c2c_f3d9`; broader editor migration, full E2E recovery, real gateway/EgoLite acceptance and deployment remain pending.

## 2026-09-16 - Codex - Router-owned Sheet history restoration

**What:** `web/prism/src/components/Sheet.tsx`, `web/prism/e2e/{helpers.ts,modal-daily.spec.ts}`, and `docs/design/prism-modal-workspace-refinement-20260916.md`; removed `web/prism/src/components/modalNavigationGuard.ts`.

**Why:** The earlier busy Sheet listener reconstructed a HashRouter entry with `pushState` after a browser traversal. That could sever intervening or forward entries and leave router index metadata inconsistent after a multi-step Back or Forward.

**Other side:** FYI. Pending writes and dirty forms now rely only on React Router's indexed POP blocker, which restores the original entry without mutating browser history directly. The regression creates a real, router-owned stack around API-key issuance, observes both the blocked departure and restoration for multi-step Back and Forward, then proves post-receipt Back → Keep editing and Back → Discard work without a reload or duplicate key issuance. The fixture hold records only a request method/path count. Focused browser regression and frontend type check passed; C2C review, broader editor migration, full E2E recovery, real gateway/EgoLite acceptance and deployment remain pending.

## 2026-09-16 - Codex - Channel authorization Sheet lifecycle

**What:** `web/prism/src/components/Sheet.tsx`, `web/prism/src/design/modal.css`, `web/prism/src/features/accounts/{AuthorizationCodeDialog,KimiDeviceDialog,KiroDeviceDialog,GrokDeviceWizard}.tsx`, `web/prism/e2e/channel-authorization.spec.ts`, and the Prism modal-workspace design ledger.

**Why:** Channel authorization panels still buried phase actions in scrolling content. Kimi/Kiro hid a valid code and official link while polling, while an accepted close or router departure could close a live authorization without first ending the channel session.

**Other side:** FYI. The shared Sheet now offers an opt-in asynchronous dismissal boundary and a live-session route blocker: a channel cancels a live authorization before a close, Escape or router-admitted departure; failed cancellation leaves the panel and its error active. Callback authorization has a real form, associated footer submit action and inline validation. Kimi/Kiro retain code and verified link during compact or in-flight polling; if a consuming poll becomes uncertain after its server session disappears, polling stops, the stale challenge is removed and the only path is an explicitly labelled local exit, with no replay or cancellation request. Kiro options use a native form. A terminal poll whose credential persisted but whose configuration application failed is a saved-but-not-applied outcome, not a retryable poll. Grok preserves the native consent/identity path while cancellation happens before close. The fixture tests hold only method/path routes, verify the challenge remains visible during an actual held poll, direct cancellation, Kimi/Kiro unknown-result exit and browser Back cancellation before departure; no secrets or callback values are observed. C2C task `c2c_5477` approved the lifecycle correction and matching seven-case Chromium record at iteration 7. This checkpoint does not yet migrate the legacy credential OAuth wizard or certify real-provider login, production behavior, full E2E or deployment.

## 2026-09-19 - Codex - Legacy credential OAuth renewal lifecycle

**What:** `web/prism/src/features/upstreams/{OAuthWizard,CredentialSheet,model}.tsx`, `model.test.ts`, `web/prism/src/dev/fixtures.ts`, `web/prism/e2e/credential-oauth.spec.ts`, and the modal-workspace design ledger.

**Why:** The remaining legacy renewal dialog used body-local actions, discarded its official challenge whenever compact status omitted the URL, and could leave a live server-side OAuth session when an operator closed or traversed away. The fixture also disagreed with production by consuming the legitimate session after a callback from another browser flow.

**Other side:** FYI. A renewal attempt now binds its selected configuration, selection generation, session generation and credential before starting. Its challenge is retained only for that attempt; ordinary status responses do not repeat or erase it, and the full URL is no longer printed in the panel. Attempts carry a module-monotonic identity, use an exact query key and remove only their own query cache on retirement, preventing a reopened renewal from borrowing an old complete or expired status. The callback is a native transient form with an associated stable footer submit action, contract-length validation and field-local feedback; it is dispatched by an owned async function rather than retained as TanStack mutation variables. Only an acknowledged callback creates a success receipt. The legacy string-shaped OAuth error response is normalized at the client boundary; a rejected callback is reconciled through a fresh status read before the correct callback is allowed to be submitted again, including when the first reconciliation read temporarily fails. A process-restart durable-account fallback, a lost callback response, or acknowledged cancellation followed by a failed or still-pending status reread remain explicitly unconfirmed and offer safe reread/local exit without replaying either write. Completion performs an exact redacted credential reread before claiming it; failure of that reread leaves the acknowledged receipt intact without claiming the reread succeeded. A close, Escape or router Back cancels a pending server session before the Sheet admits departure; failed cancellation keeps the official link and actionable error. The fixture mirrors idempotent pending start, compact status, production-shaped non-consuming mismatched callbacks and controlled durable/transport cases. The renewal entry accepts the existing server-projected Codex metadata fallback when it is opened from Provider or Runtime rather than the account directory; `oauth_json` alone remains insufficient. Fourteen serial Chromium cases cover cache isolation for expired/complete results, validation, production-envelope rejection recovery after a transient status failure, acknowledged completion with reread failure, lost response, cancellation uncertainty/pending status, durable fallback and Back. C2C task `c2c_8a4d` approved the B1–B5 correction at iteration 4. Full gates, real gateway/EgoLite acceptance and deployment remain pending.

## 2026-09-19 - Codex - Prism provider workspace checkpoint

**What:** `web/prism/src/features/upstreams/{UpstreamsPage,SubresourcePanel,subresourceModel}.tsx`, `subresourceModel.test.ts`, `web/prism/src/{app/v6.css,design/modal.css,dev/fixtures.ts}`, `web/prism/e2e/subresource-crud.spec.ts`, and `docs/design/prism-modal-workspace-refinement-20260916.md`.

**Why:** Provider interface and account management used fragile local selection, technical table rows and scrolling form actions. A post-save inventory reset could also be cancelled by a revision change, leaving the current workspace on an indefinitely stale read.

**Other side:** FYI. The provider workspace selection is route-backed; endpoint/account rows use protocol, host, identity, access method and status rather than internal IDs. Endpoint and binding forms use the shared Sheet footer and constrained scheduling fields. Advanced binding tables become labelled cards on mobile while desktop preserves the comparison matrix. Resource and publication response loss, missing ETag receipt, version mismatch, owner cancellation and endpoint target preconditions now resolve to truthful non-replayable outcomes. Endpoint comparisons use declared fields rather than serialization order; a pending receipt remains tied to its original Provider workspace. Endpoint and credential fixture DELETE responses now return the required revision receipt, matching the configuration task boundary. Current local evidence: 41 unit files / 337 tests, 47 serial Chromium cases, four-file double build / SPA gate and 3 embedded-management UI Rust tests. C2C task `c2c_b63f` approved this Provider action and transaction checkpoint at iteration 3. No management contract, generated client, backend source, Provider request, production configuration or production data changed.

## 2026-09-19 - Codex - Prism account maintenance checkpoint

**What:** `web/prism/src/features/accounts/AccountBatchDialog.tsx`, `AccountRuntimePanel.tsx`, `AccountsPage.tsx`, `CredentialUpdateDialog.tsx`, `RuntimeApplyNotice.tsx`, `accountActionModel.ts`, `accountActionModel.test.ts`; `web/prism/src/features/runtime/PoolActionSheet.tsx`; `web/prism/src/dev/fixtures.ts`; `web/prism/e2e/account-actions.spec.ts`, `helpers.ts`, `managed-accounts.spec.ts`, and `resource-names.spec.ts`; and `docs/design/prism-modal-workspace-refinement-20260916.md`.

**Why:** Account maintenance still mixed ordinary and native resource authority, could present stale or changed account targets during confirmation, and had inconsistent partial-operation receipts and form actions.

**Other side:** FYI. Batch operations now retain the exact captured account namespace and Provider owner, verify them before ordinary/native writes, and distinguish applied, saved, rejected, unexecuted and unconfirmed items. Credential replacement preserves its owner and requires an explicit supported status when the observed operational status is read-only. Runtime details resolve ordinary credentials through their exact endpoint binding or native accounts through complete paginated inventory, then hand a stable target to the editor; failed rereads cannot open cached maintenance. Account actions use the shared Sheet footer and guarded receipt. C2C `c2c_a74e` approved the full account-maintenance batch at iteration 6. Current local checks: type check, 42 unit files/340 tests, 31 serial account Chromium cases, four-file double build and management SPA gate, three embedded UI Rust tests, plus an isolated real gateway/EgoLite one-account disable and server-readback. Official login, whole-app visual acceptance and deployment remain pending. No authoritative OpenAPI, generated client, backend source, production account or historical data changed.

## 2026-09-20 - Codex - Prism model and catalog lifecycle checkpoint

**What:** `web/prism/src/features/models/{ConnectModelDialog,ModelConnectionsDialog,ModelsPage,useModelConnections,modelTask}.ts*`, `modelTask.test.ts`; `web/prism/src/features/catalog/{CatalogPage,UpstreamModelBrowser,CatalogConnectDialog}.tsx`; `web/prism/src/features/config-versions/{configurationTask.ts,configurationTask.test.ts}`; `web/prism/src/dev/fixtures.ts`; `web/prism/e2e/{model-catalog-lifecycle,effective-models,route-candidates,helpers}.ts`; and `docs/design/prism-modal-workspace-refinement-20260916.md`.

**Why:** Model and directory actions mixed exact upstream identity with a first candidate, allowed several competing editor forms, hid partial writes and active no-op forks, and could confirm a changed directory or endpoint. The catalog also lost its bounded continuation control.

**Other side:** FYI. One captured source is edited or removed through a stable Sheet action and non-replayable staged receipt; an actual last-enabled removal discloses the model-disable consequence. Directory selection freezes an explicit endpoint/account, exact IDs, source revision and catalog snapshot; every page and the admitted working context are rechecked before writes. Active duplicate connections are detected before forking. Source fields remain locked during candidate writes and publication. Current evidence: 43 unit files/347 tests, 24 serial model/catalog/route Chromium cases, 56 related regression cases, type check, double-build/four-file SPA gate, three embedded UI Rust tests, and isolated real-gateway/EgoLite source edit, catalog activation, no-op version-count and 390px checks. C2C `c2c_2f8c` approved this complete checkpoint at iteration 3. The API contract, generated client, backend source, production accounts and historical data did not change. Key/access, whole-app acceptance and deployment remain pending.

## 2026-09-20 - Codex - Prism key, access and publication lifecycle checkpoint

**What:** `web/prism/src/features/access/{AccessPage,IssueKeyDialog,KeyPermissionsDialog,GroupKeyDialog}.tsx`, `web/prism/src/app/DraftDock.tsx`, `web/prism/src/features/config-versions/LifecycleConfirmation.tsx`, `web/prism/src/dev/fixtures.ts`, relevant `web/prism/e2e/{key-access-lifecycle,draft-publication-lifecycle,flows,modal-daily,session-ownership,smoke}.spec.ts`, and the Prism modal-workspace design ledger.

**Why:** Key permissions could lose their original baseline during updates, one-time signing could recover under a replacement owner, draft validation could stack a second modal over a live form, and an acknowledged publish could be obscured by a failed reread.

**Other side:** FYI. Daily key edits capture the exact key/group/routes and preserve siblings through a private replacement group. Grant writes use only the authoritative input fields. Issuance, permission edits and revocation return stage-aware outcomes; lost responses do not replay writes or imply the full key can be recovered. Advanced group signing binds its session and selected draft across response and recovery reads. Draft validation owns one loading/result Sheet; publication retains its acknowledged receipt and captured CAS headers. The fixture now enforces the closed grant input schema. Current checks: 43 unit files/347 tests, 15 serial focused Chromium cases and earlier related browser coverage, type check, double-build/four-file SPA gate, three embedded UI Rust tests, plus isolated real-gateway restricted-key 200/404 and mobile editor checks. C2C `c2c_2f8c` approved the complete checkpoint at iteration 5. No authoritative OpenAPI, generated client, backend source, real Provider or production state changed; remaining advanced editors, whole-app acceptance and deployment are still pending.

## 2026-09-20 - Codex - Prism advanced model and routing checkpoint

**What:** `web/prism/src/features/models/{ModelsPage,ModelEditorDialog,ModelDeleteDialog,ModelAliasDialog,RouteWorkbench,RouteCreateDialog,RouteDialog,CandidateDialog,advancedRoutingModel,advancedRoutingTask,model,modelTask,useRoutingPages,RoutingInventory}.ts*`, `web/prism/src/features/access/AccessPage.tsx`, `web/prism/src/components/ResourcePicker.tsx`, `web/prism/src/features/config-versions/LifecycleConfirmation.tsx`, `web/prism/src/design/modal.css`, fixture and focused browser/unit regressions; `crates/gateway-control/src/management_service.rs`; and the design ledger.

**Why:** Advanced model, alias, route and candidate forms could drift from their opened target or revision, replay after ambiguous responses, misrepresent deletion scope, lose route validation provenance, or fail to recover the exact working draft. A shared query-key change also left the Access route-list reread button ineffective. A local real-gateway stale-origin publication exposed nested Store revision conflicts mapped to 503 instead of 409.

**Other side:** FYI. Captured owner/revision preflights, fixed-revision writes, non-replayable staged receipts and exact-version review now guard the affected workflows, including uncertain route creation. Untouched arbitrary capability keys survive candidate edits, including whitespace, equals signs and explicit false values; complex keys use a JSON editor. Access route reread restarts first-page and cursor failures. The backend maps nested known publication revision conflicts to 409 without changing other Store failures. Current evidence: 45 unit files/356 tests, 27 serial Chromium cases, type-check, four-file double-build/SPA gate, three fresh embedded UI Rust tests plus prior lifecycle checks, and isolated local gateway alias/route/candidate/publish/mock readback. C2C `c2c_2f8c` approved the complete checkpoint at iteration 8. No authoritative contract, generated client, real Provider, production configuration or history changed.

## 2026-09-20 - Codex - Prism billing workspace checkpoint (review pending)

**What:** `web/prism/src/features/billing/{BillingPage,CatalogImportDialog,CatalogRestoreDialog,CatalogInspector,PricePolicyDialog,PriceEntriesEditor,billingTask,model,billing.css}.ts*`, their unit tests, `web/prism/e2e/{billing,billing-lifecycle}.spec.ts`, `web/prism/src/dev/fixtures.ts`, and the modal-workspace design ledger.

**Why:** Catalog import/restore and routing-price policy used separate modal flags and shared mutable result state. A global catalog could persist before configuration publication failed, while the UI still conflated the two outcomes. The policy picker implicitly selected the first directory; table/JSON conversion could truncate or blank entries; mobile preview kept the form visually present after freezing it; and ordinary catalog rows exposed generated IDs.

**Other side:** FYI. One owner-bound billing action now presents stable form, confirmation, inspector and non-replayable receipt states. Exact catalog history is append-only and globally visible; policy CAS stays selected-draft-only. Import and restore distinguish global persistence, draft revision and publication, retain exact recovery targets, and never auto-bind. All six price rates and resolved public-model names remain exact. Fixture rollback source and duplicate/closed-body validation now match production. Current evidence: 46 unit files/369 tests, 45 serial Chromium billing/compatibility cases, four-file build/SPA gate, two billing HTTP plus three embedded-management Rust tests, and isolated real-gateway/EgoLite import, policy, restore, alias→public-name→upstream-name loopback billing with unchanged prior ledger records. C2C `c2c_2f8c` iteration 9 review is pending. No authoritative contract, generated client, real Provider or production state changed.

## 2026-09-20 - Codex - confirmed remaining-plan implementation (in progress)

**What:** Billing pagination/editor/inspector and inline workspace, `components/{OperationBoundary,InlineWorkspace,Sheet}.tsx`, `app/{AppShell,DraftDock}.tsx`, deferred completion option in `features/config-versions/configurationTask.ts`, Access group limits and busy admission in Access/Versions/CompatibleProxyPanel, with targeted browser regressions and plan/design ledger updates.

**Why:** The confirmed grill decisions require whole-price review without truncation, page-contained complex editing, one departure owner and independent global-catalog persistence. The runtime rejects active groups with nonempty limits, so the UI must not offer unsupported new limits or silently erase old values.

**Other side:** Implementation is uncommitted and review pending. Exact legacy limits can be preserved only in disabled groups or explicitly cleared. Fresh Access E2E 7/7 and billing/configuration unit tests 49/49 passed; inline integration and broader acceptance remain pending. The bounded C2C iteration 9 amendment is received, not APPROVED. No authoritative schema/generated client, production state, account data, historical ledger or Provider inference changed.

Checkpoint continuation additionally touches `features/access/KeyPermissionsDialog.tsx`, `features/config-versions/{versionStore,configurationTask}.ts`, runtime deep-link resource presentation, and monitoring regressions. See `docs/reports/prism-remaining-checkpoint-20260920.md` for fresh 371-unit / 54-focused-browser / 3-embedded-Rust evidence and the explicit remaining scope. C2C approval and final visual acceptance remain pending; K3 requested max resolved to high and requires the user's route/strength decision.

Review B14 also requires a narrow backend correctness repair: `crates/gateway-store/src/{control_plane,lib}.rs`, `crates/gateway-control/src/management_mutation_service.rs`, `crates/gateway-http-actix/src/management_resources.rs`, and billing HTTP tests now enforce the global 256-catalog capacity atomically. Authority `docs/openapi/management-v1.json` documents existing Error/400 semantics; vendored contract is updated only by sync-contract. No schema or deletion. Review B12/B13 adds same-component navigation retirement and shared pending-batch policy admission. Fresh regression details and remaining scope are in the checkpoint report; no production change or final completion claim.

C2C `c2c_2f8c` iteration 9 approved B12/B13/B14 corrections after source and released-output review. This is a local functional checkpoint only; combined change review, full remaining editor migration, final visual acceptance and deployment remain open. Final gateway build and three embedded UI tests passed; no production service was changed.

## 2026-09-20 - Codex - pending configuration lifecycle integration

**What:** `web/prism/src/features/config-versions/` lifecycle host, inline pending/history review, explicit draft creation/adoption and store guards; `app/{AppShell,DraftDock}.tsx`; fixture lifecycle CAS/audit and fork grant ownership; related configuration, billing ownership and smoke E2E.

**Why:** The confirmed remaining plan requires one explicit review/validate/apply workflow, accurate durable versus uncertain results, and explicit draft adoption without revision regression or automatic write replay. Retrying comparison now refreshes version metadata before comparing again.

**Other side:** FYI. This is a local implementation checkpoint, not full-plan acceptance or deployment. Current evidence: 390 frontend units, 5 publication lifecycle and 1 stale-pagination browser cases, 10 pending/billing/edit integration cases, double-build/four-file management gate and 3 embedded Rust tests. Real-gateway lifecycle acceptance, remaining editors and final visual QA are still pending. This turn intentionally does not use Codex with ChatGPT per the user's explicit instruction. No API contract, production state or real Provider inference changed.

## 2026-09-20 - Codex - real-gateway draft adoption navigation

**What:** `web/prism/src/features/config-versions/{ConfigurationLifecycleHost,VersionsPage}.tsx` and `web/prism/e2e/configuration-edit.spec.ts`.

**Why:** Actual gateway/EgoLite acceptance found that successful draft adoption remounted the outlet before its navigation completed. The stable shell lifecycle host now owns post-adoption navigation after the dialog retires, with a selected-target check.

**Other side:** FYI. Four focused browser cases and TypeScript passed; rebuilt local embedded UI confirmed automatic inline review after adoption, actual publication/rollback and stale lifecycle 409 protection. Three viewport structural checks found no horizontal overflow. Full visual/accessibility acceptance and remaining editor migrations are not complete. No C2C, production mutation or Provider inference.
