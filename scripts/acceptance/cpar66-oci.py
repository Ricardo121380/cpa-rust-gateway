#!/usr/bin/env python3
"""Qualify an unchanged #66 OCI image using a separate task-owned Docker controller."""
import argparse
import gzip
import hashlib
import http.client
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import sqlite3
import struct
import subprocess
import tarfile
import time
import urllib.parse

spec = importlib.util.spec_from_file_location("stock66", Path(__file__).with_name("cpar66-reader-serve.py"))
stock = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stock)


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def archive_binding(artifact, docker_archive):
    """Verify the whole P12 set and both representations before touching Docker."""
    manifest = json.loads((artifact / "artifact-manifest.json").read_bytes())
    files = manifest["files"]
    names = [r["name"] for r in files]
    expected = {"gateway-aarch64-unknown-linux-gnu", "gateway-build-metadata.json",
                "gateway-image.oci.tar", "gateway-sbom.cdx.json", "signing-identity.json"}
    assert len(names) == len(expected) and set(names) == expected, "incomplete P12 payload set"
    assert {p.name for p in artifact.iterdir()} == expected | {"artifact-manifest.json"}, "unexpected P12 files"
    for row in files:
        path = artifact / row["name"]
        assert path.is_file() and not path.is_symlink(), "non-regular payload"
        assert path.stat().st_size == row["bytes"] and stock.sha(path) == row["sha256"], "payload mismatch"

    def members(path):
        with tarfile.open(path) as archive:
            rows = archive.getmembers()
            assert len({r.name for r in rows}) == len(rows), "duplicate archive member"
            assert all(r.isfile() or r.isdir() for r in rows), "unsafe archive member"
            return {r.name: archive.extractfile(r).read() for r in rows if r.isfile()}

    oci = members(artifact / "gateway-image.oci.tar")
    docker = members(docker_archive)
    assert set(docker) == set(oci) | {"manifest.json"}, "Docker/OCI member mismatch"
    assert all(docker[name] == value for name, value in oci.items()), "Docker/OCI bytes differ"
    index = json.loads(oci["index.json"])
    assert index["schemaVersion"] == 2 and len(index["manifests"]) == 1, "single platform required"

    def blob(descriptor):
        data = oci["blobs/" + descriptor["digest"].replace(":", "/")]
        assert digest(data) == descriptor["digest"] and len(data) == descriptor["size"], "OCI descriptor mismatch"
        return data

    descriptor = index["manifests"][0]
    image_manifest = json.loads(blob(descriptor))
    config = json.loads(blob(image_manifest["config"]))
    assert config["architecture"] == "arm64" and config["os"] == "linux", "wrong platform"
    cfg = config["config"]
    assert cfg["User"] == "65532:65532" and cfg["Entrypoint"] == ["/usr/local/bin/gateway"], "wrong runtime identity"
    assert cfg["WorkingDir"] == "/nonexistent", "wrong working directory"
    assert cfg["Labels"]["org.opencontainers.image.revision"] == manifest["revision"], "revision mismatch"
    layers, gateway = [], None
    for row in image_manifest["layers"]:
        assert row["mediaType"] == "application/vnd.oci.image.layer.v1.tar+gzip", "unsupported layer"
        raw = gzip.decompress(blob(row))
        layers.append({"digest": row["digest"], "bytes": row["size"], "diff_id": digest(raw)})
        with tarfile.open(fileobj=io.BytesIO(raw)) as layer:
            for member in layer:
                name = member.name.removeprefix("./")
                if name == "usr/local/bin/gateway":
                    assert member.isfile(), "gateway is not a regular image file"
                    gateway = digest(layer.extractfile(member).read())
                assert name not in ("usr/local/bin/.wh.gateway", "usr/local/bin/.wh..wh..opq"), "gateway whiteout"
    assert [r["diff_id"] for r in layers] == config["rootfs"]["diff_ids"], "rootfs mismatch"
    binary = next(r for r in files if r["name"].startswith("gateway-aarch64"))
    assert gateway == "sha256:" + binary["sha256"], "image executable differs from binary"
    bridge = json.loads(docker["manifest.json"])
    assert len(bridge) == 1, "Docker archive has multiple images"
    assert bridge[0]["Config"] == "blobs/" + image_manifest["config"]["digest"].replace(":", "/"), "Docker config mismatch"
    assert bridge[0]["Layers"] == ["blobs/" + r["digest"].replace(":", "/") for r in layers], "Docker layer order mismatch"
    return {"status": "PASS", "artifact_source": manifest["revision"],
            "artifact_manifest_sha256": stock.sha(artifact / "artifact-manifest.json"),
            "oci_archive_sha256": stock.sha(artifact / "gateway-image.oci.tar"),
            "docker_archive_sha256": stock.sha(docker_archive), "config_digest": image_manifest["config"]["digest"],
            "manifest_digest": descriptor["digest"], "config": config, "layers": layers,
            "image_gateway_digest": gateway, "signed": False}


