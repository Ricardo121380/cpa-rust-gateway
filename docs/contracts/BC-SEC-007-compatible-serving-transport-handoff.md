# BC-SEC-007: Compatible serving transport handoff

Status: **P13-11C LOCAL_PASS_PENDING_PHASE_GATE**

## Scope

This contract governs request-time egress selection for generic compatible endpoints using
`openai-compatible.chat-completions`, `openai-compatible.responses`, or
`anthropic-compatible.messages`. It does not govern native Grok/Kiro adapters, browser clearance,
Autoreg, credential refresh, or real proxy probes.

## Required behavior

1. The active Route scheduler owns the only Credential lease. Compatible serving may acquire only
   an egress lease for that exact Credential ID, kind, revision, Endpoint and Upstream.
2. Config Version, EgressPolicy, Endpoint URL shape, pool revision/schedule, Health, account state,
   Quota, model state, expiry and egress capacity must all remain exact and fail closed.
3. The selected proxy must be applied to the existing JSON/SSE mode-specific timeout profile;
   response-mode deadlines and browser-emulation state must not be weakened or inferred.
4. Credential and egress capacity remain held until the returned event source is dropped.
5. `CredentialAndEgress` stickiness may retain only one exact node for one exact Credential in one
   runtime instance. It must not cross Endpoint or Upstream, and an unavailable retained node must
   not silently rotate.
6. Pre-response transport failure feedback is applied once to the configured scope:
   `Endpoint`, `EndpointCredential`, or exact `EgressNode`. The ordinary orchestrator must not add
   a second Endpoint-wide cooldown for a failure already classified here.
7. No adapter-level replay is allowed after HTTP submission begins. The existing Route retry budget,
   candidate exclusions and First Semantic Event rules remain authoritative.
8. Debug, error, observation, event and test receipt values must not expose base URLs, proxy URLs,
   API keys, OAuth material, cookies, request/response bodies, client-key digests, or DNS answers.

## Default and deferred boundaries

- The deployed active graph uses one Direct registry per generic Upstream until a separate protected
  Config-Version proxy-pool schema is approved.
- Fixed/pool behavior is exercised with deterministic local fixtures only in P13-11C.
- Automatic probe, recovery, persistent sticky state, remote-DNS HTTP/HTTPS proxies, Grok Web
  clearance, Provider-native auxiliary HTTP, staging and production rollout are deferred.
- No management OpenAPI, Prism, frontend or public request shape changes are part of this contract.

## Acceptance evidence

- exact selected-Credential handoff without a second Credential cursor or lease;
- JSON/SSE timeout preservation with selected Direct/SOCKS5 proxy identity;
- lease lifetime across success, stream, cancellation, error and drop;
- exact Endpoint/Credential/EgressNode failure feedback with no double cooldown;
- sticky same-node and unavailable-node fail-closed tests;
- generic serving loopback tests with zero real Provider/proxy traffic;
- strict Clippy, formatting, docs, source/crate boundaries, secret scan and local Fast Gate.
