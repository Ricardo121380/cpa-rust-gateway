# P12-10D native Grok workers

Status: `DONE`

## Outcome

CPAR now owns a bounded, durable control-plane worker boundary for native Grok credential refresh
and quota synchronization. This slice adds no provider network client and does not read or migrate
live grok2api accounts. Provider-specific HTTP/OAuth operations remain injected executors; durable
claiming, credential custody, CAS commit, scheduling and failure state belong to CPAR.

## Implemented boundary

- schema v11 persists refresh/quota due times, last-success times, failure counters and one shared
  per-account worker claim; expired claims can be reclaimed after a process restart;
- refresh and quota work share the claim, so two worker kinds cannot concurrently mutate one
  account;
- the coordinator claims at most the configured concurrency, runs only that bounded batch and
  leaves a panicked operation claimed until lease expiry;
- refresh success seals the complete replacement credential, advances the account revision with a
  compare-and-swap, resets failure state and schedules the next refresh with deterministic
  five-to-eight-minute pre-expiry jitter;
- stale refresh results cannot overwrite a newer revision;
- transient failures persist deterministic exponential backoff with bounded jitter; a definitive
  reauthentication result persists `reauth_required` and removes the due time;
- quota success replaces all account/model windows in one transaction, retains source, confidence
  and observation time, and restores those exact targets into the existing runtime quota registry
  when a routing snapshot is compiled after restart;
- all credential values and claim identifiers remain absent from Debug and durable evidence.

## Acceptance evidence

The dedicated P12-10D suite covers deterministic jitter, cross-kind singleflight, lease-expiry
recovery after reopen, refresh success, stale-revision rejection, durable backoff/reauth, atomic
account/model quota restoration and bounded parallel execution. The gateway-store migration and
rollback suite, all provider-grok tests and strict Clippy also pass.

This is synthetic/local evidence only. It changes no production graph, service, account pool,
grok2api process or external credential.

## Next boundary

P12-10E implements the native Grok Console runtime and completes the Web production binding using
strict request/response/SSE/tool/usage/error fixtures and the existing Chat, Responses and Messages
canonical bridges. Live accounts and production traffic remain outside that slice until the later
controlled migration and parity gates.
