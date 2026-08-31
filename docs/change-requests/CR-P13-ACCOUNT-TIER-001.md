# CR-P13-ACCOUNT-TIER-001 · Provider/channel-scoped account entitlement

## 1. Status and authority

| Field | Value |
|---|---|
| Status | **APPROVED / IN_PROGRESS** |
| CPAR task | `P13-12` |
| Authority | User approval on 2026-08-31; channel-boundary correction on 2026-09-01 |
| Delivery class | Persisted, secret-free account metadata plus protected management projection |
| Excluded project | Autoreg; no registration, login, browser automation, credential repair or replenishment |

`P13-12` is reassigned to this CPAR-owned task. The former “Autoreg handoff” row is removed from
the CPAR phase sequence: Autoreg is an independent project and does not consume a CPAR phase ID.
Historical Autoreg receipts remain historical evidence only.

## 2. Problem

CPAR can currently say whether an imported credential is authenticated, routable, healthy, within
quota and eligible for a lease, but it cannot answer which account entitlement was actually
observed. A successful Build probe or `quota=available` is not plan evidence. Likewise, a raw plan
label from one channel must never be reused for another channel in the same Provider family.

This task adds a value-free, observed entitlement projection. It does not make entitlement a
registration concern and does not infer a tier from traffic volume, model availability, quota or
HTTP success.

## 3. Frozen channel domains

| Domain | Closed normalized tier vocabulary | Initial evidence source | First-slice status |
|---|---|---|---|
| `grok_build` | `free`, `supergrok`, `heavy`, `unknown` | live Build subscription response first; signed Build token claim only as bounded fallback | Persist + management projection + controlled live sync |
| `grok_web` | `basic`, `super`, `heavy`, `unknown` | Web-owned subscription/session evidence only | Separate domain; no Build fallback; deferred source wiring |
| `grok_console` | none yet | Console workspace/billing contract, if separately frozen | `entitlement: null`; no Build/Web vocabulary |
| `chatgpt` | `free`, `go`, `plus`, `pro5x`, `pro20x`, `unknown` | explicit imported metadata or signed ChatGPT/Codex claim | Strict normalization foundation; ordinary-pool wiring follows the same contract |
| `claude` | `free`, `pro`, `max5x`, `max20x`, `unknown` | explicit imported metadata or a separately verified signed claim | Strict normalization foundation; ordinary-pool wiring follows the same contract |

The normalized value is always paired with its domain. Bare `supergrok`, `pro` or `free` is not a
globally comparable account class. Grok Build, Grok Web and Grok Console remain three independent
channels even when they use the same human identity.

## 4. Evidence and confidence

One entitlement observation contains:

- exact `domain` and normalized `tier`;
- closed `source`: `provider_subscription`, `signed_token` or `imported_metadata`;
- closed `confidence`: `authoritative`, `derived` or `declared`;
- non-negative `observed_at_ms`.

The allowed pairs are strict:

- a successful, exact Provider subscription response is `provider_subscription/authoritative`;
- a JWT-shaped token claim decoded locally is `signed_token/derived`; CPAR does not independently
  verify its signature here, so it can never be `authoritative`;
- an explicit export label is `imported_metadata/declared`.

Unknown or unsupported raw labels normalize to `unknown`; raw labels, JWTs, responses, e-mail
addresses and account identities are not persisted in this table or returned by management HTTP.
Missing evidence remains `entitlement: null` and is not fabricated as `free` or `unknown`.

## 5. Initial Grok Build classifier

The controlled sync path targets a batch containing exactly one enabled native Build account and performs at most
one bodyless `GET /v1/user?include=subscription` through the existing DNS-pinned, redirect-denying
egress boundary. It accepts only a bounded successful JSON response and maps the live subscription
tier before considering the locally available signed-token claim. It does not send inference,
retry, refresh OAuth or switch accounts.

The currently selected new Build account was independently observed as `subscriptionTier=GrokPro`
and token tier `1`; both normalize to `grok_build/supergrok`. The durable backfill must be performed
with the new exact-batch command after a database backup and must print only a value-free receipt.

## 6. Management contract

`GET /admin/operations/provider-account-pools` adds nullable `entitlement` to each exact
Provider/channel/account row:

```json
{
  "domain": "grok_build",
  "tier": "supergrok",
  "source": "provider_subscription",
  "confidence": "authoritative",
  "observed_at_ms": 1780000000000
}
```

The response remains protected, `Cache-Control: no-store`, secret-free and snapshot-bound. The
frontend must render the domain with the tier, show missing evidence as “not observed”, and must not
merge Build/Web/Console or use the value as an overall health indicator.

## 7. Non-goals

- no Autoreg code, database, API, task, browser, registration or account repair;
- no scheduler preference, quota multiplier, billing inference or automatic account promotion;
- no cross-channel fallback and no family-wide Grok tier;
- no production route, Caddy, DNS, proxy pool or public traffic change;
- no management mutation endpoint for arbitrary manual tier claims.

## 8. Acceptance

1. Migration is additive and rollback removes only the entitlement table.
2. Store rejects a domain that does not match the native account provider and rejects invalid
   source/confidence/tier combinations.
3. Grok Build live-response and signed-token mappings are table-tested, including unknown labels.
4. ChatGPT and Claude exact normalizers are table-tested without fuzzy substring promotion.
5. Account-pool snapshots preserve domain separation, absence and pagination; HTTP/OpenAPI agree.
6. Source unavailability remains safe `503`, not `500`.
7. OpenAPI change is recorded in `docs/cross-boundary-log.md`; `web/prism/**` remains Claude Code
   ownership during this implementation.
8. Focused tests, fmt, strict Clippy, migration checks, contract tests and final review pass before
   the task is called complete.

## 9. Rollback

The schema change is an additive child table. Old binaries ignore it. Code rollback removes the
projection; data rollback may drop only `grok_account_entitlements`. The account credential,
account revision, Health, Quota, Circuit, lease and route state remain independent.
