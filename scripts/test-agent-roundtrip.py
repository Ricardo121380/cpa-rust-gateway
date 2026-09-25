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
