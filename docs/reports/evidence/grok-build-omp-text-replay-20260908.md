# Grok Build OMP assistant-text replay recovery — 2026-09-08

## Scope and diagnosis

Source task: `codex://threads/01a050c1-d402-7253-9366-7a90677a69b9`
(`调研 Pi Agent 扩展配置 (3)`). Its C12 resume receipt reported that a simple
tool stream worked, but a longer OMP Inspect conversation returned HTTP 503
`CredentialUnavailable`. The protected OMP coding acceptance was not completed.

On the previously deployed `fb93c489bbbc6e2d5aa35b41b3da413b7ab5851e`, the
controlled OMP installation's actual Pi Responses client reproduced this split:
the initial tool call and tool-result response passed; replaying the resulting
assistant text into a third request failed. A minimal synthetic request with
assistant `id`, `status: completed`, and output-text `annotations: []` also
returned HTTP 503 `CredentialUnavailable`. The target Build credential was
runtime `active / available` with no outstanding lease or refresh failure.

The canonical router rejected those known history fields as unknown extensions,
excluding the candidate before inference. Previous two-request acceptance never
replayed an assistant text response, so it did not cover this case.

## Change

- Preserve bounded assistant message IDs, completed status, known commentary /
  final-answer phase, and empty text annotations for Responses-to-Responses.
- Keep unknown extensions, invalid values, and cross-protocol conversion rejected.
- Extend `scripts/verify-pi-responses-tools.mjs` to four requests per model:
  tool call, text response, another tool call with the full history, final text.
  The supplied Pi module must come from the client installation being diagnosed.
- No frontend, management schema, credential, or client-protocol change.

## Local validation

- `cargo test --locked -p gateway-router -p provider-grok -p protocol-openai-responses`:
  413 passed, 5 existing ignored, 0 failed.
- `cargo clippy --locked -p gateway-router --all-targets -- -D warnings`: passed.
- Node syntax check and `git diff --check`: passed.
- Regression exercises lossless preservation, both cross-protocol rejections,
  malformed IDs/status/phase, unexpected metadata, and invalid annotations.

## Acceptance boundary

This is a CPAR repair. It does not mark OMP C12, other provider channels, or the
P13-15 Delivery Gate complete. Synthetic SDK cost coefficients in the diagnostic
script are not evidence of zero real billing; provider cost remains unverified.
Production rollout and the four-request results are recorded below after execution.
