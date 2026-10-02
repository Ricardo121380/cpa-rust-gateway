"""Task-specific target compatibility, isolated migration and full old-state restore.

No production file writes, service operations, account recovery, pin or real inference.
Receipts contain aggregate counts/HMACs only. Logs, plaintext and snapshot bytes stay private.
"""
import datetime
import hashlib
import hmac
import importlib.util
import json
import os
import pathlib
import re
import shutil
import sqlite3
import stat
import subprocess
import uuid

os.umask(0o077)
ROOT = pathlib.Path("/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884")
ARTIFACT = ROOT/"artifact-542a3c30aae7"
SOURCE = ROOT/"selected-source"
TOOLS = ROOT/"operator-tools"
SHA = "80f5838902eb56be6dccc3ce047eb227d6c18e6c"
NEW_HASH = "542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87"
OLD_SHA = "b0cf36e387b774439acc23830221389eb3ac74d0"
OLD_HASH = "0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b"
UID, GID = 995, 986
DBS = ["control.sqlite3", "admin-auth.sqlite3"]
KEY_NAMES = ["management-key", "management-csrf", "master-key", "backup-key", "client-key-pepper", "grok-build-cache-key"]
def q(name): return '"'+name.replace('"','""')+'"'
def ro(path): return sqlite3.connect(f"file:{path}?mode=ro", uri=True)
def digest_rows(db, table, columns, key, prefix_schema=False):
    sql = "SELECT "+",".join(q(c) for c in columns)+" FROM "+q(table)
    if prefix_schema: sql += " WHERE version<=31"
    rows = sorted(json.dumps(list(row),ensure_ascii=False,separators=(",",":"),default=lambda b:{"sqlite_blob_hex":b.hex()}).encode() for row in db.execute(sql))
    digest = hmac.new(key,digestmod=hashlib.sha256)
    for row in rows: digest.update(len(row).to_bytes(8,"big")); digest.update(row)
    return {"count":len(rows),"original_columns_hmac_sha256":digest.hexdigest()}
def preserved(state, baseline, key, upgraded=False):
    result = {}
    for name in DBS:
        with ro(state/name) as db:
            db.execute("BEGIN")
            assert db.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
            assert not db.execute("PRAGMA foreign_key_check").fetchall()
            result[name] = {}
            for table, prior in baseline[name].items():
                actual = digest_rows(db,table,prior["columns"],key,upgraded and table=="schema_migrations")
                expected = {k:v for k,v in prior.items() if k!="columns"}
                result[name][table] = {**actual,"preserved":actual==expected}
    return result
def write(name, data):
    (ROOT/name).write_text(json.dumps(data,indent=2)+"\n")
def state_copy(source, dest):
    shutil.copytree(source,dest,symlinks=True)
    for p in [dest]+list(dest.rglob("*")):
        if p.is_symlink(): continue
        s=(source/p.relative_to(dest)).stat(); os.chown(p,s.st_uid,s.st_gid); os.chmod(p,stat.S_IMODE(s.st_mode))
