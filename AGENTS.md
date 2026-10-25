# CPAR backend development

CPAR is the Rust implementation of CPA/CLIProxyAPI's gateway behavior. For protocol,
account, routing or provider changes, start from the matching Go implementation at
an explicit reference commit, then change the corresponding Rust module. Use
[the architecture map](docs/03-target-architecture-draft.md) to find both sides.
Preserve source attribution when porting upstream code or fixtures.

For current status, priorities or the upstream reference version, start with
[the current roadmap](docs/engineering-roadmap.md). Update its affected rows when
implementation, production wiring or qualification changes; keep historical
matrices and receipts bound to their original dates and sources.

The current product is backend-only. The old Prism frontend and Cargo SPA build
were removed by the user's 2026-10-23 decision. Future UI work starts from
`docs/openapi/management-v1.json`; its design and build are separate work.

## Working entry points

- `apps/gateway`: process composition and runtime adapters.
- `crates/protocol-*` / `crates/provider-*`: protocol codecs / upstream behavior.
- `crates/gateway-router`: selection and attempt coordination.
- `crates/gateway-control` / `crates/gateway-store`: management use cases / persistence.
- `crates/gateway-http-actix`: public HTTP and protected management API.

Keep changes within these existing modules unless a concrete responsibility needs
its own file. Validate the affected behavior; consult `docs/quality-gates.md` for
shared build, dependency and integration changes. Historical phase reports retain
their original scope and are not the current work queue.

## Agent skills

### Issue tracker

Issues 和 specs 使用 GitHub Issues：`Ricardo121380/cpa-rust-gateway`。
涉及工单的读取、创建或更新时，先读 `docs/agents/issue-tracker.md`。

### Triage labels

使用五个默认 triage 标签。分诊或修改工单状态前，读
`docs/agents/triage-labels.md`。

### Domain docs

采用 single-context：根目录 `CONTEXT.md` 与 `docs/adr/`。
探索领域概念或提出架构变更前，读 `docs/agents/domain.md`。
