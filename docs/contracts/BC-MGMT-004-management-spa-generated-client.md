# BC-MGMT-004 Management SPA generated client

| Field | Value |
|---|---|
| Contract | `BC-MGMT-004` |
| Task | `P10-03` |
| Status | Accepted |
| Domain | Static management SPA and generated API client |

> **2026-08-11 · 部分被取代。** 本记录描述的 `scripts/generate-management-client.mjs`
> 与 `web/admin-ui` 已随管理前端合并删除。**决策本身仍然成立**:生成客户端是唯一
> API 通道,唯一生成输入是 `docs/openapi/management-v1.json`,手写客户端仍被禁止。
> 变的只是实现位置 —— 生成器现为 `web/prism/scripts/generate-client.mjs`,
> 新鲜度由 `npm --prefix web/prism run check` 机械校验。
> 见 [cross-boundary-log](../cross-boundary-log.md) 与
> [08 · 管理前端开发计划](../08-management-frontend-development-plan.md)。

## Inputs and output

The only API-generation input is the tracked P10-01
[`management-v1.json`](../openapi/management-v1.json) contract. The generator accepts only its
OpenAPI `3.1.0`, `contract_only` state, local component references, `/admin/` paths and unique
identifier-safe `operationId` values. It emits the tracked TypeScript client in
`web/admin-ui/src/generated/management-client.ts`; no server endpoint, configuration file,
environment variable, Cookie, browser profile or network schema is an input.

Every generated operation preserves only the frozen HTTP method, literal relative route template,
declared path/query/header names and requiredness, plus whether a body is absent, JSON or binary.
It has one named wrapper. Future response typing and page-specific validation stay with the P10
task that implements the matching endpoint; P10-03 does not claim a route is live.

## Client safety invariants

1. `ManagementApi` accepts Management Key and optional CSRF providers only as process-memory
   callbacks. Neither header can be supplied through a page request object, browser persistence,
   Cookie API or arbitrary base URL.
2. A call may supply only contract-declared parameter names. Required path/query/header values and
   required bodies fail before `fetch`; unknown inputs and a body on a bodyless operation fail.
3. All calls use a relative `/admin/...` URL, `credentials: "same-origin"` and
   `redirect: "error"`. The client cannot select a scheme, host, port, origin or redirect target.
4. Unsafe methods require the separately supplied CSRF token. P10-02 remains the final server-side
   authority for actual peer, Management Key and origin admission.
5. P10-03's shell never creates the client or calls `fetch`. It uses static same-origin assets,
   an explicit CSP and no inline executable script. It has no CRUD, key-entry, OAuth, backup,
   Provider or data-plane behavior.

## Build and isolation invariants

`npm ci --ignore-scripts` uses the tracked lock file. `build-management-spa.sh` first rejects a
stale generated client, then compiles TypeScript and copies only the local HTML/CSS into ignored
`web/admin-ui/dist`. It must produce identical file paths and SHA-256 asset digests on two clean
successive builds. No generated `dist` artifact is embedded, served, loaded by Rust or included in
the public inference process in P10-03; P10-09 exclusively owns that integration and its
hot-path measurement.

## Corresponding checks

`check-management-spa.mjs` compares all OpenAPI operation IDs with generated wrappers, checks
generation freshness, builds twice, compares asset digests, asserts required assets/CSP and
rejects browser storage, shell-side requests and relaxed same-origin/redirect client behavior.
The local/CI fast and full gates run it after a locked npm install. These checks have no live HTTP,
Provider, OAuth, listener, database or credential action.
