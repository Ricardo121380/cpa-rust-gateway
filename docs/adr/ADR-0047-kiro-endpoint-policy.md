# ADR-0047 Kiro IDE/CLI endpoint policy

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P7-02` |
| Matrix / Contract | `C39-C40`; [BC-PROVIDER-005](../contracts/BC-PROVIDER-005-kiro-endpoint-policy.md) |

## Decision

`provider-kiro` owns one pure `KiroEndpointPolicy` for the two protocol-distinct Kiro surfaces.
It receives a strictly validated API Region and derives exactly one of these HTTPS endpoints:

- IDE: `https://q.{region}.amazonaws.com/generateAssistantResponse`
- CLI: `https://runtime.{region}.kiro.dev/`

The policy is the sole owner of the endpoint-specific non-secret request head: `content-type`,
`origin`, the CLI `x-amz-target`, and `tokentype: API_KEY` only for `ksk_` credentials. It also
publishes the future Thinking placement (`thinking` wrapper for IDE, `output_config.effort` for
CLI), without transforming a request yet.

No caller can provide an arbitrary URL, host, path, Header, proxy, machine identifier, client
version, `profileArn`, or Secret-bearing authentication value through this policy. P7-03 owns
Profile semantics; P7-04/P7-07 own request/authentication and Thinking conversion.

## Consequences

- IDE and CLI behavior is centralized and snapshot-testable; later Kiro code cannot spread
  endpoint-kind conditionals through request construction.
- API Region is independent of P7-01 Enterprise `auth_region`: the former derives API hosts and
  the latter is only an OAuth-refresh input.
- `url` is a narrow local validation dependency, not a transport dependency. The policy opens no
  socket and does not accept redirects or endpoint overrides.
- A later concrete request builder must add credential secrets only after this fixed policy has
  selected the destination and its non-secret header profile.
