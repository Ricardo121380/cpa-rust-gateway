//! Per-request joins use observed public/native IDs, never array position or adapter counters.
use super::*;
use gateway_core::RawExtensions;
use gateway_store::request_metadata::SqliteRequestMetadataReader;
use sha2::{Digest, Sha256};

pub(super) fn frames(row: &Captured) -> Result<Vec<Value>, Box<dyn Error>> {
    assert!(!row.wire_truncated);
    let wire = std::str::from_utf8(&row.wire)?;
    if !wire.ends_with("\n\n") {
        return Err("incomplete captured SSE record".into());
    }
    let semantic = wire
        .split_inclusive("\n\n")
        .filter(|record| {
            !record
                .lines()
                .all(|line| line.is_empty() || line.starts_with(':'))
        })
        .collect::<String>();
    public_frames(semantic.as_bytes())
}

#[actix_web::test]
async fn cli_capture_rejects_unterminated_semantic_record() -> TestResult {
    let wire = b"event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n";
    let mut row = Captured {
        wire: wire.to_vec(),
        ..Captured::default()
    };
    assert_eq!(frames(&row)?.len(), 1);
    for removed in [1, 2] {
        row.wire = wire[..wire.len() - removed].to_vec();
        assert!(
            frames(&row).is_err(),
            "missing SSE boundary cannot be synthesized"
        );
    }
    Ok(())
}

