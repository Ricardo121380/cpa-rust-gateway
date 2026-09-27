# BE-FE-KIMI-METADATA-003 — Empty usage result and OAuth count compatibility

Status: implemented locally; not released. User requires verified repair before cutover.

Evidence: three isolated real-adapter checks on 2026-09-27. Identity present and
four models observed; `/coding/v1/usages` supplied valid JSON with no recognized
windows. Two schema-only checks confirmed the entire object was empty, including
a comparison using the same CPAR device headers as catalog/inference. No secrets
or response values were exported; no inference or refresh grants used.

Contract: add `empty_response` to `KimiMetadataFailure.code`. Existing optional
error fields and quota window DTO remain compatible. Empty object is distinct
from timeout, HTTP rejection, unrecognized nonempty payload, and genuine 0% usage.
Run sync-contract; do not edit generated clients manually.

Provider parser also supports the official Python Kimi CLI's `usage` weekly
summary and explicit 5-hour/7-day `limits[].window` count format, alongside the
TypeScript client's ratio format. Only valid finite counts and positive limits
produce ratios; absent used/remaining does not imply zero. Preferred ratio
windows suppress duplicate counted windows. This does not fix an empty upstream
payload or prove the current account has an available quota.

Frontend: retain existing evidence panel; explicitly say the provider supplied
no quota data. No new request, login flow, storage, schema migration or model grant.

Sources checked 2026-09-27:
- https://github.com/MoonshotAI/kimi-cli/blob/main/src/kimi_cli/ui/shell/usage.py
- https://github.com/MoonshotAI/kimi-code/blob/main/packages/oauth/src/managed-usage.ts
- https://github.com/router-for-me/Cli-Proxy-API-Management-Center/blob/main/src/utils/quota/builders.ts
