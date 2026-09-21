# Prism module URL hotfix — 2026-09-21

Status: signed hotfix `b518d925fdaf03c92d235fa6ca4a7c5565d04043` deployed successfully.

The user reported a Safari `{} is not iterable` crash. The deployed entry maps stack `hN`, line14,column19495 to UnlockPage's `const [params] = useSearchParams()`. In the current vendor export `o` is useSearchParams; in the previous signed763b570 vendor it is useNavigate. Serving the real current entry with the extracted previous vendor reproduces the same hN,line14 failure in EgoLite (Chromium wording: `It is not a function or its return value is not iterable`). This proves a mixed-release failure, but does not recover or prove the user's exact Safari cache contents.

The build now derives a deterministic revision from its emitted contents and appends it to all HTML asset URLs and the main-to-vendor import. Four physical files remain, with no extra chunk, storage or CSP relaxation. New build invariants require all four references to carry the same revision. The two identical builds pass. TypeScript and three Rust embedded-UI tests pass.

EgoLite space18 verifies the old mixed bundle fails and the corrected bundle reaches the login form. Further authenticated and production readback are pending. The earlier M4/M5 full-production acceptance wording is superseded by this incident; passing health/assets/login alone is not full workflow acceptance.

## Browser reproduction and corrected path

Native Safari reproduced the exact screenshot error, including `{} is not iterable` and `hN:14:19495`, using the real mismatched release files. In the same Safari window, the corrected embedded gateway rendered the administrator login normally. No Safari cache was cleared. EgoLite completed real local administrator login, a 14-route loading smoke (not a complete business-operation rerun), and opened Kimi Coding onboarding with the correct authorization action and no provider/endpoint selector. No official authorization was triggered.

Fresh [signing run35575301854](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35575301854) and [formal gate35575299107](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35575299107) both passed.

## Production verification

- Both signed architectures passed; independent ARM64 verification and disconnected production-copy fallback/candidate/fallback/candidate passed.
- Cutover reached readiness in1,214ms. Installed binary SHA256: `7fe6890bab69bdb472ef43e43b604133d25d9ad3664a7cce8582fb8fad47931f`. Immediate compatible fallback is `a190767`; it predates this URL isolation fix, so prefer roll-forward if browser symptoms recur.
- Public HTML entry/preload/CSS and main vendor import share revision `f0d6f5b5aef275d5df534b84`. Versioned URLs were fetched successfully. Four-file hashes, CSP, listener/auth isolation and retained data checks passed; no production data was restored or deleted.
- Native Safari loaded the public login after deployment without clearing cache, used its existing saved credentials to log in successfully, loaded all seven production account entries, opened onboarding, selected Kimi Coding and confirmed its authorization action plus JSON import with no provider/endpoint choice, then cancelled through the dirty-state confirmation. No grant, account or configuration was submitted or changed. Password values were neither read nor printed.
- This is targeted production browser verification of the crash fix and the stated interaction path. It is not a new full all-workspace business acceptance or a real-provider consent test. Earlier broad completion wording has been corrected in the plan and original delivery report.
