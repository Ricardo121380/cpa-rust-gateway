# BE-FE-KIMI-METADATA-001 — Kimi Coding account observations

Status: implemented and locally validated; production release and real-account acceptance pending.

Problem: successful device authorization persisted a credential without binding it to the prepared endpoint. The management metadata projection contained no Kimi profile or quota and treated these omissions as unavailable provider data.

Contract: extend `getCredentialMetadata` with nullable `kimi` and `kimi_error`. Kimi observations contain allowlisted identity, official plan, observation timestamp, independent profile/quota availability and named usage ratios/reset timestamps. Missing data is not zero. The GET can perform two bounded official metadata reads through the current serving generation, exact credential lease and existing egress/client policy. Success is cached for five minutes; partial failures for thirty seconds. Cache is generation-local and resets on publication/restart. It does not modify model grants, credentials, or billing. Disabled/unbound accounts cannot trigger upstream calls.

Account inventory uses cached identity/plan and an observation revision in pagination cursors. First device authorization adds its exact binding before configuration publication; renewal does not replace, enable or recreate existing connections. Model discovery remains separately account scoped and manual model opening remains required.

Source: MoonshotAI/kimi-code `packages/oauth/src/managed-userinfo.ts` and `managed-usage.ts`, inspected 2026-09-27. Official phone numbers may be masked; no unmasking or invented email.
