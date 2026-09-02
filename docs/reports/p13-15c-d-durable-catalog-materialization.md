# P13-15C/D · Durable model catalog and exact-Credential materialization

## Status

- Date: 2026-09-03
- Plan: `docs/06-development-plan.md` v1.316
- Change request: `CR-P13-UPSTREAM-MODEL-CATALOG-001`
- Result: **PRODUCTION_PASS_FOR_GROK_BUILD_PENDING_REMAINING_CHANNELS_AND_FORMAL_GATE**
- Implemented source classes: Grok Build and official Codex only
- Deferred from this result: Grok Web, Grok Console, xAI Official, Kiro, generic compatible
  endpoints, P13-15E multi-channel live matrix and formal Delivery Gate

## Outcome

P13-15C/D connects the previously separate upstream discovery and serving boundaries. Successful
Build/Codex model observations are now retained per exact Config Version, Endpoint and Credential;
the runtime compiles eligible observations into a new immutable route snapshot and publishes it
atomically. Public model IDs remain exact upstream IDs. A request can lease only a Credential whose
own catalog listed that model.

This work does not add a CPAR model whitelist, infer models from account tiers, or make
`GET /v1/models` call Providers. Account entitlement remains independent metadata. Registration,
first OAuth and recovery of a revoked refresh grant remain outside CPAR's catalog worker.

## P13-15C persistence contract

Migration `0021_model_catalog` adds three strict tables:

| Table | Ownership | Retained data |
|---|---|---|
| `model_catalog_targets` | Config Version + Endpoint + Credential | monotonic success version, observed time, Fresh/refresh/expiry deadlines |
| `model_catalog_models` | exact target + model ID | last-success presence, consecutive successful misses, first-miss and removal deadlines |
| `model_catalog_failures` | exact target | latest timestamp and closed safe failure class only |

The default timing contract is Fresh for 6 hours, refresh due after 24 hours and hard expiry after
72 hours. A model is removed only after at least three successful omissions and at least 24 hours
of isolation. Failed discoveries neither replace the last success nor increment omission evidence.
Persisted deadlines are validated against the active policy when loaded; inconsistent state fails
closed instead of silently changing freshness semantics.

HTTP failures are classified without retaining Provider bodies: authentication, authorization,
rate limit, transport, upstream or internal. `GET /admin/catalog/status` exposes only exact opaque
Endpoint/Credential IDs, freshness, observation time, success version, refresh-due flag, retained
model count and the latest safe failure time/class.

## P13-15D materialization contract

The stored active Config Version remains the authority for permission, route policy, capabilities,
protocol transformation and rollout rollback. Catalog publication derives a runtime-only snapshot:

1. A same-route configured Candidate that appears in the exact Credential catalog is the source
   anchor.
2. A newly observed exact upstream ID is materialized only when exactly one route is a valid
   anchor; ambiguity produces no public route.
3. The derived route inherits that route's Access Group grants and protocol/capability template.
4. The derived Candidate records the exact set of Credentials that listed the model.
5. Every ordinary, health/quota, Provider-scoped, pinned, continuation and quota-recovery lease
   path rechecks Candidate-to-Credential eligibility before capacity is acquired.
6. Fresh observations outrank Stale observations; Expired observations are hard-ineligible.

The data plane publishes the whole derived snapshot through one atomic swap. Snapshot-backed HTTP
authentication returns the exact immutable snapshot used for model resolution, and execution
carries that pointer to the scheduler. Selection loads one Candidate scheduler generation,
compares its snapshot identity with the ingress pin, then selects from that same loaded generation.
A publication that wins the race therefore causes the old request to fail closed; it cannot combine
old authorization/model resolution with a new Credential set.

## Runtime behavior

- Existing durable state is materialized during composition before new Provider discovery.
- The worker starts after listeners bind, runs once immediately and then hourly.
- A target is contacted only when it has no successful snapshot or its 24-hour refresh deadline is
  due; hourly expiry re-evaluation does not imply hourly Provider traffic.
- Discovery obtains the exact runtime Credential lease and reuses that Endpoint's egress policy,
  DNS resolver, non-streaming transport profile and shared upstream client pool.
- Deleted/non-live pool Credentials are filtered before materialization.
- Current runtime source dispatch is deliberately closed to native Grok Build and the official
  Codex endpoint shape. Other channels are not guessed from names or JSON structure.

## Verification

Completed local evidence before commit:

