import datetime
import gzip
import hashlib
import json
import pathlib
import re
import struct
import subprocess

repo = pathlib.Path.cwd()
root = repo / 'output/cpar-build-live-20261010'
assets = repo / 'docs/reports/assets/cpar-build-live-20261010'
sha = '1b6023f52f4c00ea6d8296524d306c141e4edae7'
tag = 'cpar-build-candidate:1b6023f5'
docker = ['docker', '--context', 'colima-cpar-acceptance-20261010']
image = json.loads(subprocess.check_output(docker + ['image', 'inspect', tag]))[0]
assert image['Architecture'] == 'arm64'
tracked = subprocess.check_output(['git', 'ls-tree', '-rz', '--name-only', sha]).decode().rstrip('\0').split('\0')
omitted = json.loads((assets / 'native-source-f346a03f.json').read_text())['dockerignore_omitted_files']
assert all(p in tracked for p in omitted)
files = [p for p in tracked if p not in omitted]
checks = {p: hashlib.sha256(subprocess.check_output(['git', 'show', sha + ':' + p])).hexdigest() for p in files}
observed = subprocess.run(docker + ['run', '--rm', '-i', '--network', 'none', '--entrypoint', '/bin/sh', tag,
    '-c', 'cd /src && xargs -0 sha256sum --zero --'], input=('\0'.join(files) + '\0').encode(), capture_output=True, check=True)
actual = {row[66:].decode(): row[:64].decode() for row in observed.stdout.split(b'\0') if row}
assert actual == checks
proof = {
    'status': 'PASS', 'source_sha': sha, 'tracked_files_total': len(tracked), 'tracked_files_checked': len(files),
    'dockerignore_omitted_files': omitted,
    'dockerignore_sha256': hashlib.sha256(subprocess.check_output(['git', 'show', sha + ':.dockerignore'])).hexdigest(),
    'native_admitted_source_equals_exact_commit': True, 'entire_tracked_tree_claim': False,
    'source_hash_manifest_sha256': hashlib.sha256(json.dumps(checks, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
    'network': 'none', 'image_id': image['Id'], 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
}
(assets / 'native-source-1b6023f5.json').write_text(json.dumps(proof, indent=2) + '\n')
a = root / 'artifact-1b6023f5'
a.mkdir()
container = 'cpar-build-extract-1b6023f5'
subprocess.run(docker + ['create', '--name', container, tag], check=True, capture_output=True)
try:
    subprocess.run(docker + ['cp', container + ':/src/target/release/gateway', str(a / 'gateway-aarch64-unknown-linux-gnu')], check=True, capture_output=True)
finally:
    subprocess.run(docker + ['rm', container], check=True, capture_output=True)
binary = (a / 'gateway-aarch64-unknown-linux-gnu').read_bytes()
assert binary[:4] == b'\x7fELF' and struct.unpack('<H', binary[18:20])[0] == 183
embedded = sorted(set(v.decode() for v in re.findall(rb'gateway-release-revision=([0-9a-f]{40})\n', binary)))
assert embedded == [sha]
metadata_command = ['ruby', 'scripts/p12-release-artifact.rb', 'build-metadata', '--revision', sha,
    '--rust-toolchain', '1.97.1', '--target', 'aarch64-unknown-linux-gnu', '--version', '0.1.0',
    '--output', str(a / 'gateway-build-metadata.json')]
subprocess.run(metadata_command, check=True, capture_output=True)
manifest = {
    'schema_version': 'cpa-rust-gateway-artifact-manifest-v1', 'revision': sha,
    'rust_toolchain': '1.97.1', 'target': 'aarch64-unknown-linux-gnu',
    'files': [{'name': name, 'bytes': (a / name).stat().st_size, 'sha256': hashlib.sha256((a / name).read_bytes()).hexdigest()}
              for name in ['gateway-aarch64-unknown-linux-gnu', 'gateway-build-metadata.json']],
}
(a / 'artifact-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
versions = subprocess.check_output(docker + ['run', '--rm', '--network', 'none', '--entrypoint', '/bin/sh', tag,
    '-c', 'rustc --version && node --version'], text=True).splitlines()
assert versions == ['rustc 1.97.1 (8bab26f4f 2026-07-14)', 'v25.8.0']
guard = json.loads((root / 'artifact-f346a03f/candidate-artifact.json').read_text())
candidate = {
    'status': 'PASS', 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'product_source_sha': sha, 'source_head_with_guard_fix': sha,
    'binary_file': '1b6023f5/gateway-aarch64-unknown-linux-gnu', 'binary_bytes': len(binary),
    'binary_sha256': hashlib.sha256(binary).hexdigest(), 'elf_machine': 183, 'target': 'aarch64-unknown-linux-gnu',
    'embedded_revisions': embedded, 'image_id': image['Id'], 'image_architecture': image['Architecture'],
    'toolchain_versions': versions,
    'manifest_file_sha256': hashlib.sha256((a / 'artifact-manifest.json').read_bytes()).hexdigest(),
    'manifest_canonical_sha256': hashlib.sha256(json.dumps(manifest, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
    **{key: guard[key] for key in ['guard_source_commit', 'guard_source_sha256', 'guard_sha256', 'guard_bytes', 'guard_path']},
    'artifact_scope': 'temporary local native ARM64 binary and build metadata; not a signed P12 release bundle',
    'build_metadata_command': metadata_command, 'guard_unchanged': True,
    'repair_basis': 'observed official stable CLI version drift; HTTP426 root cause remains NOT_PROVEN',
    'runtime_started': False, 'real_provider_requests': 0,
}
(a / 'candidate-artifact.json').write_text(json.dumps(candidate, indent=2) + '\n')
(assets / 'candidate-artifact-1b6023f5.json').write_text(json.dumps(candidate, indent=2) + '\n')
for name, public_name in [('gateway-build-metadata.json', 'gateway-build-metadata-1b6023f5.json'), ('artifact-manifest.json', 'artifact-manifest-1b6023f5.json')]:
    (assets / public_name).write_bytes((a / name).read_bytes())
raw = (root / 'arm64-1b6023f5-build.raw.log').read_bytes()
(assets / 'arm64-1b6023f5-build.raw.log.gz').write_bytes(gzip.compress(raw, mtime=0))
print(json.dumps({'status': 'PASS', 'native_source_checked': len(files), 'native_source_omitted': len(omitted), 'candidate': candidate}))
