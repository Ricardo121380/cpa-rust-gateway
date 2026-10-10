import hashlib
import json
import os
import pathlib
import stat
import sys
import tarfile

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert stat.S_IMODE(base.stat().st_mode) == 0o700
epoch = base / 'private/revalidation-20261010'
binary_dir = base / 'bin/1b6023f5'
assert not epoch.exists() and not binary_dir.exists()
epoch.mkdir(mode=0o700)
binary_dir.mkdir(mode=0o700)
expected = {
    'gateway-aarch64-unknown-linux-gnu': 'ea89d5c72628a871f7683f32d0b51a76dc9c588f7172c86345d1c989f1bd9063',
    'artifact-manifest.json': '20ef11961ca40d4dd659ae278c1e7ee629f9f70319f4a4b006d2d8685a5c9777',
    'gateway-build-metadata.json': 'a1e47a1ad7c132e223d5692bdee95b613d3f6465107c38867b9c58b9506c4895',
}
seen = set()
with tarfile.open(fileobj=sys.stdin.buffer, mode='r|') as archive:
    for member in archive:
        assert member.isfile() and member.name in expected and member.name not in seen
        data = archive.extractfile(member).read()
        assert hashlib.sha256(data).hexdigest() == expected[member.name]
        target = binary_dir / member.name if member.name.startswith('gateway-aarch64') else epoch / member.name
        with target.open('xb') as output:
            output.write(data)
        target.chmod(0o700 if member.name.startswith('gateway-aarch64') else 0o600)
        seen.add(member.name)
assert seen == set(expected)
print(json.dumps({'status': 'PASS', 'uploaded_hashes': expected, 'epoch': str(epoch), 'unknown_paths_overwritten': 0, 'provider_requests': 0}))
