#!/usr/bin/env python3
"""Retire an explicit list of unpublished test drafts; retain audit tombstones.

Dry-run by default. Apply requires a new private SQLite backup. This does not
purge event history, billing, accounts in other versions or administrator auth.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import time

AUDITS = {"management_audit_events", "management_resource_audit_events"}


def quoted(name):
    if not re.fullmatch(r"[a-zA-Z_][a-zA-Z_0-9]*", name):
        raise ValueError("unsupported SQL identifier")
    return '"' + name + '"'


def inspect(db, targets):
    tables = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
    scoped = [name for name in tables if name not in AUDITS and "config_version_id" in [r[1] for r in db.execute("PRAGMA table_info(" + quoted(name) + ")")]]
    result = []
    for identifier, expected_revision in targets.items():
        row = db.execute("SELECT status, revision FROM config_versions WHERE id=?", (identifier,)).fetchone()
        if row != ("draft", expected_revision):
            raise ValueError("target is missing, changed or not a draft")
        if db.execute("SELECT 1 FROM config_versions WHERE parent_id=? LIMIT 1", (identifier,)).fetchone():
            raise ValueError("target has descendants")
        if db.execute("SELECT 1 FROM management_audit_events WHERE (config_version_id=? OR replaced_config_version_id=?) AND action != 'config_created' LIMIT 1", (identifier, identifier)).fetchone():
            raise ValueError("target participated in publication")
        if db.execute("SELECT 1 FROM gateway_event_log WHERE instr(payload_json, ?) > 0 LIMIT 1", (identifier,)).fetchone():
            raise ValueError("target appears in persistent events")
        counts = {name: db.execute("SELECT count(*) FROM " + quoted(name) + " WHERE config_version_id=?", (identifier,)).fetchone()[0] for name in scoped}
        result.append({"id": identifier, "revision": expected_revision, "resource_rows": {k: v for k, v in counts.items() if v}})
    if db.execute("PRAGMA foreign_key_check").fetchone():
        raise ValueError("existing foreign key violation")
    return tables, scoped, result


def protected_digest(db, tables, targets, audit_max):
    """Stream every retained row into a digest; values never enter output."""
    digest = hashlib.sha256()
    marks = ",".join("?" for _ in targets)
    for name in tables:
        columns = [r[1] for r in db.execute("PRAGMA table_info(" + quoted(name) + ")")]
        key = "id" if name == "config_versions" else "config_version_id"
        where, parameters = "", ()
        if name in AUDITS:
            where, parameters = " WHERE id <= ?", (audit_max[name],)
        elif key in columns:
            where, parameters = " WHERE " + quoted(key) + " NOT IN (" + marks + ")", tuple(targets)
        digest.update(name.encode())
        for row in db.execute("SELECT * FROM " + quoted(name) + where + " ORDER BY rowid", parameters):
            digest.update(repr(row).encode())
    return digest.hexdigest()


def retire(database, targets, backup=None):
    if not targets or len(targets) > 50 or any(not isinstance(v, int) or v < 0 for v in targets.values()):
        raise ValueError("expected 1–50 exact IDs with integer revisions")
    database = Path(database).resolve(strict=True)
    with sqlite3.connect(database.as_uri() + "?mode=rw", uri=True, timeout=5) as db:
        db.execute("PRAGMA foreign_keys=ON")
        deadline = time.monotonic() + 20
        db.set_progress_handler(lambda: int(time.monotonic() > deadline), 10000)
        tables, scoped, summary = inspect(db, targets)
        if backup is None:
            return {"mode": "dry-run", "drafts": summary, "retained": "audit, history, billing, published configuration"}
        backup = Path(backup)
        if not backup.parent.is_dir() or backup.parent.stat().st_mode & 0o077:
            raise ValueError("backup directory must already be private (0700)")
        fd = os.open(backup, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        os.close(fd)
        with sqlite3.connect(backup) as destination:
            db.backup(destination)
            if destination.execute("PRAGMA quick_check").fetchone() != ("ok",):
                raise ValueError("backup verification failed")
        db.execute("BEGIN IMMEDIATE")
        try:
            tables, scoped, summary = inspect(db, targets)
            audit_max = {name: db.execute("SELECT coalesce(max(id),0) FROM " + quoted(name)).fetchone()[0] for name in AUDITS}
            before = protected_digest(db, tables, targets, audit_max)
            # Delete children before parents, even where cascading is available.
            dependencies = {name: {r[2] for r in db.execute("PRAGMA foreign_key_list(" + quoted(name) + ")") if r[2] in scoped and r[2] != name} for name in scoped}
            order = []
            while dependencies:
                parents = set().union(*dependencies.values())
                leaves = sorted(set(dependencies) - parents)
                if not leaves:
                    raise ValueError("unsupported resource dependency cycle")
                order.extend(leaves)
                for name in leaves:
                    del dependencies[name]
            for identifier, revision in targets.items():
                for name in order:
                    db.execute("DELETE FROM " + quoted(name) + " WHERE config_version_id=?", (identifier,))
                db.execute("UPDATE config_versions SET status='archived', revision=revision+1, description=? WHERE id=? AND status='draft' AND revision=?", ("已清理的测试草稿（保留审计）", identifier, revision))
                db.execute("INSERT INTO management_resource_audit_events(action,actor,occurred_at_ms,config_version_id,resource_kind,resource_id) VALUES(?,?,?,?,?,?)", ("test_draft_retired", "operator:test-draft-cleanup", int(time.time() * 1000), identifier, "config_version", identifier))
            if db.execute("PRAGMA foreign_key_check").fetchone():
                raise ValueError("cleanup would break a reference")
            if protected_digest(db, tables, targets, audit_max) != before:
                raise ValueError("cleanup would change protected data")
            db.commit()
        except BaseException:
            db.rollback()
            raise
    return {"mode": "applied", "drafts": summary, "backup": str(backup), "protected_data_unchanged": True, "audit_records_added": len(targets)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", required=True)
    parser.add_argument("--manifest", required=True, help="JSON object: exact draft ID to expected integer revision")
    parser.add_argument("--backup", help="Apply changes, with an exclusively created 0600 backup here")
    args = parser.parse_args()
    try:
        print(json.dumps(retire(args.database, json.loads(Path(args.manifest).read_text()), args.backup), ensure_ascii=False))
    except (ValueError, sqlite3.Error, OSError) as error:
        # Do not emit arbitrary SQL diagnostics containing stored values.
        raise SystemExit("test draft cleanup stopped: " + (str(error) if isinstance(error, ValueError) else type(error).__name__))
