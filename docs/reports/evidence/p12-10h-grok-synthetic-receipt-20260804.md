# P12-10H Grok synthetic smoke and decommission receipt

Status: `SYNTHETIC_PASS_REAL_TRAFFIC_PENDING`

Date: 2026-08-04

## Scope

The operator authorized stopping grok2api because there is currently no Grok business traffic.
This receipt records the stop and a CPAR-only synthetic smoke check. It does not claim a real Grok
upstream success, does not satisfy the 72-hour/1250-success observation gate, and does not retire
the old CPA rollback package.

## Decommission state

- The server `grok2api` Docker container was stopped. No grok2api request or source-pool mutation
  occurred afterward.
- `cpa-rust-gateway.service` remained active.
- CPAR loopback `/healthz` returned a success status class.
- An unauthenticated CPAR `/v1/models` request remained rejected with an authentication status
  class.
- The CPAR management listener remained loopback-only.
- No public listener, Caddy route, DNS record, CC Switch configuration or old CPA state changed.

## Synthetic harness

The reviewed [synthetic Grok harness](../../../scripts/run-p12-10h-grok-synthetic.sh) ran locally with
fixed fixtures and no credential input. These five native CPAR targets passed:

- native account pool;
- native account scheduling;
- native account workers;
- Console/Web runtime;
- grok2api memory migration contract.

The harness ended with `upstream_request=not_sent`. It validates CPAR code, state isolation and
contract behavior only; it cannot prove that a live Grok account currently accepts a request.

## Remaining gate

P12-10H remains open until a representative CPAR-only observation is available. The existing
observer still requires the approved duration, minimum successful durable Attempts, zero P0/P1
invariant violations, database integrity and final review. With no current Grok traffic, the
synthetic harness is a maintenance smoke check and not a replacement for that gate.

The stopped grok2api container remains recoverable through the server's existing Docker rollback
procedure. Permanent old CPA retirement and the final P12/G12 closeout have not started.
