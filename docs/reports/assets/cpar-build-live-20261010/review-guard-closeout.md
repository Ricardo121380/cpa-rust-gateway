# Independent guard repair closeout

Fixed increment: `98febf057f37bd784e81e9a2563adb36aef8ebbf...a3dc0c235f87b50685da72d0009d131d8e1118b5`.
Both agents were one-shot/read-only and independently read the C increment, rules and existing native receipts. They did not execute tests or remote operations. Prior six-file coverage remains in review-initial.md.

## Standards

Agent `/root/build_guard_tcp_standards_closeout`: **PASS**; incremental hard findings 0, highest severity none, actionable smell findings 0.

`cpar-build-egress-guard.c:97` uses existing getsockopt interfaces. SO_TYPE must equal SOCK_STREAM and SO_PROTOCOL must equal IPPROTO_TCP; either query failure denies remote admission. The check sits at the remote boundary alongside the known address and port constraints. No dependency, configuration or future extension mechanism was added. The change is backend-owned. Lean guidance explicitly says required trust-boundary validation is not bloat.

Coverage is only the C increment. It can be combined with the earlier six-file Standards PASS, without claiming those files were reviewed again.

## Spec

Agent `/root/build_guard_tcp_spec_closeout`: **PASS**, hard findings 0; original P3 closed within this increment.

README's "TCP/443 connections to those resolved addresses" requirement is implemented by successful SO_TYPE/SO_PROTOCOL checks at C lines 97–101 and their remote admission condition at 119–122. Unix and IPv4/IPv6 loopback return before that condition. Fixed Build DNS and audit-failure rejection are preserved.

Existing native receipts show UDP connect admitted by the old library and EACCES with the repaired library; HTTP requests and data sends are zero. The compiled green source hash matches the fixed C. This closes the observed UDP declaration gap.

Dynamic limitations: this review did not test candidate loading, real HTTP or basic acceptance. The existing green does not include remote Build TCP, Unix or audit-fault dynamic positive/negative controls. Actual candidate observations are recorded separately after startup.

Per-axis incremental findings: Standards 0 (none); Spec 0 (original P3 closed). Global specification, CI, release and production acceptance remain independent.