class Engine:
    """Only the local Engine operations used by this bounded acceptance."""
    def call(self, method, path, body=None, raw=False, missing=False):
        conn = http.client.HTTPConnection("localhost", timeout=65)
        conn.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        conn.sock.settimeout(65)
        conn.sock.connect("/var/run/docker.sock")
        try:
            conn.request(method, path, body=None if body is None else json.dumps(body),
                         headers={"Content-Type": "application/json"})
            response = conn.getresponse()
            data = response.read()
            if missing and response.status == 404:
                return None
            if response.status >= 300:
                raise RuntimeError("Docker API " + method + " " + path + " HTTP " + str(response.status))
            return data if raw else json.loads(data) if data else None
        finally:
            conn.close()

    def inspect(self, container, missing=False):
        return self.call("GET", "/containers/" + container + "/json", missing=missing)

    def logs(self, container):
        data = self.call("GET", "/containers/" + container + "/logs?stdout=1&stderr=1", raw=True)
        streams = {1: bytearray(), 2: bytearray()}
        while data:
            assert len(data) >= 8 and data[0] in streams, "malformed Docker stream"
            size = struct.unpack(">I", data[4:8])[0]
            assert len(data) >= 8 + size, "truncated Docker stream"
            streams[data[0]].extend(data[8:8 + size])
            data = data[8 + size:]
        return bytes(streams[1]), bytes(streams[2])


