# Commands and identity boundary

These are the commands actually run. All candidate acceptance below used detached source
`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar64-m34-full-20261022-9035551d`
at `9035551dcef79e130e17f7591fa3ca8a0ff3a265` plus the explicit eighteen-file overlay.
For a rerun, choose a new validation worktree, target and output names; the existing evidence
must remain intact. The strict CLI output must be beneath Python's current OS temporary directory.

```sh
CARGO_BUILD_JOBS=4 \
CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022/target-full-9035551d \
CHECK_REPORT_PATH=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022/full-final-9035551d.md \
bash scripts/check.sh full

CARGO_BUILD_JOBS=2 python3 scripts/acceptance/cpar64-claude.py \
  --output /private/tmp/cpar64-m34-9035551d-final-20261022 \
  --target-dir /Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022/target-full-9035551d

CARGO_BUILD_JOBS=2 \
CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022/target-full-9035551d \
python3 scripts/acceptance/cpar62-cli.py --require-prompt-release --require-codex-success \
  --output /private/var/folders/tk/90cjjmks0h1b2l13fry36ccm0000gn/T/cpar62-m34-9035551d-final-20261022

python3 scripts/acceptance/test_cpar62_cli.py -v

CARGO_BUILD_JOBS=2 \
CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022/target-full-9035551d \
cargo test --locked -p gateway --bin gateway \
  standard_responses::lifecycle::messages::local_controls -- --nocapture

python3 scripts/acceptance/cpar64-metadata-recovery.py \
  --current-binary /Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022/target-full-9035551d/debug/gateway \
  --historical-binary /Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022/target-schema37/debug/gateway \
  --support-binary /Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022/target-full-9035551d/debug/deps/cpar65_process_support-a3587e2b788d466b \
  --current-source /Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar64-m34-full-20261022-9035551d \
  --historical-source /Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar64-metadata-schema37-4099 \
  --output /Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar64-local-controls-20261022/process-final-9035551d

cargo audit
```

Full exited0, 45/45. Actual Claude's observation test exited0 but the unmodified strict wrapper
exited1 because compatibility is still BLOCKED. The strict Codex/Pi wrapper exited0 with all
seven scenarios and six Codex success checks. The bounded public suite passed11, and the stock
process matrix passed267 checks across8 processes. Cargo audit exited0 both times, but its
registry yanked query timed out both times; that query remains UNKNOWN.

The first Full at32da5424 failed the unchanged native Anthropic inverse equality assertion.
Its original report/log are retained separately. The metadata raw-token inverse fix was
tested with `cargo test --locked -p protocol-anthropic --lib` (34/0/0) and targeted Clippy before
the new9035551d Full. No assertion, timeout or admission whitelist was weakened.

The process support binary was selected from `cargo test --locked -p gateway --test
cpar65_process_support --no-run --message-format=json`, not by guessing an executable glob.
The schema37 binary was freshly built from4099d260 in its own detached source/target. Client,
normal gateway and support-test binaries have separate identities in the accompanying receipts.
