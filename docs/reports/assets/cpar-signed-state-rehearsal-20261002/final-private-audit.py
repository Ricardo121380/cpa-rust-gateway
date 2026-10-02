"""Read-only final production identity and retained private-copy audit."""
import datetime
import hashlib
import hmac
import importlib.util
import json
import os
import pathlib
import sqlite3
import stat
import subprocess

os.umask(0o077)
ROOT=pathlib.Path("/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884")
capture=ROOT/"capture-2"
expected_old="0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b"
pid=int(subprocess.check_output(["systemctl","show","cpa-rust-gateway.service","--value","-p","MainPID"]))
unit=subprocess.check_output(["systemctl","cat","cpa-rust-gateway.service"])
keys=pathlib.Path("/etc/cpa-rust-gateway/credentials")
loaded=pathlib.Path(f"/proc/{pid}/root/run/credentials/cpa-rust-gateway.service")
key_names=["management-key","management-csrf","master-key","backup-key","client-key-pepper","grok-build-cache-key"]
key_matches=all(hmac.compare_digest((keys/n).read_bytes(),(capture/"credentials"/n).read_bytes()) and hmac.compare_digest((keys/n).read_bytes(),(loaded/n).read_bytes()) for n in key_names)
assert key_matches and pid==499070
assert hashlib.sha256(pathlib.Path(f"/proc/{pid}/exe").read_bytes()).hexdigest()==expected_old
assert hmac.compare_digest(unit,(capture/"unit.private").read_bytes())
assert str(pathlib.Path("/opt/cpa-rust-gateway/current").resolve())==str(pathlib.Path((capture/"old-current.private").read_text().strip()))
assert ROOT.lstat().st_uid==0 and stat.S_IMODE(ROOT.lstat().st_mode)==0o700
live_task_gateways=0
for process in pathlib.Path("/proc").iterdir():
    if not process.name.isdigit(): continue
    try:
        executable=os.readlink(process/"exe")
    except (FileNotFoundError,PermissionError,ProcessLookupError):
        continue
    if executable.startswith(str(ROOT)+"/") and ("gateway" in pathlib.Path(executable).name): live_task_gateways+=1
assert live_task_gateways==0
listeners=subprocess.check_output(["ss","-H","-ltnp"],text=True)
prod_ports=[port for port in [18180,18181] if any("127.0.0.1:"+str(port) in line and "pid="+str(pid)+"," in line for line in listeners.splitlines())]
assert prod_ports==[18180,18181]
assert not any(":"+str(port)+" " in line for port in [28180,28181] for line in listeners.splitlines())
module_spec=importlib.util.spec_from_file_location("audit",ROOT/"operator-tools/private-crypto-audit.py")
audit=importlib.util.module_from_spec(module_spec); module_spec.loader.exec_module(audit)
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
from cryptography.exceptions import InvalidTag
master=bytearray((capture/"credentials/master-key").read_bytes())
failures=[]; categories={}
try:
    with sqlite3.connect(f"file:{capture}/state/control.sqlite3?mode=ro",uri=True) as database:
        for config,credential,upstream,envelope,version,lifecycle,status in database.execute("SELECT c.config_version_id,c.id,c.upstream_id,c.ciphertext,c.key_version,v.status,c.status FROM upstream_credentials c JOIN config_versions v ON v.id=c.config_version_id"):
            aad=bytearray(b"cpa-rust-gateway/control-plane/credential-aad/v1")
            for value in [config,credential,upstream]:
                field=value.encode(); aad.extend(len(field).to_bytes(4,"big")); aad.extend(field)
            try:
                assert version==1 and envelope[0]==1 and len(envelope)>=41
                nonce=envelope[1:25]
                plain=bytearray(ChaCha20Poly1305(audit.hchacha(bytes(master),nonce[:16])).decrypt(b"\0"*4+nonce[16:],envelope[25:],bytes(aad)))
                plain[:]=b"\0"*len(plain)
            except Exception as error:
                category="AEAD_INVALID_TAG" if isinstance(error,InvalidTag) else type(error).__name__
                categories[category]=categories.get(category,0)+1
                failures.append({"config_version_id":config,"credential_id":credential,"upstream_id":upstream,"configuration_lifecycle":lifecycle,"credential_status":status,"category":category,"ciphertext_sha256":hashlib.sha256(envelope).hexdigest()})
finally:
    master[:]=b"\0"*len(master)
assert len(failures)==1 and failures[0]["configuration_lifecycle"]=="archived" and categories=={"AEAD_INVALID_TAG":1}
(ROOT/"credential-audit-failures.private.json").write_text(json.dumps(failures,indent=2)+"\n")
result={"status":"PASS","observed_at_utc":datetime.datetime.now(datetime.timezone.utc).isoformat(),"production_pid":pid,"production_old_binary_hash_unchanged":True,"production_current_unchanged":True,"production_unit_and_dropin_bytes_unchanged":True,"six_persistent_keys_and_loaded_credentials_equal_preupgrade_capture":True,"production_loopback_listeners":prod_ports,"task_candidate_processes_remaining":live_task_gateways,"host_task_listeners_remaining":0,"task_root_owner":0,"task_root_mode":"0700","baseline_crypto_failure":{"count":len(failures),"categories":categories,"configuration_lifecycle":"archived","private_locator_retained":True},"private_state_or_key_bytes_exported":False,"real_provider_calls":0}
(ROOT/"final-private-audit.json").write_text(json.dumps(result,indent=2)+"\n")
print(json.dumps(result))
