# P13-15B Build/Codex discovery + P13-16A runtime OAuth refresh

Status: `BUILD_PRODUCTION_PASS / CODEX_REAUTH_REQUIRED / P13-15C-D_PENDING`

Date: `2026-09-02`

## Scope

This slice responds to two operator corrections:

1. Credentials already saved in CPAR must not require an external Autoreg scheduler for ordinary
   OAuth renewal.
2. Public model IDs must originate from each exact upstream catalog. A short operator-maintained
   list is not model pass-through.

The implementation is intentionally split across two plan items. P13-15B adds exact
Endpoint/Credential-scoped catalog sources for Grok Build and official Codex. P13-16A composes
startup and periodic refresh for the same active OAuth channels. P13-15C/D durability and automatic
route materialization are still required before the public list follows catalog changes without a
new configuration publication.

## Upstream observations

All observations were made with the official channel request shape and were reduced to non-secret
model identifiers before being recorded:

| Channel | Exact upstream result used by the implementation | Interpretation |
|---|---|---|
| Grok Build | `grok-4.6`, `grok-4.5` | The source used the exact imported Build Credential; tier evidence remains independent and no Web or Console catalog was reused. |
| ChatGPT/Codex | `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.5`, `gpt-5.4-mini` | Only entries marked both client-visible and API-supported are admitted. Hidden `gpt-reserve` and `codex-auto-review` entries are excluded. |

The Codex response itself includes plan availability metadata. CPAR does not turn that metadata into
a hard-coded tier-to-model table: the exact Credential's successful catalog response remains the
authority. The local credential used for this observation projected `free`; this is sufficient to
prove Luna is not a Go-only hard-coded exception, but it is not a claim about every account.

## Discovery boundary implemented

- Grok Build uses the fixed CLI chat-proxy `/v1/models` operation and the complete supported Build
  header profile. The adapter accepts only its own exact `(EndpointId, CredentialId)` target.
- Codex uses the fixed ChatGPT Codex catalog operation with the exact OAuth account binding and
  current client-version query. It keeps only `visibility=list` and `supported_in_api=true` entries.
- Both transports reuse DNS-pinned egress admission, deny redirects, bound successful bodies to
  1 MiB, bound entries to 512 and model IDs to 512 bytes, reject duplicate JSON names and return
  value-free errors.
- Exact upstream IDs are preserved and deduplicated. No Provider prefix, friendly alias or package
  whitelist is introduced.
- The source adapters do not mutate routes, retry with another Credential or cross channel.

This completes only the Build and Codex portion of P13-15B. Grok Web, Grok Console, xAI Official,
generic OpenAI/Anthropic-compatible endpoints, Claude, Kiro and future channels remain separate
source owners. Existing Official and Kiro source primitives are not yet equivalent to active-graph
composition.

## Runtime refresh boundary implemented

- One startup catch-up pass runs before serving pools compile.
- One periodic owner runs every minute after both listeners bind, outside listener workers.
- Native Grok jobs are filtered to `provider=build`; a Build executor cannot claim Console/Web rows.
- Codex refresh selects only active Credentials bound to the exact official Codex endpoint, not
  every JSON object labelled `oauth_json`.
- Durable replacement uses encrypted revision CAS and management audit. Runtime replacement uses
  an atomic material pointer; an in-flight lease retains its old revision until it ends.
- The Build serving path now accepts CPAR's authenticated compact post-refresh form as well as the
  supported import JSON forms, while still rejecting expired access tokens for inference.
- API keys remain non-refreshable. Registration, initial OAuth and revoked-token interactive
  recovery remain outside CPAR's routine renewal loop.

P13-16A covers all refreshable OAuth Credentials in the current production graph (Grok Build and
official Codex). Claude and Kiro refresh composition remain follow-on work before those deferred
channels can be activated in production.

## Local evidence

The final local run completed with no failure:

- `cargo test -p gateway --all-features --no-fail-fast --quiet`: `118/118`;
- `cargo test -p gateway-upstream -p provider-grok -p provider-openai-compatible --all-features --no-fail-fast --quiet`: all executed tests passed; four explicitly authorized/live tests remained ignored by their existing boundary;
- Build catalog focused suite: `3/3`;
- Codex catalog focused suite: `3/3`;
- provider-scoped refresh claim regression: `1/1`;
- exact active-channel refresh-scope regression: `1/1`;
- strict Clippy for the four changed crates with warnings, `unwrap` and `expect` denied: passed;
- `cargo fmt --all -- --check` and `git diff --check`: passed.

No file under `web/prism/**` changed. The management OpenAPI shape is unchanged. The four
pre-existing untracked helper scripts were not read, edited, executed or added to Git.

## Production acceptance

The exact implementation commits were pushed to GitHub and deployed to Oracle Singapore:

| Evidence | Result |
|---|---|
| Source commits | `e0b2d82b13a9363fbad3e71ec994f2f8e1ea58fc`, followed by bounded Codex retry fix `388e156cdf8c4c0693ecaed02ddd772f89e03962` |
| Deployed aarch64 artifact | SHA-256 `4691d6b34dd09f8eff101f7e65f3ad5094bc54b8e1ae39faee8b0da6b095b5fc`; embedded revision equals `388e156cdf8c4c0693ecaed02ddd772f89e03962` |
| Rollback safety | Prior immutable release retained; SQLite backup `control-pre-388e156-20260902T074654Z.sqlite3` is non-empty and passed `PRAGMA quick_check` |
| Startup Build refresh | two due Build jobs claimed; one succeeded and moved durable/runtime revision `0 -> 1`; one invalid grant moved to `reauth_required` |
| Codex refresh | both exact active Codex bindings reached the refresh path but their old grants were rejected; failed retries use process-local exponential `1/2/4/.../60` minute backoff |
| Serving continuity | service active; `/healthz` succeeded; live SQLite `quick_check` succeeded |
| Real public Responses | the Pi-specific Client Key called exact `grok-4.5`; HTTP `200`, `status=completed`, output marker `CPAR_OK` |

This proves that a valid refreshable Build credential already stored in CPAR renews without an
Autoreg scheduler and that the rotated material continues serving. It does not prove a valid Codex
refresh because both production Codex credentials currently report plan `free` and both stored
refresh grants are no longer accepted. They are not ChatGPT Go credentials. An operator must
reauthorize the intended account once; subsequent ordinary renewal remains CPAR-owned.

The authenticated production `/v1/models` response still contains only `gpt-5.6-terra`,
`grok-4.20-0309`, and `grok-4.5`. It does not yet contain the source-observed `grok-4.6` or
`gpt-5.6-luna`. This is affirmative evidence that P13-15C/D have not run: the source result must be
persisted with exact Credential provenance and atomically materialized into an exact-model route
before it can be advertised. No model was added manually to Pi, CPAR configuration, or a frontend
allowlist to conceal that gap.
