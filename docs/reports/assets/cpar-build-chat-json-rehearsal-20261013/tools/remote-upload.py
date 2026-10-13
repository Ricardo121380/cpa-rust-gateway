"""Exclusive upload into this batch's fixed protected directory; no execution."""
import hashlib
import json
import os
import pathlib
import re
import stat
import sys
import tarfile

os.umask(0o077)
base=pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
assert json.loads((base/'owner.json').read_text())['task']=='cpar-build-20261010'
assert not base.is_symlink() and base.stat().st_uid==0 and stat.S_IMODE(base.stat().st_mode)==0o700
assert len(sys.argv)==2 and re.fullmatch('[0-9a-f]{64}',sys.argv[1])
directory=base/'bin/chat-json-rehearsal-20261013'
assert not directory.exists()
allowed={'gateway-aarch64-unknown-linux-gnu','artifact-manifest.json','gateway-build-metadata.json',
         'cpar-batch-c-live.py','cpar_new_build_acceptance.py','cpar_build_json_diagnostic.py',
         'remote-readiness.py','remote-prepare.py','remote-start.py','remote-pin.py','remote-chat.py','remote-exit.py','user-authorization.json','historical-cost-authorization.json','launch_recipe.py','qualification.py'}
with tarfile.open(fileobj=sys.stdin.buffer,mode='r|') as archive:
    members=iter(archive)
    first=next(members)
    assert first.isfile() and first.name=='freeze.json' and first.size<=65536
    raw=archive.extractfile(first).read()
    assert hashlib.sha256(raw).hexdigest()==sys.argv[1]
    freeze=json.loads(raw)
    assert freeze['kind']=='chat_json_502_diagnostic' and freeze['epoch']=='chat-json-rehearsal-20261013'
    assert re.fullmatch('[0-9a-f]{40}',freeze['source_sha'])
    assert set(freeze['files'])==allowed
    assert all(re.fullmatch('[0-9a-f]{64}',value) for value in freeze['files'].values())
    directory.mkdir(mode=0o700)
    with (directory/'freeze.json').open('xb') as output: output.write(raw)
    seen=set()
    for member in members:
        assert member.isfile() and member.name in allowed and member.name not in seen and member.size<=134217728
        value=archive.extractfile(member).read()
        assert hashlib.sha256(value).hexdigest()==freeze['files'][member.name]
        with (directory/member.name).open('xb') as output: output.write(value)
        if member.name=='gateway-aarch64-unknown-linux-gnu': (directory/member.name).chmod(0o700)
        seen.add(member.name)
assert seen==allowed
assert all(hashlib.sha256((directory/name).read_bytes()).hexdigest()==checksum for name,checksum in freeze['files'].items())
print(json.dumps({'status':'PASS','directory':str(directory),'source_sha':freeze['source_sha'],
                  'uploaded_hashes':freeze['files'],'overwrite_count':0,'provider_calls':0}))