def sandbox(binary, state=None, credentials=None, work=None, source=None, command=None, service_identity=True):
    if service_identity:
        rootfs=ROOT/("sandbox-root-"+uuid.uuid4().hex); rootfs.mkdir(mode=0o700)
        spec={"rootfs":str(rootfs),"host_namespaces":{name:os.readlink("/proc/self/ns/"+name) for name in ["net","mnt","pid","ipc","uts"]},"bindings":[],"service_identity":True,"environment":{},"cwd":"/"}
        def private_bind(path,destination,readonly=False):
            spec["bindings"].append({"source":str(path),"destination":destination,"readonly":readonly})
        private_bind(binary,"/gateway",True)
        for path,destination,readonly in [(state,"/state",False),(credentials,"/credentials",True),(work,"/work",False),(source,"/source",True)]:
            if path: private_bind(path,destination,readonly)
        extra=list(command or ["--","/gateway","--help"])
        while extra and extra[0] != "--":
            option=extra.pop(0)
            if option == "--ro-bind":
                private_bind(extra.pop(0),extra.pop(0),True)
            elif option == "--setenv":
                name,value=extra.pop(0),extra.pop(0)
                spec["environment"][name]=value
            else:
                raise AssertionError("unsupported_private_namespace_option")
        assert extra and extra.pop(0)=="--"
        spec["command"]=extra
        return ["unshare","--mount","--net","--pid","--fork","--kill-child=SIGKILL","--ipc","--uts","--","/usr/bin/python3",str(TOOLS/"private-namespace-runner.py"),json.dumps(spec,separators=(",",":"))]
    args=["bwrap","--unshare-all","--cap-drop","ALL","--die-with-parent","--new-session","--clearenv","--ro-bind","/usr","/usr","--symlink","usr/bin","/bin","--symlink","usr/lib","/lib","--symlink","usr/lib64","/lib64","--proc","/proc","--dev","/dev","--tmpfs","/tmp"]
    def bind(path, destination, read_only=False):
        fd=os.open(path,os.O_PATH|os.O_CLOEXEC)
        args.extend(["--ro-bind-fd" if read_only else "--bind-fd",str(fd),destination])
    bind(binary,"/gateway",True)
    if state: bind(state,"/state")
    if credentials: bind(credentials,"/credentials",True)
    if work: bind(work,"/work")
    args += ["--ro-bind","/etc/ssl/certs","/etc/ssl/certs","--ro-bind","/etc/ssl/openssl.cnf","/etc/ssl/openssl.cnf"]
    if source:
        args += ["--ro-bind",str(source),"/source","--chdir","/source","--setenv","GIT_CONFIG_COUNT","1","--setenv","GIT_CONFIG_KEY_0","safe.directory","--setenv","GIT_CONFIG_VALUE_0","/source"]
    args += ["--setenv","PATH","/usr/bin:/bin","--setenv","PYTHONDONTWRITEBYTECODE","1"]
    if service_identity: args += ["--uid",str(UID),"--gid",str(GID)]
    extra=list(command or ["--","/gateway","--help"])
    for index in range(len(extra)-2):
        if extra[index] == "--ro-bind" and extra[index+1].startswith(str(ROOT)+"/"):
            fd=os.open(extra[index+1],os.O_PATH|os.O_CLOEXEC)
            extra[index]="--ro-bind-fd"; extra[index+1]=str(fd)
    args += extra
    if service_identity:
        args=["setpriv","--reuid",str(UID),"--regid",str(GID),"--clear-groups","--inh-caps=-all","--ambient-caps=-all","--"]+args
    return args
def isolated_run(args,**kwargs):
    fds=tuple(int(args[i+1]) for i in range(len(args)-1) if args[i] in ["--bind-fd","--ro-bind-fd"])
    try:
        return subprocess.run(args,pass_fds=fds,**kwargs)
    finally:
        for fd in fds: os.close(fd)
def startup(pristine_state, pristine_keys, binary, version, schema, label, baseline, integrity_key):
    work=ROOT/label; work.mkdir(mode=0o700); os.chown(work,UID,GID)
    state=work/"state"; state_copy(pristine_state,state)
    # The original production backup staging directory belongs to the service identity.
    # Archive payloads stay pristine; the derived probe's writable staging directory must
    # reproduce that identity instead of the root owner introduced by operator copytree.
    os.chown(state/"backups",UID,GID)
    keys=work/"credentials"; shutil.copytree(pristine_keys,keys)
    for p in [keys]+list(keys.iterdir()): os.chown(p,UID,GID)
    assert all(hmac.compare_digest((keys/n).read_bytes(),(pristine_keys/n).read_bytes()) for n in KEY_NAMES)
    args=sandbox(binary,state,keys,work,service_identity=True,command=["--ro-bind",str(TOOLS/"isolated-startup-probe.py"),"/probe.py","--","/usr/bin/python3","/probe.py",version,NEW_HASH if version==SHA else OLD_HASH,str(schema)])
    run=isolated_run(args,capture_output=True,timeout=65)
    (work/"namespace-runner.private.log").write_bytes(run.stdout+run.stderr)
    receipt_path=work/"startup-receipt.json"
    if not receipt_path.exists():
        return {"status":"FAIL","failure_category":"sandbox_or_runner_failed","return_code":run.returncode,"private_log":str(work/"namespace-runner.private.log")}
    result=json.loads(receipt_path.read_text())
    result["snapshot_derived_keys_identical"]=True
    after=preserved(state,baseline,integrity_key,schema==34)
    changed={db:[t for t,r in tables.items() if not r["preserved"]] for db,tables in after.items()}
    result["startup_copy_changed_original_tables"]={db:tables for db,tables in changed.items() if tables}
    result["pristine_migration_or_restore_copy_retained"]=True
    result["background_external_access_blocked_by_network_namespace"]=True
    write(label+"-receipt.json",result)
    return result

