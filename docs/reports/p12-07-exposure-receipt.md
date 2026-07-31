# P12-07 pre-exposure verification receipt

| Field | Value |
|---|---|
| Plan version | `v1.74` |
| Task | `P12-07` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Date | `2026-07-31` |
| Artifact revision | `428be9396881ebf7468f946643b09fa1a54c7ca6` |
| Artifact provenance | Private GitHub Actions run `30533211028`, aarch64 job; keyless GitHub OIDC Sigstore signature and Rekor index `2289216552` independently re-verified with `cosign verify-blob` before any server write |
| Binary SHA-256 | `e8d7b534e44a794615b0dd4946d1af795a6c2b66f54e3f59338466c41b8b3f62` |
| Test domain | `cpar.example.invalid` → `203.0.113.10`, grey-cloud (DNS only) |
| Scope | First public route to the new gateway, data plane only. No upstream credential, Provider, Route, Client Key or inference configuration was entered; no incumbent configuration or credential was changed. |

## Why this was a full deployment, not a service start

The earlier P12-04 Staging acceptance ran on the **previous Jakarta VPS**, against an x86_64
artifact at revision `111f60a4`. On this host, `cpa-rust-gateway.service`, the `cpa-gateway`
account, `/opt/cpa-rust-gateway`, `/var/lib/cpa-rust-gateway` and
`/etc/cpa-rust-gateway/credentials` did not exist. The whole chain was therefore performed fresh.

## Installation

| Step | Evidence |
|---|---|
| Provenance | Repository verifier passed with `--require-signature --require-receipt`; `cosign verify-blob` returned `Verified OK` against the pinned `release-artifact.yml@refs/heads/codex/p12-deployment` identity and the GitHub Actions OIDC issuer |
| Transfer integrity | `sha256sum` on the server matched the signed manifest record before installation, and again after installation at `/opt/cpa-rust-gateway/current/gateway` |
| Native execution | The aarch64 binary printed its real `serve`/`admin` usage on this host, confirming the dual-target work solved the original blocker |
| Service account | `cpa-gateway` created as a system account, uid 995, no home, `nologin` shell |
| Directories | `/opt/cpa-rust-gateway` 0755 root, `/etc/cpa-rust-gateway/credentials` 0700 root, `/var/lib/cpa-rust-gateway` 0700 `cpa-gateway` |
| Credentials | Five files generated on the server with `openssl rand`; values never left the host. Shapes verified against the runbook contract without reading contents: `master-key`, `backup-key`, `client-key-pepper` at exactly 32 raw bytes; `management-key` and `management-csrf` at 69 ASCII bytes with the required `mgmt_` and `csrf_` prefixes; all root:root 0600 |
| Unit | Installed byte-identical to `deploy/systemd/cpa-rust-gateway.service`; `systemd-analyze verify` accepted it; service is `active` and `disabled` at boot |
| Resource envelope | Observed `MemoryMax=805306368` (768 MiB), `TasksMax=512`; the running process held `VmRSS 10668 kB` across 5 threads |

## Isolation before exposure

| Check | Result |
|---|---|
| Listener binding | Only `127.0.0.1:18180` and `127.0.0.1:18181`; no wildcard bind |
| Data-plane port from the public address | TCP connect to `203.0.113.10:18180` refused |
| Management port from the public address | TCP connect to `203.0.113.10:18181` refused |
| Data plane without a key | `POST /v1/responses` → `401` |
| Management plane on loopback without a key | `404` from `management_denied_response`, which hides route existence rather than returning `401` — a stronger posture for a management surface |
| Incumbent | `cli-proxy-api` remained `active` throughout |

## Caddy change and exposure verification

The live Caddyfile was backed up first to `/root/Caddyfile.preimage-p12-07`
(SHA-256 `8429a93422a856de2ef5e1ffff3922cd7af34beebabb6532d6122dcffa06ea62`, 0600) as the rollback
preimage. The candidate was validated **before** installation: `caddy validate` reported
`Valid configuration`, and `caddy adapt` confirmed the compiled result added only `cpar` → `18180`
while `cpa`, `cpam`, `grok`, `kiro` and `sub` kept their upstreams and the server kept
`timeouts: NONE`. Only then was it installed and reloaded.

After the reload, `cpa` returned `200` and `cpam` returned `307`, so the incumbent path was
unaffected.

`scripts/p12-07-verify-exposure.sh` then passed all seven applicable assertions:

| Assertion | Result |
|---|---|
| `dns_resolves_to_host` | PASS — both authoritative nameservers return the expected address |
| `dns_not_proxied` | PASS — record points at the origin, so no edge idle cutoff sits in front of SSE |
| `tls_certificate_matches` | PASS — Let's Encrypt certificate `CN=cpar.example.invalid`, valid `2026-07-31` to `2026-10-29`, HTTP/2, `ssl_verify_result=0` |
| `data_plane_reachable` | PASS — health endpoint returned the exact expected body |
| `unauthenticated_rejected` | PASS — `401` |
| `invalid_key_rejected` | PASS — `401`, so the previous check is not merely a missing-header path |
| `management_plane_unexposed` | PASS — `/admin/config-versions`, `/admin-ui/`, `/admin/observability/metrics` and `/admin/upstreams` all `404` through the public hostname |

A streaming request without a key was rejected in 0.46s rather than hanging, confirming the proxy
does not buffer the rejection.

## What this receipt does not claim

- `valid_key_accepted` was **SKIPPED**: no upstream credential, Provider, Route or Client Key was
  entered, so the data plane fail-closes on every inference request. That assertion, and any
  latency or semantic evidence, requires the configuration-graph step which was deliberately
  deferred.
- **No rate limiting exists on this domain.** `caddy list-modules` on this host confirms the
  standard Caddy build ships no `rate_limit` module, so the rate limiting `CR-P12-ROLLOUT-001`
  lists for the test domain is not enforced. Compensating controls: client-key enforcement, the
  4 MiB inbound body cap, the 30s body-read bound, the total binding concurrency cap of 16, and the
  fact that this hostname is unpublished and is removed when P12-07 closes. Adding a limiter would
  require a custom Caddy build, which would change the incumbent's TLS terminator.
- The service is `disabled` at boot by design; it will not survive a reboot without an explicit
  `systemctl enable`.

## Rollback

Restore `/root/Caddyfile.preimage-p12-07` over `/etc/caddy/Caddyfile` and reload Caddy to remove the
public route; `systemctl stop cpa-rust-gateway` to stop the service. Neither step touches the
incumbent, and the service is already disabled at boot.
