# BC-SEC-004 Grok Web 403 egress/account attribution

| Field | Value |
|---|---|
| Contract | `BC-SEC-004` |
| Task | `P9-07` |
| ADR | [ADR-0067](../adr/ADR-0067-grok-web-403-egress-account-attribution.md) |
| Matrix | `C29-C31`、`D28`、`E10`、`E22`、`E27-E29` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001`; zero-network |
| Domain | Value-free Web 403 classification and exact local egress/account availability state |

## Preconditions and bounds

1. The classifier receives only an HTTP status and a bounded `None` or `ConfirmedForbidden` account-evidence category. It never receives, parses, retains, or logs an HTTP body/header/Cookie/URL.
2. Status must be within `100..=599`. `ConfirmedForbidden` is valid only for status `403`.
3. Egress-state construction and transition receive a non-negative caller time and one unexpired exact `GrokWebBrowserEgressSession`.
4. Account-state construction and transition receive the same session binding but deliberately do not use an egress-session ID for account lifecycle availability.

## Required behavior

| Concern | Required behavior |
|---|---|
| Unknown 403 | `403 + None` is `EgressRejected/Egress` with only `RebuildEgressSession`; it may reject only its exact account/lineage/revision/expiry/egress-session binding. |
| Confirmed account 403 | `403 + ConfirmedForbidden` is `CredentialForbidden/Account` with only `MarkExactAccountForbidden`; it blocks sibling egress sessions only for the same account/lineage/revision/expiry lifecycle. |
| Invalid evidence | An out-of-range status or account-forbidden evidence on any non-403 fails closed before disposition or state mutation. |
| Owner enforcement | Egress state rejects non-egress actions; account state rejects non-account actions. Failed transition leaves availability unchanged. |
| Revision isolation | Account or egress state cannot apply after account, lineage, revision, expiry, or (for egress) session-ID mismatch. A replacement credential revision is not blocked by the prior state. |
| Other statuses | 401 requires reauthorization; 429 and 408/5xx cool only `grok.web` Provider state; other failures have no local account/egress transition. |
| Cross-provider isolation | No Build, Official, generic Router, or Kiro account state is read or mutated. |
| I/O boundary | No HTTP, Web protocol parser, browser/profile, Cookie, proxy/TUN, DNS, TLS, filesystem, server, scheduler, or production-configuration operation exists. |
| Ownership | P9-09/G9 owns approved live WAF/account evidence validation and any transport wiring. |

## Corresponding tests

- `unknown_403_rejects_only_the_exact_egress_session_and_preserves_account_state`
- `confirmed_403_account_evidence_marks_only_the_exact_account_lifecycle_not_egress`
- `invalid_or_wrong_owner_evidence_fails_closed_without_state_mutation`
