# ADR-0060: Grok Official authorized one-probe boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P8-07` |
| Matrix / Contract | `C01`、`C03`、`C31`、`G24-G27`; [BC-E2E-004](../contracts/BC-E2E-004-grok-official-authorized-one-probe.md) |

## Context

P8-01 through P8-06 prove the Grok Official API-key adapter with deterministic local evidence,
but §20.1 requires a real client and test account at every Provider Gate. Grok Official uses a
separate API-key plane: it is neither a Kiro OAuth credential nor dependent on P7/G7 under
`CR-P7-DEFER-002`. A live check must not discover credentials, proxy settings, catalogs, or models,
and must retain no API key, endpoint variant, request/response body, header, account, or generated
text in output or committed evidence.

## Decision

1. Provide one ignored `provider-grok` integration test that performs zero network I/O by default.
2. It runs only after an exact authorization value, an exact one-request cap, one opaque label, one
   selected mode, one API key, and one upstream model are supplied directly to its dedicated
   environment variables.
3. It uses the native Official adapter, the fixed Official Responses URL, DNS-pinned direct egress,
   redirects denied, one pooled connection, and finite connect/first-byte/idle/total bounds.
4. It has exactly one `execute` path. Retries, failover, credential refresh/rotation, catalog
   discovery, proxy/environment discovery, generic provider configuration, and both-mode execution
   are excluded.
5. The only successful result is Canonical `ResponseStart`, `TextDelta`, and `ResponseEnd` with no
   `StreamError`. Output contains only the opaque label, mode, and a safe lifecycle outcome.

## Consequences

- G8 has a narrowly auditable true-Provider E2E path when the user independently authorizes it.
- No P7 Kiro state is read, changed, or used as a precondition.
- A non-streaming success proves one Official tuple only; SSE requires a separately registered and
  independently authorized one-request invocation.
- Failure stops the probe without remediation and does not establish a cause from the redacted
  transport/protocol outcome.

## Alternatives considered

- Treating local fixtures as the §20.1 E2E: rejected because they cannot establish current remote
  acceptance.
- Reusing Grok Build OAuth or Kiro OAuth: rejected because they are different credential planes.
- A catalog-plus-generation workflow or a mode matrix: rejected because it would exceed the narrow
  one-send proof boundary.

## Validation and rollback

The normal test target covers absent authorization, invalid request cap, and complete synthetic
configuration preparation without DNS or transport. Formatting and focused Clippy must pass before
any ignored invocation. Rollback removes only this ignored harness and its documentation; it has no
effect on the existing Official adapter, accounts, API keys, servers, proxy/TUN configuration,
routes, or production traffic.
