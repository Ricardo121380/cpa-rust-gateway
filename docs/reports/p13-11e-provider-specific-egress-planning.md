# P13-11E provider-specific egress planning report

Status: `E0 PLANNED_REVIEWED`; implementation not started

Date: 2026-08-17

## Outcome first

P13-11E E0 is complete as a design/CR slice. The repository now has one reviewed boundary for generic
compatible endpoints, Grok Build, Grok Console, Grok Web, Official API-key, Kiro/Claude-compatible,
and other Provider adapters. The boundary keeps Credential, Quota, Egress, Session, Clearance and
Protocol failures separate, forbids implicit cross-Provider fallback, and keeps Autoreg outside CPAR.

No Rust serving code, migration, OpenAPI, Prism, frontend, Provider call, proxy request, DNS lookup,
server, staging or production configuration was changed by E0.

## Frozen design evidence

- [CR-P13-11-PROVIDER-SPECIFIC-EGRESS-001](../change-requests/CR-P13-11-PROVIDER-SPECIFIC-EGRESS-001.md)
  records the user-approved scope, channel matrix, failure ownership and E0-E5 slices.
- [ADR-0097](../adr/ADR-0097-provider-specific-egress-health-recovery.md) freezes the decision that
  provider-specific behavior must be capability-declared and locally scoped.
- [BC-SEC-008](../contracts/BC-SEC-008-provider-specific-egress-health-recovery.md) defines the state
  domains, failure matrix, redaction and required tests.
- [P13-11 Task Card](../06-development-plan.md) records E0 as the current planning slice and E1 as the next implementation slice.

## Review result

The E0 review confirms:

1. “Krill” is treated as a generic `base_url + api_key` endpoint instance; CPA/Sub2API/OAuth labels
   do not choose an egress implementation.
2. Build, Console and Web have distinct egress/session/clearance namespaces. Console auxiliary DPoP/
   bootstrap calls and Web Statsig/clearance calls cannot bypass a one-shot or bounded ledger.
3. CPAR owns imported-credential runtime Health/Quota/Circuit, lease, cooldown, failure feedback and
   selection. Autoreg owns registration, SSO/OAuth, refresh, account entitlement and replenishment.
4. Official API-key, Kiro, Claude-compatible and arbitrary compatible endpoints do not inherit Grok
   browser behavior. P8/P7 external evidence remains deferred.
5. E0 makes no management contract change; therefore no OpenAPI/Prism sync or Claude Code action is
   required in this slice. An E4 management projection would require a new contract review and a
   `docs/cross-boundary-log.md` entry.

## Verification boundary

This report is a planning receipt, not implementation evidence. It does not claim local state-machine
tests, Provider connectivity, proxy health, DNS reachability, account validity, or production safety.
The next bounded work is E1 typed state/capability seams with synthetic/fake-transport tests only.
E5 real network validation remains outside this CR and needs separate approval.

## E0 local checks

- `./scripts/check.sh docs`: passed (571 Markdown links, 107 contract references, 129 plan tasks with
  exactly one `IN_PROGRESS`, canary/Caddy checks, tracked secret scan and whitespace check).
- `scripts/secret-scan.sh --staged`: passed for the eight intentional E0 documentation files.
- `git diff --cached --check`: passed.
- Staged boundary assertion: only plan/ADR/CR/Contract/report/index/traceability files are staged;
  no Rust, script, OpenAPI, Prism, frontend, deployment or server path is included.
- The four pre-existing untracked helper files remain unmodified and unstaged.
