# ADR-0134: Versioned endpoint request rules and encrypted upstream headers

- Status: Accepted
- Date: 2026-10-25 (Asia/Shanghai)
- Task / Contract: #71 / OPT-04; BC-MGMT-022; ADR-0125/0126/0130

## Context

CPA v8.0.40 (`d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`) applies
`internal/runtime/executor/helps/payload_helpers.go::ApplyPayloadConfigWithTrackedPathsForExecutor`
after translation/thinking: default (first write), override (last write), filter (last phase).
`internal/util/header_helpers.go::ApplyCustomHeadersFromAttrs` can replace authorization/Host
and interpolate incoming headers. Those unrestricted paths conflict with #71's identity,
permission, history and Secret requirements. Existing version/revision/audit/runtime publication
already supplies configuration lifetime and must remain the only configuration service.

## Decision

Attach a typed `request_policy` to the existing Endpoint. Endpoint ownership fixes its channel,
upstream and target protocol. Rules match exact **resolved public model** names (or sole `*`)
and optional downstream protocol. Aliases, prefixes, suffixes and upstream model strings do not
create alternate matching or authorization identities. Supported adapters are the three ordinary
OpenAI Chat/Responses and Anthropic Messages compatible adapters, including their existing
OAuth channels. Dedicated Grok Build/Console/Web/Official and Kiro bodies/headers have independent
fixed profiles; nonempty policies on those adapters reject explicitly. This is the bounded
supported scope, not CPA executor-wide parity or a change to Build activation.

Only target-native sampling/output root fields are configurable; the complete whitelist and
bounds are in [the public contract](../support/request-rules-upstream-headers.md). No path syntax,
raw JSON insertion, wildcards other than whole model `*`, request conditions, incoming-header
variables or scripts are accepted. Model/route/storage/owner/credentials/metadata/tool controls,
content/history, thinking, service tier and cache identities cannot be edited or filtered.
Explicit LosslessBridge candidates reject body rules.

Ingress, permissions, source controls and tool history are validated first. Existing #69 source
and candidate-model normalization/protocol conversion run unchanged. Provider builders produce
the target body and mandatory fields. Matching rules then run in three global phases: defaults
fill missing target fields (null counts as present), overrides replace values, filters remove
optional fields. Default observes the translated body, including synthetic defaults; it does not
reconstruct CPA's separate originalTranslated/Claude max-token exception. Re-decode and validate
the final target controls/capabilities/history before marking a transport send. Provider-owned
fields are protected from rules, so OAuth stream/store and account identity remain mandatory.
Every retry starts from the original request and pinned generation, without cumulative mutation.

Header configuration contains only lowercase names and Credential IDs. Values use the existing
write-only Credential endpoint with `kind=upstream_header`, existing Secret Store/AAD, and no
serving account binding. References must be active, same-version and same-upstream; account
credentials cannot be borrowed. Runtime preparation authenticates/decrypts and bounds all values
before activation. Retained plaintext uses existing redacted, zeroizing `PlaintextSecret`.
Additional headers append after provider assembly and cannot collide with reserved account,
internal, negotiation, origin or transport names. No header value enters config DTOs, diff,
public responses, audit or diagnostic formatting. Transport marks added HeaderValues sensitive.

Schema41 migration0041 adds one JSON column with empty default to `upstream_endpoints`.
Full graph write/load, inventory, fork/reseal, revision mutation, diff, publish, restart and rollback
retain their existing owners. An endpoint mutation's audit plus the persisted version/revision
records intentional filters as configuration behavior, not an assertion of lossless delivery.
Count ignores these generation-only body fields; native Anthropic count receives the endpoint's
additional headers, while local estimation performs no network request. Neither enters inference
Attempts, Usage, ledger or quota history.

## Consequences

No route selector/affinity, frontend, configuration service or external dependency is introduced.
The composition binary directly uses the existing Anthropic codec for final-body validation,
matching its existing Chat/Responses codec dependencies. Cargo.lock changes only that workspace
edge. Old schema40 databases upgrade with empty policies; existing ledger/reviews/frozen exports
and encrypted request payloads are not rewritten.

## Alternatives considered

Copying arbitrary CPA JSON paths or header precedence would bypass protected invariants.
Plaintext header strings in the Endpoint DTO would leak through ordinary management reads and
diagnostics. New encrypted-header storage would duplicate the existing Credential lifecycle.
Modifying dedicated provider profiles without a matching body/transport contract is rejected
at configuration admission rather than silently ignoring the policy.

## Validation and rollback

Actual management socket save/validate/publish, concurrent revision, public JSON/SSE socket and
independent receiver tests cover the supported protocols; owned history, cross-Key, count,
headers, invalid fields and rollback are checked separately. Source/lock/binary/input manifests,
Full and two-axis review results belong to the dated implementation report.
Operational rollback republishes the retained version and restores its exact policy/headers.
Schema41 downgrade to40 drops policy configuration and is not operational rollback; retain original
binary, coherent artifact and all Master Key versions. V2 backup limits/empty-target semantics,
#69 controls/permissions, #73 price identity/old ledger/frozen exports and #75 restoration remain.
No real Provider, actual-client, fixed Build, CI, release or deployment qualification is inferred.

Number0134 and migration0041 are registered in the local wave3 coordination file;0133 is left for
parallel #70. No #70 implementation or workspace mutation is included.

A later read-only concurrent-state check found #70 also independently chose migration0041.
This is a known branch collision: the two schema41 layouts are not interchangeable. Integration
must reconcile migration order/identity before constructing a shared binary or restoring a
branch-specific artifact. No cross-branch schema41 compatibility or integration PASS is claimed.
