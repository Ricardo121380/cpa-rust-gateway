# Standards title recheck: PARTIAL, not PASS

Agent `/root/b03_title_closed_standards`, generic one-shot read-only with
`fork_turns=none`, returned terminal `blocked` / Standards `PARTIAL`.
Base `9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2`, final source
`5e51f5f170a01b813631824b004eb2fcbf190201`.

It verified HEAD, merge-base, the two current commit IDs, the Task-ID-first
rule and the preserved previous COMPLETE 15/15 report. It then queried
`git --no-optional-locks rev-parse refs/codex/b03/pre-title-correction`, which
exited 128 because that prefix is not a ref. It did not query the actual
`.../first` or `.../final` refs, or directly check old commit SHA objects.
The agent explicitly did not infer that old objects were missing.

Final-source blob transfer coverage was 0/15; inherited historical coverage
alone was not promoted to COMPLETE. New titles, tree/blob identity,
authors/dates and retained advisory were not completely verified by this probe.
No tests or files were changed. This is not an accepted final Standards verdict.
The next new bounded independent probe closes the gap using explicit full refs
and SHA objects. Main's ref listing already confirmed both concrete backup refs.
