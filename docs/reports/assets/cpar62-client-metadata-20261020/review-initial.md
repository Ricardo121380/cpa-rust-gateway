# Fixed candidate review

Base: 973e1f589d24d0220b58d95071d91f824f29074d
Candidate: 50957d2369f705b7c6f440801950a92fb6522c92
Tree: 95cc6f5c5ed2b5a01bd96a45f7c304070bc7a394
Inputs: validation-overlay-binding.json (18 enumerated inherited overlays).
Reviewers: /root/c4_standards_review and /root/c4_spec_review, independent read-only one-shot agents. Both complete; no tests or external writes performed by reviewers.

## Standards

PASS (static). Zero hard violations and no actionable Fowler-baseline smell. Checked all 27 fixed paths and necessary call context against eight applicable standards sources, CONTEXT and relevant ADRs. HEAD, tree and overlay manifest unchanged.

Evidence: AGENTS.md:7 backend ownership; scripts/check-crate-boundaries.rb:16/:33 composition root and core dependencies, unchanged manifests; docs/adr/README.md:9/:10 Accepted decisions and persistence ADR discipline; new ADR-0116 indexed at README:152, distinct request policy at ADR-0116:11; bounded queue gateway-observability/src/lib.rs:295/:365 and transaction event_store.rs:512/:525; safe errors and private Debug request_metadata.rs:23/:62 and private CLI usage_evidence.rs:136. Existing lifecycle.rs:864 retains the original StatusCode::OK assertion. No Full/Provider claim.

## Spec

FAIL pending fix: one P2 finding, categories (b) unintended retention and (c) lifecycle error. Static call chain verified, no behavior reproduction performed by reviewer.

`store:true` clones the complete request before C4 consumption (gateway-http-actix/src/lib.rs:864). `prepare_request_metadata` at :2085 only removes the field from execution.request. The earlier canonical_request_for_store reaches StoredResponsePayload::try_new at :3518 and is fully serialized by gateway-store/src/stored_response.rs:737. Production Build supports StoredResponses (apps/gateway/src/runtime.rs:626-633). StoredResponse uses its own creation time/TTL after upstream response, while C4 GC only clears request_metadata_evidence. This can retain the original C4 field after its Request evidence expires, violating BC-PROTOCOL-010:103/:127-129 and ADR-0116:36-49 single Request capture and lifecycle.

This finding does not claim upstream or public leakage. No store:true case existed in the initial C4 public tests. No other new C4 defect found in reviewed validation, original string, authenticated owner, pre-execution confirmation, adapter/protocol/mode qualification, Attempt association, replay/rotation/restart paths. Original Codex/#62 and all independent blockers remain separate.
