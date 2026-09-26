#!/usr/bin/env python3
"""Fixed seed M4 synthetic workload. Only creates an unused local SQLite database."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import sqlite3
import subprocess

START = 1700000000000
SPAN = 30 * 86400000
SEED = 20260926


def verify(size, spec, receipt):
    """Independent arithmetic oracle: does not read SQL or implementation query results."""
    durations, firsts, ids = [], [], []
    counts = dict(succeeded=0, failed=0, cancelled=0, unknown=0)
    attempts = 0
    buckets = {}
    for i in range(size):
        random = (i * 48271 + SEED) % 2147483647
        outcome = ('unknown' if i % 20 == 0 else 'cancelled' if i % 20 == 1
                   else 'failed' if i % 20 in (2, 3) else 'succeeded')
        finished = START + i * SPAN // size
        unknown = outcome == 'unknown'
        if unknown:
            if not spec.get('include_unknown', False):
                continue
        elif not spec.get('from_ms', 0) <= finished <= spec.get('to_ms', 2**63-1):
            continue
        actual = dict(model='model-' + str(random % 20),
                      credential_id=None if unknown else 'account-' + str(random // 20 % 100),
                      upstream_id=None if unknown else 'channel-' + str(random % 8),
                      client_key_id='key-' + str(random // 2000 % 100), outcome=outcome)
        if any(key in spec and spec[key] != actual[key] for key in actual):
            continue
        counts[outcome] += 1
        count = 0 if unknown else 1 + (i % 7 == 0)
        attempts += count
        ids.append(i)
        if not unknown:
            durations.append(random % 4000 + 1)
            firsts.append(random % 1000)
            at = finished // 86400000 * 86400000
            bucket = buckets.setdefault(at, dict(requests=0, succeeded=0, failed=0, cancelled=0,
                                                duration=0, first=0))
            bucket['requests'] += 1
            bucket[outcome] += 1
            bucket['duration'] += durations[-1]
            bucket['first'] += firsts[-1]
    durations.sort()
    n = len(durations)
    stats = dict(counts, requests=len(ids), attempts=attempts,
                 success_rate=counts['succeeded'] / n if n else None,
                 average_duration_ms=sum(durations) / n if n else None,
                 average_first_content_ms=sum(firsts) / n if n else None,
                 p50_duration_ms=durations[(n-1)//2] if n else None,
                 p95_duration_ms=durations[(n*95+99)//100-1] if n else None)
    page = receipt['page']
    if spec.get('summary', True):
        for key, expected in stats.items():
            observed = page['summary'][key]
            assert (math.isclose(expected, observed, rel_tol=1e-12)
                    if isinstance(expected, float) else observed == expected), (key, observed, expected)
        assert len(page['series']) == len(buckets)
        for row in page['series']:
            expected = buckets[row['at_ms']]
            for key in ('requests', 'succeeded', 'failed', 'cancelled'):
                assert row[key] == expected[key], (key, row, expected)
            assert math.isclose(row['average_duration_ms'], expected['duration']/expected['requests'])
            assert math.isclose(row['average_first_content_ms'], expected['first']/expected['requests'])
    else:
        assert page['summary'] is None and page['series'] == []
    ordered = list(reversed(ids))
    digest = hashlib.sha256()
    for i in ordered:
        count = 0 if i % 20 == 0 else 1 + (i % 7 == 0)
        cost = 'null' if count == 0 or i % 11 == 0 else str(count*(100+i%200))
        confidence = 'null' if count == 0 else 'unpriced' if i % 11 == 0 else 'exact'
        digest.update(f'req-{i:09d}|{count}|{count}|{cost}|{confidence}\n'.encode())
    assert receipt['all_pages'] == dict(rows=len(ids), pages=max(1, (len(ids)+99)//100), sha256=digest.hexdigest())
    assert (page['next_after'] is not None) == (len(ids) > 100)
    for offset, part in [(0, page)] + ([(100, receipt['next'])] if page['next_after'] else []):
        assert (part['next_after'] is not None) == (len(ids) > offset+100)
        assert [v['request_id'] for v in part['items']] == ['req-%09d' % i for i in ordered[offset:offset+100]]
        for row, i in zip(part['items'], ordered[offset:offset+100]):
            count = 0 if i % 20 == 0 else 1 + (i % 7 == 0)
            assert row['attempt_count'] == count and row['ledger_records'] == count
            assert row['cost_microunits'] == (None if count == 0 or i % 11 == 0 else count*(100+i%200))
            assert row['cost_confidence'] == (None if count == 0 else 'unpriced' if i % 11 == 0 else 'exact')
    return dict(correct=True, matching_requests=len(ids), attempts=attempts)


def seed(db, size, binary):
    if db.exists():
        raise SystemExit("refuse to overwrite a workload database")
    subprocess.run([str(binary), str(db), "init", "unused"], check=True)
    con = sqlite3.connect(str(db))
    con.execute("PRAGMA journal_mode=WAL")
    con.executescript(f"""
    BEGIN;
    CREATE TEMP TABLE seed AS
      WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM n WHERE i+1<{size})
      SELECT i,(i*48271+{SEED})%2147483647 r,'req-'||printf('%09d',i) id,
        {START}+i*{SPAN}/{size} time,
        CASE WHEN i%20=0 THEN 'unknown' WHEN i%20=1 THEN 'cancelled'
             WHEN i%20 IN (2,3) THEN 'failed' ELSE 'succeeded' END outcome FROM n;
    INSERT INTO gateway_event_log(event_ordinal,event_type,event_id,request_id,occurred_at_ms,payload_json)
      SELECT i*10+1,'request',id,id,CASE WHEN outcome='unknown' THEN NULL ELSE time-5000 END,
      json_object('request',json_object('request_id',id,'client_key_id','key-'||(r/2000%100),
      'public_model','model-'||(r%20),'requested_model','model-'||(r%20),
      'protocol','openai_responses','streaming',json('true'))) FROM seed;
    INSERT INTO gateway_event_log(event_ordinal,event_type,event_id,request_id,occurred_at_ms,payload_json)
      SELECT i*10+2,'attempt',id||'-1',id,time-1,
      json_object('attempt',json_object('upstream_id','channel-'||(r%8),
      'credential_id','account-'||(r/20%100),'endpoint_id','endpoint-'||(r%8),
      'attempt_number',1)) FROM seed WHERE outcome<>'unknown';
    INSERT INTO gateway_event_log(event_ordinal,event_type,event_id,request_id,occurred_at_ms,payload_json)
      SELECT i*10+3,'attempt',id||'-2',id,time-1,
      json_object('attempt',json_object('upstream_id','channel-'||(r%8),
      'credential_id','account-'||(r/20%100),'endpoint_id','endpoint-'||(r%8),
      'attempt_number',2)) FROM seed WHERE outcome<>'unknown' AND i%7=0;
    INSERT INTO gateway_event_log(event_ordinal,event_type,event_id,request_id,occurred_at_ms,payload_json)
      SELECT i*10+4,'usage',id||'-usage',id,time,
      json_object('usage',json_object('usage',json_object('input_tokens',10,'output_tokens',20)))
      FROM seed WHERE outcome<>'unknown';
    INSERT INTO gateway_event_log(event_ordinal,event_type,event_id,request_id,occurred_at_ms,payload_json)
      SELECT i*10+5,'request_finished',id,id,time,
      json_object('request_finished',json_object('request_id',id,'started_at_ms',time-5000,
      'finished_at_ms',time,'duration_ms',r%4000+1,'first_content_ms',r%1000,
      'outcome',outcome,'error_code',CASE WHEN outcome='failed' THEN 'upstream_failed' ELSE NULL END))
      FROM seed WHERE outcome<>'unknown';
    INSERT INTO billing_ledger_entries(source_event_id,source_fingerprint,request_id,response_id,
      provider_id,channel_id,account_id,model,occurred_at_ms,cost_microunits,cost_confidence,
      billing_status,retention_expires_at_ms,recorded_at_ms)
      SELECT id||'-ledger-'||a,'0000000000000000000000000000000000000000000000000000000000000000',id,id,
      'provider','channel-'||(r%8),'account-'||(r/20%100),'model-'||(r%20),time,
      CASE WHEN i%11=0 THEN NULL ELSE 100+i%200 END,
      CASE WHEN i%11=0 THEN 'unpriced' ELSE 'exact' END,
      CASE WHEN i%11=0 THEN 'unpriced' ELSE 'priced' END,time+{SPAN},time
      FROM seed CROSS JOIN (SELECT 1 a UNION ALL SELECT 2)
      WHERE outcome<>'unknown' AND (a=1 OR i%7=0);
    COMMIT;
    DROP TABLE seed;
    ANALYZE;
    PRAGMA wal_checkpoint(TRUNCATE);
    """)
    counts = {name: con.execute("SELECT count(*) FROM " + name).fetchone()[0]
              for name in ["gateway_event_log", "billing_ledger_entries"]}
    con.close()
    db.with_suffix(".dataset.json").write_text(json.dumps({
        "requests": size, "seed": SEED, "start_ms": START, "span_ms": SPAN,
        "counts": counts, "synthetic": True}, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("database", type=Path)
    parser.add_argument("--requests", type=int, choices=[100000, 1000000], required=True)
    parser.add_argument("--binary", type=Path, default=Path("target/release/examples/request_history_benchmark"))
    parser.add_argument("--verify", nargs=2, type=Path, metavar=("QUERY", "RECEIPT"))
    args = parser.parse_args()
    if args.verify:
        print(json.dumps(verify(args.requests, json.loads(args.verify[0].read_text()),
                                json.loads(args.verify[1].read_text()))))
    else:
        seed(args.database.resolve(), args.requests, args.binary.resolve())
