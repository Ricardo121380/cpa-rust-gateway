# CLAUDE.md — frontend side

You are the **frontend** assistant on this repository. The backend is worked on
by **Codex**, which reads `AGENTS.md`, not this file.

## The boundary

| Side | Tool | Owns |
|---|---|---|
| Frontend | **Claude Code** (you) | `web/prism/**` |
| Backend | **Codex** | everything else |

The management SPA (Prism) lives at `web/prism`. It is built by
`cargo build` — `crates/gateway-http-actix/build.rs` runs
`scripts/build-management-spa.sh`, then embeds exactly four files with
`include_bytes!`. So a broken frontend build is a broken **backend** build.

## Before you start

Read the tail of `docs/cross-boundary-log.md`. Entries from Codex marked
**action required** are changes made to your side that you did not make —
usually a contract change. Deal with those first.

Then, always:

```bash
npm --prefix web/prism run sync-contract   # contract is backend-owned, follow it
npm --prefix web/prism run check           # fails on generated-client drift
```

## When your change touches the backend

Frontend work reaches backend files more often than it looks: the CSP the UI
runs under, the asset routes, the build wiring and the CI toolchain all live on
Codex's side. See the shared-surface table in `AGENTS.md`.

**Append an entry to `docs/cross-boundary-log.md` in the same commit**, and add
the trailer:

```
Cross-Boundary: crates/gateway-http-actix/src/management_ui_resources.rs
```

Say what you changed, the frontend need that forced it, and whether Codex has
to do anything. If Codex reverts it without knowing why, that is your failure to
log, not its failure to guess.

## What you must not do

- **Do not edit the contract.** `docs/openapi/management-v1.json` is
  backend-owned. If you need a shape it does not have, write a change request
  under `docs/change-requests/`, log it, and build the honest empty state
  meanwhile. Prism has a long record of doing exactly this — keep it.
- **Do not hand-edit `web/prism/src/generated/`.** It is generated from the
  contract; `run check` fails on drift and that failure is load-bearing.
- **Do not touch backend source to make a frontend test pass.** Fix the
  frontend, or log the crossing and explain it.

## Frontend invariants (enforced by `web/prism/scripts/check.mjs`)

Read `web/prism/DESIGN.md` before changing UI. The mechanical gates:

- no browser storage of any kind (C6) — refresh clears the session by design;
- no raw `fetch()` outside `web/prism/src/generated` (C5);
- no inline `style={{ }}` — the shipped CSP is `style-src 'self'`;
- `dist/` is exactly four files (C3) — a fifth would be served by nothing;
- double build must be byte-identical (C10);
- `--ink-3` may not carry body text (below AA).

## Verifying for real

Fixtures (`VITE_PRISM_FIXTURES=1 npm run dev`) are for development only and are
physically excluded from production builds. Before claiming something works,
run it against the real gateway:

```bash
cargo build -p gateway --bin gateway
./target/debug/gateway serve --data-listen 127.0.0.1:18180 \
  --management-listen 127.0.0.1:18181 \
  --state-dir <dir> --credential-dir <dir>
# then open http://127.0.0.1:18181/admin-ui/
```

**Serve the panel from the management listener, not a proxy.** The gateway
derives its one allowed browser origin from that listener's own address
(`apps/gateway/src/deployment.rs::management_origin`), so every mutation from
any other origin fails closed with `404 management_access_denied`. GETs still
work, which makes this easy to misdiagnose.
