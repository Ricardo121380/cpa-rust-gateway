# P12-10I-14 Web native CPAR E2E receipt

Status: `BLOCKED_WITH_EVIDENCE`

## Boundary

This receipt covers one fresh isolated staging graph, one imported Web account, and one real
CPAR HTTP harness invocation through the loopback data listener. The harness used `curl`, first
performed the models preflight, then stopped on the first failed inference. It wrote only
value-free counts and error categories. No SSO, cookie, credential, client key, account identity,
endpoint, model, request body, response body, or token fingerprint is recorded.

Production listeners, the active production Config Version, the production account pool, Caddy,
DNS, CC Switch, the old CPA, and grok2api were not changed.

## Runtime diagnosis

The first corrected staging graph failed to start with the generic runtime-unavailable error. A
temporary local diagnostic classified the failure as `route_access`. Safe configuration inspection
identified the cause: the staging helper had written a 30-second route bootstrap timeout while
the runtime admission bound is 15 seconds. The helper was corrected to the existing 15-second
bound.

After the correction, the graph was entered, restarted before publish, published, restarted after
publish, and served both loopback listeners successfully. The route explanation contained exactly
one selected candidate.

## Real CPAR HTTP result

```text
models_preflight=PASS
attempted_calls=1
successful_calls=0
protocol_counts.responses=0
protocol_counts.chat=0
protocol_counts.messages=0
failure_categories=[http_5xx]
upstream_request=sent_via_cpar
stopped_on_first_failure=true
```

The CPAR event log classified the failed attempt as `EgressRejected/egress`. The request reached
the native Web adapter and was sent through CPAR's Web/Statsig path; the failure occurred at the
provider egress boundary before a canonical response was produced. This does not prove the Web
session is valid, and it does not prove any of the three protocol projections.

## Local gate

- `cargo fmt --all -- --check`: PASS
- `cargo test --locked -p gateway`: PASS (78 unit tests, 1 component smoke test)
- `cargo clippy --locked -p gateway -- -D warnings`: PASS
- `git diff --check`: PASS

The new deployment diagnostic is value-free and reports only a closed composition stage. It does
not log target text, credentials, models, or upstream response data.

## Verdict and next action

P12-10I-14 remains `BLOCKED_WITH_EVIDENCE`. The CPAR graph and native Web runtime now compose, and
the real CPAR entrypoint was exercised, but the Web provider boundary returned an egress-scoped
5xx on the first Responses request. Do not repeat the same account blindly. The next action is a
separate provider-session/Statsig classification or a newly authorized valid Web session, followed
by a fresh isolated E2E; no production rollout is implied.
