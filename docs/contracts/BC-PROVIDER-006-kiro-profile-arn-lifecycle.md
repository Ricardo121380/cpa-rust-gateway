# BC-PROVIDER-006 Kiro profile ARN lifecycle

| Field | Value |
|---|---|
| Task | `P7-03` |
| ADR | [ADR-0048](../adr/ADR-0048-kiro-profile-arn-lifecycle.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

- API keys omit `profileArn`; without a completed account-bound CLI observation, P7-01's
  combined Social/Builder family receives the frozen Builder default.
- Enterprise queries one injected port. Invalid/failed lookup chooses the identifiable EU or US
  region-family fallback; a valid result wins.
- Only a validated ARN can be injected at an object root. Audit records expose provenance, Region,
  and presence without the ARN or query error.
- Network, persistence sink, Canonical request conversion, and real Kiro calls remain later work.

The CPAR C runtime integration adds an OAuth-only CLI catalog observation as described in the
ADR amendment. Its ARN must match the exact API region, endpoint, credential ID and revision;
expiry is bounded by credential expiry and one hour. Stale observations fail closed. Only a
completed discovery from the active serving generation may publish this in-memory metadata.
The original P7 local status above does not certify CPAR C or real Kiro acceptance.
