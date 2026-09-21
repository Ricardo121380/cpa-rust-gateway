# Prism module URL hotfix — 2026-09-21

Status: implementation verified locally; deployment pending.

The user reported a Safari `{} is not iterable` crash. The deployed entry maps stack `hN`, line14,column19495 to UnlockPage's `const [params] = useSearchParams()`. In the current vendor export `o` is useSearchParams; in the previous signed763b570 vendor it is useNavigate. Serving the real current entry with the extracted previous vendor reproduces the same hN,line14 failure in EgoLite (Chromium wording: `It is not a function or its return value is not iterable`). This proves a mixed-release failure, but does not recover or prove the user's exact Safari cache contents.

The build now derives a deterministic revision from its emitted contents and appends it to all HTML asset URLs and the main-to-vendor import. Four physical files remain, with no extra chunk, storage or CSP relaxation. New build invariants require all four references to carry the same revision. The two identical builds pass. TypeScript and three Rust embedded-UI tests pass.

EgoLite space18 verifies the old mixed bundle fails and the corrected bundle reaches the login form. Further authenticated and production readback are pending. The earlier M4/M5 full-production acceptance wording is superseded by this incident; passing health/assets/login alone is not full workflow acceptance.
