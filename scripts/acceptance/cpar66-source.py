#!/usr/bin/env python3
"""Freeze #66's actual Git base + overlay; keep source evidence outside the P12 payload set."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import tarfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args])


def paths(root):
    candidates = set(os.fsdecode(p) for p in git(root, "ls-files", "--cached", "--others",
        "--exclude-standard", "-z").split(b"\0") if p)
    deleted = set(os.fsdecode(p) for p in git(root, "ls-files", "--deleted", "-z").split(b"\0") if p)
    return sorted(candidates - deleted)


def read(root, name):
    path = root / name
    info = path.lstat()
    if stat.S_ISLNK(info.st_mode):
        link = os.readlink(path)
        data = os.fsencode(link)
        kind = "symlink"
    elif stat.S_ISREG(info.st_mode):
        link, data, kind = None, path.read_bytes(), "file"
    else:
        raise ValueError("unsupported source input: " + name)
    return {"path": name, "type": kind, "mode": stat.S_IMODE(info.st_mode),
        "uid": info.st_uid, "gid": info.st_gid, "link": link,
        "bytes": len(data), "sha256": digest(data)}, data


def verify(root, manifest):
    binding = json.loads(manifest.with_name("source-binding.json").read_bytes())
    if digest(manifest.read_bytes()) != binding["input_manifest_sha256"]:
        raise ValueError("source manifest mismatch")
    for name in binding.get("deleted_paths", []):
        if os.path.lexists(root / name):
            raise ValueError("deleted source input reappeared: " + name)
    rows = json.loads(manifest.read_bytes())
    for expected in rows:
        actual, _ = read(root, expected["path"])
        # Ownership is normalized in the source archive, while original uid/gid remain evidence.
        for key in ("type", "mode", "link", "bytes", "sha256"):
            if actual[key] != expected[key]:
                raise ValueError("source input mismatch: " + expected["path"] + " (" + key + ")")
    return len(rows)


def freeze(root, output):
    root = root.resolve()
    output = output.resolve()
    if output == root or root in output.parents:
        raise ValueError("source evidence must be outside the checkout")
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    head = git(root, "rev-parse", "HEAD").decode().strip()
    epoch = int(git(root, "show", "-s", "--format=%ct", head))
    names = paths(root)
    rows = []
    with tarfile.open(output / "source.tar", "w", format=tarfile.PAX_FORMAT) as archive:
        for name in names:
            row, data = read(root, name)
            rows.append(row)
            entry = tarfile.TarInfo(name)
            entry.mode, entry.mtime = row["mode"], epoch
            if row["type"] == "symlink":
                entry.type, entry.linkname = tarfile.SYMTYPE, row["link"]
                archive.addfile(entry)
            else:
                entry.size = len(data)
                archive.addfile(entry, io.BytesIO(data))
    manifest = output / "input-manifest.json"
    manifest.write_bytes(encoded(rows))
    dirty = set(os.fsdecode(p) for p in (git(root, "diff", "HEAD", "--name-only", "-z")
        + git(root, "ls-files", "--others", "--exclude-standard", "-z")).split(b"\0") if p)
    git_modes = {}
    for record in git(root, "ls-files", "--stage", "-z").split(b"\0"):
        if record:
            fields, name = record.split(b"\t", 1)
            mode = int(fields.split()[0], 8)
            git_modes[os.fsdecode(name)] = 0o777 if stat.S_ISLNK(mode) else mode & 0o777
    attributes = {row["path"] for row in rows if row["path"] in git_modes
        and row["mode"] != git_modes[row["path"]]}
    overlay = [{**row, "git_status_overlay": row["path"] in dirty,
        "permission_overlay": row["path"] in attributes,
        "purpose": "actual worktree input; preserve content, permission bits and symlink target"}
        for row in rows if row["path"] in dirty | attributes]
    (output / "overlay.json").write_bytes(encoded(overlay))
    deleted = sorted(dirty - set(names))
    binding = {"schema": "cpar66-local-source-v1", "base_commit": head,
        "tree": git(root, "rev-parse", "HEAD^{tree}").decode().strip(),
        "source_form": "base_commit_plus_overlay" if overlay or deleted else "git_tree",
        "source_date_epoch": epoch, "inputs": len(rows), "overlay_files": len(overlay),
        "deleted_paths": deleted,
        "git_status_overlay_files": len(dirty), "permission_overlay_files": len(attributes),
        "input_manifest_sha256": digest(manifest.read_bytes()),
        "overlay_sha256": digest((output / "overlay.json").read_bytes()),
        "source_archive_sha256": digest((output / "source.tar").read_bytes()),
        "cargo_lock_sha256": digest((root / "Cargo.lock").read_bytes()),
        "rust_toolchain": "1.97.1", "target": "aarch64-unknown-linux-gnu",
        "reproducibility": "NOT_RUN", "archive_uid_gid": 0}
    (output / "source-binding.json").write_bytes(encoded(binding))
    verify(root, manifest)
    if names != paths(root) or head != git(root, "rev-parse", "HEAD").decode().strip():
        raise ValueError("checkout changed during source freeze")
    print(json.dumps(binding))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    command = sub.add_parser("freeze")
    command.add_argument("--repo", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    command = sub.add_parser("verify")
    command.add_argument("--source", type=Path, required=True)
    command.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "freeze":
        freeze(args.repo, args.output)
    else:
        print(json.dumps({"status": "PASS", "inputs": verify(args.source, args.manifest)}))


if __name__ == "__main__":
    main()
