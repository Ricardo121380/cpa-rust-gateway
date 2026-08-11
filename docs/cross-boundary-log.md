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