class Matrix(stock.Matrix):
    def __init__(self, args):
        super().__init__(args)
        self.engine, self.created = Engine(), []
        self.report.update(scope="unchanged_delivery_oci_serve_reader_container_adoption_exit",
                           image=args.image, helper_image=args.helper_image, containers=[],
                           controller=socket.gethostname(), script_sha256=stock.sha(Path(__file__)),
                           stock_oracle_sha256=stock.sha(Path(stock.__file__)))

    def ownership(self):
        for parent in (self.state, self.credentials, self.keys, self.private, self.root / "wrong-keys"):
            if parent.exists():
                for path in [parent, *parent.rglob("*")]:
                    os.chown(path, 65532, 65532)
                    os.chmod(path, 0o700 if path.is_dir() else 0o600)

    def mounts(self, state_readonly=False, empty_state=False):
        return [{"Type": "volume", "Source": self.args.volume, "Target": "/" + name,
                 "ReadOnly": name in ("credentials", "keys", "wrong-keys") or name == "state" and state_readonly,
                 "VolumeOptions": {"NoCopy": True, "Subpath": "run/" + ("readonly-state" if name == "state" and empty_state else name)}}
                for name in ("state", "credentials", "keys", "private", "wrong-keys")]

    def create(self, label, command, state_readonly=False, empty_state=False):
        self.ownership()
        name = self.args.controller + "-" + label + "-" + secrets.token_hex(3)
        config = {"Image": self.args.image, "Cmd": command,
                  "Labels": {"cpar66.owned": self.args.controller},
                  "HostConfig": {"NetworkMode": "container:" + self.args.controller,
                                 "ReadonlyRootfs": True, "CapDrop": ["ALL"], "PidsLimit": 128,
                                 "SecurityOpt": ["no-new-privileges"], "Mounts": self.mounts(state_readonly, empty_state)}}
        cid = self.engine.call("POST", "/containers/create?name=" + name, config)["Id"]
        self.created.append(cid)
        row = self.engine.inspect(cid)
        self.check(label + " image default User Entrypoint WorkingDir retained", row["Image"] == self.args.image
                   and row["Config"]["User"] == "65532:65532"
                   and row["Config"]["Entrypoint"] == ["/usr/local/bin/gateway"]
                   and row["Config"]["WorkingDir"] == "/nonexistent"
                   and row["Path"] == "/usr/local/bin/gateway")
        self.report["containers"].append({"id": cid, "label": label, "create": config,
                                          "runtime_config": row["Config"]})
        return cid

    def observe(self, cid, label):
        row = self.engine.inspect(cid)
        pid = row["State"]["Pid"]
        proc = Path("/proc") / str(pid)
        status = (proc / "status").read_text()
        fields = dict(line.split(":", 1) for line in status.splitlines() if ":" in line)
        actual = {"container": cid, "label": label, "pid": pid,
                  "start_time": (proc / "stat").read_text().split()[21],
                  "executable": os.readlink(proc / "exe"), "cwd": os.readlink(proc / "cwd"),
                  "exe_sha256": stock.sha(proc / "exe"),
                  "status": {k: fields[k].strip() for k in ("Uid", "Gid", "CapEff", "CapPrm", "CapBnd", "NoNewPrivs", "NSpid")},
                  "net_namespace": os.readlink(proc / "ns/net"), "pid_namespace": os.readlink(proc / "ns/pid"),
                  "mountinfo": (proc / "mountinfo").read_text(), "route": (proc / "net/route").read_text(),
                  "mounts": row["Mounts"]}
        self.check(label + " target executable and default cwd", actual["executable"] == "/usr/local/bin/gateway"
                   and actual["exe_sha256"] == self.report["binary_sha256"] and actual["cwd"] == "/nonexistent")
        self.check(label + " target UID GID capabilities NoNewPrivs", fields["Uid"].split() == ["65532"] * 4
                   and fields["Gid"].split() == ["65532"] * 4 and fields["NoNewPrivs"].strip() == "1"
                   and all(int(fields[k], 16) == 0 for k in ("CapEff", "CapPrm", "CapBnd")))
        mount_rows = [line.split() for line in actual["mountinfo"].splitlines()]
        self.check(label + " target root readonly and explicit private mounts",
                   any(r[4] == "/" and "ro" in r[5].split(",") for r in mount_rows)
                   and {m["Destination"] for m in row["Mounts"]} == {"/state", "/credentials", "/keys", "/private", "/wrong-keys"}
                   and all(not m["RW"] for m in row["Mounts"] if m["Destination"] in ("/credentials", "/keys", "/wrong-keys")))
        self.check(label + " target owned network and separate PID namespace", actual["net_namespace"] == os.readlink("/proc/self/ns/net")
                   and actual["pid_namespace"] != os.readlink("/proc/self/ns/pid") and len(actual["route"].splitlines()) == 1)
        self.report["processes"].append(actual)
        return actual

    def isolation(self):
        for tool, ports in (("iptables", (19180, 19181, 19443)), ("ip6tables", ())):
            if ports:
                subprocess.run([tool, "-A", "OUTPUT", "-m", "conntrack", "--ctstate", "ESTABLISHED,RELATED", "-j", "ACCEPT"], check=True)
                for port in ports:
                    subprocess.run([tool, "-A", "OUTPUT", "-o", "lo", "-d", "127.0.0.1", "-p", "tcp", "--dport", str(port), "-j", "ACCEPT"], check=True)
            subprocess.run([tool, "-A", "OUTPUT", "-j", "REJECT"], check=True)
        self.report["firewall"] = {tool: subprocess.check_output([tool + "-save"]).decode() for tool in ("iptables", "ip6tables")}
        (self.root / "wrong-keys").mkdir(mode=0o700)

    def serve_command(self):
        return ["serve", "--state-dir", "/state", "--credential-dir", "/credentials", "--master-key-dir", "/keys",
                "--active-master-key-version", "1", "--data-listen", "127.0.0.1:19180",
                "--management-listen", "127.0.0.1:19181", "--background-provider-tasks", "disabled"]

    def ready(self, cid, label):
        def predicate():
            if not self.engine.inspect(cid)["State"]["Running"]:
                raise AssertionError("target serve exited before readiness: " + label)
            try:
                return self.http(19180, "/healthz") == (200, b'{"status":"ok"}')
            except OSError:
                return False
        stock.base.wait_for(predicate, "OCI readiness")
        self.observe(cid, label)
        self.check(label + " schema38", self.schema() == 38)
        self.check(label + " listeners and management authentication", self.http(19180, "/", method="HEAD")[0] == 200
                   and self.http(19180, "/admin/config-versions")[0] == 404
                   and self.http(19181, "/admin/config-versions")[0] == 404
                   and self.http(19181, "/admin/config-versions", "mgmt_wrong_synthetic")[0] == 404
                   and isinstance(self.api("GET", "/admin/config-versions"), list)
                   and self.http(19180, "/v1/models")[0] == 401)

    def start(self, label):
        assert self.process is None
        if label == "metadata-new-process-restart":
            previous = self.report["processes"][-1]
            before = self.evidence_snapshot()
            self.process = previous["container"]
            self.engine.call("POST", "/containers/" + self.process + "/start")
            self.ready(self.process, "metadata-same-container-restart")
            current = self.report["processes"][-1]
            self.check("same container restart has new process identity", current["container"] == previous["container"]
                       and (current["pid"], current["start_time"]) != (previous["pid"], previous["start_time"]))
            self.check("same container restart adopts marker and encrypted rows", before == self.evidence_snapshot()
                       and "cpar66-metadata" in [r["id"] for r in self.api("GET", "/admin/config-versions")])
            self.stop()
        self.process = self.create(label, self.serve_command())
        self.engine.call("POST", "/containers/" + self.process + "/start")
        self.ready(self.process, label)
        if label == "metadata-new-process-restart":
            current = self.report["processes"][-1]
            self.check("new container adoption has distinct container and process identities", current["container"] != previous["container"]
                       and (current["pid"], current["start_time"]) != (previous["pid"], previous["start_time"]))
        if label == "empty-state":
            self.target_probes(self.process)

    def exec_probe(self, cid, script):
        eid = self.engine.call("POST", "/containers/" + cid + "/exec", {
            "AttachStdout": True, "AttachStderr": True, "Cmd": ["/bin/bash", "-c", script]})["Id"]
        self.engine.call("POST", "/exec/" + eid + "/start", {"Detach": False, "Tty": False}, raw=True)
        row = self.engine.call("GET", "/exec/" + eid + "/json")
        return row["ExitCode"]

    def target_probes(self, cid):
        unrelated = socket.socket()
        unrelated.bind(("127.0.0.1", 19999))
        unrelated.listen(1)
        unrelated.settimeout(.2)
        try:
            for host, port in (("192.0.2.1", 443), ("2001:db8::1", 443), ("127.0.0.1", 19999)):
                code = self.exec_probe(cid, "exec 3<>/dev/tcp/" + host + "/" + str(port))
                self.report["network"].append({"container": cid, "host": host, "port": port, "exit": code})
                self.check("target network denies " + host + ":" + str(port), code != 0)
            try:
                unrelated.accept()
                raise AssertionError("unrelated live receiver accepted")
            except socket.timeout:
                pass
        finally:
            unrelated.close()
        self.check("target permits owned receiver only", self.exec_probe(cid,
                   "exec 3<>/dev/tcp/127.0.0.1/19443; printf 'GET /cpar66-envelope-probe HTTP/1.0\\r\\n\\r\\n' >&3; cat <&3 >/dev/null") == 0)
        self.check("target rootfs write denied", self.exec_probe(cid, "printf denied >/etc/cpar66-forbidden-write") != 0)

    def stop(self):
        cid, process = self.process, self.report["processes"][-1]
        started = time.time()
        self.engine.call("POST", "/containers/" + cid + "/stop?t=45")
        elapsed = time.time() - started
        row = self.engine.inspect(cid)
        stdout, stderr = self.engine.logs(cid)
        (self.out / (process["label"] + ".log")).write_bytes(stdout + stderr)
        process.update(exit_code=row["State"]["ExitCode"], reaped=not row["State"]["Running"],
                       stop_seconds=elapsed, terminal=row["State"])
        self.process = None
        query = urllib.parse.urlencode({"since": str(int(started)), "until": str(time.time()),
                                         "filters": json.dumps({"container": [cid]})})
        events = self.engine.call("GET", "/events?" + query, raw=True)
        process["stop_events"] = [json.loads(line) for line in events.splitlines() if line]
        signals = [e["Actor"]["Attributes"].get("signal") for e in process["stop_events"] if e.get("Action") == "kill"]
        self.check("container SIGTERM exits normally without timeout or forced kill", process["exit_code"] == 0
                   and not row["State"]["OOMKilled"] and row["State"]["Status"] == "exited" and elapsed < 45
                   and "15" in signals and "9" not in signals and not Path("/proc", str(process["pid"])).exists())
        for port in (19180, 19181):
            with socket.socket() as sock:
                self.check("target listener released " + str(port), sock.connect_ex(("127.0.0.1", port)) != 0)
        with sqlite3.connect(self.db) as db:
            self.check("container writer drained and DB can be adopted", db.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()[0] == 0)

    def execute_reader(self, command):
        translated = []
        for arg in command[1:]:
            for source, target in ((str(self.state), "/state"), (str(self.keys), "/keys"),
                                   (str(self.private), "/private"), (str(self.root / "wrong-keys"), "/wrong-keys")):
                if arg == source or arg.startswith(source + "/"):
                    arg = target + arg[len(source):]
                    break
            translated.append(arg)
        cid = self.create("reader", translated, state_readonly=True)
        self.engine.call("POST", "/containers/" + cid + "/start")
        result = self.engine.call("POST", "/containers/" + cid + "/wait?condition=not-running")
        stdout, stderr = self.engine.logs(cid)
        row = self.engine.inspect(cid)
        self.report["containers"][-1].update(exit=result["StatusCode"], terminal=row["State"])
        self.check("reader container terminated without OOM", row["State"]["Status"] == "exited" and not row["State"]["OOMKilled"])
        infos = [path.stat() for path in self.private.iterdir()]
        self.check("target reader output UID GID and 0600", all(info.st_uid == info.st_gid == 65532
                   and info.st_mode & 0o777 == 0o600 for info in infos))
        return subprocess.CompletedProcess(translated, result["StatusCode"], stdout, stderr)

    def remaining_gateway_processes(self):
        return [cid for cid in self.created if self.engine.inspect(cid)["State"]["Running"]]

    def run(self):
        super().run()
        empty = self.root / "readonly-state"
        empty.mkdir(mode=0o700)
        os.chown(empty, 65532, 65532)
        cid = self.create("readonly-state-negative", self.serve_command(), state_readonly=True, empty_state=True)
        self.engine.call("POST", "/containers/" + cid + "/start")
        result = self.engine.call("POST", "/containers/" + cid + "/wait?condition=not-running")
        self.report["containers"][-1].update(exit=result["StatusCode"], terminal=self.engine.inspect(cid)["State"])
        stdout, stderr = self.engine.logs(cid)
        (self.out / "readonly-state-negative.log").write_bytes(stdout + stderr)
        self.check("target readonly state rejected before readiness", result["StatusCode"] != 0
                   and not self.engine.inspect(cid)["State"]["OOMKilled"])
        for path in (self.state, self.credentials, self.keys, self.private):
            self.check("target private mount directory 0700", path.stat().st_mode & 0o777 == 0o700)

    def cleanup(self):
        for cid in self.created:
            row = self.engine.inspect(cid, missing=True)
            if row is not None:
                if row["State"]["Running"]:
                    self.engine.call("POST", "/containers/" + cid + "/stop?t=45")
                self.engine.call("DELETE", "/containers/" + cid + "?v=1")
            self.check("owned target container removed " + cid[:12], self.engine.inspect(cid, missing=True) is None)
        self.check("ordinary target logs have no private sentinel", all(b"PRIVATE_" not in p.read_bytes() for p in self.out.glob("*.log")))


