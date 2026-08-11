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
