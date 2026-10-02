"""Verify retained archive bytes and restore original ownership in task copies only."""
import hashlib
import hmac
import json
import os
import pathlib
import shutil
import stat

ROOT=pathlib.Path("/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884")
capture=ROOT/"capture-2"
archive=pathlib.Path("/var/lib/cpa-rust-gateway/backups")
key=(capture/"integrity-hmac-key").read_bytes()

def entries(directory):
    result={}
    for item in [directory]+sorted(directory.rglob("*")):
        attributes=item.lstat()
        value={"uid":attributes.st_uid,"gid":attributes.st_gid,"mode":stat.S_IMODE(attributes.st_mode)}
        if item.is_symlink(): value.update(kind="symlink",target=os.readlink(item))
        elif item.is_dir(): value.update(kind="directory")
        else:
            assert stat.S_ISREG(attributes.st_mode)
            value.update(kind="file",bytes=attributes.st_size,sha256=hashlib.sha256(item.read_bytes()).hexdigest())
        result[str(item.relative_to(directory))]=value
    return result

before=entries(archive)
snapshot_entries=entries(capture/"state/backups")
assert before.keys()==snapshot_entries.keys()
assert all({k:v for k,v in metadata.items() if k not in ["uid","gid"]}=={k:v for k,v in snapshot_entries[name].items() if k not in ["uid","gid"]} for name,metadata in before.items())
destinations=[capture/"state/backups",ROOT/"migration-entry-probe/state/backups",ROOT/"restored-pristine/state/backups"]
for destination in destinations:
    assert destination.resolve().is_relative_to(ROOT.resolve())
    current=entries(destination)
    assert current.keys()==before.keys()
    for name,metadata in before.items():
        path=destination/name
        assert {k:v for k,v in current[name].items() if k not in ["uid","gid"]}=={k:v for k,v in metadata.items() if k not in ["uid","gid"]}
        os.chown(path,metadata["uid"],metadata["gid"],follow_symlinks=False)
        if metadata["kind"]!="symlink": os.chmod(path,metadata["mode"])
    assert entries(destination)==before
assert entries(archive)==before, "production_archive_changed_during_read_only_audit"
for name in ["unit.private","old-current.private","restore-metadata.private.json","logical-before.private.json","integrity-hmac-key"]:
    dest=ROOT/"restored-pristine"/name
    shutil.copy2(capture/name,dest)
    assert hmac.compare_digest((capture/name).read_bytes(),dest.read_bytes())
original=(capture/"restore-metadata.private.json")
shutil.copy2(original,capture/"restore-metadata-initial.private.json")
for package in [capture,ROOT/"restored-pristine"]:
    metadata={str(p.relative_to(package)):{"uid":p.lstat().st_uid,"gid":p.lstat().st_gid,"mode":stat.S_IMODE(p.lstat().st_mode),"bytes":p.lstat().st_size} for d in [package/"state",package/"credentials"] for p in [d]+list(d.rglob("*"))}
    (package/"restore-metadata.private.json").write_text(json.dumps(metadata,indent=2)+"\n")
digest=hmac.new(key,json.dumps(before,sort_keys=True,separators=(",",":")).encode(),hashlib.sha256).hexdigest()
result={"status":"PASS","scope":"retained production backup archives and full old-state restore package metadata","original_archive_bytes_and_symlink_targets_equal_private_capture":True,"production_archive_stable_during_read_only_audit":True,"copy_ownership_matches_original_after_operator_metadata_correction":True,"original_archive_entry_count":len(before),"entry_kind_counts":{kind:sum(x["kind"]==kind for x in before.values()) for kind in ["file","directory","symlink"]},"archive_bytes_and_metadata_hmac_sha256":digest,"restore_dependencies_retained":["both databases","backup artifacts and symlinks","six persistent keys","old binary","unit and drop-in text","old current target","ownership and modes","private integrity HMAC key and row baseline"],"private_metadata_correction_only":True,"production_permission_changes":False,"private_content_exported":False}
(ROOT/"complete-private-restore-audit.json").write_text(json.dumps(result,indent=2)+"\n")
print(json.dumps(result))
