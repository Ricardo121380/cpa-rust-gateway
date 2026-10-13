# Independent reviews of 9aa08db6...1d9cb436

These historical findings are retained. A later fix does not rewrite this FAIL
or turn either incomplete Spec attempt into a complete review.

## Standards — terminal FAIL

Reviewer `/root/standards_linux_start`, head
`1d9cb436afbced7244993d6fca20789afeaa18cf`: hard finding 1; judgement finding 1.

H1: successful-start `tools/remote-exit.py:149-157` stopped after only one
PID/start, state argument and exe-hash check. It lacked exact argv/exe path,
guard/unique environment checks, a bound durable stop intent and subsequent
PID/start recheck. This violates `batch-contract.md:19-20`, which requires full
current ownership, durable stopped budget/intent and a fresh PID/start recheck.
The no-start-receipt tests and startup-failure cleanup tests did not cover this
successful-start branch. No actual wrong-process stop was observed.

J1: possible Duplicated Code in active identity checks across startup_probe,
remote-start and both remote-exit branches. This is a judgement item, not a hard
violation and not an instruction to delete historical system-driver-v1 evidence.

The reviewer checked applicable ownership rules, preservation, fixed hashes,
actual PASS/FAIL/FAIL receipts and all three resource exits; no SSH, Provider or
private credential reads were performed.

## Spec — terminal PARTIAL with incomplete coverage

Reviewer `/root/spec_linux_start`: one disclosed partial requirement, the full
real Linux gate. Run2 lacks kernel exec status203, the corrected exec fixture and
final deltas were not run, and all three authorized system slots are exhausted.
The reviewer verified pre-assert persistence, launch bounds, failed-identity
latch, cleanup ordering, real normal65s evidence, retained wrong-key FAIL,
zero formal calls/null lease, UNKNOWN Provider receives/cost and the controller's
PARTIAL-gate rejection. No confirmed new code defect or scope creep was reported.

The reviewer stopped after a zsh unmatched-path error and explicitly did not
claim a complete full-scope review. `/root/spec_linux_start_final` was a clean
one-shot replacement; after a single request to stop expanding and converge,
it still had no terminal answer and was interrupted. It has no final verdict.
Neither attempt authorizes formal execution or counts as full independent PASS.
