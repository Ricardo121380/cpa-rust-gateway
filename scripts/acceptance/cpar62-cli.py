#!/usr/bin/env python3
"""Opt-in #62 actual, unmodified CLI driver. Synthetic accounts; loopback only."""
import json
import os
from pathlib import Path
import subprocess
import sys
import queue
import threading
import time
import re
import signal
import hashlib
import tempfile
import argparse
import gzip


CODEX_ROOT = Path("/opt/homebrew/lib/node_modules/@openai/codex")
PI_ROOT = Path("/opt/homebrew/lib/node_modules/@earendil-works/pi-coding-agent")


LOOPBACK_PROFILE = """(version 1)
(allow default)
(deny network-outbound)
(deny network-inbound)
(allow network-outbound (remote ip "localhost:*"))
(allow network-inbound (local ip "localhost:*"))
"""


def client_fence(root):
    # The test binary, driver, actual clients and their tools inherit one outer OS fence.
    # The failed nested-profile experiment is retained separately in the report.
    probe = """import socket,json,errno,os,sys
results=[]
for family,host in [(socket.AF_INET,'192.0.2.1'),(socket.AF_INET6,'2001:db8::1')]:
 s=socket.socket(family);s.settimeout(1)
 try:s.connect((host,443));raise RuntimeError('non-loopback connection permitted')
 except OSError as e:
  results.append({'host':host,'errno':e.errno});assert e.errno==errno.EPERM
 finally:s.close()
try:
 fd=os.open(sys.argv[1],os.O_WRONLY);os.close(fd);raise RuntimeError('write escaped temporary directories')
except OSError as e:assert e.errno==errno.EPERM;results.append({'outside_write_errno':e.errno})
try:
 fd=os.open('/Users/huangrui',os.O_RDONLY);os.close(fd);raise RuntimeError('real user directory readable')
except OSError as e:assert e.errno==errno.EPERM;results.append({'real_home_read_errno':e.errno})
print(json.dumps(results))
"""
    repository = Path(__file__).resolve().parents[2]
    result = subprocess.run(["/usr/bin/python3", "-c", probe, str(repository / "AGENTS.md")],
                            cwd=root / "work", capture_output=True, text=True, timeout=5)
    (root / "isolation.json").write_text(json.dumps({"exit": result.returncode,
        "stdout": result.stdout, "stderr": result.stderr}, indent=2) + "\n")
    if result.returncode:
        raise RuntimeError("OS isolation probe failed; CLI was not started")
    return []


def stop_process(process):
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)


