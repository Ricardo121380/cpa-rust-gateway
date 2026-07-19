# BC-SEC-002 EgressPolicy SSRF admission

| Field | Value |
|---|---|
| Contract | `BC-SEC-002` |
| Task | `P2-09` |
| Status | DONE |
| Domain | Outbound URL, DNS, CIDR, and redirect admission |

## Boundary

`gateway-upstream::EgressPolicy` is a transport-neutral, immutable allowlist. It receives a URL
and an injected `EgressDnsResolver` and returns an `AdmittedEgressTarget` containing a canonical
scheme, Host, effective port, and the exact approved DNS addresses. It does not connect a socket,
follow HTTP redirects, read `SQLite`, decrypt a Credential, or retain a Provider/HTTP type.

```text
configured URL or redirect Location
  -> canonical URL shape and exact Host rule
  -> scheme and effective-port rule
  -> resolve DNS once for this admission
  -> validate every returned IP against CIDR/default-deny rules
  -> AdmittedEgressTarget { approved addresses pinned for this attempt }
  |-> GatewayError(EgressRejected, Egress)
  |-> GatewayError(EgressUnavailable, Egress)
```

## URL and address admission

- Only `http` and `https` can be configured. A policy must explicitly list every permitted scheme,
  Host, and effective port; an absent or unsupported scheme, missing Host, user-info, opaque URL,
  or nonmatching Host/port is rejected.
- DNS-name rules are exact canonical names; there is no suffix, glob, implicit proxy, or arbitrary
  redirect exception. IP-literal Hosts require an explicit matching IP Host rule.
- A domain is resolved once for every admission. Empty DNS output, resolver failure, or any denied
  answer rejects the whole target; selecting only the first safe answer is forbidden.
- A nonempty allowed-CIDR list restricts every answer to that list. With an empty CIDR list, public
  addresses remain eligible but loopback, unspecified, private/shared, link-local, multicast,
  documentation, and cloud-metadata ranges are denied.
- Only loopback, RFC1918, and IPv6 ULA addresses may use a matching explicit CIDR exception that
  is wholly contained by that private/local range. This permits deliberate `127.0.0.1/32` or
  `10.0.0.0/8` testing rules but rejects a broad bypass such as `0.0.0.0/0`. Metadata, shared,
  link-local, multicast, documentation, reserved, NAT64/6to4-related, and other special ranges
  stay denied even when a configured CIDR contains them.
- The caller must connect only to the returned addresses and must not perform a second hostname
  resolution before dialing. A new request always obtains a new admission result.

## Redirect admission

- The default redirect mode is `deny`.
- `same_origin` permits a bounded redirect only when canonical scheme, Host, and effective port are
  unchanged; the redirected URL is nevertheless fully re-admitted and re-resolved.
- `revalidate` permits a bounded redirect to another configured Host only after full URL, DNS, and
  CIDR admission. Relative `Location` values are resolved against the current admitted URL first.
- A caller increments the redirect count before each follow. The policy rejects a count at or above
  its configured positive maximum and never delegates automatic redirect following to an HTTP
  client.

## Persistence and error semantics

- Every stored EgressPolicy belongs to one Config Version. A non-null Upstream policy reference
  must resolve within that same Version. A referenced policy identity cannot be renamed; deleting
  a policy atomically clears its nullable references, and an enabled Upstream without a replacement
  fails closed at publication. Malformed JSON lists or semantic policy values likewise fail closed
  before an enabled upstream can be published or used.
- Migration 3 converts a legacy non-null opaque reference into an explicitly unconfigured policy
  with empty allowlists in the same Version. The original ID remains traceable, but compilation
  rejects it until an operator supplies a valid policy; no legacy row gains egress access.
- Policy mismatches map to `EgressRejected/Egress`. Resolver inability maps to
  `EgressUnavailable/Egress`. Neither reveals URL text, Host, IP, CIDR, Credential, proxy, or
  DNS diagnostics in a client-facing error.
- An egress-policy rejection never marks a Client Key or Credential as forbidden.

## Deferred behavior

P2-09 does not implement a network client, socket dialing, TLS verification, proxy protocols,
connection pools, response body/header limits, provider retries, health/circuit state, management
HTTP/CLI endpoints, or real-provider traffic. `SnapshotPublicationService` invokes the static
compiler before it activates a Version, but P3 must carry the compiled policy into its secret-free
runtime connection view and perform DNS admission for each dial. P2-10, P3, P4, and later Provider
phases own those operations.

## Corresponding tests

- Scheme, canonical Host, port, exact IP Host, and explicit CIDR policy tests cover admission and
  safe rejection.
- SSRF regressions cover IPv4/IPv6 loopback, private/shared, link-local/cloud metadata,
  multicast, documentation, reserved, NAT64/6to4-related, and mixed DNS answers; an explicit
  private/local CIDR is the only exception, and it cannot admit a metadata target.
- A rotating resolver proves every request re-admits DNS and returns pinned approved addresses;
  rebinding to a denied address is rejected.
- Redirect tests cover deny-by-default, relative Location handling, same-origin enforcement,
  revalidation, DNS re-checking, and hop bounds.
- Repository and publication tests prove same-version references, referenced-policy identity
  immutability, deletion without a dangling reference, and whole-Version rejection before an
  invalid policy could replace the active Snapshot.
