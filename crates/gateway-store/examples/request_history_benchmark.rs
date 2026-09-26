//! Isolated M4 workload reader. No network or production state is used.
use gateway_store::control_plane::{RequestHistoryQuery, SqliteControlPlaneRepository};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{error::Error, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err(
            "expected: database query-json receipt-json (query-json=init creates schema)".into(),
        );
    }
    let repository = SqliteControlPlaneRepository::open(&args[1])?;
    if args[2] == "init" {
        return Ok(());
    }
    let spec: Value = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    let query = RequestHistoryQuery {
        from_ms: spec["from_ms"].as_i64(),
        to_ms: spec["to_ms"].as_i64(),
        model: spec["model"].as_str().map(str::to_owned),
        upstream_id: spec["upstream_id"].as_str().map(str::to_owned),
        credential_id: spec["credential_id"].as_str().map(str::to_owned),
        client_key_id: spec["client_key_id"].as_str().map(str::to_owned),
        outcome: spec["outcome"].as_str().map(str::to_owned),
        limit: 100,
        bucket_ms: 86_400_000,
        summary: spec["summary"].as_bool().unwrap_or(true),
        include_unknown: spec["include_unknown"].as_bool().unwrap_or(false),
        ..Default::default()
    };
    let reader = repository.resource_inventory_reader().ok_or("reader")?;
    let start = Instant::now();
    let page = reader.requests(&query)?;
    let warmup_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut samples = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let repeated = reader.requests(&query)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(repeated.summary, page.summary);
        assert_eq!(repeated.items, page.items);
    }
    let next = reader.requests(&RequestHistoryQuery {
        snapshot: Some(page.snapshot),
        ledger_snapshot: Some(page.ledger_snapshot),
        after: page.next_after,
        summary: false,
        ..query.clone()
    })?;
    // Outside latency samples: verify every cursor page, using bounded memory.
    let mut digest = Sha256::new();
    let mut rows = 0_u64;
    let mut pages = 0_u64;
    let mut current = reader.requests(&RequestHistoryQuery {
        snapshot: Some(page.snapshot),
        ledger_snapshot: Some(page.ledger_snapshot),
        summary: false,
        ..query.clone()
    })?;
    let mut previous_cursor = i64::MAX;
    loop {
        pages += 1;
        if pages.is_multiple_of(1000) {
            eprintln!("verified cursor pages: {pages}");
        }
        for item in &current.items {
            let line = format!(
                "{}|{}|{}|{}|{}\n",
                item["request_id"].as_str().ok_or("id")?,
                item["attempt_count"],
                item["ledger_records"],
                item["cost_microunits"],
                item["cost_confidence"].as_str().unwrap_or("null")
            );
            digest.update(line.as_bytes());
            rows += 1;
        }
        let Some(after) = current.next_after else {
            break;
        };
        assert!(after < previous_cursor && !current.items.is_empty());
        previous_cursor = after;
        current = reader.requests(&RequestHistoryQuery {
            snapshot: Some(page.snapshot),
            ledger_snapshot: Some(page.ledger_snapshot),
            after: Some(after),
            summary: false,
            ..query.clone()
        })?;
    }
    let all_pages = json!({"rows":rows,"pages":pages,"sha256":format!("{:x}",digest.finalize())});
    std::fs::write(
        &args[3],
        serde_json::to_vec_pretty(&json!({
            "all_pages":all_pages, "warmup_ms": warmup_ms, "samples_ms":samples, "page":page, "next":next
        }))?,
    )?;
    Ok(())
}
