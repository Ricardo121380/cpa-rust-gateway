# BC-PROVIDER-005 Kiro IDE/CLI endpoint policy

| Field | Value |
|---|---|
| Task | `P7-02` |
| ADR | [ADR-0047](../adr/ADR-0047-kiro-endpoint-policy.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

## Required behavior

| Concern | Requirement |
|---|---|
| API Region | Accept only bounded lowercase AWS-Region-shaped values (two or more non-empty lowercase name segments followed by a numeric segment, for example `us-east-1` or `us-gov-west-1`). It cannot contain a URL delimiter, dot, slash, uppercase letter, or underscore. |
| IDE policy | Derive exactly `https://q.{region}.amazonaws.com/generateAssistantResponse`; emit `content-type: application/json` and `origin: AI_EDITOR`; emit no CLI target Header. |
| CLI policy | Derive exactly `https://runtime.{region}.kiro.dev/`; emit `content-type: application/x-amz-json-1.0`, `origin: KIRO_CLI`, and `x-amz-target: AmazonCodeWhispererStreamingService.GenerateAssistantResponse`. |
| API key marker | Add `tokentype: API_KEY` only for the P7-01 `ApiKey` family. Social and Enterprise Header sets must not inherit it. The actual secret authentication value is not part of this policy. |
| Thinking placement | Expose `IdeThinkingWrapper` or `CliOutputConfigEffort`; do not infer model support or transform a request in P7-02. |
| Purity | The policy must not discover configuration, contact a host, accept an endpoint override, parse a response, create a TLS client, or expose a Secret. |

## Deferred behavior

This contract does not query/inject `profileArn`, attach machine/client fingerprints, construct an
authenticated request/body, transform Canonical messages/Tools/Thinking, parse EventStream, discover
models, classify errors, or make a real Kiro request.

## Corresponding tests

- `ide_and_cli_snapshots_have_exact_urls_headers_origins_and_thinking_placement`
- `api_key_header_is_explicit_and_oauth_never_inherits_it`
- `api_region_is_bounded_and_cannot_change_the_derived_host_shape`
