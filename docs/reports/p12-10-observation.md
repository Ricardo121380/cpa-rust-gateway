# P12-10 production observation

Status: `INCOMPLETE_OPERATOR_STOP`

Started: `2026-08-03T09:03:57Z`

Stopped: `2026-08-03`; the receipt records `operator_stop`, ten successful synthetic attempts, and
no terminal acceptance.

This report is value-free. It contains no endpoint, credential, model, request/response body,
identifier, token fingerprint, or mutable resource ID.

## Boundary

- The production hostname routes completely to CPAR; no percentage, key or fallback split exists.
- The old CPA container remains running but receives no production traffic. It may be stopped only
  after the 72-hour window, at least 1250 successful requests, and the complete G12 review pass.
- Newapi retains its two existing public aliases and uses the dedicated CPAR client key. CC Switch
  remains outside inspection, modification and migration rehearsal.
- The fixed-revision CPAR service is active, disabled at boot, and exposes data/management listeners
  only on loopback. The management plane has no public route.

## Observer

`p12-10-observation.service` is a transient root-only systemd unit with a 128 MiB memory limit, CPU
quota, `NoNewPrivileges`, private temporary storage, read-only system/home views and a single
receipt-directory write allowance. It executes the reviewed `scripts/p12-10-observe.py` copy.

The observer:

1. admits only an HTTPS production endpoint and reads endpoint, key and model from root-only files;
2. requires the live Caddyfile to remain byte-identical to the reviewed CPAR candidate;
3. sends one minimal Chat request approximately every 180 seconds through the public TLS path;
4. separately records synthetic attempts/successes/failures, estimated real successes, and total
   durable succeeded Attempts since the window baseline;
5. records only client-side TTFB/total-latency percentiles, never response content;
6. polls database `PRAGMA quick_check` and the four P1 queue/durability counters; and
7. writes the receipt atomically with mode `0600`.

The terminal receipt is
`/var/backups/cpa-rust-gateway/p12-10-20260803T090357Z/observation.json`. The first formal sample was
successful, database integrity was `ok`, and all four P1 deltas were zero. An earlier sandbox-start
attempt exited before any request because the gateway service's private credential mount is not
visible to another unit; the formal unit uses the reviewed source credential path and starts a new
window from zero.

The observer was later stopped because the production graph had only one upstream and the window
contained no representative client traffic. Its service state is terminal/failed by design and the
receipt remains `INCOMPLETE`; elapsed wall time is not reinterpreted as acceptance. A new observation
may start only after the production channel graph is representative.

## Completion rules for a replacement observation

- `elapsed_seconds >= 259200` and `durable_successes >= 1250`;
- no route drift, database-integrity failure, Required quarantine/write failure, Required queue full,
  sink closure, semantic regression, unexplained route, secret exposure or cross-request leakage;
- synthetic failures remain at or below the approved error boundary and failures count in the
  denominator rather than being discarded;
- final client-side latency evidence is reviewed against the frozen CPA baseline; and
- the final receipt, process state, production matrix, Newapi state and rollback package all receive
  an explicit review before old CPA retirement.

Until every condition passes in a replacement window, P12-10 and G12 remain incomplete and no
Release 1 tag may be created.
