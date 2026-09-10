# CR-PRISM-DOMAIN-001 — Explicit HTTPS management origin

2026-09-10. Accepted for implementation under the user's request to use the existing `cpar`
subdomain from other devices after the Oracle Singapore deployment.

## Problem and behavior

The service is deployed, but the existing domain proxies only the data listener. Prism therefore
returns 404 publicly. Forwarding the management listener alone would still reject browser writes:
`serve` previously derived its sole admitted browser origin from the loopback listener.

Add optional `gateway serve --management-origin <canonical-https-origin>`. When absent, preserve
the existing HTTP loopback browser origin. When supplied, accept exactly one canonical HTTPS
origin and retain Management Key, CSRF, actual loopback peer admission and header validation.
Reject HTTP overrides, paths, trailing slash, userinfo, query, fragment, origin lists and wildcards.
Do not derive trust from Host, Forwarded or X-Forwarded-* and do not rewrite browser Origin in Caddy.

The approved deployment routes `/admin-ui` and `/admin` (including their subpaths) to the management
listener, redirects only the exact root to `/admin-ui/`, and retains the existing data-plane fallback.
Both backend listeners remain loopback-only. DNS already exists; no DNS/CDN or other site change is
needed. This new request authorizes this specific CPAR site change, without changing generic
frontend/backend ownership or the default private-management deployment template.

## Contract and validation

This is a CLI/deployment option. HTTP DTOs, OpenAPI operations, SPA assets and credential formats
are unchanged; no generated client synchronization is needed.

Validation: parser rejection cases; real composed admission with correct/wrong Origin, key, CSRF
and peer; forwarded-header spoofing rejection; existing management-security regression; signed
artifact identity; isolated real gateway startup and browser-origin writes; validated Caddy site
change; public TLS/UI/assets, authorized write/readback, anonymous/wrong-origin/CSRF rejection,
data-plane health/model read, loopback and unrelated service/config preservation.

Production rollback restores the prior binary and service arguments and removes only the new
site routing. Both versions use schema 22: do not reuse the older down-22 rollback. Preserve the
existing 87 legacy Usage failure records/checkpoint and all new writes. No Provider inference is
needed to validate this management-only change.
