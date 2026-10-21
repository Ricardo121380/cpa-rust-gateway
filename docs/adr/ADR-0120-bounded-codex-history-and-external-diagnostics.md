# ADR-0120: Bounded Codex history and external sandbox diagnostics

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-22 |
| Task / Matrix / Contract references | #62 combined Codex acceptance; ADR-0115/0116/0118/0119; [BC-PROTOCOL-014](../contracts/BC-PROTOCOL-014-bounded-codex-history.md), [BC-PROTOCOL-010](../contracts/BC-PROTOCOL-010-local-codex-client-metadata.md) |

## Context

Actual fixed Codex 0.161.0 replays reasoning without id and results with their own id. Its internal
Seatbelt conflicts with the inherited outer acceptance fence; the supported externalSandbox entry
reports external/external-sandbox in C4. All three shapes were rejected under the old contracts.
The [prior report](../reports/cpar-62-codex-combined-20261021.md) retains RED and the exact proposal.
The user explicitly authorized these three bounded additions on 2026-10-22.

## Decision

Adopt BC-PROTOCOL-014's closed history subset. Keep missing reasoning identity and absent/null
content/encrypted_content intact; never invent an id, ciphertext or reasoning body. Result item id
uses existing format and cross-kind uniqueness, separately from call_id. Add independent local
history qualification through the existing private cfg(test) assembly, owned loopback transport
and dependency test feature. Runtime candidate admission and Build builder default to rejection.
Other field flags, request/config/header/URL/UA cannot confer it. Only HTTP Responses, actual Build
and Canonical/CanonicalBridge qualify. Generic reasoning encoding still rejects unidentified input.

Extend only C4's selector vocabulary with the paired external/external-sandbox values. Either new
selector requires both; original optional selectors retain their behavior. This supersedes only
the original selector restriction of ADR-0116. Complete C4 stays encrypted locally and removed
before Build; declarations never become CPAR execution/approval/isolation authority. Independent
OS denial of non-loopback, real home reads and ordinary writes outside the temporary tree remains.

## Consequences

No storage format, schema, reader, key/AAD/TTL or Canonical-field change; no historical rewrite.
Existing raw envelopes preserve original values. Older executors can reject new histories;
readability grants no execution rights. Existing identified Pi history keeps its summary projection.
Fixed Build 1.0.50 remains BUILD_ACTIVATION_BLOCKED; #61 companion/cost remain independent and
unobserved fees UNKNOWN. Controlled success does not qualify a serving profile.

## Alternatives considered

- Generate reasoning ids/remove result ids: changes actual client semantics.
- Infer history permission from C6 or transport: grants an unrelated field family.
- Relax ordinary OS isolation: changes the authorized boundary.
- Forward C4 or trust external as confinement proof: changes recipient and authority.

## Validation and rollback

Keep previous failures; verify exact native histories, C4 authorized raw readback, absent/null,
invalid and cross-kind IDs, unknown members and independent default rejections through public
listener/compiler/factory/leased adapter/independent receiver. Run original client tools, disk
resume, upstream failure and same-process interrupt, all six gates and Pi regressions. Bind affected
tests, Full and separate Standards/Spec reviews to immutable inputs/binaries. Restore execution
rejection to roll back; retain data/keys. No Provider, paid account, remote write or release.
