# CPAR development

Read [AGENTS.md](AGENTS.md) for the current project rules and
[the architecture map](docs/03-target-architecture-draft.md) for code entry points.
For current implementation status, upstream baseline and next work, use
[the current roadmap](docs/engineering-roadmap.md).

The old Prism frontend was retired on 2026-10-23. This checkout provides the Rust
backend and its management API. New frontend design starts from the backend-owned
[OpenAPI contract](docs/openapi/management-v1.json) when that work is requested.
The former assistant ownership split and Prism-specific build commands are historical.

## Oracle Singapore VPS handoff

Before connecting to the Oracle Singapore host for real-gateway verification, read
[`docs/handoffs/claude-code-oracle-singapore-vps.md`](docs/handoffs/claude-code-oracle-singapore-vps.md)
in full. The local SSH alias is `new-vps`; the handoff is read-only by default and explains the
required loopback tunnel, origin constraint, CPAR/Autoreg separation, redaction rules, and stop
conditions. It does not authorize deployment, restart, Provider requests, or traffic changes.
