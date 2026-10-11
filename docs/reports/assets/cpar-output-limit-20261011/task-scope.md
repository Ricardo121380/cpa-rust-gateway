# Authorized implementation and independent review scope

Date: 2026-10-11. Repository Ricardo121380/cpa-rust-gateway. Fixed base:
`af7480b2da3edffc324d9a42d81f7d0990f50358`, branch `codex/prism-v4-delivery`.
Origin: user request in this session, channel #47, delivery #57, parent specification #9.
This file records review requirements; it is not a real-provider execution authorization.

- Reproduce the failing public POST /v1/chat/completions with synthetic input,
  controlled model, `max_completion_tokens=4096`, `stream=false`. Record actual HTTP,
  Request/Attempt/Usage and independently counted local upstream receives. Keep RED.
- Repair the existing semantic owner with the smallest necessary Rust/contract change.
  Native Chat preserves the selected control. Modern Chat/Responses total-output
  limits map exactly. Legacy behavior must be explicit. Do not silently delete fields,
  choose among controls, weaken capabilities, or replace formal parameters.
- Reject both Chat fields even when equal; reject foreign namespaces and invalid
  values before upstream execution. Cover missing, null, zero, negative, fractional,
  string and out-of-range cases. Preserve existing strict positive-u64 policy.
- Use actual gateway process, synthetic credentials and OS/local egress restriction.
  The existing formal CLI and collector must execute Chat JSON/SSE and Messages
  JSON/SSE with text3, single-tool2, consecutive-tool4; preserve full history, actual
  tool output, exact terminal, Usage and ledger identity. Cover affected Responses
  JSON/SSE. Distinguish generic loopback from native Build in-process assembly.
- An associated defect in the same output-limit chain may be minimally repaired.
  Generic Channel Pin RED revealed its target namespace in a source request; its
  correction is included. Native Build Pin, credentials, schema, frontend, retry
  policy and billing algorithms remain outside implementation scope.
- Run affected tests and required clean-source Full. Independently review only base
  through the frozen implementation head: Standards and Spec remain separate.
  Subagents are read-only, one-shot and fork_turns=none. Main agent owns edits and
  final verification. Preserve all prior dirty files and old candidates.
- Prepare local Linux ARM64 candidate from frozen source and bind source/binary/
  manifest/checks. No upload, remote instance, push, publication, deployment or real
  Provider call. A concrete unavailable build environment may be reported as such.
- Commit only owned paths, archive red/green and intermediate failures, then append
  sanitized progress to the existing #47/#57/#9 records and independently read back.
  Preserve original AC, dependencies, old FAIL and OPEN state.
- Old Responses JSON/SSE real 9/9 results retain product source 1b6023f5. New binary
  evidence is independent. Plan fresh six-combination real acceptance as six new
  Pins plus 54 basics (60 maximum); historical 33 intentions /32 confirmed sends and
  UNKNOWN independent Provider receives/cost stay separate. Do not reuse old38.
  Stop this task after local delivery and GitHub sync, before any real continuation.
