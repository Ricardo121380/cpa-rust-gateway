# P12-10I-12 Review

Decision: `PASS`

## Findings

- **No P0/P1 finding.** The exact frozen grok2api reference independently reproduced an upstream
  HTTP 403 on the same host and egress, so the current Console blocker is not isolated to CPAR.
- **The control was attributable.** The copied database retained exactly one enabled Console
  account selected from a historical successful audit. The public request produced a reference
  audit with two upstream-response attempts and no transport failure.
- **Readiness noise did not consume the controlled request.** Early unavailable model checks were
  made before cache population and sent no generation. The one authorized generation was sent only
  after model readiness returned HTTP 200.
- **Source differences were not converted into speculative code.** Optional clearance renewal and
  two additional client-hint headers exist in grok2api, but the current source snapshot contained no
  clearance or egress records and the exact reference still failed. No CPAR patch is warranted from
  that evidence.
- **The result is correctly scoped.** Reference parity strengthens the external-egress diagnosis;
  it is not a Console success, does not validate Web or Build, and does not close P12-10H.

## Evidence reviewed

- Frozen source revision and exact reference image version.
- Root-only SQLite online backup and loopback-only temporary runtime.
- Console inventory, historical-success control selection, model readiness, public status, and
  attempt-stage categories.
- Copied database integrity, temporary-resource cleanup, and production invariants.
- Prior CPAR 25-account Console sweep for cross-implementation comparison.

## Conclusion

P12-10I-12 may be accepted as a bounded diagnostic with
`BLOCKED_EXTERNAL_EGRESS_WITH_REFERENCE_PARITY`. Keep the CPAR Console implementation unchanged
unless new evidence demonstrates a CPAR-only divergence. Continue with the independent Web
credential-lifetime policy analysis.