assert ROOT.lstat().st_uid==0 and stat.S_IMODE(ROOT.lstat().st_mode)==0o700
if (ROOT/"rehearsal-receipt.json").exists():
    prior=json.loads((ROOT/"rehearsal-receipt.json").read_text()); assert prior["status"]=="PASS"
    print(json.dumps({**prior,"reused":True})); raise SystemExit(0)
snapshot=json.loads((ROOT/"backup-receipt.json").read_text()); assert snapshot["status"]=="PASS"
capture=pathlib.Path(snapshot["private_capture_directory"]); assert capture.parent==ROOT
baseline=json.loads((capture/"logical-before.private.json").read_text()); integrity_key=(capture/"integrity-hmac-key").read_bytes()
spec=importlib.util.spec_from_file_location("crypto_probe",TOOLS/"private-crypto-audit.py"); crypto=importlib.util.module_from_spec(spec); spec.loader.exec_module(crypto)
binary=ARTIFACT/"gateway-aarch64-unknown-linux-gnu"
manifest=json.loads((ARTIFACT/"artifact-manifest.json").read_text()); receipt=json.loads((ARTIFACT/"artifact-receipt.json").read_text())
assert manifest["revision"]==receipt["revision"]==SHA
assert set(p.name for p in ARTIFACT.iterdir())==set([f["name"] for f in manifest["files"]]+["artifact-manifest.json","artifact-manifest.json.sig","artifact-manifest.sigstore.json","artifact-receipt.json"])
for record in manifest["files"]:
    p=ARTIFACT/record["name"]; assert p.stat().st_size==record["bytes"] and hashlib.sha256(p.read_bytes()).hexdigest()==record["sha256"]
for name,expected in receipt["files"].items(): assert hashlib.sha256((ARTIFACT/name).read_bytes()).hexdigest()==expected
assert hashlib.sha256(binary.read_bytes()).hexdigest()==NEW_HASH
elf=binary.read_bytes(); assert elf[:4]==b"\x7fELF" and elf[4:6]==b"\x02\x01" and int.from_bytes(elf[18:20],"little")==183
embedded={k.decode():v.decode() for k,v in re.findall(rb"gateway-release-(revision|rust-version|target)=([^\n]{1,100})\n",elf)}
assert embedded=={"revision":SHA,"rust-version":"1.97.1","target":"aarch64-unknown-linux-gnu"}
os.chmod(binary,0o755)
ldd=subprocess.run(["ldd",str(binary)],capture_output=True,text=True,check=True); assert "not found" not in ldd.stdout+ldd.stderr
versions=subprocess.check_output(["readelf","--version-info",str(binary)],text=True)
required=sorted(set(re.findall(r"GLIBC_(\d+\.\d+)",versions)),key=lambda s:tuple(map(int,s.split("."))))
help_run=isolated_run(sandbox(binary,service_identity=True),capture_output=True,text=True,check=True)
compat={"status":"PASS","source_sha":SHA,"binary_sha256":NEW_HASH,"elf_machine":183,"elf_class":"ELF64 little endian","embedded":embedded,"libc":os.confstr("CS_GNU_LIBC_VERSION"),"max_required_glibc":required[-1],"ldd":ldd.stdout.splitlines(),"missing_dynamic_libraries":False,"isolated_help_exit_code":help_run.returncode,"target_artifact_and_receipt_digests_match":True}
write("target-compatibility.json",compat)
crypto_before=crypto.authenticate(capture/"state/control.sqlite3",capture/"credentials"); write("crypto-before.json",crypto_before)
if (ROOT/"signed-binary-regression.json").exists():
    regression_summary=json.loads((ROOT/"signed-binary-regression.json").read_text()); assert regression_summary["status"]=="PASS" and regression_summary["binary_sha256"]==NEW_HASH and regression_summary["source_sha"]==SHA and regression_summary["checks_passed"]==410
