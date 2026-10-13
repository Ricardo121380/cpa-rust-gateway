"""Declared synthetic inputs for the complete, frozen remote-exit entry point."""
import hashlib
import json
from pathlib import Path
import subprocess

from startup_probe import save


def production_identity():
    raw = subprocess.check_output(['systemctl', 'show', 'cpa-rust-gateway.service',
        '--property=MainPID,NRestarts,ExecMainStartTimestampMonotonic,ActiveState,SubState,FragmentPath'],
        text=True, timeout=5)
    value = dict(row.split('=', 1) for row in raw.splitlines() if '=' in row)
    proc = Path('/proc', value['MainPID'])
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    value.update(process_start=(proc / 'stat').read_text().rsplit(')', 1)[1].split()[19],
        argv_sha256=digest(proc / 'cmdline'), exe_sha256=digest(proc / 'exe'),
        unit_sha256=digest(Path(value['FragmentPath'])),
        current_target=str(Path('/opt/cpa-rust-gateway/current').resolve()),
        caddyfile_sha256=digest(Path('/etc/caddy/Caddyfile')))
    return value


def save_budget(path, budget):
    assert budget['provider_intent_limit'] == budget['provider_intents'] == 0
    assert budget['new_pin_intents'] == budget['new_basic_intents'] == 0
    assert budget['new_inference_limit'] == 0 and budget['cumulative_inference_limit'] == 35
    assert budget['historical_inference_requests'] == 35
    assert budget['historical_gateway_pin_confirmed_sends'] == 34
    save(path, budget)


def adapt_exit(source, epoch_name, unit):
    """Every substitution must match once; retain the complete successful branch."""
    source = source.replace('chat-json-linux-final-20261013', epoch_name)
    source = source.replace('cpar-build-' + epoch_name + '.service', unit)
    begin = "source = ast.parse((base / 'private/bootstrap.py').read_text())"
    end = "exec(compile(functions, 'production-proof', 'exec'))"
    assert source.count(begin) == source.count(end) == 1
    first, last = source.index(begin), source.index(end) + len(end)
    source = source[:first] + 'from system_exit_adapter import production_identity as production' + source[last:]
    begin = "source_args = pathlib.Path('/proc', start['production_before']['MainPID'], 'cmdline').read_bytes().decode().split('\\0')"
    end = "assert active == start['production_active_configuration']"
    assert source.count(begin) == source.count(end) == 1
    first, last = source.index(begin), source.index(end) + len(end)
    source = source[:first] + "active = []  # Synthetic run: production configuration HTTP is NOT_RUN.\nassert start['production_active_configuration'] == active" + source[last:]
    # This negative-start branch is not exercised by A. Remove its credential
    # HTTP seam too, so the adapter cannot read a real key on any branch.
    begin = "    args = pathlib.Path('/proc', expected['production']['MainPID'], 'cmdline').read_bytes().decode().split('\\0')"
    end = "    assert active == expected['production_config']"
    assert source.count(begin) == source.count(end) == 1
    first, last = source.index(begin), source.index(end) + len(end)
    source = source[:first] + "    active = []\n    assert expected['production_config'] == active" + source[last:]
    source = source.replace('import cpar_build_json_diagnostic as diagnostic', 'import system_exit_adapter as diagnostic')
    assertion = "assert budget['new_inference_limit'] == 2 and budget['cumulative_inference_limit'] == 37"
    assert source.count(assertion) == 1
    source = source.replace(assertion, "assert budget['new_inference_limit'] == 0 and budget['cumulative_inference_limit'] == 35\nassert budget['provider_intent_limit'] == budget['provider_intents'] == 0\nassert budget['new_pin_intents'] == budget['new_basic_intents'] == 0")
    source = source.replace("'management_get_intents_in_exit': 1", "'management_get_intents_in_exit': 0, 'production_configuration_http': 'NOT_RUN; synthetic adapter'")
    compile(source, 'synthetic-complete-remote-exit', 'exec')
    return source


class AuditTailClock:
    """Real clock and waits; append before returning the first deadline crossing."""
    def __init__(self, real_time, probe, audit, marker):
        self.real, self.probe, self.audit, self.marker = real_time, probe, audit, marker
        self.injection = None
        self.begin = None

    def monotonic(self):
        value = self.real.monotonic()
        if self.begin is None:
            self.begin = value  # The real quiet_audit() begin call, before its record.
        self.inject_at_deadline(value)
        return value

    def sleep(self, seconds):
        self.real.sleep(seconds)
        self.monotonic()

    def inject_at_deadline(self, value):
        import os
        starts = [row for row in self.probe.trace['observations'] if row['phase'] == 'quiet_audit_started']
        if not starts or self.injection is not None:
            return
        elapsed = value - self.begin
        if elapsed >= 65:
            assert not self.audit.exists() or self.audit.stat().st_size == 0
            row = {'event': 'synthetic_audit_tail', 'synthetic': True, 'marker': self.marker,
                   'monotonic_seconds': value, 'elapsed_seconds': elapsed}
            raw = (json.dumps(row, separators=(',', ':')) + '\n').encode()
            with self.audit.open('ab') as output:
                output.write(raw)
                output.flush()
                os.fsync(output.fileno())
            directory = os.open(self.audit.parent, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
            self.injection = {**row, 'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw)}
            save(self.probe.epoch / 'synthetic-injection.json', self.injection)
