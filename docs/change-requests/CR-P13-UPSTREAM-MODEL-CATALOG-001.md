# CR-P13-UPSTREAM-MODEL-CATALOG-001 · All-channel upstream model catalog pass-through

## 1. Status and authority

| Field | Value |
|---|---|
| Status | **APPROVED / IN_PROGRESS — P13-15A local pass; P13-15B-E pending** |
| CPAR task | `P13-15` |
| Authority | User correction on 2026-09-01: reverse-proxied channels expose upstream model IDs instead of CPAR-invented aliases; applies to every channel |
| Delivery class | Provider/channel/endpoint/credential-scoped discovery, immutable authorized projection and exact-model routing |
| Protocol boundary | Independent of ingress protocol; OpenAI Responses remains the Pi/Grok Build protocol |

This change does not rename P13-13 Media or P13-14 additional Providers. It adds a new CPAR task
because model discovery and route materialization are shared infrastructure, not an account-tier
field and not a one-provider configuration tweak.

## 2. Problem

CPAR currently serves `/v1/models` from static `PublicModelConfiguration.model_name` records. A
deployment can therefore advertise an operator-created value such as `grok-cpar-build` while the
selected upstream actually receives `grok-4.5`. That is an alias catalog, not upstream model
pass-through. Adding `grok-4.5` and `grok-4.6` manually would make the strings look correct but
would still fail the requirement when an account gains, loses or renames a model.

The repository already contains storage-neutral, exact `(EndpointId, CredentialId)` discovery
primitives, last-success freshness, diff/removal isolation and compiler catalog admission in
`gateway-catalog`. They are not yet composed into the serving `/v1/models` projection or automatic
route materialization for all channels.

## 3. Frozen meaning of pass-through

“Pass-through” means all of the following:

1. The client-visible `id` is the exact upstream-provided model ID, retaining case and punctuation;
   CPAR does not prepend a Provider name or invent a friendly alias.
2. Discovery is scoped to the exact Provider channel, Endpoint and Credential that produced the
   evidence. A model observed for Grok Build is not evidence for Grok Web or Grok Console.
3. `/v1/models` is the deterministic, Client-Key-authorized union of currently eligible immutable
   catalog snapshots. It does not make an upstream network request on every client list call.
4. A request for an exact model may lease only a Credential/Endpoint/channel binding whose own
   catalog admitted that model. Selection cannot silently switch to a candidate with a different
   upstream model.
5. A duplicate exact model ID from several channels is legal only when the active Config Version
   contains an explicit, deterministic route policy for that ID. Otherwise publication fails
   closed as ambiguous; CPAR does not solve the collision by inventing a renamed model.
6. Client-Key and Access-Group filtering happens before serialization. One tenant cannot learn a
   model available only to another tenant, Endpoint or Credential pool.

OpenAI-compatible JSON remains wire-compatible (`object=list`, entries with exact `id`). CPAR may
retain channel/endpoint provenance in protected management views, events and internal snapshots;
it must not require Pi or another ordinary OpenAI client to send a CPAR alias.

## 4. Source matrix

| Channel class | Discovery owner | Authentication and isolation rule |
|---|---|---|
| Grok Build | Build-native catalog operation observed from the supported Build protocol | exact imported Build OAuth Credential; no Web/Console fallback |
| Grok Web | Web-native model/bootstrap source | exact Web account, sticky egress and session domain; no Build catalog reuse |
| Grok Console | Console-native model source | exact Console SSO/workspace Credential and Console egress domain |
| xAI Official | existing `GrokOfficialCatalogAdapter` | exact Official Endpoint/API key target |
| OpenAI-compatible `baseURL + API key` | bounded endpoint-relative model-list operation | exact Endpoint/Credential and configured API format; redirects and cross-origin auth forwarding denied |
| Anthropic-compatible `baseURL + API key` | bounded provider-compatible model-list operation when supported | exact Endpoint/Credential; unsupported endpoints remain explicitly manual, not guessed |
| Kiro | existing Kiro dynamic-catalog boundary | exact Kiro Credential capability observation |
| Any additional channel | its Provider adapter or an explicitly declared compatible source | no global hard-coded Provider switch and no inference from a display name |

