# P12-10I-11 Autoreg SSO Build exact-artifact receipt

Status: `DONE`

## Boundary

This continuation verifies only the P12-10I-10 batch-scoped native Build probe correction. It uses
one Autoreg SSO source, one SSO-to-Build conversion, one isolated CPAR import, and one native Build
probe. No production graph, public route, listener, Caddy/DNS, CC Switch, old CPA, grok2api, or
production account row was changed.

No SSO, OAuth, cookie, endpoint, model, request/response body, account identity, or token
fingerprint is recorded here.

## Artifact and delivery gate

| Check | Result |
|---|---|
| Exact revision | `4d3bee8b63544f3c5442cc7383bc453777097547` |
| Release workflow | GitHub Actions run `31018616575`; x86_64 and ARM64 jobs both succeeded |
| ARM64 private artifact | ID `8935861737`; exact revision/target name |
| Repository verification | Manifest, receipt, target, version, revision, SBOM, and OCI metadata passed |
| Independent signing verification | GitHub OIDC/Sigstore identity verification returned `Verified OK` |
| Executable shape | ARM64 Linux ELF |
| Delivery gate | GitHub CI run `31018599798`: Fast, Full supply-chain, and Required gates passed |

The artifact binary was copied only to a named temporary path on the Oracle Singapore host. It
did not replace `/opt/cpa-rust-gateway/current` and was never started as a service.

## Isolated preflight

- The production host reported ARM64, `cpa-rust-gateway.service=active`, and disabled-at-boot.
- The production SQLite database returned `quick_check=ok`, contained one Build account, and kept
  active Config Version `p12-09-codex-production-v2`.
- A root-only temporary directory received an online SQLite backup, a copy of the master key, and
  the verified candidate binary. The staging backup also returned `quick_check=ok` and initially
  contained the same one Build account.

## SSO conversion and import

The first pipe invocation stopped before import because macOS `date` produced an invalid
millisecond argument for the Linux CLI. CPAR rejected `--observed-at-ms`; no account was written and
no upstream request was sent. Its transient conversion output was discarded with the failed pipe.

The corrected invocation used an actual Unix-millisecond value. The source SSO then flowed directly
from Jakarta Autoreg through its existing SSO-to-Build converter and two SSH pipes into the isolated
CPAR importer. No credential file was created. The import returned:

```text
native_grok_import=PASS source_records=1 accepted_accounts=1 rejected_records=0 accepted_links=0 created_accounts=1 unchanged_accounts=0
```

The isolated pool then contained two Build accounts, while exactly one active/enabled account
belonged to the probe batch. This is the multi-account condition that the old whole-pool
cardinality check rejected before upstream execution.

## Native Build probe

The corrected artifact leased only the selected batch account and completed one real upstream
request:

```text
native_grok_probe=PASS provider=grok_build account_attributed=true health=available quota=available canonical_complete=true chat=true responses=true messages=true
```

This proves the Autoreg SSO can produce a Build credential that CPAR imports, selects from a
multi-account pool, executes, and projects into Chat, Responses, and Messages. It does not create a
public Grok route or prove Console/Web availability.

## Rollback and cleanup

- `grok-rollback` removed the one imported account. The isolated copy returned to one Build
  account and `quick_check=ok`.
- The production database independently remained `quick_check=ok` with one Build account; its
  active Config Version, service state, and current release pointer were unchanged.
- The remote temporary database, copied master key, candidate binary, and staging directory were
  removed. The locally downloaded private artifact was moved to a recoverable Trash location.

## Verdict

P12-10I-11 is `DONE`. The batch-scoped probe fix is verified by a signed exact-revision artifact
and one real SSO-derived Build request in the required two-account staging pool. P12-10I-10 remains
`BLOCKED_WITH_EVIDENCE` only for its independent Console egress and Web lifetime-policy boundaries.