else:
    synthetic=ROOT/("synthetic-2" if (ROOT/"synthetic").exists() else "synthetic"); synthetic.mkdir(mode=0o700)
    args=sandbox(binary,work=synthetic,source=SOURCE,service_identity=False,command=["--setenv","CPAR_GATEWAY_BINARY","/gateway","--setenv","TMPDIR","/work","--setenv","PYTHONPATH","/source/scripts/acceptance","--","/usr/bin/python3","/source/scripts/acceptance/cpar-batch-b-http.py","/work/evidence"])
    test=isolated_run(args,capture_output=True,timeout=240)
    (synthetic/"run.private.log").write_bytes(test.stdout+test.stderr)
    test_receipt=synthetic/"evidence/receipt.json"
    assert test.returncode==0 and test_receipt.exists(), "signed_binary_synthetic_regression_failed"
    regression=json.loads(test_receipt.read_text()); assert regression["status"]=="PASS" and regression["source_sha"]==SHA and regression["binary_sha256"]==NEW_HASH
    assert len(regression["checks"])==410 and all(x["status"]=="PASS" for x in regression["checks"])
    regression_summary={"status":"PASS","source_sha":SHA,"binary_sha256":NEW_HASH,"checks_passed":len(regression["checks"]),"all_checks_pass":all(x["status"]=="PASS" for x in regression["checks"]),"synthetic_provider_request_count":len(regression["provider_calls"]),"real_provider_calls":0,"scope":"exact signed ARM64 binary; real gateway+SQLite+loopback TLS/SOCKS; private network/mount/pid namespace"}
    write("signed-binary-regression.json",regression_summary)
upgraded=ROOT/"migration-entry-probe"
if not upgraded.exists():
    upgraded.mkdir(mode=0o700); state_copy(capture/"state",upgraded/"state")
    migration=isolated_run(sandbox(binary,state=upgraded/"state",service_identity=True,command=["--","/gateway","admin","audit","--db","/state/control.sqlite3"]),capture_output=True,timeout=30)
    (upgraded/"audit.private.log").write_bytes(migration.stdout+migration.stderr)
    (upgraded/"audit-exit.json").write_text(json.dumps({"return_code":migration.returncode}))
if not (upgraded/"credentials").exists(): shutil.copytree(capture/"credentials",upgraded/"credentials")
# open_local first migrates the real store, then uses empty CLI Catalog/capability
# views to compile the populated graph. Its documented bootstrap error is retained.
# A successful service startup with real persisted views is required separately.
audit_log=(upgraded/"audit.private.log").read_bytes()
assert b"RouteSnapshot compilation failed" in audit_log and b"missing_endpoint_capability_profile" in audit_log
with ro(upgraded/"state/control.sqlite3") as db:
    schema=db.execute("SELECT max(version) FROM schema_migrations").fetchone()[0]; assert schema==34
    assert db.execute("SELECT count(*) FROM gateway_attempt_execution").fetchone()[0]==0
    assert db.execute("SELECT count(*) FROM native_account_import_receipts").fetchone()[0]==0
    assert db.execute("SELECT count(*) FROM grok_account_import_batches WHERE updated_count<>0").fetchone()[0]==0
    assert all(json.loads(value)=={"provenance":"unknown","input_accounting":"unknown"} for (value,) in db.execute("SELECT usage_evidence_json FROM billing_ledger_entries"))
    event_counts=dict(db.execute("SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type"))
