# P10-07 Configuration lifecycle workflow plan

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-07` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Date | `2026-07-24` |
| Scope | Protected Config Version metadata, validation, atomic publication, retained-predecessor rollback, and bounded P2 lifecycle-audit projections. |
| Inputs | [BC-CONTROL-002](../contracts/BC-CONTROL-002-local-management-lifecycle.md), [BC-ROUTER-002](../contracts/BC-ROUTER-002-route-snapshot-publication.md), P2-10 lifecycle evidence, P10-02 admission, and the frozen management OpenAPI. |

## Fixed delivery boundary

P10-07 is a transport adapter over the existing P2-10 `ManagementService`, not a second
publication algorithm. It may create/list/get Config Version metadata, validate a Version, invoke
the transactionally auditable `publish_configuration` and retained-predecessor
`rollback_configuration` operations, and project the P2 lifecycle audit stream. It must preserve
the P2 ordering: compile/prepare first, SQLite activation plus matching lifecycle audit atomically,
then the infallible in-memory Snapshot commit.

Every lifecycle request requires P10-02 admission. Publication and rollback additionally require
the generated-client CSRF header and their frozen exact `If-Match` precondition; creation retains
the frozen OpenAPI's transaction-bound create-with-audit contract without a prior revision, while
validation is read-only. A stale revision, invalid Version, compile failure, missing rollback
predecessor, audit failure, or unavailable lifecycle dependency must fail without changing the
active Snapshot. The handler never turns a compiler diagnostic into a management response.

Audit output is limited to the existing P2 lifecycle event schema: monotonic ID, closed action,
bounded actor, time, target Version, and optional replaced Version. P10-06 resource-operation
audits remain structurally separate until a future contract defines a safe unified schema; this
task must not coerce resource identifiers or payloads into the P2 lifecycle stream.

## Explicit exclusions

- P10-08 exclusively owns encrypted backup creation, restore preflight, restore execution,
  schema-version recovery and backup-material handling.
- No Provider request, endpoint probe, Catalog discovery, credential Secret/ciphertext, URL,
  Header, Body, proxy, TLS setting, or inference hot-path handle may enter the lifecycle HTTP
  state.
- No arbitrary archived Version selector is allowed for rollback: it uses only P2's retained
  predecessor.
- The UI does not display compiler errors, audit payloads, a backup action, a restore control, or
  a configuration export.

## Implementation and verification sequence

1. Add a separately injected, fail-closed lifecycle facade around the existing `ManagementService`
   boundary, with bounded safe mappings for Config Version metadata, validation, publication and
   audit events.
2. Add only the frozen protected routes and integration tests covering unauthenticated admission,
   CSRF/precondition failure, failed publication preserving the active Snapshot, successful
   publish/rollback, ordered safe audit projection, and a default unavailable facade.
3. Add generated-client-only SPA controls for lifecycle read/validate/publish/rollback/audit and
   a loopback browser fixture. Exercise publish failure followed by publish/rollback success;
   confirm reload clears the in-memory session and browser storage remains empty.
4. Review the exact P2 transaction/Snapshot ordering, response redaction and P10-08 exclusion;
   then run targeted Rust/SPA checks, format, Secret scan, docs checks, and `git diff --check`.

## Completion evidence

- The protected HTTP regression covered admission concealment, create/read, read-only validation,
  failed and stale publication, successful publication, retained-predecessor-only rollback,
  ordered safe lifecycle audit, archived-Version reactivation rejection, and the fail-closed
  unavailable facade.
- The loopback browser fixture exercised a failed publish (`409`), Version 1 create/validate/
  publish, Version 2 create/publish, and rollback to Version 1. Its final audit projection had
  five ordered lifecycle records ending in `config_rolled_back`.
- Browser storage was empty (Cookie, `localStorage`, and `sessionStorage`); reload cleared the
  in-memory management session. The fixture had no backup/restore control, provider transport,
  persistence, proxy, or external egress.
- Focused review confirmed that list/get reads use root Config Version metadata only, lifecycle
  error mapping reveals neither compiler nor store details, the P2 transition owns publication
  ordering, and P10-08 materials remain absent.
