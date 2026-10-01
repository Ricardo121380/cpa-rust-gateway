#!/usr/bin/env bash
set -euo pipefail
cd /workspace
test "$(git rev-parse HEAD)" = 80f5838902eb56be6dccc3ce047eb227d6c18e6c
CPAR_GATEWAY_BINARY=/candidate/gateway-aarch64-unknown-linux-gnu TMPDIR=/private python3 scripts/acceptance/cpar-batch-b-http.py /evidence/native-http
python3 /artifact-compatibility.py
