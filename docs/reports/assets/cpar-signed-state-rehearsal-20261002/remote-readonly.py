"""Read-only target inventory. Outputs metadata only; never prints credential material."""
import datetime
import hashlib
import json
import os
import pathlib
import re
import shutil
import sqlite3
import stat
import subprocess

service = "cpa-rust-gateway.service"
def unit():
    raw = subprocess.check_output(["systemctl", "show", service, "-p", "MainPID", "-p", "ActiveState", "-p", "SubState", "-p", "ExecMainStartTimestamp", "-p", "User", "-p", "Group", "-p", "FragmentPath", "-p", "DropInPaths"], text=True)
    return dict(line.split("=", 1) for line in raw.splitlines() if "=" in line)

before = unit()
pid = int(before["MainPID"])
binary = pathlib.Path(f"/proc/{pid}/exe").read_bytes()
state = pathlib.Path("/var/lib/cpa-rust-gateway")
credentials = pathlib.Path("/etc/cpa-rust-gateway/credentials")
raw_unit = subprocess.check_output(["systemctl", "cat", service], text=True)
allowed_directives = [line for line in raw_unit.splitlines() if line.startswith(("LoadCredential=", "User=", "Group=", "WorkingDirectory=", "StateDirectory=", "StateDirectoryMode="))]
databases = {}
for name in ["control.sqlite3", "admin-auth.sqlite3"]:
    with sqlite3.connect(f"file:{state/name}?mode=ro", uri=True) as db:
        db.execute("BEGIN")
        tables = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
        databases[name] = {"journal_mode": db.execute("PRAGMA journal_mode").fetchone()[0], "quick_check_ok": db.execute("PRAGMA quick_check").fetchall() == [("ok",)], "foreign_key_violation_count": len(db.execute("PRAGMA foreign_key_check").fetchall()), "tables": {table: db.execute('SELECT count(*) FROM "' + table.replace('"', '""') + '"').fetchone()[0] for table in tables}}
        if "schema_migrations" in tables:
            databases[name]["schema_version"] = db.execute("SELECT max(version) FROM schema_migrations").fetchone()[0]
        databases[name]["user_version"] = db.execute("PRAGMA user_version").fetchone()[0]

def metadata(root):
    paths = sorted(root.iterdir())
    if root == state:
        paths = [p for p in paths if p.name in {"control.sqlite3", "control.sqlite3-wal", "control.sqlite3-shm", "admin-auth.sqlite3", "admin-auth.sqlite3-journal", "backups"}]
    return [{"name": p.name, "kind": "symlink" if p.is_symlink() else "directory" if p.is_dir() else "file", "bytes": p.lstat().st_size, "mode": oct(stat.S_IMODE(p.lstat().st_mode)), "uid": p.lstat().st_uid, "gid": p.lstat().st_gid} for p in paths]

after = unit()
assert before == after
print(json.dumps({"observed_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "scope": "read_only_no_provider_no_service_change", "unit": before, "safe_unit_directives": allowed_directives, "current": str(pathlib.Path("/opt/cpa-rust-gateway/current").resolve()), "process_exe": os.readlink(f"/proc/{pid}/exe"), "binary_sha256": hashlib.sha256(binary).hexdigest(), "embedded": {k.decode(): v.decode() for k, v in re.findall(rb"gateway-release-(revision|rust-version|target)=([^\n]{1,100})\n", binary)}, "architecture": os.uname().machine, "libc": os.confstr("CS_GNU_LIBC_VERSION"), "databases": databases, "state_files": metadata(state), "persistent_credential_files": metadata(credentials), "free_bytes": {p: shutil.disk_usage(p).free for p in [str(state), "/opt/cpa-rust-gateway"]}, "tools": {name: shutil.which(name) for name in ["python3", "unshare", "ip", "ldd", "readelf", "setpriv", "bwrap", "strace", "sqlite3", "openssl"]}, "stable_process": True}, indent=2))