pub(super) fn correspondence(
    database: &std::path::Path,
    ingress: &Ingress,
    peer: &SocketPeer,
) -> Result<Value, Box<dyn Error>> {
    let events = SqliteEventStore::open_read_only(database)?.list_events()?;
    let ledger = SqliteBillingLedger::open_read_only(database)?.list_bounded(20)?;
    let reader = SqliteRequestMetadataReader::open(database, test_secret_store()?)?;
    let received = peer.received.lock().map_err(|_| "receiver")?;
    let receipts = peer.receipts.lock().map_err(|_| "receiver receipts")?;
    assert_eq!(received.len(), receipts.len());
    let captured = ingress.lock().map_err(|_| "ingress")?;
    let mut joined = Vec::new();
    let mut seen_requests = BTreeSet::new();
    let mut seen_receivers = BTreeSet::new();
    for row in captured
        .iter()
        .filter(|row| !row.probe && row.status == 200)
    {
        let request_id = public_request_id(row)?;
        assert!(seen_requests.insert(request_id.clone()));
        let request = events
            .iter()
            .find_map(|event| match event.event() {
                GatewayEvent::Request(value) if value.request_id().as_str() == request_id => {
                    Some(value)
                }
                _ => None,
            })
            .ok_or("observed public Request ID")?;
        let attempts: Vec<_> = events
            .iter()
            .filter_map(|event| match event.event() {
                GatewayEvent::Attempt(value) if value.request_id() == request.request_id() => {
                    Some(value)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            attempts.len(),
            1,
            "one actual Attempt for this public request"
        );
        let attempt = attempts[0];
        let public = frames(row)?;
        let starts: Vec<_> = public
            .iter()
            .filter(|v| v["type"] == "response.created")
            .collect();
        assert_eq!(starts.len(), 1);
        let response_id = starts[0]["response"]["id"]
            .as_str()
            .ok_or("native response ID")?;
        let matching: Vec<_> = receipts
            .iter()
            .filter(|v| v["response_id"] == response_id)
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "independent receiver response ID is unique"
        );
        let ordinal = usize::try_from(matching[0]["ordinal"].as_u64().ok_or("receiver ordinal")?)?;
        assert!(seen_receivers.insert(ordinal));
        let upstream = &received[ordinal];
        let fields = fidelity(row, upstream, matching[0])?;
        let c4 = metadata(&reader, row, request, attempt)?;
        let usages: Vec<_> = events
            .iter()
            .filter_map(|event| match event.event() {
                GatewayEvent::Usage(value) if value.request_id() == request.request_id() => {
                    Some((event.event_id(), value))
                }
                _ => None,
            })
            .collect();
        assert!(usages.len() <= 1);
        assert_usage_rows(&usages, &ledger, attempt, response_id, &request_id);
        let finished: Vec<_> = events
            .iter()
            .filter_map(|event| match event.event() {
                GatewayEvent::RequestFinished(value)
                    if value.request_id == *request.request_id() =>
                {
                    Some(value)
                }
                _ => None,
            })
            .collect();
        assert_eq!(finished.len(), 1);
        joined.push(json!({"receiver_ordinal":ordinal,"response_id":response_id,"request_id":request_id,
            "attempt_id":attempt.attempt_id().as_str(),"upstream_model":attempt.upstream_model(),
            "owner":request.client_key_id().as_str(),"usage_source_event_id":usages.first().map(|(id,_)|id),
            "usage":usages.first().map(|(_,v)|v.usage()),"ledger_rows":usages.len(),"cost":"UNKNOWN",
            "outcome":format!("{:?}",finished[0].outcome),"c4":c4,"fields":fields}));
    }
    assert_eq!(
        seen_receivers.len(),
        received.len(),
        "every receive joins one actual public request"
    );
    Ok(json!(joined))
}

fn fidelity(row: &Captured, upstream: &Value, receipt: &Value) -> Result<Value, Box<dyn Error>> {
    assert!(upstream.get("client_metadata").is_none());
    let headers: Vec<(String, String)> = serde_json::from_value(receipt["headers"].clone())?;
    assert!(headers.iter().all(|(name, _)| !matches!(
        name.as_str(),
        "client_metadata"
            | "x-codex-turn-metadata"
            | "root_turn_id"
            | "session_id"
            | "thread_id"
            | "turn_id"
            | "x-codex-installation-id"
            | "x-codex-window-id"
    )));
    let mut fields = serde_json::Map::new();
    for key in [
        "tools",
        "parallel_tool_calls",
        "tool_choice",
        "reasoning",
        "include",
    ] {
        assert_eq!(
            row.request.get(key),
            upstream.get(key),
            "native field {key}"
        );
        fields.insert(
            key.into(),
            json!({"present":row.request.get(key).is_some(),"value":row.request.get(key)}),
        );
    }
    // Production moves root instructions to one developer message. Everything after it is exact.
    let mut expected = row.request["input"]
        .as_array()
        .ok_or("actual input history")?
        .clone();
    for item in &mut expected {
        if item.get("role").is_some() {
            if item.get("type").is_none() {
                item["type"] = json!("message");
            }
            if let Some(text) = item["content"].as_str() {
                item["content"] = json!([{"type":if item["role"] == "assistant" {"output_text"} else {"input_text"},"text":text}]);
            }
        }
    }
    if let Some(instructions) = row.request.get("instructions") {
        expected.insert(
            0,
            json!({"type":"message","role":"developer",
            "content":[{"type":"input_text","text":instructions}]}),
        );
    }
    assert_eq!(
        upstream["input"],
        json!(expected),
        "complete ordered native input, including absent/null members"
    );
    fields.insert(
        "input".into(),
        json!({"items":expected.len(),"exact_after_instruction_projection":true,
            "non_assistant_ids":expected.iter().enumerate().filter(|(_,item)|
                matches!(item["role"].as_str(),Some("user"|"developer"|"system")))
                .map(|(index,item)|json!({"index":index,"role":item["role"],"id_present":item.get("id").is_some(),"id":item.get("id")})).collect::<Vec<_>>()}),
    );
    fields.insert(
        "client_metadata".into(),
        json!({"present":row.request.get("client_metadata").is_some(),"forwarded":false}),
    );
    Ok(Value::Object(fields))
}

fn metadata(
    reader: &SqliteRequestMetadataReader,
    row: &Captured,
    request: &gateway_core::RequestEvent,
    attempt: &gateway_core::AttemptEvent,
) -> Result<Value, Box<dyn Error>> {
    let root: RawExtensions = serde_json::from_slice(&row.request_bytes)?;
    let diagnostic = reader.get_owned(
        request.client_key_id(),
        request.request_id(),
        Some(attempt.attempt_id()),
        system_now_ms_runtime()?,
    )?;
    let Some(raw) = root.get("client_metadata") else {
        assert!(diagnostic.is_none());
        return Ok(json!({"present":false,"receipt":"UNKNOWN","absent_field_no_capture":true}));
    };
    let diagnostic =
        diagnostic.ok_or("confirmed encrypted C4 evidence for this actual Request/Attempt")?;
    assert_eq!(&diagnostic.record.request, request);
    assert_eq!(diagnostic.record.evidence.raw_json(), raw.get());
    // The C4 reader authenticates the canonical durable event. The event-store reader also
    // hydrates its separate execution-identity companion, which is not serialized in that event.
    assert!(attempt.execution_identity().is_some());
    assert_eq!(
        serde_json::to_value(&diagnostic.attempts)?,
        serde_json::to_value([attempt])?
    );
    assert!(
        reader
            .get_owned(
                &ClientKeyId::try_new("cpar62-foreign-owner")?,
                request.request_id(),
                None,
                system_now_ms_runtime()?
            )?
            .is_none()
    );
    assert!(
        reader
            .get_owned(
                request.client_key_id(),
                request.request_id(),
                Some(&gateway_core::AttemptId::try_new("cpar62-wrong-attempt")?),
                system_now_ms_runtime()?
            )?
            .is_none()
    );
    Ok(
        json!({"present":true,"exact_raw_readback":true,"raw_bytes":raw.get().len(),
        "raw_sha256":format!("{:x}",Sha256::digest(raw.get().as_bytes())),"authorized_actual_attempt":true,"foreign_owner_rejected":true}),
    )
}

fn assert_usage_rows(
    usages: &[(&str, &gateway_core::UsageEvent)],
    ledger: &[gateway_store::billing_ledger::BillingLedgerEntry],
    attempt: &gateway_core::AttemptEvent,
    response_id: &str,
    request_id: &str,
) {
    for (source, usage) in usages {
        assert_eq!(usage.attempt_id(), Some(attempt.attempt_id()));
        assert_eq!(usage.response_id().as_str(), response_id);
        assert_eq!(
            usage.usage().provenance,
            gateway_core::UsageProvenance::Measured
        );
        assert_eq!(
            usage.usage().input_accounting,
            gateway_core::InputTokenAccounting::Inclusive
        );
        assert_eq!(
            (
                usage.usage().reasoning_tokens,
                usage.usage().cache_read_tokens,
                usage.usage().cache_creation_tokens,
                usage.usage().cached_tokens
            ),
            (None, None, None, None)
        );
        let entries: Vec<_> = ledger
            .iter()
            .filter(|entry| entry.source_event_id == *source)
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].request_id, request_id);
        assert_eq!(entries[0].usage, *usage.usage());
        assert_eq!(entries[0].cost_microunits, None);
    }
}

fn public_request_id(row: &Captured) -> Result<String, Box<dyn Error>> {
    let headers: Vec<(String, Vec<u8>)> = serde_json::from_value(row.response_headers.clone())?;
    let ids: Vec<_> = headers
        .iter()
        .filter(|(name, _)| name == "x-request-id")
        .collect();
    assert_eq!(ids.len(), 1);
    Ok(String::from_utf8(ids[0].1.clone())?)
}
