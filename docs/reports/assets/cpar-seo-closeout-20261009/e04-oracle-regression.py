"""Check targeted acceptance exit/receipt agreement with the real owned gateway.

The negative child changes only the in-memory matrix callback; no product hook is added.
"""
import json
import os
import pathlib
import runpy
import subprocess
import sys


if sys.argv[1] == "--child-negative":
    script = pathlib.Path(sys.argv[2])
    destination = sys.argv[3]
    sys.path.insert(0, str(script.parent))
    import cpar_seo_isolation_http

    def forced_failure(namespace):
        namespace["check"]("oracle_negative_forced_matrix_failure", False)

    cpar_seo_isolation_http.run = forced_failure
    sys.argv = [str(script), destination]
    runpy.run_path(str(script), run_name="__main__")
    raise AssertionError("failed matrix unexpectedly returned")


destination = pathlib.Path(sys.argv[1]).resolve()
negative = len(sys.argv) > 2 and sys.argv[2] == "negative"
script = pathlib.Path.cwd() / "scripts/acceptance/cpar-batch-b-http.py"
command = (
    [sys.executable, str(pathlib.Path(__file__).resolve()), "--child-negative", str(script), str(destination)]
    if negative else [sys.executable, str(script), str(destination)]
)
child = subprocess.run(command, env={**os.environ, "CPAR_SEO_ONLY": "1"}, capture_output=True, text=True)
receipt = json.loads((destination / "receipt.json").read_text())
printed = [json.loads(line) for line in child.stdout.splitlines() if line.startswith("{")]
comparison = {
    "negative_failure_injection": negative,
    "child_exit_code": child.returncode,
    "printed_results": printed,
    "persisted_status": receipt["status"],
    "checks": len(receipt["checks"]),
    "all_completed_checks_pass": all(item["status"] == "PASS" for item in receipt["checks"]),
    "forced_failure_reached": "oracle_negative_forced_matrix_failure" in child.stderr if negative else None,
    "source_sha": receipt["source_sha"],
}
(destination / "oracle-comparison.json").write_text(json.dumps(comparison, indent=2) + "\n")
print(json.dumps(comparison))
if negative:
    assert child.returncode != 0 and receipt["status"] == "FAIL" and comparison["forced_failure_reached"]
    assert not any(item.get("status") == "PASS" for item in printed)
else:
    assert child.returncode == 0 and receipt["status"] == "PASS" and comparison["all_completed_checks_pass"]
    assert len(printed) == 1 and printed[0]["status"] == "PASS" and printed[0]["checks"] == len(receipt["checks"])
