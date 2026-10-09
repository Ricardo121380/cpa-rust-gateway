import json, os, pathlib, subprocess, sys
repo = pathlib.Path.cwd()
output = pathlib.Path(sys.argv[1]).resolve()
negative = len(sys.argv) > 2 and sys.argv[2] == "negative"
script = repo / "scripts/acceptance/cpar-batch-b-http.py"
if negative:
    code = "import pathlib, runpy, sys; p=pathlib.Path(sys.argv[1]); sys.path.insert(0,str(p.parent)); import cpar_seo_isolation_http; exec(\"def forced_failure(ns):\\n    ns[\\\"check\\\"](\\\"oracle_negative_forced_matrix_failure\\\", False)\"); cpar_seo_isolation_http.run=forced_failure; sys.argv=[str(p),sys.argv[2]]; runpy.run_path(str(p),run_name=\"__main__\")"
    command = [sys.executable, "-c", code, str(script), str(output)]
else:
    command = [sys.executable, str(script), str(output)]
child = subprocess.run(command, env={**os.environ,"CPAR_SEO_ONLY":"1"}, capture_output=True, text=True)
receipt = json.loads((output/"receipt.json").read_text())
printed = [json.loads(line) for line in child.stdout.splitlines() if line.startswith("{")]
result = {"negative_failure_injection":negative,"child_exit_code":child.returncode,"printed_results":printed,"persisted_status":receipt["status"],"checks":len(receipt["checks"]),"all_completed_checks_pass":all(item["status"]=="PASS" for item in receipt["checks"]),"source_sha":receipt["source_sha"]}
(output/"oracle-comparison.json").write_text(json.dumps(result,indent=2)+"\n")
print(json.dumps(result))
if negative:
    assert child.returncode != 0 and receipt["status"] == "FAIL" and not any(item.get("status")=="PASS" for item in printed), "failed matrix must neither exit successfully nor persist PASS"
else:
    assert child.returncode == 0 and receipt["status"] == "PASS" and len(printed)==1 and printed[0]["status"] == "PASS" and printed[0]["checks"] == len(receipt["checks"]) and result["all_completed_checks_pass"], "successful targeted matrix exit and persisted receipt disagree"
