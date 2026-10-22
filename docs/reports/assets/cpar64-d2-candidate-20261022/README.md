# #64 D2 controlled-candidate evidence

Parent report: [cpar-64-d2-candidate-20261022.md](../../cpar-64-d2-candidate-20261022.md).
Code candidate: `4015bd4542dbc52fde8984c14c01b34843ef3c59`.

- `candidate-final-binding.json`, `final-source-inputs-before.json.gz` and
  `final-input-verification.json` bind source, lock, inherited overlay and post-run attributes.
- `full-check.md`, `final-full-summary.json`, `final-full.log.gz` record final Full.
  `full.log.gz` is the earlier 7c246c58 Full and cannot replace the final run.
- `d2-*-observations.json` retain complete value-free Request/Attempt/receive/Usage/ledger
  associations, authorized summary/metadata readback verdicts and native result states.
- `request-shapes.json` retains before/after field presence and raw request hashes without
  metadata values. Original and D2 run results remain separate; original strict exit is 1.
- `codex-pi-run.json` and `evidence-summary.json` retain strict actual-client regression gates.
- `development-history.json` and compressed logs preserve RED, failed and intermediate runs.
- `raw-final-manifest.json.gz` and `development-run-manifest.json.gz` locate private originals
  by hash under the mode0700 output root named in the parent report. They do not contain raw
  HTTP bodies, native stdout, credential values, metadata values or database bytes.

The two immutable implementation commits precede this evidence-only addition. Nothing here
qualifies original Claude, fixed Build service, an external Provider, remote CI or a release.
