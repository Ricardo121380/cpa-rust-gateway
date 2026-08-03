# P12-10E native Grok Console and Web production binding

Status: `DONE`

## Outcome

CPAR now has an executable native Grok Console Responses adapter and a fixed-target Grok Web
production request boundary. Console decodes one upstream execution into Canonical events and the
existing outer projections serve OpenAI Chat Completions, OpenAI Responses and Anthropic Messages.
Web remains deliberately text-only and does not advertise native Function Tool support.

The implementation was derived from the frozen grok2api `v3.0.10` source at revision
`c27f0545197b3edf41d5deedcc2c3c3597887766`; grok2api remains a behavior reference, not a runtime
dependency.

## Implemented boundary

- Console pins `https://console.x.ai/v1/responses`, the browser/SSO header profile, cluster header,
  stateless storage, model limits, reasoning envelope and search/function-tool request shape;
- the SSO value is bounded, rejects cookie delimiters, is zeroized where owned and never appears in
  `Debug`;
- the shared strict Responses encoder/JSON decoder/SSE decoder is reused instead of maintaining a
  second protocol implementation; arbitrary SSE chunking preserves Reasoning, Tool, text and Usage;
- the executable Console adapter uses the shared DNS-pinned upstream client and has no implicit
  retry or account fallback; pre-start HTTP failures are classified to request, credential, egress,
  quota or endpoint ownership;
- Web pins the existing reviewed conversation endpoint and browser-session/Statsig binding,
  disables memory and image generation, uses temporary conversations and feeds the existing strict
  live decoder;
- Web rejects Tool, Reasoning, cache, extension, opaque-content and unknown-model requests before
  transport, preventing a false capability claim.

## Acceptance evidence

The dedicated seven-test suite verifies fixed targets and headers, normalized requests, JSON and
every selected SSE chunk size, Tool/Reasoning/Usage semantic parity, all three public protocol
projections, safe failure attribution, bounded `Retry-After`, executable injected JSON/SSE transport,
the Web browser request profile/live decoder and negative Web capability admission.

Commands completed locally:

```text
cargo test --locked -p provider-grok --test p12_10e_console_web_runtime
cargo test --locked -p provider-grok
cargo clippy --locked -p provider-grok --all-targets -- -D warnings
```

Review also confirmed that request diagnostics retain only target/header-count/body-length metadata,
that Console is the only new executable adapter in this slice, and that Web remains a composable
production binding until native-account migration supplies its credential/session orchestration.

This slice read no live account, sent no external request, changed no production graph or service,
and did not modify grok2api or CC Switch.

## Next boundary

P12-10F adds the bounded memory-stream migration adapter from grok2api to CPAR. It must never write
plaintext credentials to disk, must re-encrypt accepted records immediately under the CPAR key, and
must produce only value-free counts with transactional rollback and idempotent-rerun evidence.
