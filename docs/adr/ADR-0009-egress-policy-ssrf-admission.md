# ADR-0009: EgressPolicy SSRF admission

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-09`; `L36`, `E10`, `J19`; Behavior 20; [BC-SEC-002](../contracts/BC-SEC-002-egress-policy-ssrf-admission.md) |

## Context

P2-01 intentionally stored an opaque optional `upstreams.egress_policy_id` and accepted arbitrary
Base URL text. That is not safe once a Provider can connect to an operator-configured endpoint:
URL parsing, DNS resolution, redirect handling, and connection timing can otherwise reach loopback,
private, link-local, or cloud-metadata addresses.

The policy must protect local CPA/Kiro-RS/grok2api testing without accidentally turning a broad
private range into a default route. It must also provide a reusable, transport-neutral admission
result for P3's real HTTP connector rather than relying on an HTTP client's implicit redirect or
DNS behavior.

## Decision

- Add version-scoped `egress_policies` persistence. Its stable ID, name, exact Host allowlist,
  allowed schemes, ports, CIDRs, redirect mode, and redirect-hop bound are structural storage
  fields. A same-version `upstreams.egress_policy_id` reference is checked on writes. A referenced
  policy identity cannot be changed; deleting a policy atomically clears its nullable references,
  so typed compilation fails closed if an enabled Upstream is left without a replacement.
- Migration 3 preserves a legacy non-null opaque policy ID as a same-version, explicitly
  unconfigured policy with empty allowlists. It cannot be published until an operator replaces it
  with a valid policy, but upgrading neither loses the identifier nor creates network access.
- `gateway-upstream` owns the storage-neutral `EgressPolicy` and `EgressDnsResolver` boundary.
  Only `http` and `https` schemes are representable. Hosts are exact canonical DNS names or
  explicit IP literals; suffix/wildcard matching, user-info, opaque URLs, and implicit proxy
  behavior are deliberately excluded.
- A target is admitted only when its scheme, Host, effective port, and every resolved address pass.
  If the CIDR list is nonempty, every resolved address must be within it. Without a CIDR list,
  public addresses remain eligible but loopback, unspecified, private/shared, link-local,
  multicast, documentation, and metadata ranges fail closed.
- A loopback, RFC1918, or IPv6 ULA address can be used only when it matches an explicit CIDR
  Allowlist entry contained within that private/local range. Broad entries such as `0.0.0.0/0`
  cannot override the default private-address protections. Metadata, shared, link-local,
  multicast, documentation, reserved, NAT64/6to4-related, and other special ranges remain
  non-overrideable even when a broad CIDR is present.
- DNS is resolved once for each admission, all returned addresses are checked, and the admitted
  result retains the approved addresses. A future connector must dial one retained address rather
  than re-resolving the hostname. Each new request and redirect re-runs admission, so a later DNS
  rebinding response is rejected before it can be dialed.
- Redirects are disabled by default. Explicit same-origin and fully revalidated modes have a
  positive, bounded hop limit; each location is resolved relative to the current URL and passes
  the complete URL/DNS/CIDR admission process before use.

## Consequences

An enabled upstream will require a compiled EgressPolicy before `SnapshotPublicationService` can
publish its Config Version or a later Provider connector can use it. The publication-time compiler
checks only structural URL policy; it intentionally does not resolve DNS. Policy construction and
DNS admission produce safe typed errors that map to
`EgressRejected/Egress` or `EgressUnavailable/Egress`, never to a credential-forbidden result.
The policy contains no Credential, Client Key, HTTP-client instance, proxy implementation, or
database handle.

P2-09 provides policy, persistence, publication-time static admission, and deterministic resolver
tests only. The P2-07 `RouteSnapshot` does not yet carry Endpoint connection details, so P3 must
add a secret-free runtime target view that carries the compiled policy without querying the
Repository on a request. P3 owns the actual HTTP client, connection pinning implementation, proxy
selection, timeout configuration, and redirect response loop; that implementation must consume
this contract rather than bypass it.

## Alternatives considered

- Trusting the HTTP client's hostname resolution and redirect defaults was rejected because it
  permits DNS rebinding and redirect SSRF after management-time validation.
- Allowing a host wildcard or a global private-network switch was rejected because they make local
  testing convenience silently broaden production egress.
- Resolving only during configuration publication was rejected because DNS answers can change after
  publication; admission must occur at every connection attempt.
- Putting URL/DNS code in `gateway-store`, HTTP, or a Provider crate was rejected because the same
  policy must be reusable by inference and model discovery without persistence or transport coupling.

## Validation and rollback

Tests cover HTTPS-only default policy, exact Host/port/CIDR rejection, IPv4/IPv6 loopback and
special-range blocking, cloud-metadata blocking despite a ULA CIDR, explicit local CIDR allowance,
mixed DNS answer rejection, per-attempt rebinding, relative redirects, same-origin rejection,
revalidated redirects, publication rejection, and persistence reference integrity. Rolling back
P2-09 removes the policy table and runtime module; it does not create any network connection or
modify encrypted Secret/Client Key data.
