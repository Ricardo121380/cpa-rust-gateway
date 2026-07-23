# BC-MGMT-003 Management HTTP admission

| Field | Value |
|---|---|
| Contract | `BC-MGMT-003` |
| Task | `P10-02` |
| Status | Accepted |
| Domain | Management HTTP shell security |

## Entry and preconditions

`configure_management` is the only P10-02 registration point for future `/admin/` handlers. The
Actix application must provide `ManagementHttpState`; otherwise every protected request fails
closed. P10-02 supplies no CRUD, UI, OAuth, backup, catalog, Provider or database handler.

The state contains one independently configured `mgmt_` Management Key, a peer policy and a
browser policy. It has no Client Key authenticator, Provider credential, database connection,
proxy configuration or ambient environment lookup.

## Admission sequence and invariants

1. Read `HttpRequest::peer_addr()` only. Missing address rejects. `LoopbackOnly` accepts only
   loopback. `LoopbackOrPrivate` additionally accepts RFC1918 IPv4 and `fc00::/7` IPv6 ULA. It
   does not read `Forwarded`, `X-Forwarded-For`, `X-Real-IP`, link-local or carrier-grade NAT as
   authority.
2. Accept exactly one `X-Management-Key`; require valid Header bytes and a constant-time match to
   the configured `mgmt_` secret. `Authorization`, `X-Api-Key`, an absent header, duplicate
   header and wrong key all reject. The request cannot fall back to a data-plane Client Key.
3. No `Origin` header is accepted by default. Under an explicit exact HTTP(S) same-origin policy,
   a matching canonical Origin may read; an unsafe method must also contain exactly one
   constant-time matching `X-Management-CSRF-Token`. All `OPTIONS` browser preflights reject.
4. A successful request receives only the non-secret `ManagementRequestPrincipal` containing the
   fixed durable audit actor `management-key`. Later resource mutations must use that actor for
   their transactional management audit event.
5. Every rejection produces the same `404`, `Cache-Control: no-store`, bounded JSON error. It
   has no `WWW-Authenticate` or `Access-Control-Allow-Origin` header and no value-dependent
   status/message/body field. Successful requests also receive no CORS grant.

## Error and isolation semantics

No rejected request reaches a registered management handler. Authentication and browser rejection
have no SQLite, Snapshot, Client Key, Provider, OAuth, audit-row or external-network side effect.
The public `/v1/*` routes do not use this state or header. Key/CSRF values have no output accessor
and their Debug forms are redacted.

## Corresponding tests

`p10_02_management_security` issues in-process requests against synthetic guarded routes. It
proves exact header namespace/duplicate rejection, source-address and forwarding-header behavior,
default and opt-in browser policy, CSRF, missing state, audit identity and secret redaction. It
performs no listener bind, filesystem write, database mutation, real credential operation or
Provider request.
