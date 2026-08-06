# P12-10I-14 Web Native Runtime Local Review

Decision: `PASS_LOCAL; SIGNED_ORACLE_E2E_REQUIRED`

## Findings

- **No P0/P1 finding.** Fixed URL equality is checked both at Config composition and at outbound
  transport conversion. Web index and signer traffic remain under the same explicit P2 egress
  policy and DNS-pinned client boundary.
- **Credential isolation holds.** Runtime Web credentials are parsed only from the selected native
  Web lease, bound into an immutable browser session, and never accepted by Build or Console.
- **Capability claims are narrow.** Only text plus Streaming is declared. Existing request
  projection rejects Tool, Reasoning, cache and unowned extension semantics before transport.
- **Retry stays transparent.** A 403 retry occurs only before a Canonical Event exists, only once,
  and reuses the same selected credential lease. Post-start failures become one terminal stream
  error and cannot trigger account fallback.
- **Concurrent invalidation is safe.** The rejected signature is compared with the current cached
  signature. A stale 403 cannot delete a newer value, while its request may still read the new value
  for its own single retry.
- **Signer response is no longer shape-only.** Production decoding now enforces the frozen
  reference's exact Base64-decoded 70-byte length and rejects extra JSON fields.
- **No false live claim.** All tests used injected transports and synthetic credentials. The live
  Oracle CPAR HTTP path remains the acceptance boundary.

## Remaining execution boundary

Commit and push the reviewed source once, obtain and independently verify the signed ARM64 artifact,
then create one root-only Oracle staging copy with an isolated Web route and client key. Import only
the controlled Web account into that copy, call the CPAR HTTP interface, record only status/lifecycle
categories, and restore the production invariants. Do not modify the active production graph or
restart unrelated services.