- `cargo test -p gateway-catalog`: 19 passed.
- P13-15B Build catalog tests: 3 passed.
- P13-15B Codex catalog tests: 3 passed.
- `cargo test -p gateway-router -p gateway-http-actix -p gateway --no-fail-fast`:
  gateway 119 passed, HTTP unit/integration suites passed, router 175 passed; explicitly authorized
  live/soak tests remained ignored.
- New deterministic cases cover deadline tamper rejection, failure-before-success, last-success
  retention, three-miss/24-hour removal, exact Credential lease restriction, old/new publication
  non-mixing and protected management success/failure-only projection.

The final workspace Clippy/fmt/diff results and production revision are recorded in the closing
commit/plan update. No test in this local slice sent a real Provider request.

## Oracle Singapore production acceptance

The exact implementation commit `058805e556d5b22a00bd56b846f75ed8b81696fd` was built by
[release workflow 33651468181](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/33651468181).
Both Linux `aarch64-unknown-linux-gnu` and `x86_64-unknown-linux-gnu` jobs passed. The signed ARM64
artifact and OCI metadata were verified before installation; the deployed binary SHA-256 is
`69acbe01c8aa4934867f473e6eeaa063185eeae364865d8730fd88f8ab6344af`.

Immediately before cutover, the service binary, release pointer and an online SQLite backup were
captured under `/var/backups/cpa-rust-gateway/p13-15-deploy-final-20260902T163647Z`. The current
official Grok CLI Build grant was imported through the existing CPAR root-only atomic import path
without a plaintext credential file, and its Build entitlement synchronized as authoritative
`supergrok`. No token, cookie, refresh grant, principal, Credential ID or Client Key is retained in
this receipt.

After the atomic release switch and restart of only `cpa-rust-gateway.service`:

- the running release and binary hashes matched the approved revision and artifact;
- loopback and public health checks returned HTTP 200;
- schema version 21 was active, `PRAGMA quick_check` returned `ok`, and foreign-key violations were
  zero;
- authenticated `/v1/models` returned four exact IDs and included `grok-4.6`;
- `GET /admin/catalog/status` returned four targets: one Fresh Build target containing two models,
  and three Missing targets retaining only safe failure classes;
- SQLite contained one successful catalog target, two catalog model rows and one exact
  `grok-4.6` row; and
- the single approved real non-streaming `POST /v1/responses` canary for `grok-4.6` returned HTTP
  200 with `status=completed`, the requested model and a non-empty output. The output content was
  neither printed nor stored in this receipt.

The rollback closure was not triggered. The production backup remains available; temporary local
and remote upload/preflight directories were removed. Autoreg, Caddy, Cloudflare, DNS, frontend and
other services were not modified. See the value-free
[production acceptance receipt](evidence/p13-15c-d-oracle-production-acceptance-20260903.md).

## Rollback and remaining gates

Rollback is the previous binary plus the prior immutable runtime snapshot. Migration 0021 is
additive; its down migration drops only the three P13-15C tables. If the worker or materializer
cannot load a coherent snapshot, startup/publication fails closed rather than reverting to guessed
model constants.

P13-15 is not DONE yet. Grok Build C/D production acceptance is complete, but Codex Luna remains
blocked until an actual Go Credential is reauthorized and produces its own production catalog
evidence. Remaining B channel sources, the P13-15E isolation/live matrix, Claude Code contract/UI
integration and the formal Delivery Gate still require separate work.

---

## 中文摘要

P13-15C/D 已在本地把“真实上游发现”和“公网可路由模型”正式接通：目录按 Config Version、Endpoint、
Credential 精确持久化；成功版本单调递增，失败保留最后成功，模型移除需要连续三次成功遗漏且隔离满
24 小时。运行时只把 Fresh/Stale 目录物化为 exact model route，并把每个 Candidate 限定到实际列出
该模型的 Credential 集合。鉴权、模型解析与最终租约必须使用同一不可变快照世代；并发发布只会让
旧请求安全失败，不会混用新旧权限或账号池。

本片没有硬编码 `grok-4.6`、`gpt-5.6-luna` 或套餐到模型的映射，也没有把 Grok Build 的
`supergrok`、ChatGPT 的 `go` 当作目录来源。当前自动 source 只覆盖 Grok Build 与 official Codex；
Web、Console、Official API key、Kiro 和通用 `baseURL + API key` 仍需各自独立 source。Oracle
Singapore 已部署 exact commit `058805e5`；生产授权目录现已动态包含 `grok-4.6`，且唯一一次真实
non-streaming Responses 验收返回 `200/completed`。因此 C/D 的 Grok Build 生产边界已通过，但
P13-15 总项仍等待其余渠道、E 隔离矩阵、Claude Code 接线和正式 Delivery Gate，不能标记 DONE。