A generic relay name such as Krill has no special behavior: it is one instance of the generic
`baseURL + API key` class. JSON import format, Provider display name and account tier do not decide
catalog behavior.

## 5. Delivery slices

### P13-15A · Exact-model serving boundary

- add an access-group-filtered exact upstream-model projection to the immutable Route Snapshot;
- reject an exact-model request unless its route can select only candidates carrying that model;
- change `/v1/models` to serialize exact upstream IDs rather than operator aliases;
- retain legacy alias input only as an explicit compatibility path during migration, never as an
  advertised model and never as evidence that pass-through is complete;
- change Pi and CC Switch to a discovered exact ID only after the public list and real Responses
  request pass.

### P13-15B · Provider/channel discovery composition

- compose a `ModelCatalogSource` per supported channel class;
- preserve the existing exact `(EndpointId, CredentialId)` singleflight boundary;
- bound status, body size, entry count, ID length, redirects, timeouts and content type;
- store no raw credential, URL query, response body or account identity in catalog records.

### P13-15C · Durable last-success, refresh and removal

- persist successful target-local snapshots and monotonic versions;
- retain a last success on transient discovery failure;
- use the existing Fresh/Stale/Expired deadlines and three-successful-miss plus 24-hour removal
  isolation; a failed call never increments removal evidence;
- publish refresh status and safe failure class through protected management views.

### P13-15D · Exact route materialization

- compile discovered exact IDs into immutable Client-Key-authorized routes without modifying a
  draft Config Version behind an operator's back;
- bind model eligibility to the exact Credential lease, not merely to an Endpoint-wide union;
- make additions atomic with snapshot publication and make ambiguous cross-channel IDs fail closed;
- ensure Chat, Responses, Messages and WebSocket ingress use the same exact-model resolver while
  retaining their own protocol encoders.

### P13-15E · Real acceptance and migration

- prove Grok Build list + non-streaming Responses with the imported real account;
- prove at least one generic `baseURL + API key` source;
- prove two Credentials on one Endpoint cannot see or lease each other's exclusive model;
- prove Build/Web/Console separation and duplicate-ID ambiguity handling;
- update Pi/CC Switch away from `grok-cpar-build`, then remove the obsolete advertised alias only
  after rollback evidence exists.

## 6. Non-goals

- no model-name invention, fuzzy matching, tier-based model inference or local-cache-only claim of
  upstream authority;
- no cross-Provider or cross-channel implicit fallback;
- no registration, OAuth login UI, account repair or replenishment (Autoreg remains separate);
- no switch from OpenAI Responses to Anthropic Messages;
- no unbounded live fan-out from the public `/v1/models` handler;
- no frontend-owned model whitelist.

## 7. Acceptance

1. The public list contains exact upstream IDs and contains no CPAR-invented alias for migrated
   routes.
2. Every listed ID has at least one hard-eligible, authorized, exact-model route; every accepted
   request selects only a matching candidate and Credential catalog.
3. Catalog output and request resolution are pinned to the same immutable snapshot and Access
   Group.
4. Channel/Endpoint/Credential isolation, staleness, removal and collision behavior have
   deterministic tests.
5. Grok Build/Web/Console and generic endpoints use their own source boundaries.
6. Pi uses `openai-responses`, CPAR `/v1` as base URL and an exact discovered model ID in a real
   non-streaming call.
7. Claude Code handoff and bilingual deployment/operator documentation describe the dynamic
   catalog and never recommend a hard-coded frontend list.

## 8. Rollback

Keep the previous immutable Route Snapshot and legacy alias resolver during the migration window.
If discovery or materialization fails, stop publishing new dynamic snapshots and restore the last
known-good Snapshot/binary; do not replace the catalog with guessed static IDs. Removing the
legacy alias is the final reversible migration step, not the first implementation step.
