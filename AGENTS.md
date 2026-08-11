# Frontend / backend boundary

This repository is worked on by two different assistants:

| Side | Tool | Owns |
|---|---|---|
| Backend | **Codex** | everything except `web/prism/**` |
| Frontend | **Claude Code** | `web/prism/**` |

Neither tool is expected to read the other's working notes. This file, its twin
`CLAUDE.md`, and `docs/cross-boundary-log.md` are the only channel between them.

## Shared surfaces

These paths belong to one side but are routinely changed by the other. A change
here **requires a log entry** (see below), because the other tool will not
otherwise know it happened.

| Path | Owner | Why the other side touches it |
|---|---|---|
| `docs/openapi/management-v1.json` | backend | frontend consumes it via `npm --prefix web/prism run sync-contract`; a contract change silently invalidates the generated client |
| `crates/gateway-http-actix/src/management_ui_resources.rs` | backend | embeds the SPA: asset routes, content types, and the CSP header the UI runs under |
| `crates/gateway-http-actix/build.rs` | backend | runs the SPA build and asserts its four output files |
| `scripts/build-management-spa.sh` | backend | the SPA build invocation |
| `crates/gateway-http-actix/tests/p10_09_embedded_management_ui.rs` | backend | asserts the exact embedded bytes, paths and headers |
| `.github/workflows/*.yml`, `scripts/check-ci-workflow.rb`, `scripts/check.sh` | backend | pin the Node toolchain and `npm ci` working directory for `web/prism` |

## The trace protocol

**If your change touches a path the other side owns, append an entry to
`docs/cross-boundary-log.md` in the same commit.** Not afterwards, not in the
commit message only — the log is what the other tool reads.

An entry answers three questions:

1. **What** changed, by exact path;
2. **Why** — the need on your side that forced it;
3. **What the other side must know**: is action required, or is this FYI?

Add a git trailer too, so the change is greppable from history:

```
Cross-Boundary: crates/gateway-http-actix/src/management_ui_resources.rs
```

`git log --grep=Cross-Boundary` then lists every crossing.

## Before you start

Read the tail of `docs/cross-boundary-log.md`. If the last entries are from the
other tool and marked **action required**, deal with those first — they are
changes made to your side that you did not make.

## Contract discipline

The contract is backend-owned and one-directional: the backend edits
`docs/openapi/management-v1.json`, the frontend follows with `sync-contract`.

The frontend must never hand-edit the generated client
(`web/prism/src/generated/`) or its vendored copy
(`web/prism/contracts/management-v1.json`) — `npm --prefix web/prism run check`
fails on drift, and that failure is the point.

When the frontend needs a shape the contract does not have, it does **not** add
it. It writes a change request under `docs/change-requests/` and logs it. The
backend decides.
