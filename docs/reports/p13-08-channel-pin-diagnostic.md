# P13-08 Channel Pin diagnostic report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

## Objective

Provide operators with one deterministic management diagnostic that pins a
single Provider, channel, route and credential and reports whether an upstream
send occurred, without exposing secrets or changing the public inference data
plane.

## Frozen scope

- protected management-only `POST /admin/operations/channel-pin`;
- runtime-current active Config Version plus exact Provider/channel/route/
  credential/model; draft/archived versions are no-send snapshot conflicts;
- JSON and SSE projections through the generic OpenAI Chat/Responses and
  Anthropic Messages adapters whose transport boundary is one inference send;
  native Grok Console/Web adapters with hidden bootstrap/refresh HTTP are
  rejected before lease/network and deferred to a Provider-specific one-shot
  policy task;
- fixed bounded probe payload, no arbitrary caller body in the first slice;
- at most one admitted upstream inference request, first failure is terminal, no retry, quota
  recovery, sibling selection or cross-Provider fallback;
- existing auth, CSRF, `If-Match` revision/ETag, egress, capability, Health/Quota,
  credential lease and event-stage boundaries remain active;
- the admitted `If-Match` revision is carried into the executor and compared with
  the runtime composition revision before exact lease;
- value-free receipt and pre-execution audit fields only; the returned receipt
  is the terminal outcome, while the ordinary serving Attempt event is
  intentionally not emitted before source drain;
- `NativeExact` candidates are rejected before lease/network;
- at most two pins are admitted concurrently; a third is rejected without a
  lease or Provider call;
- deterministic mock tests first; no Provider traffic, production/staging
  mutation, server rollout or additional CI Delivery Gate.

## Implementation evidence

The implementation slice adds a management facade over the same runtime
composition used by serving. It does not duplicate credential stores,
schedulers, egress pools, or Provider adapters. The executor produces a closed
receipt with request id, selected identities, protocol/mode, outcome,
`upstream_sent`, `attempt_count`, response-started flag, observed closed stage,
and observation time. The request and `channel_pin_started` pre-execution
action are recorded through the existing value-free audit boundary before any
Provider call; the action stores only the route resource id. The returned
receipt is the terminal outcome and no post-send audit append is performed.
Exact attribution remains in the returned receipt, and no audit identifier is
exposed.

The exact-credential lease path bypasses both route and credential weighted
cursors, so a diagnostic cannot perturb the next ordinary serving choice.
Health, Quota, expiry and capacity are revalidated immediately before the
atomic lease. The runtime admits only reviewed generic Chat/Responses/Messages
adapters; native Grok/Kiro/Official adapters and `NativeExact` candidates fail
before the in-flight slot, lease or network boundary.

## Verification evidence

The final local matrix passed on 2026-08-15:

- `gateway-upstream`: 32/32, including exact lease, capacity, expiry and cursor
  preservation;
- `gateway-router`: 134/134, including pinned identity, Health/Quota/capacity,
  zero fallback, one driver call and lease release;
- `gateway` binary: 105/105, including bounded concurrency, restart-unique
  request ids and native-adapter pre-transport rejection;
- management HTTP/OpenAPI/security fixtures: 19/19 across P13-08, P10-01,
  P10-02 and P10-04;
- Grok Web one-shot regression: 4/4, proving its scoped 403 recovery can be
  disabled even though this P13-08 slice rejects the native adapter;
- strict Clippy for all touched Rust packages, `cargo fmt`, Prism contract/client
  check, docs/link/contract/plan/secret gate and `git diff --check` all passed.

Independent final review found no remaining P1 blocker after exact cursor-free
leasing, revision rechecks, native-adapter admission, bounded canonical drain,
receipt state closure and pre-send audit ordering were reconciled.

## Required review questions

1. Can any malformed, stale, foreign, ambiguous or disabled target reach the
   Provider driver?
2. Can a retry, quota recovery or cross-Provider fallback happen after the
   first failure?
3. Does the receipt/audit/log path exclude URL, body, header, cookie, token,
   ciphertext, plaintext, digest and raw upstream error data?
4. Is the selected credential both the one leased and the one attributed?
5. Are JSON and SSE bounded by existing transport limits, with leases released
   on every terminal path?
6. Does the management route leave ordinary public serving and Config Version
   state unchanged?
7. If OpenAPI changes, did the commit synchronize Prism and leave a precise
   Claude Code handoff in `docs/cross-boundary-log.md`?

## Acceptance status

The local implementation slice is `LOCAL_PASS_PENDING_PHASE_GATE`. It does not
claim native Grok Console/Web support, a real Provider call, production/staging
behavior, a full runtime transport E2E canary, or a formal P13 Delivery Gate.
The native one-shot adapter boundary is an explicit follow-up task rather than
an implicit retry exception. No production, staging, server or active Config
Version state changed during this local acceptance.
