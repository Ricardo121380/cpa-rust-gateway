"""Root prepares private namespaces, then drops to the actual production service UID.

Invoked only under unshare by the task operator. No production state or credential
path is mounted in the process root. All host mounts are made private first.
"""
import json
import os
import pathlib
import subprocess
import sys

TASK = pathlib.Path("/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884")
spec = json.loads(sys.argv[1])
assert os.geteuid() == 0
assert all(os.readlink("/proc/self/ns/"+name) != original for name, original in spec["host_namespaces"].items())
root = pathlib.Path(spec["rootfs"])
assert root.parent == TASK and root.is_dir() and not root.is_symlink()

def run(*args):
    subprocess.run(args, check=True, stdout=subprocess.DEVNULL)

run("mount", "--make-rprivate", "/")
run("mount", "-t", "tmpfs", "-o", "mode=755,nosuid", "task-private-root", str(root))

def bind(source, destination, readonly=False):
    source = pathlib.Path(source)
    destination = root/destination.lstrip("/")
    destination.parent.mkdir(parents=True, exist_ok=True)
    if source.is_dir():
        destination.mkdir(exist_ok=True)
    else:
        destination.touch(exist_ok=True)
    run("mount", "--bind", str(source), str(destination))
    if readonly:
        run("mount", "-o", "remount,bind,ro,nosuid", str(destination))

bind("/usr", "/usr", True)
for link, target in [("bin", "usr/bin"), ("sbin", "usr/sbin"), ("lib", "usr/lib"), ("lib64", "usr/lib64")]:
    (root/link).symlink_to(target)
for directory in ["proc", "dev", "tmp", "etc/ssl"]:
    (root/directory).mkdir(parents=True, exist_ok=True)
run("mount", "-t", "proc", "-o", "nosuid,nodev,noexec", "proc", str(root/"proc"))
run("mount", "-t", "tmpfs", "-o", "mode=755,nosuid", "task-private-dev", str(root/"dev"))
for device in ["null", "zero", "random", "urandom"]:
    bind("/dev/"+device, "/dev/"+device)
(root/"dev/fd").symlink_to("/proc/self/fd")
run("mount", "-t", "tmpfs", "-o", "mode=1777,nosuid,nodev", "task-private-tmp", str(root/"tmp"))
bind("/etc/ssl/certs", "/etc/ssl/certs", True)
bind("/etc/ssl/openssl.cnf", "/etc/ssl/openssl.cnf", True)
for item in spec["bindings"]:
    source = pathlib.Path(item["source"])
    assert source.resolve().is_relative_to(TASK.resolve())
    bind(source, item["destination"], item["readonly"])
run("ip", "link", "set", "lo", "up")
assert not pathlib.Path("/proc/net/route").read_text().splitlines()[1:]

os.chroot(root)
os.chdir(spec["cwd"])
environment = {"PATH": "/usr/bin:/bin", "PYTHONDONTWRITEBYTECODE": "1", **spec["environment"]}
command = spec["command"]
if spec["service_identity"]:
    command = ["/usr/bin/setpriv", "--reuid", "995", "--regid", "986", "--clear-groups", "--inh-caps=-all", "--ambient-caps=-all", "--bounding-set=-all", "--no-new-privs", "--"]+command
result = subprocess.run(command, env=environment)
sys.exit(result.returncode)
