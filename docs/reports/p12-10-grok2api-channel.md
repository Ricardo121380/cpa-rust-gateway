# P12-10 grok2api channel expansion

Status: `IN_PROGRESS_EXACT_REVISION_BUILD`

This report is value-free. It does not record endpoint, credential, upstream model, request or
response content, token fingerprint, or mutable production identity.

## Decision

CPAR currently holds one encrypted OpenAI-compatible Bearer credential for the Codex/Krill channel.
The existing grok2api deployment manages a substantially larger OAuth account pool, refresh,
cooldown, quota and egress state. CPAR therefore integrates grok2api as one independent loopback
OpenAI-compatible upstream; it does not import or duplicate the account OAuth records.

Read-only inventory found the Build pool unsuitable as the primary route because almost all enabled
accounts require reauthentication. The Console-backed Responses route has a large active pool and
recent successful traffic. A bounded loopback probe returned valid 2xx Models, Responses JSON and
Responses SSE results before any CPAR configuration mutation.

## Successor graph

The management API records `parent_id` as lineage only, so the successor re-enters the complete
production graph. It retains the current Codex Chat and Responses endpoints and its three public
protocol routes, then adds one grok2api Responses endpoint and Chat, Responses and Messages public
routes. Cross-protocol Chat/Messages candidates use `lossless_bridge`; Responses remains canonical.
One access group covers all six routes and one replacement client key is issued for the sole known
production client migration.

The grok2api target is admitted only when all of the following are exact: IPv4 loopback host,
explicit single port, HTTP scheme, `127.0.0.1/32` CIDR and redirects denied. The public management API
continues to reject every other plaintext target, including localhost names, wider CIDRs, missing or
multiple ports, userinfo, queries and mixed schemes.

## Current execution state

- The control database and predecessor client key have a root-only preimage backup; SQLite
  `quick_check` passed.
- The first draft stopped before its grok2api policy was created because the management HTTP layer
  previously rejected all HTTP even though the underlying egress layer already supports explicit
  narrow private/local CIDRs.
- Production was not published or switched. The incumbent production graph, Caddy, Newapi and old
  CPA rollback target remain unchanged.
- The narrow management admission fix and no-network graph helper tests pass locally. An exact-SHA
  signed ARM64 release artifact is required before the successor draft can be validated and
  published.

After deployment, acceptance requires preservation of the existing Codex path plus Grok-backed
Chat, Responses and Messages JSON/SSE probes, single-selected Explain results, zero P1 counter
growth, both SQLite integrity checks, and a proven graph rollback. Only then may a representative
P12-10 observation restart.