def sha_file(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def installed_manifest():
    """Freeze install bytes, modes and links; no user config or credential files are read."""
    records = {}
    for package in (CODEX_ROOT, PI_ROOT):
        for path in sorted(package.rglob("*")):
            if path.is_symlink():
                records[str(path)] = {"link": os.readlink(path), "mode": path.lstat().st_mode & 0o777}
            elif path.is_file():
                records[str(path)] = {"sha256": sha_file(path),
                                      "mode": path.stat().st_mode & 0o777}
    for path in (Path("/opt/homebrew/bin/node").resolve(), Path("/opt/homebrew/bin/codex"),
                 Path("/opt/homebrew/bin/pi")):
        records[str(path)] = {"realpath": str(path.resolve()),
                             "sha256": sha_file(path), "mode": path.lstat().st_mode & 0o777,
                             "link": os.readlink(path) if path.is_symlink() else None}
    return records


def codex_success_checks(root):
    """A required success gate is separate from the historical expected-400 preparation oracle.

    Missing observations fail closed. A future positive CLI scenario must supply every receipt;
    this gate alone does not qualify a new client mode or #61 encrypted retention.
    """
    directory = root / "codex-standard"
    receipt = json.loads((directory / "client.json").read_text())
    observations = json.loads((directory / "observations.json").read_text())
    ingress = [row for row in json.loads((directory / "ingress.json").read_text()) if not row["probe"]]
    upstream = json.loads((directory / "upstream.json").read_text())
    events = [json.loads(line) for line in receipt["stdout"].splitlines() if line.strip()]
    history = upstream[-1].get("input", []) if upstream else []
    history = [row for row in history if isinstance(row, dict)] if isinstance(history, list) else []
    calls = [row for row in history if isinstance(row, dict) and row.get("type") == "function_call"]
    results = [row for row in history if isinstance(row, dict) and row.get("type") == "function_call_output"]
    correlated = len(calls) == 2 and len(results) == 2
    if correlated:
        call_ids = [row.get("call_id") for row in calls]
        correlated = len(set(call_ids)) == 2 and all(call_ids) and all(
            sum(result.get("call_id") == call_id for result in results) == 1
            and history.index(calls[index]) < next(
                position for position, result in enumerate(history)
                if result.get("type") == "function_call_output" and result.get("call_id") == call_id)
            for index, call_id in enumerate(call_ids))
    accounting = observations["accounting"]
    checks = {
        "cli_exit_zero": receipt["exit"] == 0,
        "answer_tool_task_and_follow_up": sum(row.get("type") == "turn.completed" for row in events) == 3
            and not any(row.get("type") in ("turn.failed", "error") for row in events),
        "five_incremental_public_responses": len(ingress) == 5
            and all(row["status"] == 200 and row["chunks"] > 2 for row in ingress),
        "independent_receives_and_exact_accounting": len(upstream) == 5
            and all(accounting.get(name) == 5 for name in ("client_requests", "attempts", "usage", "ledger")),
        "two_correlated_tool_results": correlated,
        "actual_tool_file_result": (directory / "work/result.txt").exists()
            and (directory / "work/result.txt").read_text() == "42\n",
    }
    return {"status": "PASS" if all(checks.values()) else "FAIL", "checks": checks}


def run_qualification(output, require_prompt_release=False, require_codex_success=False):
    if sys.platform != "darwin":
        raise RuntimeError("this fixed-client runner requires macOS Seatbelt")
    root = Path(output).resolve()
    if not root.is_relative_to(Path(tempfile.gettempdir()).resolve()):
        raise ValueError("output must be a new directory beneath the OS temporary directory")
    root.mkdir(mode=0o700)
    repository = Path(__file__).resolve().parents[2]
    before = installed_manifest()
    (root / "installed-files.json.gz").write_bytes(gzip.compress(json.dumps(before, sort_keys=True).encode(), mtime=0))
    build = subprocess.run(["cargo", "test", "--locked", "-p", "gateway", "--bin", "gateway",
                            "actual_cli_public_build_qualification", "--no-run", "--message-format=json"],
                           cwd=repository, capture_output=True, text=True, timeout=300)
    (root / "build.log").write_text(build.stdout + build.stderr)
    if build.returncode:
        raise RuntimeError("qualification binary build failed; see build.log")
    binaries = [row["executable"] for line in build.stdout.splitlines()
                if (row := json.loads(line)).get("reason") == "compiler-artifact"
                and row.get("executable") and row.get("profile", {}).get("test")]
    if len(binaries) != 1:
        raise RuntimeError("expected one exact gateway test executable")
    binary = Path(binaries[0]).resolve()
    profile = root / "isolated.sb"
    profile.write_text(LOOPBACK_PROFILE +
        '\n(deny file-read* (subpath "/Users") (subpath "/Volumes/Asgard/Agentprojects"))\n' +
        f'(allow file-read* (literal {json.dumps(str(binary))}) (literal {json.dumps(str(Path(__file__).resolve()))}))\n' +
        '(deny file-write*)\n' +
        f'(allow file-write* (subpath {json.dumps(str(Path(tempfile.gettempdir()).resolve()))}) (literal "/dev/null"))\n')
    probe_home = root / "identity-home"
    probe_home.mkdir()
    env = {"PATH": "/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin", "HOME": str(probe_home),
           "CODEX_HOME": str(probe_home), "PI_CODING_AGENT_DIR": str(probe_home),
           "PI_OFFLINE": "1", "PI_TELEMETRY": "0", "TERM": "dumb"}
    versions = {}
    for name, command, expected in [
        ("codex", ["/opt/homebrew/bin/codex", "--version"], "codex-cli 0.161.0"),
        ("pi-shell", ["/opt/homebrew/bin/pi", "--version"], "0.84.3"),
        ("pi-declared-bin", ["/opt/homebrew/bin/node", str(PI_ROOT / "dist/bundle/cli.js"), "--version"], "0.84.3"),
        ("node", ["/opt/homebrew/bin/node", "--version"], "v26.8.2"),
    ]:
        result = subprocess.run(["/usr/bin/sandbox-exec", "-f", str(profile), *command],
                                cwd=root, env=env, capture_output=True, text=True, timeout=20)
        versions[name] = {"command": command, "exit": result.returncode,
                          "stdout": result.stdout, "stderr": result.stderr}
        if result.returncode or result.stdout.strip() != expected:
            raise RuntimeError(f"{name} identity drift; fixed-client qualification stopped")
    packages = {}
    for path in PI_ROOT.rglob("package.json"):
        value = json.loads(path.read_text())
        if value.get("name") and value.get("version"):
            packages[str(path)] = {"name": value["name"], "version": value["version"]}
    identity = {"versions": versions, "packages": packages,
        "node": str(Path("/opt/homebrew/bin/node").resolve()),
        "codex_actual_native": str(CODEX_ROOT / "vendor/aarch64-apple-darwin/bin/codex"),
        "pi_actual_entry": str(Path("/opt/homebrew/bin/pi").resolve()),
        "pi_declared_bin": json.loads((PI_ROOT / "package.json").read_text())["bin"],
        "only_my_pi": {"link": os.readlink("/Users/huangrui/.local/bin/pi"),
                       "realpath": str(Path("/Users/huangrui/.local/bin/pi").resolve()), "executed": False},
        "install_manifest_sha256": hashlib.sha256((root / "installed-files.json.gz").read_bytes()).hexdigest(),
        "source_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repository, text=True).strip(),
        "test_binary_sha256": sha_file(binary)}
    (root / "client-identity.json").write_text(json.dumps(identity, indent=2) + "\n")
    env["CPAR62_RUN"] = str(root)
    if require_prompt_release:
        env["CPAR62_REQUIRE_PROMPT_RELEASE"] = "1"
    command = ["/usr/bin/sandbox-exec", "-f", str(profile), str(binary),
               "actual_cli_public_build_qualification", "--ignored", "--nocapture"]
    with (root / "qualification.log").open("w") as log:
        result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=160)
    unchanged = before == installed_manifest()
    cancel_path = root / "pi-cancel/cancel-diagnostic.json"
    cancel = json.loads(cancel_path.read_text()) if cancel_path.exists() else None
    codex_gate = codex_success_checks(root) if (root / "codex-standard/observations.json").exists() else {
        "status": "FAIL", "reason": "missing actual Codex observations"}
    (root / "run.json").write_text(json.dumps({"command": command, "exit": result.returncode,
        "runner_status": "FAIL" if result.returncode or not unchanged
            or require_codex_success and codex_gate["status"] != "PASS" else "PASS",
        "require_prompt_release": require_prompt_release, "cancellation": cancel,
        "require_codex_success": require_codex_success, "codex_success_gate": codex_gate,
        "installs_unchanged": unchanged, "claim": "local controlled independent preparation; #62 remains blocked"}, indent=2) + "\n")
    if result.returncode or not unchanged:
        raise RuntimeError("qualification failed or installed bytes drifted; see receipts")
    if require_codex_success and codex_gate["status"] != "PASS":
        raise RuntimeError("Codex success gate FAILED; expected-400 preparation is not compatibility; see run.json")
    print(f"Preparation oracles PASS; Codex BLOCKED; Pi cancellation latency is recorded separately; #62 OPEN/BLOCKED. Receipts: {root}")


