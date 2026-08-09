# P12 Codex official endpoint switch — receipt and review

Date: 2026-08-09
Change request: `CR-P12-CODEX-OAUTH-004`
Scope: replace the production Codex OAuth upstream that was incorrectly sharing the Krill/API-key endpoint shape.

## Decision

The active CPAR graph now uses a dedicated official ChatGPT Codex Responses upstream. It is not the Krill endpoint, not a Krill egress policy, and not a generic API-key compatibility route. The official upstream is admitted by its own HTTPS egress policy and carries the OAuth-specific request profile.

The implementation follows the behavior found in the legacy CPA and Sub2API paths while keeping CPAR's Rust control/data-plane boundaries:

- preserve the OAuth account binding and expiry/refresh material;
- attach the account binding and Codex client identity headers at the official adapter boundary;
- force `store:false` and upstream SSE for the official OAuth Responses request;
- use the official Responses event decoder with private reasoning suppression;
- bridge Chat Completions and Anthropic Messages through an explicit lossless bridge, then project the result back to the requested protocol;
- subtract `reasoning` from the Chat candidate capability profile because Chat cannot represent the upstream private reasoning events;
- re-encrypt the credential through the management lifecycle for the successor Config Version. Credential ciphertext is version-bound, so copying ciphertext between versions is intentionally rejected.

No secret, raw OAuth document, token, cookie, request body, or response body is included in this receipt.

## Deployment evidence

- Active Config Version: `p12-09-codex-official-oauth-v5`.
- Oracle Singapore `cpa-rust-gateway.service`: `active`; data and management listeners remain loopback-only.
- A root-only rollback bundle was retained before the endpoint switch.
- The old CPA, Caddy, DNS, grok2api, and CC Switch configuration were not changed.
- No GitHub Actions or public artifact push was used; this was the approved ARM64 local-build deployment path.

## Real public CPAR matrix

All requests used the real CPAR public base URL and the existing client-key boundary. Values below are status-only; bodies are deliberately omitted.

| Client protocol | JSON | SSE | SSE data frames |
|---|---:|---:|---:|
| OpenAI Responses | 200 | 200 | 9 |
| OpenAI Chat Completions | 200 | 200 | 4 |
| Anthropic Messages | 200 | 200 | 6 |

The public `/v1/models` preflight returned HTTP 200. Each protocol had one JSON and one SSE request, and each request reached the official OAuth route through CPAR (`upstream_request=sent_via_cpar`). No failure category was recorded in the final matrix.

## Local verification

The following checks passed after the graph and adapter changes:

```text
cargo fmt --all -- --check
cargo test -p provider-openai-compatible
cargo test -p gateway
cargo clippy -p gateway --all-targets --all-features -- -D warnings
```

The focused suites cover official OAuth headers/body shaping, forced upstream streaming, `store:false`, reasoning-suppressed decoding, lossless Chat/Messages bridge projection, credential revision/AAD handling, and the three-protocol route contract.

## Review

### Findings addressed

1. **Wrong upstream identity** — the previous graph reused the Krill/API-key endpoint shape. The active graph now has an independent official Codex endpoint and egress policy.
2. **Credential revision mismatch** — importing ciphertext directly into a successor Config Version failed because the AEAD associated data contains the version identity. The credential was recreated through the management API, which seals it with the correct version-bound AAD.
3. **Official model compatibility** — the legacy graph model label was rejected by the official ChatGPT account boundary. The route now uses a compatible official catalog entry obtained from the upstream model discovery response; the label is intentionally omitted from this receipt.
4. **Cross-protocol publication** — Chat was initially rejected because the official target advertises Reasoning. The Chat candidate now uses lossless bridging plus an explicit `reasoning=false` capability override; Messages and Responses keep their appropriate bridge/canonical modes.

### Verdict

`PASS` — the active production graph no longer shares the Krill endpoint, and the real CPAR public matrix is green for JSON and SSE across Responses, Chat Completions, and Anthropic Messages. Rollback remains available from the root-only pre-switch bundle.