tables=preserved(upgraded/"state",baseline,integrity_key,True)
assert all(r["preserved"] for rows in tables.values() for r in rows.values()),"migration_changed_original_columns"
crypto_after=crypto.authenticate(upgraded/"state/control.sqlite3",upgraded/"credentials"); assert crypto_after==crypto_before
migration_result={"status":"PASS","source_sha":SHA,"binary_sha256":NEW_HASH,"schema_before":31,"schema_after":schema,"migration_entry":"exact candidate gateway admin audit through the real store migration path; no serve/refresh/network during migration","cli_audit_bootstrap":{"status":"FAIL","return_code":1,"category":"documented_empty_CLI_catalog_bootstrap_for_populated_graph","raw_log_retained_privately":True,"service_startup_required_separately":True},"integrity":"ok","foreign_key_violations":0,"all_original_table_columns_exact":True,"tables":tables,"event_type_counts":event_counts,"legacy_execution_sidecars":0,"legacy_native_import_receipt_rows":0,"legacy_import_updated_count":"zero_unknown_no_invented_updates","legacy_usage_evidence":"unknown_no_backfill","crypto":crypto_after,"private_migrated_state_retained":str(upgraded)}
write("migration-receipt.json",migration_result)
new_start=startup(upgraded/"state",upgraded/"credentials",binary,SHA,34,"candidate-startup",baseline,integrity_key)
restored=ROOT/"restored-pristine"; restored.mkdir(mode=0o700)
state_copy(capture/"state",restored/"state"); shutil.copytree(capture/"credentials",restored/"credentials"); shutil.copy2(capture/"old-gateway",restored/"old-gateway")
assert hashlib.sha256((restored/"old-gateway").read_bytes()).hexdigest()==OLD_HASH
assert all(hashlib.sha256((restored/"state"/n).read_bytes()).hexdigest()==snapshot["database_sha256"][n] for n in DBS)
assert all(hmac.compare_digest((restored/"credentials"/n).read_bytes(),(capture/"credentials"/n).read_bytes()) for n in KEY_NAMES)
restored_tables=preserved(restored/"state",baseline,integrity_key)
assert all(r["preserved"] for rows in restored_tables.values() for r in rows.values())
with ro(restored/"state/control.sqlite3") as db: assert db.execute("SELECT max(version) FROM schema_migrations").fetchone()[0]==31
restored_crypto=crypto.authenticate(restored/"state/control.sqlite3",restored/"credentials"); assert restored_crypto==crypto_before
old_start=startup(restored/"state",restored/"credentials",restored/"old-gateway",OLD_SHA,31,"old-restored-startup",baseline,integrity_key)
restore_result={"status":"PASS" if old_start["status"]=="PASS" else "FAIL","old_source_sha":OLD_SHA,"old_binary_sha256":OLD_HASH,"schema_restored":31,"integrity":"ok","full_state_databases_byte_identical_before_startup":True,"all_original_table_columns_exact":True,"matching_six_persistent_keys_restored":True,"crypto":restored_crypto,"tables":restored_tables,"startup":old_start,"private_restored_state_retained":str(restored),"method":"restore independent full pre-upgrade runtime state (both databases, backup directory, six keys, old binary); no migration down"}
write("restore-receipt.json",restore_result)
result={"status":"PASS" if new_start["status"]==old_start["status"]==crypto_before["status"]=="PASS" else "BLOCKED","source_sha":SHA,"binary_sha256":NEW_HASH,"target_compatibility":compat,"signed_binary_regression":regression_summary,"migration":migration_result,"candidate_startup":new_start,"full_old_state_restore":restore_result,"production_service_changed":False,"production_state_modified":False,"real_provider_calls":0,"completed_at_utc":datetime.datetime.now(datetime.timezone.utc).isoformat()}
write("rehearsal-receipt.json",result)
print(json.dumps(result))
raise SystemExit(0 if result["status"]=="PASS" else 2)
