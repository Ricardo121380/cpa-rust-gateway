# P7-09 Kiro-RS clean-room differential and `--bare` E2E report

## Static differential

The clean-room differential corpus is the P7-02/04/05/06/07/08 public-boundary suite: exact
IDE/CLI endpoint separation, model union without synthetic `-thinking` aliases, per-Credential
snapshot isolation, strict EventStream CRC and chunk invariance, Claude Code Tool/Plan/Thinking
semantics, and non-destructive failure ownership. The focused P7-07 fixture executes the frozen
Kiro-RS behaviours: `toolUseEvent`, `reasoningContentEvent`, historical Tool pairs,
`AskUserQuestion.questions[].question`, and no-argument Plan Tools. It is deliberately
clean-room: no source, credentials, request body, response body, or private mapping was copied.

## Native gateway vertical slice

The native `KiroInferenceAdapter` now composes the already-approved P7 request, profile,
EventStream, semantic, failure-classification, and DNS-pinned egress boundaries without a Kiro-RS
runtime dependency. It accepts an explicitly selected Credential, endpoint policy, conversation,
profile resolution, model, and transport; it does not discover a local cache, proxy, Kiro-RS
configuration, or server account, and it has no implicit refresh/retry/failover behavior.

The local fixture suite proves all of the following without sending a network request:

- IDE request -> native EventStream -> Canonical `ResponseStart`, text/reasoning/Plan Tool, and
  clean `ResponseEnd` across arbitrary byte chunks.
- CLI API-key request keeps the CLI target, marker, and Thinking placement; an unknown 403 remains
  `EgressRejected` rather than mutating account state.
- An injected, value-free independent quota signal is retained through the transport handoff and
  maps to only the Credential quota owner; generic transports retain `None` rather than parsing
  or storing an error body.
- A malformed body after `ResponseStart` emits one terminal `StreamError`, while a profile
  resolution from another Credential family is rejected before sending.

This closes the previous structural gap between the pure P7 converters and the shared
`InferenceAdapter`/Router execution boundary. It is local evidence only: it does not demonstrate
that the upstream Kiro account currently accepts a real native request.

## Server reference observation

Read-only server inspection on 2026-07-23 found the existing Kiro-RS service running as its
dedicated account with `toolCompatibilityMode=claude-code`, Thinking extraction enabled, IDE as
the default endpoint, and Kiro-RS `2.6.0`. Its protected local `/v1/models` operation returned
`200` with 15 advertised models. This proves the Kiro-RS client-facing API key and model catalog
surface were reachable; it does not prove a Kiro inference account is currently usable.

## Bounded `--bare` result

The installed local Claude Code `2.1.214` was pointed through a temporary SSH loopback forward to
the existing Kiro-RS service, with the service's client key passed only in process memory. The
fixed `--bare --print --no-session-persistence --output-format=json` request did **not** return a
successful JSON/marker result. An initial runner-status mistake started a second identical local
client before the first process was observed; both were stopped and no further request was sent.
This is recorded as an execution deviation, not treated as a passing or retryable probe.

The contemporaneous, read-only Kiro-RS trace projection reported `final_status=error` and
`error_type=auth_failed` before any semantic response, with the service's configured account
failover attempts. No raw trace error, URL, Header, token, credential, prompt, or response was
read or recorded. The public client API had already accepted `/v1/models`, so this is evidence of
the server Kiro upstream credential state, not a claim that the new native converter succeeded or
failed against Kiro.

**Status: `BLOCKED_AUTH_REAUTH_REQUIRED`.** P7-09 and G7 cannot pass until the service's Kiro
account is interactively reauthenticated or replaced by the user. Do not retry the same live
request before that change. After the user completes it, one new explicitly recorded short
`--bare` tuple may verify normal response semantics, then G7 may run. The original P8 ordering
sentence is superseded by `CR-P7-DEFER-002`: P8-G12 may progress on their non-Kiro dependencies,
but Kiro remains blocked and must be completed before any Kiro-inclusive release.

## Local verification and review

| Check | Result |
|---|---|
| P7-01 through P7-09 local `provider-kiro` tests | PASS; 42 tests, including native IDE/CLI adapter, EventStream, Tool/Thinking, catalog, and failure-owner regressions. |
| `cargo clippy --offline -p provider-kiro --all-targets -- -D warnings` | PASS after the native adapter addition. |
| `./scripts/check.sh full` | PASS after the native adapter addition: plan-state, format, workspace Clippy/tests, source/crate boundaries, document links, Secret scan, dependency policy, and RustSec audit. |
| Kiro-RS `/v1/models` safe projection | PASS; `200`, 15 models. |
| Kiro-RS `--bare` inference tuple | BLOCKED; upstream `auth_failed`, no Canonical/Claude success evidence. |

Review conclusion: the native path now has local executable evidence, while the remaining failure
is contained to the external Kiro-RS account state. No server configuration, credential, route,
proxy, TUN, scheduler, or repository-secret mutation has occurred.
