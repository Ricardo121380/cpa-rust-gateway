# P12-06 Kiro channel functional verification and performance baseline

| Field | Value |
|---|---|
| Plan version | `v1.79` |
| Task | `P12-06` (Kiro portion) |
| Status | `IN_PROGRESS` |
| Scope | Kiro channel only. Per `CR-P12-06-003` this is **not** the incumbent differential: the incumbent CPA has no Kiro channel, so there is no second side to compare against. The OpenAI-compatible differential is separate and not covered here. |

## Why this is a comparison and not a one-sided baseline

`CR-P12-06-003` scoped the Kiro evidence as functional verification plus a performance baseline,
because the incumbent has no Kiro channel. During execution a better reference appeared: the user's
own `kiro-rs` runs on this host and serves an **Anthropic-compatible** surface on
`127.0.0.1:8990` against the **same** Kiro upstream, using the **same** credential family. It is
also the implementation cpa-rust's Kiro channel was modelled on.

That makes a genuine paired comparison possible — same upstream, same credential, same prompt, same
harness — which is strictly more informative than a single-sided baseline. The measurement therefore
reports both arms. It remains **not** the incumbent differential, and does not count toward Kiro's
P7-09/G7 Provider Gate, which stays `DEFERRED`.

## The defect this task found

The first real request through the newly wired channel was rejected, and so was every subsequent
one. The channel had a **100% failure rate** for any compliant Anthropic client, failing before any
network call with `ClientRequestError`.

Both sides were individually correct:

| Component | Behaviour | Why it is right |
|---|---|---|
| `protocol-anthropic` decoder | Anthropic Messages *requires* `max_tokens`; `ROOT_FIELDS` excludes it, so it is retained as the root extension `anthropic.messages.max_tokens` | The Canonical core has no shared output-limit field, and inventing one would pollute the core for a single protocol's requirement |
| `provider-kiro` converter | Rejects every Canonical root extension (`BC-PROVIDER-007`) | Kiro's `conversationState` carries `content`, `modelId`, `origin`, `envState`, optional `tools` and optional `outputConfig.effort` — and nothing that expresses an output maximum. A converter must not silently discard a semantic it cannot express |

Multiplied together, the two correct rules made the channel unusable. This was not visible to any
existing test, because both components are tested against their own contracts and neither contract
is wrong.

`CR-P12-06-005` resolves it at the composition root: drop exactly that one extension for Kiro, keep
every other. Dropping an output *ceiling* cannot corrupt a response — the client asked for at most N
tokens and receives a complete answer that may be shorter or longer — while any other extension
still fails closed inside the converter with the converter's own classification. This matches the
reference implementation, which accepts `max_tokens` on its Anthropic surface and forwards nothing.

Local isolated verification of the fix:

| Stage | Before | After |
|---|---|---|
| Error surfaced to the client | `invalid_request_error` | `api_error` |
| Attempt error code | `ClientRequestError` | `ProviderPermanent` |
| Meaning | Failed during request conversion, no packet sent | Converted, egressed to `runtime.us-east-1.kiro.dev`, rejected by the upstream for the deliberately fake credential |

The second row is the desired outcome for a fake credential: it proves the whole path — conversion,
`profileArn` resolution, egress admission, DNS pinning, TLS, and Kiro's own rejection — is live.

## Configuration-graph admission rules found by rehearsal

The graph sequence was rehearsed against a scratch gateway before touching the server. Three
requirements are not in the runbook and would each have failed the real run:

| Requirement | Failure if omitted | Why the runtime demands it |
|---|---|---|
| Candidate `capability_override` must be exactly `{"allow_unlisted_model": true}` | `catalog_model_not_eligible` | P12 performs no catalog import at all, so no real upstream model is Catalog-eligible; this explicit exception is the designed admission path |
| Public Model `capabilities` must be empty | `candidate_capability_mismatch` | The map is a *requirement* the effective Candidate profile must satisfy, and P12 injects an empty profile for every Endpoint until a controlled request proves a capability |
| One service restart between creating the Endpoint and validating | `missing_endpoint_capability_profile` | The Route Compiler builds its Endpoint capability table once at process start, so a draft introducing a new Endpoint identity cannot validate in the process that created it |

The third is the same restart-after-publication lifecycle the deployment already has, so the entry
script is split into `enter` → restart → `--finish` → restart → `--explain-only` phases rather than
working around it.

## Measurement method

