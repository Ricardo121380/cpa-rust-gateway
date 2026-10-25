# ADR-0124: CPA-first Rust backend and retirement of the old frontend

| Field | Value |
|---|---|
| Status | Accepted |
| Date | 2026-10-23 |
| Task / Matrix / Contract references | User architecture simplification request; BC-MGMT-003, BC-MGMT-004, BC-MGMT-008 |

## Context

The user clarified that CPAR is the Rust implementation of CPA/CLIProxyAPI and requested
architecture cleanup, removal of obsolete material, and deletion of the old frontend before a
later redesign from backend interfaces. Prism currently makes every HTTP crate build depend on npm.

## Decision

Use pinned CPA Go implementations as the first reference for existing gateway behavior. Port
into the corresponding Rust modules and reuse applicable examples; introduce new mechanisms
only for concrete Rust requirements or channels CPA does not cover.

Retire `web/prism`, static embedding and frontend-only build/check/browser tools. Keep `/admin`
OpenAPI, authentication, authorization, configuration and operations APIs. Rust builds and their
CI/release paths no longer require Node/npm. Both listeners return 404 for the retired UI routes.
Future frontend design and packaging are separate work.

Move the existing management runtime facade into `runtime/management.rs`, retaining its existing
interface, actual serving-snapshot source and explicit recovery behavior.

## Consequences

ADR-0071 and ADR-0075 are superseded for the removed frontend; BC-MGMT-004 becomes historical.
BC-MGMT-008 now checks API-only listener separation. Backend OpenAPI identifier checks move from
the deleted frontend checker into the existing Rust contract suite. Backend security, protocol
and persistence contracts remain in force. Historical client and release results retain their
original source and do not qualify this changed binary.

## Alternatives considered

Keeping an optional Prism feature would retain obsolete code and two build paths. Rewriting the
whole Rust workspace to mirror Go directories would add migration work without changing the
requested gateway behavior. Neither is needed for this cleanup.

## Validation and rollback

Check workspace compilation, formatting, Clippy, Rust tests, API login and actual local serve
isolation, plus the affected scripts and documentation. No real Provider or deployment is
required. The old frontend and retired files are recoverable from pre-change Git history and a
local maintenance copy; inherited uncommitted documents have a separate baseline snapshot.
