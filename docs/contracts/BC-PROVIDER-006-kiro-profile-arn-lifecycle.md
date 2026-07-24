# BC-PROVIDER-006 Kiro profile ARN lifecycle

| Field | Value |
|---|---|
| Task | `P7-03` |
| ADR | [ADR-0048](../adr/ADR-0048-kiro-profile-arn-lifecycle.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

- API keys omit `profileArn`; P7-01's combined Social/Builder family receives only the frozen
  Builder default.
- Enterprise queries one injected port. Invalid/failed lookup chooses the identifiable EU or US
  region-family fallback; a valid result wins.
- Only a validated ARN can be injected at an object root. Audit records expose provenance, Region,
  and presence without the ARN or query error.
- Network, persistence sink, Canonical request conversion, and real Kiro calls remain later work.
