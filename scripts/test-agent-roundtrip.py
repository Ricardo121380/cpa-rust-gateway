#!/usr/bin/env python3
"""Mandatory, finite real-gateway Agent regression. All providers are local synthetic peers."""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

repo = Path(__file__).resolve().parents[1]
receipt = Path(tempfile.mkdtemp(prefix='cpar-agent-gate-'))
passed = False
try:
    with (receipt / 'controller.log').open('w') as log:
        controller = subprocess.Popen(
            [sys.executable, str(repo / 'scripts/acceptance/prism-local-gateway.py'), str(receipt)],
            cwd=repo, stdout=log, stderr=log, start_new_session=True)
        info = None
        try:
            deadline = time.monotonic() + 60
            while True:
                assert controller.poll() is None, 'local gateway setup failed; see owned controller log'
                if (receipt / 'local-preview.json').exists():
                    info = json.loads((receipt / 'local-preview.json').read_text())
                if '"ready": true' in (receipt / 'controller.log').read_text():
                    break
                assert time.monotonic() < deadline, 'local gateway setup timed out'
                time.sleep(.1)
            # Publishing the fixture starts its catalog worker asynchronously. That worker
            # leases the same concurrency-one credentials and publishes a new admission
            # snapshot. Wait for its actual completion, not a delay or an inference retry.
            state = Path(info['root'])
            while True:
                assert controller.poll() is None, 'gateway stopped during catalog initialization'
                records = []
                for line in (state / 'gateway.log').read_text().splitlines():
                    try:
                        record = json.loads(line)
                    except ValueError:
                        continue
                    if record.get('target') == 'model_catalog':
                        records.append(record.get('fields', {}))
                completed = [record for record in records
                             if record.get('message') == 'model Catalog pass completed']
                if completed:
                    assert completed[-1]['attempted'] == completed[-1]['succeeded'] == 5, completed[-1]
                    print(json.dumps({'fixture_catalog_ready': completed[-1]}), flush=True)
                    break
                assert not any(record.get('message') == 'model Catalog pass unavailable'
                               for record in records), 'fixture catalog initialization failed'
                assert time.monotonic() < deadline, 'fixture catalog initialization timed out'
                time.sleep(.1)
            subprocess.run([sys.executable, str(repo / 'scripts/acceptance/prism-agent-roundtrip.py'),
                            str(receipt)], cwd=repo, check=True, timeout=90)
            subprocess.run([sys.executable, str(repo / 'scripts/acceptance/prism-request-chain.py'),
                            str(receipt)], cwd=repo, check=True, timeout=90)
            passed = True
        finally:
            if controller.poll() is None:
                controller.terminate()
                try:
                    controller.wait(timeout=45)
                except subprocess.TimeoutExpired:
                    os.killpg(controller.pid, signal.SIGKILL)
                    controller.wait()
            if info and passed:
                state = Path(info['root'])
                if info.get('synthetic') and state.name.startswith('prism-complete-local-'):
                    shutil.rmtree(state)
finally:
    if passed:
        shutil.rmtree(receipt)
    else:
        print(f'Agent regression evidence retained at {receipt}', file=sys.stderr)
