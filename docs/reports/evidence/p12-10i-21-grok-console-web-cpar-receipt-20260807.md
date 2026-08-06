# P12-10I-21 Grok Console/Web CPAR HTTP receipt

## Scope

This receipt records one local-only, root-owned ARM64 staging run after the Web recovery port. It
uses the CPAR public data-plane boundary (`/v1/models` plus one stop-on-first-failure inference)
with a staging client key. Production CPAR, the production Config Version, Caddy/DNS, CC Switch,
and the incumbent CPA were not changed.

No credential, cookie, endpoint secret, account identity, model value, request body, or response
body is recorded here.

## Console

| Check | Result |
|---|---|
| Source | Jakarta grok2api v3 data, 63 distinct SSO records observed |
| Import | Five separated samples (first, second, third, middle, last) imported one at a time; each batch receipt was `1/1` accepted |
| Route lifecycle | Isolated Console route entered, restarted, published, and single-candidate explain returned `candidate_count=1`, `single_selected=true` |
| `/v1/models` | Passed for every sample (`2xx` JSON and selected Grok model visible) |
| Real inference | Five single Responses JSON attempts, each stopped at the first failure |
| Failure | `http_5xx`; CPAR attempt projection was uniformly `CredentialUnauthorized/credential` |
| Protocol coverage | Chat, Messages, and SSE were not sent after the first failure, per the stop-on-first-failure rule |

The result is `BLOCKED_WITH_EVIDENCE`: the CPAR route and authentication boundary are reachable,
but the sampled SSO credentials are unauthorized at the provider credential boundary. This is not
a protocol-projection or route-selection pass.

## Web source check

The grok2api service was started only long enough for a read-only admin export inspection and was
stopped afterward. Its source database reported 898 active Web accounts. All sampled source rows
had no usable `expires_at` or `refresh_due_at`, and the exported SSO token payloads contained only a
session identifier (no decodable expiry claim). CPAR therefore rejected the Web source before
import: an arbitrary synthetic expiry would violate the 90-day session policy and would not be
evidence of a usable session.

The Web result is `BLOCKED_WITH_EVIDENCE` with fixed category `web_expiry_unavailable`; no Web
inference request was sent in this run.

## Isolation and rollback

- The ARM64 staging process used loopback-only listeners on the temporary staging ports.
- The Console account batch was rolled back with `removed_accounts=1` after the final sample.
- The active staging Config Version was rolled back to the production parent and the temporary
  staging directory was moved to the server's recoverable trash area.
- Production health remained `{"status":"ok"}`; production active version remained the existing
  Codex production version and the production Grok account count remained unchanged.
- The temporary grok2api container was stopped after the read-only source inspection.

## Decision

P12-10I-21 is complete as a diagnostic receipt, not as a Console/Web acceptance. Build remains the
only closed Grok text path. Console needs a newly authorized/current SSO credential or an upstream
credential-boundary fix; Web needs a source session with an explicit valid expiry (or a separately
authorized reauthentication flow) before another CPAR HTTP attempt is meaningful.
