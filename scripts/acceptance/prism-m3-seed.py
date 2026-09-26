"""Create a second real request-history page using only the owned loopback mock."""
import json
import pathlib
import sys
import urllib.request

out = pathlib.Path(sys.argv[1]).resolve()
info = json.loads((out / "local-preview.json").read_text())
assert info["synthetic"]
root = pathlib.Path(info["root"])
assert root.name.startswith("prism-complete-local-")
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
for index in range(52):
    request = urllib.request.Request(
        f"http://127.0.0.1:{info['data_port']}/v1/responses",
        headers={"Authorization": "Bearer " + (root / "client-key").read_text(), "Content-Type": "application/json"},
        data=json.dumps({"model": "Exact/Model-v2", "input": "synthetic M3 pagination", "max_output_tokens": 16}).encode(),
    )
    with http.open(request, timeout=10) as response:
        assert response.status == 200
        assert json.load(response)["status"] == "completed"
(out / "pagination-seed.json").write_text(json.dumps({"loopback_requests": 52, "real_provider_inference_calls": 0}))
print("52 loopback requests completed; no real provider calls")
