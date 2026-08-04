# P12-10H Grok account recovery and Console multi-account receipt

Status: `CONSOLE_MULTI_PASS_BUILD_REAUTH_EXTERNAL`

Date: 2026-08-04

## Boundary

This follow-up used the Oracle Singapore host only through a temporary loopback staging boundary.
grok2api was started long enough to run its existing administrative refresh operation and export a
bounded Console subset, then stopped. CPAR production, Caddy, DNS, CC Switch, the production
database and public traffic were not changed. No credential, token, endpoint, model or request /
response value is retained here.

## Build refresh / reauthentication result

The source database reported 829 Build accounts: one `active` account and 828 enabled accounts in
`reauthRequired`. All 828 reauthentication-required credentials were already marked permanently
unrefreshable by grok2api and still carried refresh-failure state.

The existing `POST /api/admin/v1/accounts/refresh-tokens` operation was invoked once. Its value-free
server receipt reported one submitted and one successful refresh, which corresponds to the single
active/due Build account. The post-operation source state remained one `active` Build account and
828 `reauthRequired` accounts. No bulk reauthentication was attempted: those 828 accounts require
an interactive OAuth repair, not another automatic refresh request.

## Console multi-account result

The source database reported 898 active Console accounts. The root-only memory exporter confirmed
`source_count=898` and emitted five bounded records. Each record was imported into its own isolated
CPAR staging database and batch so the native pool compiler saw exactly one account per probe; no
database field was edited to hide sibling accounts.

| Measure | Result |
|---|---:|
| Console records exported | 5 |
| Native probes attempted | 5 |
| Native probes passed | 5 |
| Native probes failed | 0 |
| Chat projection | 5/5 |
| Responses projection | 5/5 |
| Messages projection | 5/5 |
| Account health / quota | available for all five |
| Per-account rollback | 5/5 removed one account |
| Post-rollback SQLite | `quick_check=ok`, foreign-key violations `0` |

Every successful probe preserved exact account attribution and a complete Canonical lifecycle. The
five probes are evidence for a bounded Console subset, not a claim that all 898 accounts have been
individually exercised.

## Cleanup and verdict

- Temporary staging directories were removed and no staging listener remained.
- grok2api ended `Exited` after the bounded export.
- The production CPAR unit remained `active` with its two established loopback listeners.
- No Console source account was disabled, deleted or rewritten. The only intentional source-side
  mutation in this follow-up was the one authorized active Build token refresh.

The Console multi-account slice passes. Build automatic recovery does not pass for the 828
permanently reauthentication-required accounts; manual OAuth repair remains an external dependency.
The earlier native Build HTTP E2E therefore remains `BLOCKED_EXTERNAL_RATE_LIMIT`, and P12-10H / the
final P12-10 retirement closeout remain open.
