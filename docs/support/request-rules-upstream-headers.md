# Request rules and custom upstream headers

#71 implements bounded CPA-style configuration through the existing protected management API.
[ADR-0134](../adr/ADR-0134-versioned-request-rules-upstream-headers.md) and
[BC-MGMT-022](../contracts/BC-MGMT-022-request-rules-upstream-headers.md) define its boundary.

An Endpoint owns `request_policy` within a Config Version. Omitted/empty policy preserves existing
behavior. Existing PATCH replaces the complete Endpoint input, including policy; omit it to clear
policy in a draft. PATCH never modifies active/archived configuration. Use the normal fork,
`If-Match`, validate, publish and rollback operations. All changes are local until published.

| Scope | Meaning |
|---|---|
| Channel | Exact Endpoint and its owning Upstream; no shared/global rule namespace |
| Target | `openai-compatible.chat-completions`, `openai-compatible.responses`, `anthropic-compatible.messages`; target field names use that Endpoint's protocol |
| Model | Case-sensitive resolved public names with a Candidate for this Endpoint; sole `*` matches every already-authorized model |
| Protocol | Optional exact downstream `openai/chat-completions`, `openai/responses`, `anthropic/messages`; absent/null matches all |
| Key | Existing route/Access Group permissions apply before rules; no client-settable owner/Key/rule selector |

Dedicated `grok.build.responses`, `grok.console.responses`, `grok.web.responses`,
`grok.official.responses` and `kiro.messages` reject nonempty policy. Their fixed provider
profiles are unchanged. This feature does not qualify those profiles or enable native fields.

| Target protocol | Allowed fields |
|---|---|
| Chat | `temperature`, `top_p`, `stop`, `seed`, `presence_penalty`, `frequency_penalty`, `max_tokens`, `max_completion_tokens` |
| Responses | `temperature`, `top_p`, `max_output_tokens` |
| Messages | `temperature`, `top_p`, `top_k`, `stop_sequences`, `max_tokens` |

Temperature is0..2 (Messages0..1), top_p0..1, penalties-2..2, seed signed64-bit.
Limits/top_k are positive integers through4294967295. Stops are1..4 nonempty strings, each at
most1024 UTF-8 bytes; Chat also accepts one string. Values cannot be null. Messages max_tokens
is mandatory and cannot be filtered; other protocol/collision/budget checks still apply after
rules. Overlapping model/protocol rule scopes cannot force both Chat output-limit names after
the final filter phase; a filter that removes one spelling can explicitly resolve that conflict. These bounds do not promise a
provider accepts every value up to the configured limit.

Protected fields include model/routing identity, credentials, ownership, internal/client metadata,
messages/input/system/content/history, tools/tool choice/parallel controls, reasoning/thinking,
service_tier, cache identity, store/previous_response_id and stream. Nested paths, JSON Pointer,
raw/expression/query fields and extra DTO members reject. Body rules reject LosslessBridge.

Rules are ordered, with at most32 entries and64 model selectors per entry. Within one rule, a
field cannot occur in multiple operations or twice in filter. Across matching rules, all defaults
run first, keeping the first default and any existing target/client value; all overrides run
next, keeping the last; all filters run last. A present null is present. Defaults see the actual
translated body, including translator-created defaults. They do not imply a source option that
was omitted by compatibility conversion. A client-dependent final collision returns a safe
request error before transport. Invalid ingress/history cannot be repaired by filters.

#69 name resolution/permissions/body control validation and model normalization remain before
provider assembly and policy. Rules do not edit those controls. Final target validation preserves
control/budget/capability constraints; Provider stream/store/model/auth/framing requirements remain
protected. Intentional overrides/filters change the configured request semantics. This is
**compatibility conversion**, not native byte preservation or lossless equivalence. The saved
version/revision and endpoint audit record identify the configuration that selected it.

Headers are endpoint-wide, including native Anthropic count. At most16 headers; lowercase ASCII
letters/digits/hyphens,1..64 bytes. Values are1..4096 printable ASCII bytes; CR/LF, tabs, all control
bytes and non-ASCII reject. Aggregate names+values must not exceed16384 bytes. Reserved names
include account/auth/cookie/key/token/proxy/secret/credential/internal names, `x-cpar-*`, `x-msh-*`,
`anthropic-*`, `openai-*`, Host/framing/hop-by-hop, Content-Type/Encoding, Accept/Encoding,
User-Agent, Origin/Referer, chatgpt-account-id, originator and version. Additional headers never
replace a built-in header. Case variants do not create a bypass. No `$HEADER`/session interpolation.

Each header value is a separate existing `upstream_header` Credential, active, unbound and under
the same Upstream/Config Version. The management API accepts its value write-only and returns
only `secret_present`. Endpoint GET/list/diff/audit contain references, never values. Missing,
foreign-upstream, disabled, account-kind or serving-bound references block publication; bad bytes
fail Credential save, and stored values are authenticated/checked again during runtime preparation.
Keep all Master Key versions for restart/fork/recovery. Custom headers affect inference only for
the supported adapters; they are not added to discovery, refresh, token exchange or other tasks.

Messages count ignores all generation rule fields. Its input content/history/model remain the
counting contract's own values. First-party native count adds configured endpoint headers;
third-party/local estimation sends nothing. Neither path produces inference Usage/ledger.

Example: on an existing draft `next`, first save a header Credential through the protected
`POST /admin/upstreams/provider/credentials` using the current revision:

```json
{"id":"request-tag","kind":"upstream_header","secret":"example-tag","status":"active"}
```

Use the returned revision in the normal complete Endpoint PATCH input. A Responses Endpoint can
include this policy (the actual model must have a Candidate for this Endpoint):

```json
{
  "rules": [
    {"models":["public-model"],"protocol":"openai/responses",
     "default":{"temperature":0.2},"override":{"max_output_tokens":2048},"filter":["top_p"]}
  ],
  "headers": {"x-request-tag":"request-tag"}
}
```

The property is `request_policy`; headers map to IDs, not literal values. Save alone does not
change serving. `POST /admin/config-versions/next/validate`, then publish with its current
`If-Match: rev-N`; existing lifecycle/expected-active/account-restoration fences still apply.
Rollback uses `POST /admin/config-versions/rollback` and the target's current revision. A stale
revision returns conflict without a partial rule/header update or extra audit row.

The integrated schema42 chain preserves routing/affinity as41 and adds the Endpoint policy
column as42. Common40 and the routing-only standalone41 upgrade; the historical #71 policy-only
standalone41 is structurally incompatible and rejects before migration or restore publication.
Keep its original binary/artifact/keys; do not edit the version number or drop configured policy
to force admission. [ADR-0135](../adr/ADR-0135-integrated-schema-identity-and-atomic-downgrade.md)
records this limited successor to ADR-0134.

Config fork reseals header Credentials under the new AAD. Normal version rollback retains policies.
Schema downgrade rejects nonempty/unknown policies; semantically empty policies may downgrade.
All requested down steps form one transaction, so a later routing/affinity guard failure preserves
both the original schema and data. Earlier successful down migrations retain their historical
contracts; schema downgrade is not a general lossless operational rollback.
#75 v1/v2 formats, empty-target restore and independent keys retain their existing limits.
[The joint receipt](../reports/wave3-integration-20261025.md) verifies nonempty rules/header secrets,
Build affinity and reviewed prices after a new stock gateway adopts the restored schema42 database.
Real providers/clients, fixed Build and releases remain separate verification scopes.
