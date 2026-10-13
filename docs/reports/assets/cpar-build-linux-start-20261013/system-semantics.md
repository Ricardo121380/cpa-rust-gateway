# Current Linux semantics and limits

Observed new-vps: Ubuntu24.04.4/aarch64, kernel6.17.0-1018-oracle,
systemd255 (255.4-1ubuntu8.17). The fresh exit read independently records current
systemd/kernel identity. Local manpages are absent; no installation was attempted.

Official v255 documentation: [systemd.service](https://raw.githubusercontent.com/systemd/systemd/v255/man/systemd.service.xml),
[systemd-run](https://raw.githubusercontent.com/systemd/systemd/v255/man/systemd-run.xml),
[systemd.exec](https://raw.githubusercontent.com/systemd/systemd/v255/man/systemd.exec.xml).
The service manual states that simple considers start complete after fork and
before execve; exec waits for both to succeed. This proves the semantic gap in
the old immediate sampler, not the exact root cause of the historic argv failure.
Type=exec does not prove application initialization, which retains independent
kernel-listener, management eligibility and quiet-audit checks.

Actual private-network runs1/3 had distinct namespaces, only lo, and both IPv4
and IPv6 TEST-NET connect_ex returned ENETUNREACH101. This is OS namespace
isolation, not only an LD_PRELOAD rule. Global network and production were not
modified. No network enforcement claim is made for run2: no live unit was
observed. Synthetic account is disabled, with synthetic state and independently
generated keys. It does not qualify a real account or Provider request.

Run2 returned1 and its first observation had MainPID0/LoadState=not-found,
ExecMainStatus0. Its preset oracle required203 and failed. Native command
stderr was not retained by that first driver; only its hash survives. Do not
invent a precise failure reason. The repaired fixture creates an executable
invalid-format file so a later authorized test can reach execve instead of
relying on an absent path. That repaired fixture has not run on Linux.

Run3 received404 from the first incorrect-management-Key GET and stopped safely.
Candidate product source is unchanged: management_security.rs:332 rejects a
mismatched key; middleware at467 calls management_denied_response at498, which
returns404. The original401 oracle is wrong for this candidate. Its FAIL receipt
is immutable; the corrected oracle's offline replay is separate from execution.

The final native-launch-capture, malformed-state diagnostic retention, sticky
failed identity gate and negative fixture corrections are not a new Linux run.
The complete system gate remains PARTIAL and cannot admit formal transport.