def qualify(matrix):
    try:
        try:
            matrix.run()
        finally:
            try:
                if matrix.process is not None:
                    matrix.stop()
            finally:
                try:
                    matrix.peer.shutdown()
                    matrix.peer.server_close()
                    matrix.peer_thread.join(timeout=5)
                    with socket.socket() as sock:
                        matrix.check("controlled receiver listener and thread released", not matrix.peer_thread.is_alive()
                                     and sock.connect_ex(("127.0.0.1", 19443)) != 0)
                finally:
                    matrix.cleanup()
        matrix.report["status"] = "PASS_RUNTIME_PENDING_CONTROLLER_CLEANUP"
    except BaseException as error:
        matrix.report.update(status="FAIL", failure_type=type(error).__name__, failure=str(error))
        raise
    finally:
        matrix.save()


def outside(args):
    args.output.mkdir(mode=0o700, parents=False)
    output = args.output.resolve()
    report = {"status": "RUNNING", "commands": [], "product_changed": False}
    docker = ["docker", "--host", args.docker_host]
    prefix = "cpar66-oci-" + secrets.token_hex(6)
    volume, controller = prefix + "-work", prefix + "-controller"
    created_volume = created_controller = False

    def call(command, check=True, timeout=120):
        result = subprocess.run(docker + command, capture_output=True, timeout=timeout)
        report["commands"].append({"argv": docker + command, "exit": result.returncode})
        if check and result.returncode:
            raise RuntimeError("Docker command failed: " + command[0] + "; see command receipt")
        return result

    try:
        binding = archive_binding(args.artifact, args.docker_archive)
        stock.base.dump(output / "image-binding.json", binding)
        assert args.image in (binding["manifest_digest"], binding["config_digest"]), "wrong image/artifact binding"
        call(["load", "--input", str(args.docker_archive.resolve())])
        image = json.loads(call(["image", "inspect", args.image]).stdout)[0]
        assert image["Id"] == args.image and image["RootFS"]["Layers"] == binding["config"]["rootfs"]["diff_ids"], "loaded image mismatch"
        assert image["Config"] == binding["config"]["config"], "loaded runtime config mismatch"
        stock.base.dump(output / "loaded-image.json", image)
        call(["image", "inspect", args.helper_image])
        driver = output / "driver"
        driver.mkdir()
        for name in ("cpar66-oci.py", "cpar66-reader-serve.py", "cpar65-process-recovery.py"):
            shutil.copyfile(Path(__file__).with_name(name), driver / name)
        fixtures = output / "fixtures"
        shutil.copytree(args.fixtures, fixtures)
        for path in [fixtures, *fixtures.rglob("*")]:
            os.chmod(path, 0o700 if path.is_dir() else 0o600)
        report["script_inputs"] = [{"name": p.name, "sha256": stock.sha(p)} for p in driver.iterdir()]
        call(["volume", "create", "--label", "cpar66.owned=" + prefix, volume])
        created_volume = True
        command = ["create", "--name", controller, "--pull=never", "--platform", "linux/arm64", "--network", "none",
                   "--pid=host", "--read-only", "--cap-drop", "ALL", "--cap-add", "NET_ADMIN", "--cap-add", "CHOWN",
                   "--cap-add", "DAC_OVERRIDE", "--cap-add", "FOWNER", "--cap-add", "SYS_PTRACE", "--security-opt", "no-new-privileges",
                   "--pids-limit", "256", "--tmpfs", "/tmp:rw,nosuid,nodev,size=64m",
                   "--mount", "type=bind,src=/var/run/docker.sock,dst=/var/run/docker.sock",
                   "--mount", "type=volume,src=" + volume + ",dst=/work",
                   "--mount", "type=bind,src=" + str(driver) + ",dst=/driver,readonly",
                   "--mount", "type=bind,src=" + str(fixtures) + ",dst=/fixtures,readonly",
                   "--mount", "type=bind,src=" + str(args.artifact.resolve()) + ",dst=/artifact,readonly",
                   args.helper_image, "python3", "/driver/cpar66-oci.py", "--inside", "--artifact", "/artifact",
                   "--fixtures", "/fixtures", "--output", "/work/run", "--image", args.image,
                   "--helper-image", args.helper_image, "--volume", volume, "--controller", controller]
        call(command)
        created_controller = True
        result = call(["start", "--attach", controller], check=False, timeout=600)
        (output / "controller.log").write_bytes(result.stdout + result.stderr)
        call(["cp", controller + ":/work/run", str(output / "runtime")])
        terminal = json.loads(call(["inspect", controller]).stdout)[0]
        stock.base.dump(output / "controller-terminal.json", terminal)
        runtime = json.loads((output / "runtime/qualification.json").read_text())
        assert result.returncode == 0 and terminal["State"]["ExitCode"] == 0 and not terminal["State"]["Running"], "controller failed"
        assert runtime["status"] == "PASS_RUNTIME_PENDING_CONTROLLER_CLEANUP", "runtime failed"
        report["runtime_checks"] = len(runtime["checks"])
    except BaseException as error:
        report.update(status="FAIL", failure_type=type(error).__name__, failure=str(error))
        raise
    finally:
        try:
            if created_controller:
                # Failure paths may have left a target alive: clean only this controller's label.
                ids = call(["ps", "-aq", "--filter", "label=cpar66.owned=" + controller]).stdout.decode().split()
                for cid in ids:
                    call(["stop", "--timeout", "45", cid])
                    call(["rm", cid])
                call(["rm", "--force", controller])
                assert call(["inspect", controller], check=False).returncode != 0, "controller still present"
            if created_volume:
                call(["volume", "rm", volume])
                assert call(["volume", "inspect", volume], check=False).returncode != 0, "private volume still present"
            if report["status"] != "FAIL":
                report["status"] = "PASS_LOCAL_OCI"
        except BaseException as error:
            report.update(status="FAIL", cleanup_failure_type=type(error).__name__, cleanup_failure=str(error))
            raise
        finally:
            stock.base.dump(output / "run.json", report)
    print(json.dumps({"status": report["status"], "checks": report["runtime_checks"], "output": str(output)}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inside", action="store_true", help=argparse.SUPPRESS)
    for name in ("artifact", "fixtures", "output", "docker-archive"):
        parser.add_argument("--" + name, type=Path, required=name != "docker-archive")
    for name in ("image", "helper-image", "docker-host", "controller", "volume"):
        parser.add_argument("--" + name)
    args = parser.parse_args()
    os.umask(0o077)
    assert args.image and re.fullmatch(r"sha256:[0-9a-f]{64}", args.image), "immutable image ID required"
    assert args.helper_image and re.fullmatch(r"sha256:[0-9a-f]{64}", args.helper_image), "immutable helper image ID required"
    if args.inside:
        assert os.uname().machine == "aarch64" and args.volume and args.controller
        args.output.mkdir(mode=0o700)
        qualify(Matrix(args))
    else:
        assert args.docker_host and args.docker_host.startswith("unix://") and args.docker_archive, "explicit local task Engine/archive required"
        outside(args)


if __name__ == "__main__":
    main()
