# First fixed dual-axis review — retained

Base: `312584184e7680594878460499c39af0a244d440`.
Head: `a09611e2e70d54338b9fb2f6782bc19926146f63`; 49 files.
Both independent read-only reviewers returned complete. These verdicts are retained after repair.

## Standards

PASS; hard findings0, suggestions0. Reviewer `/root/final_standards` inspected all49 files and the
12 smell heuristics. It checked AGENTS ownership, ADR format/status, the two-package-only lock diff,
duplicate warnings, archived Full hashes and preserved AGENTS. GitHub script/public bindings only;
no private Provider inputs or tool reruns. It made no Spec verdict.

## Spec

Batch consistency **FAIL**, new violations1, scope creep0. Reviewer `/root/final_spec` inspected
all49 files and39 actual GitHub response/payload bindings.

P2: review-spec.md requires concrete consumers; acceptance matrix lines60–62 promises npm ci
reproduction. consumer-package-lock.json lines13/78 used external relative temporary-directory
package keys, not project node_modules entries. npm11.19.1 virtual-tree validation found both root
SDK dependencies MISSING, so the documented clean reproduction was inferred to fail. The reviewer
did not install packages and did not invalidate the previously executed SDK behavior receipts.

The parent reproduced that failure with actual clean offline npm ci, preserved the original
manifest/lock and RED log, corrected only portable package metadata/keys, and ran clean npm ci
plus the seven-case probe successfully. Versions, URLs, integrities of all9 packages and all7
result bytes stayed equal. See consumer-clean-replay.json; a new fixed-head review is required.

Other reviewed requirements were consistent: two minimum dependency patches, Full45/1515/0/13
plus both historic failures, synthetic TLS/SDK boundaries, exactly3 existing-comment body PATCHes
and full readback/prefix/body/AC/metadata/dependency checks. Original #9 remains PARTIAL/BLOCKED;
profile implementation, Build alias relation and actual cost are not newly proven.
