# CPAR #59 final independent review

Baseline: `3369c730e18f095bb4d9e1b506061e985a475fb9`.
Candidate: `58bbec33b59c682167490c8d11e09b11dbb8c71a`.
Tree: `be4c74357114872fa86fb64d1f8a73757dc7de13`.
Diff: `git diff 3369c730e18f095bb4d9e1b506061e985a475fb9...58bbec33b59c682167490c8d11e09b11dbb8c71a`.
Commit: `58bbec33 CPAR-59: complete encrypted diagnostics for rejected Build usage (#59)`.
The two agents independently inspected immutable sources and existing evidence, read-only.
HEAD remained the starting baseline during review. These are review objects, not branch movement.

## Standards

**PASS.** No remaining hard standard violations. **0 hard violations, 1 nonblocking smell,
1 nonblocking documentation ambiguity; worst P3.**

The initial P2 is fixed: native_usage::read_event loads every indexed event column and calls
existing event_store::decode_stored_event, whose checks cover type/id/correlation/time and
canonical payload, as BC-OBS-002 requires. The regression damages correlation and time for
Request/Attempt/Usage, requires both ordinary/private readers to reject, then checks restored
success. Genuine RED is retained; storage-index-green.log records 8 passed / 0 failed.

P3, possible Duplicated Code, a judgement call: gateway-observability's ordinary confirmation
and private confirmation repeat oneshot/enqueue/2s timeout/failure-counter handling. Admission
semantics differ; extraction is optional and does not block this PASS. Main agent retained the
short explicit methods to keep #59 changes focused.

Documentation ambiguity: the new Usage contract and preexisting Attempt execution contract
both used BC-OBS-005. Exact links distinguished them, but a bare ID was ambiguous. No specified
standard demanded unique IDs, so this was not reported as a hard breach. **Resolved after review**:
the new contract is BC-OBS-006, with explicit links updated; existing BC-OBS-005 is untouched.

Private companion is separate from events/telemetry/quarantine; SQL/AEAD stay in the blocking
writer; failures do not claim retention; owner/AAD/TTL/read-only CLI/GC/recovery have evidence.
This PASS is a bounded static/evidence review, not a fresh test run or real Provider qualification.

## Spec

**PASS. Seven of seven ACs implemented and evidenced. Missing/partial 0, scope creep 0,
incorrect implementation 0; worst within Spec: none.**

AC1/AC7: actual compiler/factory and public Responses JSON/SSE, independent TCP receives each1,
original projection rejection, zero EOF reads, faults with exactly one legal SSE failure.
AC2: original strict-decoded Usage slice retains numeric lexemes and structure, exact
Request/Attempt/Client Key/source/completeness validated by public readback.
AC3/AC4: source and sensitive companion in one transaction; existing AEAD and keys; authenticated
owner/AAD/source lineage; index fix reuses ordinary decoder; private exclusive0600 CLI output.
AC5: one Usage/source/ledger with verified counts; native prices do not enter settlement.
AC6: ADR, format,30-dayTTL, GC, legacy UNKNOWN, rotation, exclusive expiry, reopen, nonempty
encrypted backup/recovery and actual CLI subprocess tests.

Candidate hashes21/21 and archived log hashes35/35 matched. Original public RED, index RED and
initial Full schema FAIL remain retained. Storage8 PASS, final Full PASS. No test rerun or state
mutation by the reviewer. Scope is #59 failed diagnostics, not live Provider/three clients/release.

Standards: 0 hard + 2 nonblocking findings (worst P3; document ambiguity resolved).
Spec: 0 findings (worst none).

## Post-review delta and finalization

Only review/delivery receipts, status prose and the new contract's identifier/links changed after
this candidate. All21 product files remain byte-for-byte those tested by full-after-review.log
and reviewed above. Document links, contract references, staged secrets and whitespace are checked
before the single branch commit. The local commit SHA and GitHub59 comment/closure readback are
reported at delivery, without creating an extra docs-only commit. Round1 FAIL is in review-round1.md.
