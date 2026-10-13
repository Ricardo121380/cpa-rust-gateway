"""Bounded exec handshake and durable, value-safe Linux startup observations."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import time


def save(path, value):
    temporary = path.with_suffix('.tmp')
    with temporary.open('w') as output:
        json.dump(value, output, indent=2)
        output.flush()
        os.fsync(output.fileno())
    temporary.replace(path)
    descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


class StartupError(RuntimeError):
    """A closed reason code, never arbitrary command or HTTP error text."""


class StartupProbe:
    def __init__(self, epoch, unit, binary, binary_hash, launch, guard, ports=(19180, 19181)):
        self.epoch, self.unit = Path(epoch), unit
        self.binary, self.binary_hash, self.guard = Path(binary), binary_hash, Path(guard)
        self.expected_args = launch[launch.index(str(binary)):]
        self.expected_environment = dict(row.split('=', 1) for row in shlex.split(next(
            value.split('Environment=', 1)[1] for value in launch if value.startswith('--property=Environment='))))
        self.ports = sorted(ports)
        self.path = self.epoch / 'startup-observations.json'
        self.trace = {'unit': unit, 'launch': {'status': 'NOT_RUN'}, 'observations': [],
                      'gates': dict.fromkeys(['exec', 'identity', 'listeners', 'application', 'quiet_audit'], 'UNKNOWN')}

    def record(self, phase, **value):
        row = {'phase': phase, 'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
               'monotonic_seconds': time.monotonic(), **value}
        self.trace['observations'].append(row)
        save(self.path, self.trace)
        return row

    def systemd(self):
        fields = ['Type', 'LoadState', 'ActiveState', 'SubState', 'MainPID', 'ExecMainPID',
                  'ExecMainCode', 'ExecMainStatus', 'Result', 'ExecMainStartTimestampMonotonic',
                  'ActiveEnterTimestampMonotonic', 'InactiveEnterTimestampMonotonic', 'ControlGroup']
        raw = subprocess.check_output(['systemctl', 'show', self.unit, '--property=' + ','.join(fields)],
                                      text=True, timeout=5)
        return {name: value for name, value in (row.split('=', 1) for row in raw.splitlines() if '=' in row)
                if name in fields}

    def observe(self):
        state, process, failures = {}, {}, []
        try:
            state = self.systemd()
            pid = int(state['MainPID'])
            process['pid'] = pid
            if pid:
                proc = Path('/proc', str(pid))
                start = (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19]
                raw = (proc / 'cmdline').read_bytes()
                process.update(process_start=start,
                    exe_path=str((proc / 'exe').resolve()),
                    exe_sha256=hashlib.sha256((proc / 'exe').read_bytes()).hexdigest(),
                    argv_sha256=hashlib.sha256(raw).hexdigest())
                args = raw.decode().rstrip('\0').split('\0')
                environment = (proc / 'environ').read_bytes().decode().split('\0')
                status = dict(row.split(':', 1) for row in (proc / 'status').read_text().splitlines() if ':' in row)
                process.update(argv_exact_match=args == self.expected_args,
                    state_dir_match='--state-dir' in args and args.index('--state-dir') + 1 < len(args)
                        and args[args.index('--state-dir') + 1] == str(self.epoch / 'state'),
                    guard_mapped=str(self.guard) in (proc / 'maps').read_text(),
                    environment_matches={name: sum(row.split('=', 1)[0] == name for row in environment) == 1
                        and name + '=' + value in environment for name, value in self.expected_environment.items()},
                    proxy_environment_absent=not any(row.split('=', 1)[0] in ['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY',
                        'http_proxy', 'https_proxy', 'all_proxy'] and row.split('=', 1)[1] for row in environment if '=' in row),
                    cap_eff=status['CapEff'].strip())
                checks = {'argv_mismatch': process['argv_exact_match'], 'state_dir_mismatch': process['state_dir_match'],
                    'systemd_state_mismatch': state.get('LoadState') == 'loaded' and state.get('ActiveState') == 'active'
                        and state.get('SubState') == 'running',
                    'exe_path_mismatch': process['exe_path'] == str(self.binary),
                    'exe_hash_mismatch': process['exe_sha256'] == self.binary_hash,
                    'guard_not_mapped': process['guard_mapped'], 'environment_mismatch': all(process['environment_matches'].values()),
                    'proxy_environment_present': process['proxy_environment_absent'],
                    'capabilities_mismatch': int(process['cap_eff'], 16) == 4,
                    'process_changed_during_observation': start == (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19]
                        and int(self.systemd()['MainPID']) == pid}
                failures.extend(name for name, passed in checks.items() if not passed)
            else:
                failures.append('main_pid_absent')
        except Exception as error:
            failures.append('identity_observation_unknown')
            process['observation_exception_type'] = type(error).__name__
        # Persist all safely observable fields BEFORE enforcing identity.
        return self.record('identity', systemd=state, process=process, failure_reasons=failures)

    def identity(self, observed=None):
        row = observed if observed is not None else self.observe()
        if row['failure_reasons']:
            self.trace['gates']['identity'] = 'FAIL'
            save(self.path, self.trace)
            raise StartupError('startup_identity_failed')
        value = row['process']
        if self.trace['gates']['identity'] != 'FAIL':
            self.trace['gates']['identity'] = 'PASS'
        save(self.path, self.trace)
        return {'pid': value['pid'], 'process_start': value['process_start'],
                'binary_sha256': value['exe_sha256'], 'argv_sha256': value['argv_sha256'],
                'state_dir': str(self.epoch / 'state')}

    def launch(self, command):
        self.record('launch_requested')
        error = None
        try:
            result = subprocess.run(command, capture_output=True, timeout=25)
            self.trace['launch'] = {'status': 'RETURNED', 'returncode': result.returncode,
                'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
                'stderr_sha256': hashlib.sha256(result.stderr).hexdigest(), 'timeout_seconds': 25}
        except Exception as caught:
            error = caught
            self.trace['launch'] = {'status': 'TIMEOUT' if isinstance(caught, (subprocess.TimeoutExpired, TimeoutError))
                else 'UNKNOWN', 'exception_type': type(caught).__name__, 'timeout_seconds': 25}
        captured = error if error is not None else result
        for stream in ['stdout', 'stderr']:
            raw = getattr(captured, stream, None) or b''
            if isinstance(raw, str): raw = raw.encode()
            with (self.epoch / ('launch-native.' + stream)).open('xb') as output:
                output.write(raw)
                output.flush()
                os.fsync(output.fileno())
            self.trace['launch'][stream + '_sha256'] = hashlib.sha256(raw).hexdigest()
            self.trace['launch'][stream + '_bytes'] = len(raw)
        observed = self.observe()
        if error is not None:
            self.trace['gates']['exec'] = 'UNKNOWN'
            save(self.path, self.trace)
            raise StartupError('launch_transport_unknown') from error
        if result.returncode != 0 or observed['systemd'].get('Type') != 'exec':
            self.trace['gates']['exec'] = 'FAIL'
            save(self.path, self.trace)
            raise StartupError('exec_handshake_failed')
        self.trace['gates']['exec'] = 'PASS'
        save(self.path, self.trace)
        return self.identity(observed)

    def listeners(self, proc):
        sockets = {os.readlink(fd) for fd in (proc / 'fd').iterdir()}
        ports = []
        for family in ['tcp', 'tcp6']:
            for row in (proc / 'net' / family).read_text().splitlines()[1:]:
                fields = row.split()
                address, port = fields[1].rsplit(':', 1)
                port = int(port, 16)
                if fields[3] != '0A' or port not in self.ports:
                    continue
                if family != 'tcp' or address != '0100007F' or 'socket:[' + fields[9] + ']' not in sockets or port in ports:
                    raise StartupError('listener_address_or_pid_mismatch')
                ports.append(port)
        return sorted(ports)

    def wait_listeners(self, identity):
        deadline = time.monotonic() + 30
        proc = Path('/proc', str(identity['pid']))
        while self.listeners(proc) != self.ports:
            if self.identity() != identity:
                raise StartupError('listener_process_changed')
            if time.monotonic() >= deadline:
                self.record('listener_timeout')
                raise StartupError('listener_deadline_exceeded')
            time.sleep(.2)  # Bounded kernel-state polling; no HTTP request is repeated.
        self.trace['gates']['listeners'] = 'PASS'
        self.record('listeners_ready', ports=self.ports, pid=identity['pid'])

    def quiet_audit(self, identity, audit, seconds=65):
        begin = time.monotonic()
        self.record('quiet_audit_started', required_seconds=seconds)
        while time.monotonic() - begin < seconds:
            if int(self.systemd()['MainPID']) != identity['pid']:
                raise StartupError('audit_process_changed')
            if audit.exists() and audit.stat().st_size:
                raise StartupError('quiet_audit_not_flat')
            time.sleep(.5)
        if self.identity() != identity or self.listeners(Path('/proc', str(identity['pid']))) != self.ports:
            raise StartupError('post_audit_identity_or_listener_changed')
        if audit.exists() and audit.stat().st_size:
            raise StartupError('quiet_audit_not_flat')
        self.trace['gates']['quiet_audit'] = 'PASS'
        self.record('quiet_audit_passed', elapsed_seconds=time.monotonic() - begin)
        return time.monotonic() - begin

    def stop(self, identity, persist_budget):
        if self.identity() != identity:
            raise StartupError('stop_identity_changed')
        persist_budget()
        save(self.epoch / 'startup-stop-intent.json', {**identity, 'stop_intent': True, 'startup_validation_status': 'NOT_PASSED'})
        if self.identity() != identity:
            raise StartupError('stop_identity_changed_after_intent')
        subprocess.run(['systemctl', 'stop', self.unit], check=True, timeout=25, capture_output=True)
        if int(self.systemd()['MainPID']) != 0 or Path('/proc', str(identity['pid'])).exists():
            raise StartupError('owned_stop_not_verified')
        self.record('owned_stop_verified', pid=identity['pid'], process_start=identity['process_start'])
