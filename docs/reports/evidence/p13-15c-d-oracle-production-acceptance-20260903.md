# P13-15C/D Oracle production acceptance receipt

## Verdict

- Acceptance date: 2026-09-03 Asia/Shanghai (2026-09-02 UTC on the server)
- Scope: Grok Build durable catalog, exact-Credential materialization and one real non-streaming
  Responses canary
- Result: `PASS_FOR_GROK_BUILD_PENDING_REMAINING_CHANNELS_AND_FORMAL_GATE`
- Implementation commit: `058805e556d5b22a00bd56b846f75ed8b81696fd`
- Release workflow:
  [33651468181](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/33651468181)
- Deployed ARM64 SHA-256:
  `69acbe01c8aa4934867f473e6eeaa063185eeae364865d8730fd88f8ab6344af`

This receipt deliberately contains no token, cookie, refresh grant, principal, account email,
Credential ID, Client Key, request body, model output or raw Provider error.

## Preflight and rollback boundary

- Both release jobs, `aarch64-unknown-linux-gnu` and `x86_64-unknown-linux-gnu`, completed
  successfully for the exact implementation commit.
- The ARM64 artifact signature and OCI metadata were verified using repository tooling before the
  binary was installed.
- The immediately preceding production binary SHA-256 was
  `4691d6b34dd09f8eff101f7e65f3ad5094bc54b8e1ae39faee8b0da6b095b5fc`.
- The final pre-cutover SQLite online backup, previous binary and previous release pointer are under
  `/var/backups/cpa-rust-gateway/p13-15-deploy-final-20260902T163647Z`.
- The backup database returned `PRAGMA quick_check=ok` and zero foreign-key violations.
- The deployment closure was prepared to stop CPAR, restore that exact database and release
  pointer, and restart the previous binary on any failed invariant. It was not triggered.

## Credential readiness

An existing production Build grant could refresh its stored revision but remained unauthorized for
the upstream catalog. The current official local Grok CLI Build grant independently returned HTTP
200 from the exact upstream model-list endpoint. The stale CPAR import batch was therefore rolled
back through the existing root-only administrative command and the current grant was imported via a
memory-only pipeline using its stable principal identity. No plaintext credential file was created.

The atomic import accepted and activated one Credential. The existing entitlement synchronizer then
recorded Grok Build `supergrok` from authoritative Provider subscription evidence. Registration,
account creation and account repair were not performed.

## Cutover invariants

Only `cpa-rust-gateway.service` and its `/opt/cpa-rust-gateway/current` release pointer changed.
After restart:

- the active release was `058805e556d5b22a00bd56b846f75ed8b81696fd`;
- the installed binary matched the approved SHA-256;
- CPAR was active and both loopback and public health checks returned HTTP 200;
- database schema version 21 was active;
- the live database returned `PRAGMA quick_check=ok` and zero foreign-key violations;
- Autoreg and Caddy remained active; and
- Autoreg, Caddy configuration, Cloudflare, DNS, frontend and unrelated services were not changed.

## Catalog and serving evidence

Authenticated loopback and public `/v1/models` both returned HTTP 200 with four exact model IDs,
including `grok-4.6`. The protected catalog status returned four targets:

- one Fresh target with a successful snapshot containing two models; and
- three Missing targets with only their latest closed safe failure class.

SQLite independently contained one successful target, two catalog model rows and exactly one
`grok-4.6` row. This proves that `grok-4.6` entered the authorized runtime through exact Build
Credential discovery and atomic materialization, not through a CPAR alias or frontend constant.

Exactly one approved real non-streaming `POST /v1/responses` canary was sent for `grok-4.6`. It
returned HTTP 200, `status=completed`, the requested model and a non-empty output. No second real
Provider request was sent, and the output content was not retained in this receipt.

## Cleanup and remaining work

The remote upload and isolated preflight directories and the local deployment workspace were
removed. The production backup was retained.

This receipt closes only the Grok Build production portion of P13-15C/D. P13-15 remains
`IN_PROGRESS` because Grok Web, Grok Console, xAI Official, Kiro and generic compatible sources,
the P13-15E isolation matrix, target ChatGPT Go/Codex evidence, Claude Code integration and the
formal Delivery Gate remain incomplete or explicitly deferred.
