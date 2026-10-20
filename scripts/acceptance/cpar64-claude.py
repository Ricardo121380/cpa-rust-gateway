#!/usr/bin/env python3
"""Opt-in #64 actual Claude Code qualification; synthetic credentials and owned sockets only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
from urllib.parse import urlsplit


ENTRY = Path('/Users/huangrui/.local/bin/claude')


def digest(path):
    value = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(chunk)
    return value.hexdigest()


def isolated_env(root):
    root.mkdir(mode=0o700, exist_ok=True)
    for name in ('home', 'config', 'cache', 'tmp', 'work'):
        (root / name).mkdir(mode=0o700, exist_ok=True)
    return {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': str(root / 'home'),
            'CLAUDE_CONFIG_DIR': str(root / 'config'), 'XDG_CONFIG_HOME': str(root / 'config'),
            'XDG_CACHE_HOME': str(root / 'cache'), 'TMPDIR': str(root / 'tmp'), 'TERM': 'dumb',
            'CLAUDE_CODE_TMPDIR': str(root / 'tmp'),
            'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC': '1', 'DISABLE_AUTOUPDATER': '1'}


def profile(root, port):
    # The client and all descendants inherit this fence. No arbitrary loopback service is allowed.
    network = '' if port is None else f'(allow network-outbound (remote ip "localhost:{port}"))\n'
    return ('(version 1)\n(allow default)\n'
            '(deny network*)\n'
            f'{network}'
            '(deny file-read* (subpath "/Users") (subpath "/Volumes") '
            '(subpath "/private/tmp") (subpath "/private/var/folders"))\n'
            '(deny file-read* (subpath "/Library/Application Support/ClaudeCode") '
            '(subpath "/Library/Managed Preferences") '
            '(literal "/Library/Preferences/com.anthropic.claudecode.plist"))\n'
            f'(allow file-read* (subpath {json.dumps(str(root))}))\n'
            '(deny file-write*)\n'
            f'(allow file-write* (subpath {json.dumps(str(root))}) (literal "/dev/null"))\n'
            '(deny mach-lookup (global-name "com.apple.securityd") '
            '(global-name "com.apple.securityd.xpc"))\n')


def fenced(command, root, fence, env, timeout=20):
    return subprocess.run(['/usr/bin/sandbox-exec', '-f', str(fence), *command],
                          cwd=root / 'work', env=env, capture_output=True, text=True, timeout=timeout)


def isolation(root, fence, env):
    # Probe an artificial credential outside the allowed tree, never a user's credential file.
    sentinel = root.parent / (root.name + '-outside-sentinel')
    sentinel.write_text('synthetic credential sentinel\n')
    listener = socket.socket()
    listener.bind(('127.0.0.1', 0))
    listener.listen(1)
    listener.settimeout(0.1)
    probe = '''import errno,json,os,socket,subprocess,sys
rows=[]
for family,host,port in [(socket.AF_INET,'192.0.2.1',443),(socket.AF_INET6,'2001:db8::1',443),(socket.AF_INET,'127.0.0.1',int(sys.argv[2]))]:
 s=socket.socket(family);s.settimeout(1)
 try:s.connect((host,port));raise RuntimeError('uncontrolled network permitted')
 except OSError as e:assert e.errno==errno.EPERM;rows.append({'network':host,'port':port,'errno':e.errno})
 finally:s.close()
for path,flags,label in [('/Users/huangrui',os.O_RDONLY,'real_home_read'),(sys.argv[1],os.O_RDONLY,'outside_credential_read'),(sys.argv[1],os.O_WRONLY,'outside_write')]:
 try:fd=os.open(path,flags);os.close(fd);raise RuntimeError(label+' permitted')
 except OSError as e:assert e.errno==errno.EPERM;rows.append({'probe':label,'errno':e.errno})
child=subprocess.run(['/bin/sh','-c','cat "$1"; printf forbidden > "$1"','probe',sys.argv[1]],capture_output=True,text=True)
assert child.returncode!=0 and 'synthetic credential' not in child.stdout
rows.append({'subprocess_denied':True,'exit':child.returncode})
print(json.dumps(rows))
'''
    try:
        result = fenced(['/usr/bin/python3', '-c', probe, str(sentinel),
                         str(listener.getsockname()[1])], root, fence, env)
        try:
            connection, _ = listener.accept()
            connection.close()
            raise RuntimeError('independent denied-network receiver accepted a connection')
        except socket.timeout:
            pass
        receipt = {'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr,
                   'independent_uncontrolled_loopback_accepts': 0,
                   'outside_sentinel_unchanged': sentinel.read_text() == 'synthetic credential sentinel\n'}
        (root / 'isolation.json').write_text(json.dumps(receipt, indent=2) + '\n')
        if result.returncode or not receipt['outside_sentinel_unchanged']:
            raise RuntimeError('OS isolation failed; client not started')
    finally:
        listener.close()
        sentinel.unlink()


def freeze(root):
    if sys.platform != 'darwin':
        raise RuntimeError('this runner requires macOS Seatbelt')
    source = ENTRY.resolve(strict=True)
    before = {'entry': str(ENTRY), 'link': os.readlink(ENTRY) if ENTRY.is_symlink() else None,
              'actual': str(source), 'sha256': digest(source), 'mode': source.stat().st_mode & 0o777,
              'architecture': subprocess.check_output(['/usr/bin/file', str(source)], text=True).strip()}
    binary = root / 'claude'
    shutil.copy2(source, binary)
    if digest(binary) != before['sha256']:
        raise RuntimeError('frozen executable differs')
    env = isolated_env(root)
    fence = root / 'identity.sb'
    fence.write_text(profile(root, None))
    isolation(root, fence, env)
    outputs = {}
    for flag in ('--version', '--help'):
        result = fenced([str(binary), flag], root, fence, env)
        outputs[flag] = {'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}
        if result.returncode:
            raise RuntimeError('isolated runtime identity failed')
    before.update(frozen=str(binary), runtime=outputs,
                  source_unchanged=digest(source) == before['sha256'],
                  frozen_unchanged=digest(binary) == before['sha256'])
    (root / 'client-identity.json').write_text(json.dumps(before, indent=2) + '\n')
    if not before['source_unchanged'] or not before['frozen_unchanged']:
        raise RuntimeError('executable changed while freezing')
    print(json.dumps({'version': outputs['--version']['stdout'].strip(), 'sha256': before['sha256'],
                      'actual': str(source), 'frozen': str(binary), 'isolation': 'PASS'}))


def client(root, base):
    root = Path(root).resolve(strict=True)
    identity = json.loads((root.parent / 'client-identity.json').read_text())
    binary = Path(identity['frozen'])
    if digest(binary) != identity['sha256']:
        raise RuntimeError('frozen client drift')
    endpoint = urlsplit(base)
    if endpoint.scheme != 'http' or endpoint.hostname != '127.0.0.1' or not endpoint.port:
        raise ValueError('client endpoint must be the owned numeric loopback listener')
    env = isolated_env(root)
    env.update(ANTHROPIC_BASE_URL=base, ANTHROPIC_API_KEY=os.environ['CPAR64_TEST_KEY'],
               ANTHROPIC_MODEL='grok-4.5', ANTHROPIC_DEFAULT_HAIKU_MODEL='grok-4.5',
               ANTHROPIC_DEFAULT_SONNET_MODEL='grok-4.5', ANTHROPIC_DEFAULT_OPUS_MODEL='grok-4.5',
               NO_PROXY='127.0.0.1,localhost')
    fence = root / 'client.sb'
    fence.write_text(profile(root, endpoint.port) +
                     f'(allow file-read* (literal {json.dumps(str(binary))}))\n')
    isolation(root, fence, env)
    (root / 'work/seed.txt').write_text('41\n')
    command = [str(binary), '--bare', '--print', '--no-session-persistence', '--verbose',
               '--output-format', 'stream-json', '--include-partial-messages',
               '--model', 'grok-4.5', '--tools', 'Read,Bash', '--permission-mode', 'bypassPermissions',
               'Tell me you are ready to inspect seed.txt.']
    result = fenced(command, root, fence, env, timeout=65)
    receipt = {'command': command, 'exit': result.returncode, 'stdout': result.stdout,
               'stderr': result.stderr, 'frozen_unchanged': digest(binary) == identity['sha256']}
    (root / 'client.json').write_text(json.dumps(receipt, indent=2) + '\n')
    public_env = {key: value for key, value in env.items() if key != 'ANTHROPIC_API_KEY'}
    public_env['credential_source'] = 'synthetic fixture key, value excluded'
    (root / 'configuration.json').write_text(json.dumps(public_env, indent=2) + '\n')


def qualify(root, diagnostic, target):
    repository = Path(__file__).resolve().parents[2]
    env = dict(os.environ)
    env['CARGO_TARGET_DIR'] = str(Path(target).resolve() if target else repository / 'target/cpar64')
    names = subprocess.check_output(['rg', '--files', 'apps', 'crates', '-g', '*.rs'],
                                    cwd=repository, text=True).splitlines()
    names += subprocess.check_output(['git', 'ls-files', 'Cargo.toml', ':(glob)**/Cargo.toml'],
                                     cwd=repository, text=True).splitlines()
    inputs = [repository / name for name in names]
    inputs += [repository / path for path in ('Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml',
                                              'scripts/acceptance/cpar64-claude.py')]
    source = {'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repository, text=True).strip(),
              'root': str(repository), 'inputs': [{'path': str(path.relative_to(repository)),
              'sha256': digest(path)} for path in sorted(set(inputs))]}
    (root / 'source-inputs.json').write_text(json.dumps(source, indent=2) + '\n')
    command = ['cargo', 'test', '--manifest-path', str(repository / 'Cargo.toml'), '--locked',
               '-p', 'gateway', '--bin', 'gateway', 'actual_claude_messages_build_observation',
               '--no-run', '--message-format=json']
    build = subprocess.run(command, cwd=repository, env=env, capture_output=True, text=True, timeout=600)
    (root / 'build.log').write_text(build.stdout + build.stderr)
    if build.returncode:
        raise RuntimeError('qualification binary build failed; see build.log')
    binaries = [row['executable'] for line in build.stdout.splitlines()
                if (row := json.loads(line)).get('reason') == 'compiler-artifact'
                and row.get('executable') and row.get('profile', {}).get('test')]
    if len(binaries) != 1:
        raise RuntimeError('expected one exact test executable')
    runtime = isolated_env(root / 'harness')
    runtime['CPAR64_RUN'] = str(root)
    run = [binaries[0], 'actual_claude_messages_build_observation', '--ignored', '--nocapture']
    with (root / 'qualification.log').open('w') as log:
        result = subprocess.run(run, cwd=root, env=runtime, stdout=log, stderr=subprocess.STDOUT, timeout=120)
    observation_path = root / 'standard/observations.json'
    observation = json.loads(observation_path.read_text()) if observation_path.exists() else {}
    client_path = root / 'standard/client.json'
    receipt = json.loads(client_path.read_text()) if client_path.exists() else {}
    results = [row for line in receipt.get('stdout', '').splitlines() if line.strip()
               and (row := json.loads(line)).get('type') == 'result']
    # In 2.1.302, subtype=success coexists with is_error=true on HTTP400. Check both.
    success = bool(results) and receipt.get('exit') == 0 and all(
        row.get('is_error') is False and row.get('subtype') == 'success' for row in results)
    before = json.loads((root / 'client-identity.json').read_text())
    unchanged = (ENTRY.resolve() == Path(before['actual']) and digest(ENTRY) == before['sha256']
                 and (os.readlink(ENTRY) if ENTRY.is_symlink() else None) == before['link']
                 and ENTRY.stat().st_mode & 0o777 == before['mode']
                 and digest(before['frozen']) == before['sha256'])
    (root / 'run.json').write_text(json.dumps({'exit': result.returncode, 'command': run,
        'test_binary_sha256': digest(binaries[0]), 'target': env['CARGO_TARGET_DIR'],
        'diagnostic': diagnostic, 'observation_oracles': 'PASS' if result.returncode == 0 else 'FAIL',
        'client_success': success, 'compatibility': observation.get('compatibility', 'UNKNOWN'),
        'client_install_and_copy_unchanged': unchanged,
        'claim': 'observation success cannot qualify the tool loop, summary or in-flight cancellation'}, indent=2) + '\n')
    if result.returncode or not unchanged:
        raise RuntimeError('actual-client qualification failed; original evidence retained')
    if not diagnostic:
        raise RuntimeError('Claude compatibility BLOCKED; no complete success/tool/summary/cancel evidence')
    print('Observation oracles PASS; actual Claude tool closure BLOCKED. See run.json.')


if __name__ == '__main__':
    if len(sys.argv) == 4 and sys.argv[1] == 'client':
        client(sys.argv[2], sys.argv[3])
        sys.exit(0)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--identity-only', action='store_true')
    parser.add_argument('--diagnostic', action='store_true',
                        help='verify exact pre-send rejection; never reports client compatibility PASS')
    parser.add_argument('--target-dir', help='explicit isolated Cargo target; default is this worktree target/cpar64')
    args = parser.parse_args()
    root = Path(args.output).resolve()
    if not any(root.is_relative_to(parent) for parent in
               (Path(tempfile.gettempdir()).resolve(), Path('/private/tmp'))):
        raise ValueError('output must be a new directory below the OS temporary directory')
    root.mkdir(mode=0o700)
    freeze(root)
    if not args.identity_only:
        qualify(root, args.diagnostic, args.target_dir)
