# P12-10H native Grok Console route and public CPAR curl receipt

Status: `BLOCKED_EXTERNAL_CONSOLE_403`

Date: 2026-08-04

## Boundary

This was an isolated staging execution on the Oracle Singapore host. The signed ARM artifact
for revision `82ceb1d7c6899f3c615d7ed1cff2e400d5e1118a` was installed under a new staging release;
the incumbent CPAR service, Caddy, DNS, CC Switch, public listeners, and production database were
not changed. The staging unit listened only on its loopback ports and was stopped before cleanup.

The source account pool was read-only. A root-only in-memory export emitted four active Console
records from a source pool of 898 records; CPAR accepted all four (`rejected=0`), creating three
accounts and treating one as unchanged. No source ciphertext, token, endpoint, model, request, or
response value is retained in this receipt.

## Route and public HTTP execution

After the staging restart required to refresh the endpoint capability snapshot, the native Console
route published successfully. The protected route explanation selected exactly one candidate.
The public harness then used the CPAR base URL and client key, as a real client would:

| Check | Result |
|---|---:|
| Authenticated `/v1/models` preflight | `200` / JSON |
| Native Console route explanation | `candidate_count=1`, `single_selected=true` |
| Responses JSON requests requested | `100` |
| Responses JSON attempted / succeeded | `1 / 0` |
| Chat JSON attempted / succeeded | `1 / 0` |
| Messages JSON attempted / succeeded | `1 / 0` |
| Responses SSE attempted / succeeded | `1 / 0` |
| Chat SSE attempted / succeeded | `1 / 0` |
| Messages SSE attempted / succeeded | `1 / 0` |
| Upstream request | `sent_via_cpar` for every inference attempt |
| CPAR failure category | `http_5xx` |
| Attempt projection | `EgressRejected / egress` |

The external direct probe and the CPAR attempt projection agree on an upstream 403/Egress
classification. This is an external Console access/session failure, not a missing route and not a
request-conversion failure. The run stopped on the first failure for each bounded harness; it did
not retry or fall back to another provider. The four imported accounts produced the same category.

## Implementation evidence

- The prior common output-limit extension caused a local `ClientRequestError` before any upstream
  attempt. Revision `82ceb1d` consumes only the public Responses output-limit extension and maps it
  to the Console request; unknown extensions remain rejected. The targeted Console runtime test
  passed `12/12`.
- `cargo check --locked -p gateway` passed, and the local `./scripts/check.sh fast` gate passed.
- The signed ARM artifact was independently verified for revision, target, manifest, receipt, and
  keyless Sigstore identity before staging deployment.

## Rollback and invariants

- CPAR account rollback removed `3` newly created accounts and then `1` imported account; the
  staging database was recreated as a clean, empty database after the route-evidence database was
  quarantined under a value-free evidence filename.
- Clean staging `PRAGMA quick_check` was `ok`, foreign-key violations were `0`, and no staging unit
  remained active. The temporary grok2api container was absent after the read-only export.
- Production remained active, its release pointer and database fingerprint were unchanged, and no
  production listener/configuration/public route was altered.

## Verdict

The CPAR route, account binding, output-limit conversion, and all six public protocol/mode paths
reached the upstream boundary. The live Console acceptance is **blocked** by the provider-side
403/Egress classification, so P12-10H remains `IN_PROGRESS/BLOCKED_EXTERNAL_CONSOLE_403`. This
receipt does not claim Console public availability or production readiness. A later rerun requires
newly valid Console sessions/accounts; it must reuse the same CPAR curl harness and stop at the
first failure.