def client(name, scenario, directory, base):
    root = Path(directory).resolve(strict=True)
    for item in ("home", "codex", "agent", "work", "tmp"):
        (root / item).mkdir(mode=0o700)
    (root / "work/seed.txt").write_text("41\n")
    env = {
        "PATH": "/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        "HOME": str(root / "home"),
        "CODEX_HOME": str(root / "codex"),
        "PI_CODING_AGENT_DIR": str(root / "agent"),
        "PI_OFFLINE": "1", "PI_TELEMETRY": "0",
        "TMPDIR": str(root / "tmp"), "TERM": "dumb",
        "CPAR62_TEST_KEY": os.environ["CPAR62_TEST_KEY"],
        "NODE_DEBUG": "esm",
    }
    prompt = "Read seed.txt using a tool. Then use a second related tool to add one to its value and save result.txt. Report the result."
    if name == "codex":
        (root / "codex/config.toml").write_text(
            'model = "grok-4.5"\nmodel_provider = "cpar62"\n'
            '[model_providers.cpar62]\nname = "CPAR controlled Build"\n'
            f'base_url = {json.dumps(base)}\n'
            'env_key = "CPAR62_TEST_KEY"\nwire_api = "responses"\n'
            'requires_openai_auth = false\n'
        )
        command = ["/opt/homebrew/bin/codex", "exec", "--skip-git-repo-check",
                   "--json", "--color", "never", "--sandbox", "workspace-write", prompt]
    elif name == "pi":
        (root / "agent/models.json").write_text(json.dumps({"providers": {"cpar62": {
            "baseUrl": base, "api": "openai-responses", "apiKey": env["CPAR62_TEST_KEY"],
            "models": [{"id": "grok-4.5", "reasoning": True}],
        }}}))
        command = ["/opt/homebrew/bin/pi", "--offline", "--no-approve", "--no-extensions",
                   "--no-skills", "--no-prompt-templates", "--no-themes", "--no-context-files",
                   "--provider", "cpar62", "--model", "grok-4.5", "--mode", "rpc"]
    else:
        raise ValueError("unknown client")
    command = client_fence(root) + command
    if name == "codex":
        process = subprocess.Popen(command, env=env, cwd=root / "work", stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, start_new_session=True)
        try:
            output, errors = process.communicate(timeout=65)
        finally:
            stop_process(process)
        receipt = {"command": command, "exit": process.returncode,
                   "stdout": output, "stderr": errors}
    else:
        process = subprocess.Popen(command, env=env, cwd=root / "work", stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, start_new_session=True)
        messages = queue.Queue()
        output, errors, outcomes = [], [], []

        def read_output():
            for line in process.stdout:
                output.append(line)
                messages.put(json.loads(line))
            messages.put(None)

        def read_errors():
            errors.extend(process.stderr)

        readers = [threading.Thread(target=read_output, daemon=True),
                   threading.Thread(target=read_errors, daemon=True)]
        for reader in readers:
            reader.start()
        abort_sent = False
        try:
            questions = (["Tell me you are ready to inspect seed.txt.", prompt,
                          "What value did the two tools produce, and how was it derived?"]
                         if scenario == "standard" else ["Report progress."])
            for question in questions:
                process.stdin.write(json.dumps({"type": "prompt", "message": question}) + "\n")
                process.stdin.flush()
                deadline = time.monotonic() + 20
                while True:
                    event = messages.get(timeout=max(0.01, deadline - time.monotonic()))
                    if event is None:
                        raise RuntimeError("Pi RPC exited before agent_end")
                    delta = event.get("assistantMessageEvent", {})
                    if scenario == "cancel" and not abort_sent and delta.get("type") == "text_delta":
                        (root / "cancel-ready").write_text("observed actual CLI text delta\n")
                        limit = time.monotonic() + 5
                        while not (root / "cancel-proceed").exists():
                            if time.monotonic() > limit:
                                raise TimeoutError("serving lease cancellation barrier")
                            time.sleep(0.01)
                        process.stdin.write('{"type":"abort"}\n')
                        process.stdin.flush()
                        abort_sent = True
                    if event.get("type") == "agent_end":
                        assistant = [m for m in event.get("messages", []) if m.get("role") == "assistant"]
                        stop = assistant[-1].get("stopReason") if assistant else "missing"
                        outcomes.append(stop)
                        break
                if stop in ("error", "aborted", "missing"):
                    if scenario == "cancel":
                        (root / "abort-complete").write_text(json.dumps({"rpc_alive": process.poll() is None, "pid": process.pid}) + "\n")
                        limit = time.monotonic() + 25
                        while not (root / "cancel-observed").exists():
                            if process.poll() is not None:
                                raise RuntimeError("Pi RPC exited during cancellation observation")
                            if time.monotonic() > limit:
                                raise TimeoutError("live CLI cancellation observation barrier")
                            time.sleep(0.01)
                        alive = {"rpc_alive": process.poll() is None, "pid": process.pid}
                        (root / "cancel-client-alive").write_text(json.dumps(alive) + "\n")
                        if not alive["rpc_alive"]:
                            raise RuntimeError("Pi RPC exited before final cancellation liveness proof")
                    break
        finally:
            stop_process(process)
            for reader in readers:
                reader.join(timeout=2)
        receipt = {"command": command, "shutdown_exit": process.returncode, "turn_outcomes": outcomes,
                   "stdout": "".join(output), "stderr": "".join(errors)}
    modules = sorted(set(re.findall(r"file:///opt/homebrew/[^\s'\"<>)]*", receipt["stderr"])))
    (root / "loaded-esm.json").write_text(json.dumps(modules, indent=2) + "\n")
    stderr = receipt["stderr"]
    (root / "client.stderr.log").write_text(stderr)
    receipt["stderr_sha256"] = hashlib.sha256(stderr.encode()).hexdigest()
    receipt["stderr_bytes"] = len(stderr.encode())
    receipt["stderr"] = "\n".join(line for line in stderr.splitlines()
        if line.startswith(("WARNING:", "Error:", "Reading additional", "Traceback")))
    (root / "client.json").write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    if len(sys.argv) == 6 and sys.argv[1] == "client":
        client(*sys.argv[2:])
    else:
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--output", required=True, help="new absolute directory beneath the OS temporary directory")
        parser.add_argument("--require-prompt-release", action="store_true",
                            help="also require the one-second diagnostic probe; this is not a product SLA")
        parser.add_argument("--require-codex-success", action="store_true",
                            help="fail unless actual Codex completes the answer, two tool cycles and follow-up; expected HTTP400 is not success")
        args = parser.parse_args()
        run_qualification(args.output, args.require_prompt_release, args.require_codex_success)
