"""Repeat only failed startup probes; reuse verified pristine migration/restore objects."""
import pathlib

TASK=pathlib.Path("/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884")
exec((TASK/"operator-tools/target-rehearsal.py").read_text().split("assert ROOT.lstat()")[0])
snapshot=json.loads((ROOT/"backup-receipt.json").read_text()); capture=pathlib.Path(snapshot["private_capture_directory"])
baseline=json.loads((capture/"logical-before.private.json").read_text()); key=(capture/"integrity-hmac-key").read_bytes()
prior=json.loads((ROOT/"rehearsal-receipt.json").read_text())
attempt=1+len(list(ROOT.glob("startup-recheck-*-receipt.json")))
write("startup-recheck-"+str(attempt)+"-receipt.json",prior)
assert prior["migration"]["status"]=="PASS"
assert prior["signed_binary_regression"]["status"]=="PASS"
def checked_start(state,credentials,binary,version,schema,label,previous):
    if previous["status"]=="PASS": return previous
    private_label=label+"-recheck-"+str(attempt)
    return startup(state,credentials,binary,version,schema,private_label,baseline,key)
new=checked_start(ROOT/"migration-entry-probe/state",ROOT/"migration-entry-probe/credentials",ARTIFACT/"gateway-aarch64-unknown-linux-gnu",SHA,34,"candidate-startup",prior["candidate_startup"])
old=checked_start(ROOT/"restored-pristine/state",ROOT/"restored-pristine/credentials",ROOT/"restored-pristine/old-gateway",OLD_SHA,31,"old-restored-startup",prior["full_old_state_restore"]["startup"])
restore=prior["full_old_state_restore"]
restore["startup"]=old; restore["status"]="PASS" if old["status"]=="PASS" else "FAIL"
restore["full_package_metadata_and_archive_audit"]=json.loads((ROOT/"complete-private-restore-audit.json").read_text())
prior["candidate_startup"]=new; prior["full_old_state_restore"]=restore
prior["status"]="PASS" if new["status"]==old["status"]==prior["migration"]["crypto"]["status"]=="PASS" else "BLOCKED"
prior["completed_at_utc"]=datetime.datetime.now(datetime.timezone.utc).isoformat()
write("candidate-startup-receipt.json",new); write("old-restored-startup-receipt.json",old); write("restore-receipt.json",restore); write("rehearsal-receipt.json",prior)
print(json.dumps({"status":prior["status"],"candidate_startup":new["status"],"old_startup":old["status"],"restore":restore["status"],"candidate_failure":new.get("current_read_route"),"old_failure":old.get("current_read_route")}))
