# P10-06 runtime observability management workflow plan

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-06` |
| Status | `IN_PROGRESS` |
| Date | `2026-07-24` |
| Scope | Authenticated, secret-free runtime observations and the OpenAPI-defined quota-recovery request boundary. |
| Inputs | [P4 runtime management status](p4-10-read-only-runtime-management-status.md), [BC-MGMT-001](../contracts/BC-MGMT-001-read-only-runtime-management-status.md), P4 Route Explain, the versioned management OpenAPI, and P10-02 admission. |

## Fixed delivery boundary

P10-06 owns only the five already frozen operations: `getCatalogStatus`,
`getRuntimeAvailability`, `requestQuotaRecovery`, `explainRoute`, and `listRequestAttempts`, plus
their generated-client SPA controls. Each route stays within the P10-02 `/admin` Scope and uses
an explicit injected management-time runtime facade. The facade may read only safe catalog,
runtime-management, Route Explain, and value-free attempt projections; it must not derive a
Provider URL/Header/Body, look up a Secret, acquire a lease, advance a scheduling cursor, open a
socket, issue a Provider request, or publish a configuration Snapshot.

`requestQuotaRecovery` is a controller request, not a recovery probe. It may create only the
already-defined bounded recovery-request state through an explicitly injected controller. The
handler cannot send a probe, clear a 403 account state, change a Credential, or interpret raw
provider response material. The default facade remains reject/fail-closed when a deployment has
not supplied the runtime dependencies.

## Delivered views

1. Catalog status returns bounded endpoint/credential freshness and observation time, never model
   values, raw catalog content, URLs, headers, or Credentials.
2. Runtime availability projects P4's binding-scoped Health, Quota and 403 account categories,
   preserving exact Endpoint/Credential isolation and fail-closed unavailable values.
3. Route Explain is read-only and uses an explicit observation time plus frozen protocol/model
   inputs. It never selects a live Candidate or acquires a Credential lease.
4. Request tracing exposes a bounded value-free list of recorded attempts for the requested
   `RequestId`; an embedding may add only an optional closed execution-stage category. Absent,
   lost, or unavailable storage maps to the fixed safe management error instead of an internal
   diagnostic.
5. The SPA has a dedicated runtime panel. It uses the generated client, persists no session or
   observation data, renders only safe categories, clears results on session reset, and labels a
   quota action as a request rather than a network recovery.

## Explicit exclusions

- P10-07 retains Config Version publication/rollback and audit-history UI.
- P10-08 retains backup/restore material and preflight controls.
- P10-09 retains final static embedding, listener ownership, `frame-ancestors` response-header
  delivery and hot-path measurement.
- No Provider health probe, quota reset send, account recovery completion, runtime state mutation
  other than a bounded controller request, or real external request is in scope.

## Verification sequence

1. Facade tests prove reject-by-default behavior, safe projection mapping, explicit-time Explain,
   binding-scoped 403/Quota isolation, bounded attempt rows and a recovery request that cannot send
   or complete a probe.
2. Protected HTTP integration tests cover absent credentials, malformed inputs, P10-02 admission,
   closed response schemas, safe status/error mapping, and no runtime side effects for GET.
3. SPA/static and loopback browser fixtures exercise runtime reads, Explain, trace output and the
   labelled recovery request while asserting generated-client-only calls, no browser persistence,
   no Key display and no P10-07/P10-08 actions.
4. Review confirms no Provider/runtime dataplane handle leaks into the request path, no unbounded
   tracing response, no Secret/raw diagnostic serialization and no accidental publish/restore
   route.
