"""Assemble the existing approved recipe; no process or credential access."""
from pathlib import Path


def assemble(approved, options, base, epoch, unit, source_sha, request_sha, diagnostic):
    base, epoch = Path(base), Path(epoch)
    binary = base / 'bin' / epoch.name / 'gateway-aarch64-unknown-linux-gnu'
    guard = base / 'bin/egress-guard-tcp.so'
    audit = base / 'audit' / (epoch.name + '.jsonl')
    old_binary = str(base / 'bin/gateway-aarch64-unknown-linux-gnu')
    assert approved[0] == 'systemd-run' and approved.count(old_binary) == 1
    assert approved.count('--unit=cpar-build-acceptance-20261010.service') == 1
    assert options == []  # Exact current copy-runtime-options.json, independently frozen.
    launch = []
    for value in approved:
        if value == old_binary:
            value = str(binary)
        value = value.replace('--unit=cpar-build-acceptance-20261010.service', '--unit=' + unit)
        value = value.replace(str(base / 'private/gateway.log'), str(epoch / 'gateway-v2.log'))
        value = value.replace(str(base / 'audit/candidate-egress.jsonl'), str(audit))
        value = value.replace(str(base / 'state'), str(epoch / 'state'))
        value = value.replace(str(base / 'credentials'), str(epoch / 'credentials'))
        launch.append(value)
    launch += options
    assert launch[launch.index(str(binary)):] == [str(binary), 'serve', '--state-dir', str(epoch / 'state'),
        '--credential-dir', str(epoch / 'credentials'), '--data-listen', '127.0.0.1:19180',
        '--management-listen', '127.0.0.1:19181', '--background-provider-tasks', 'disabled']
    return diagnostic.with_diagnostic_environment(launch, binary, guard, audit, epoch / 'native-capture',
                                                  request_sha, source_sha)