Server-side latency quantiles do not exist (plan gap 8: the Prometheus surface is seven counters
with no histogram, and `ManagementRequestAttempt` carries no timing by design). All timings are
therefore measured client-side, on the loopback data plane, **before** the split layer, so no proxy
hop is attributed to either gateway.

Four boundaries are recorded separately because on a thinking model they differ by seconds:

| Metric | Boundary | Why it is reported separately |
|---|---|---|
| `ttfb` | First response byte, including headers | Upstream has accepted the request |
| `first_semantic_event` | First semantic SSE event (`message_start`) | The BL-05 transparent-retry boundary: before it an attempt can still be retried invisibly |
| `first_text_delta` | First `content_block_delta` | What a human calls "the first character". Its gap from `first_semantic_event` is the upstream thinking pause |
| `total` | Stream termination | End-to-end duration |

Output token rate uses `output_tokens / (total - first_text_delta)` — the post-first-token window.
Using total duration as the denominator would fold the thinking pause into the rate and understate
steady-state throughput. First-token latency is reported separately, so nothing is concealed.

Quantiles are nearest-rank, never interpolated: an interpolated P99 of ten samples reports a number
no request produced. Sample counts travel with every summary.

Rounds are **interleaved** between the two arms rather than run in blocks. Upstream latency drifts
with load, and a block design would attribute that drift to whichever implementation happened to run
in the slower window. Per-round paired deltas are reported alongside the aggregates.

Warm-up samples are discarded rather than merged: the first request pays DNS and TLS setup that later
requests reuse, and folding it in would misreport steady-state latency.

Failures are recorded, not retried. Retrying until N successes accumulate would report the latency of
a channel that never fails, which is not the channel under test.

No prompt text, response text, or credential value appears in any output file.

## Results

### Free-credential replacement attempt (2026-08-02)

The previously selected Kiro Pro Max API-key account exhausted its allowance. Under
`CR-P12-06-007`, the remaining two headless `ksk_` credentials in the server's kiro-rs credential
store were tested without rendering or copying either value outside the host. The fourth stored
credential is IdC OAuth, not a headless API key; its access token was already expired and the P12
composition deliberately does not refresh or compose that family.

Each headless candidate received its own successor Config Version because an active Version is
immutable. Every graph used the same single Endpoint/Credential/Candidate shape, concurrency one,
`max_attempts=1`, a new version-scoped Client Key, and a root-only ledger. Both graphs validated,
published and selected the sole Kiro Candidate before one functional request was sent.

| Candidate | Config Version | Gateway functional request | Corrected same-shape direct classification | Result |
|---|---|---|---|---|
| kiro-rs id `4`, `KIRO FREE`, API key | `p12-06-kiro-v3` | one request, `502`; zero successful samples | one request, upstream `403`, Kiro JSON content type | unusable |
| kiro-rs id `2`, `KIRO FREE`, API key | `p12-06-kiro-v4` | one request, `502`; zero successful samples | one request, upstream `403`, Kiro JSON content type | unusable |

The gateway's public `502` is consistent with the Kiro boundary's fail-closed mapping of an
otherwise-unattributed upstream `403` to `EgressRejected`; it is not evidence of a DNS, TLS or local
Host/CIDR failure. Server DNS returned only public addresses, direct HTTPS reached the Kiro host,
and the persisted policy remained HTTPS-only with the exact host, port 443, no CIDR exception and
redirects denied.

Two earlier direct diagnostic commands suffered a shell-quoting defect: request JSON generation
failed but curl still sent an empty body, producing one `400` for each candidate. Those two requests
are retained as invalid diagnostics and are not counted as credential evidence. The command was
then corrected to construct and validate the complete in-memory JSON before the two `403`
classifications above.

No warm-up or performance round was started, because accumulating latency samples from a channel
with zero successful functional requests would manufacture a performance conclusion. The Kiro
portion of P12-06 remains `IN_PROGRESS`, blocked on either a usable `ksk_` credential or a separately
approved IdC/OAuth composition and refresh scope.

## What this evidence does not claim

- It is **not** the incumbent differential. The incumbent CPA has no Kiro channel.
- It does **not** satisfy Kiro's P7-09 / G7 Provider Gate, which remains `DEFERRED` per
  `CR-P7-DEFER-002`. Only the headless `ksk_` credential family is composed; Social and Enterprise
  credentials need OAuth refresh and an authenticated profile lookup that this build hard-refuses.
- It does **not** authorise any production-hostname routing for Kiro. `CR-P12-ROLLOUT-001`'s Canary
  scope is unchanged.
- Server-side real-time latency quantiles are still unavailable; every number here is a client-side
  observation.
