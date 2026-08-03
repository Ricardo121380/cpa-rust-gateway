# P12-10C native Grok scheduling composition

Status: `DONE`

## Outcome

CPAR native Grok Build, Web and Console accounts now compile into the existing
`EndpointCredentialPools` format. `RouteCredentialScheduler` therefore applies the same bounded
lease, `RuntimeHealthRegistry`, `RuntimeQuotaRegistry`, model-scoped exclusion and controlled
recovery behavior already used by every other channel. No second scheduler or provider-local hot
path was introduced.

This is a local implementation slice. It did not read or migrate live grok2api accounts, publish a
configuration version, replace a production binary, restart a service or send an upstream request.

## Composition contract

- Each provider namespace is explicitly bound to one compiler-approved Endpoint. Duplicate
  providers or Endpoint bindings fail before credential decryption.
- Enabled `active` and `reauth_required` accounts enter the immutable pool; disabled accounts and
  disabled auth state do not.
- The random native account ID becomes the standard `CredentialId`; the provider determines one
  fixed credential kind. Secrets are authenticated and decrypted under the same store lock used to
  read the account snapshot, then copied only into the existing zeroizing `CredentialSecret`.
- Native priority is higher-is-preferred. It maps monotonically to the existing pool's
  lower-is-preferred domain as `1000 - native_priority`. Weight and maximum concurrency retain
  their exact bounded values; existing pool limits continue to fail closed.
- A returned bootstrap seeds `reauth_required` as exact Endpoint/Credential unauthorized state and
  seeds unexpired persistent cooldowns into the shared Health registry. Reauthentication dominates
  cooldown. Expired cooldowns are ignored safely.
- Health and Quota are not copied or mirrored. The normal route scheduler directly checks their
  exact Endpoint/Credential/model targets before acquiring a lease.

P12-10B import was tightened so source migrations can preserve `active`, `reauth_required` or
`disabled` status and account cooldown instead of forcing every imported account active.

## Verification

Six P12-10C tests cover:

- exact 3:1 smooth-weight distribution through `EndpointCredentialPool`;
- higher native priority selection, concurrency saturation and lower-priority fallback;
- disabled exclusion plus persisted reauth/cooldown bootstrap after database reopen;
- model-scoped Health isolation preserving a healthy sibling account;
- model-scoped Quota isolation and explicit recovery-ticket completion before preferred-account
  scheduling resumes;
- duplicate Provider/Endpoint bindings and an oversized existing weighted schedule failing before
  any runtime pool publication.

The P12-10B encrypted import suite remains green with the expanded auth/cooldown import contract.
Package tests, strict Clippy, formatting, documentation links, plan-state checks, tracked secret
scan and Git whitespace all pass.

## Review

Review found one concurrency hazard in an intermediate implementation: the database lock was
released after copying encrypted rows but before decrypting and building pool inputs, allowing a
rollback to interleave with compilation. The final implementation retains the store lock through
authenticated decryption and input construction, then publishes the complete immutable pool only
after the snapshot is closed.

P12-10D is next. It will persist credential/quota refresh outcomes and run bounded proactive
workers; until then, a recovered `reauth_required` account is intentionally blocked again after a
process restart unless its durable auth state is updated by that later owner.
