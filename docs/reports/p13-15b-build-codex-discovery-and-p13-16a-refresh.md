# P13-15B Build/Codex discovery + P13-16A runtime OAuth refresh

Status: `LOCAL_PASS_PENDING_PRODUCTION`

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
| Grok Build | `grok-4.6`, `grok-4.5` | The imported Build account is independently classified as `supergrok`; no Web or Console catalog was reused. |
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

## Pending production acceptance

Before this report can move beyond `LOCAL_PASS_PENDING_PRODUCTION`:

1. push the exact revision to GitHub;
2. build an aarch64 binary from that revision on the Oracle host;
3. preserve a verified SQLite preimage and prior release symlink;
4. restart once and verify a due Build token is durably refreshed and published into the runtime
   pool without Autoreg participation;
5. verify public `/v1/models` and exact Responses calls for newly discovered IDs;
6. record whether route candidates were automatically materialized or only published as an
   operator-reviewed interim snapshot. The latter must not be described as completed P13-15C/D.
