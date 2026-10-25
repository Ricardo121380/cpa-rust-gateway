//! M3/M4 public local duties; never an original Claude or fixed-service qualification.
use super::*;
use gateway_store::request_metadata::SqliteRequestMetadataReader;
mod faults;
mod negatives;

pub(super) const METADATA: &str = r#"{ "user_id" : "{ \"device_id\":\"M4_PRIVATE_DEVICE\", \"account_uuid\":\"\", \"session_id\":\"M4_PRIVATE_SESSION\" }\u9e1f" }"#;

pub(super) fn keep_all() -> Value {
    json!({"edits":[{"type":"clear_thinking_20251015","keep":"all"}]})
}

pub(super) fn with_metadata(request: &Value, raw: &str) -> String {
    let encoded = request.to_string();
    format!("{},\"metadata\":{raw}}}", &encoded[..encoded.len() - 1])
}

pub(super) async fn send(
    fixture: &Fixture,
    url: &str,
    raw: String,
) -> Result<(u16, String, Vec<u8>), Box<dyn Error>> {
    let reply = reqwest::Client::builder()
        .no_proxy()
        .build()?
        .post(url.replace("/v1/responses", "/v1/messages"))
        .bearer_auth(&fixture.key)
        .header("content-type", "application/json")
        .body(raw)
        .send()
        .await?;
    let status = reply.status().as_u16();
    let id = reply
        .headers()
        .get("x-request-id")
        .ok_or("request id")?
        .to_str()?
        .to_owned();
    Ok((status, id, reply.bytes().await?.to_vec()))
}

pub(super) fn assert_context(bytes: &[u8], streaming: bool, requested: bool) -> TestResult {
    let container = if streaming {
        public_frames(bytes)?
            .into_iter()
            .find(|frame| frame["type"] == "message_delta")
            .ok_or("message_delta")?
    } else {
        serde_json::from_slice(bytes)?
    };
    assert_eq!(
        container.get("context_management").cloned(),
        requested.then(|| json!({"applied_edits":[]}))
    );
    Ok(())
}

fn body(streaming: bool, m3: bool, m4: bool) -> String {
    let mut request = json!({"model":"grok-4.5","max_tokens":19,"stream":streaming,
        "messages":[{"role":"user","content":"question"}]});
    if m3 {
        request["context_management"] = keep_all();
    }
    if m4 {
        with_metadata(&request, METADATA)
    } else {
        request.to_string()
    }
}

pub(super) fn assert_metadata(
    fixture: &Fixture,
    id: &str,
    raw: &str,
    attempts: usize,
) -> TestResult {
    let database = fixture.directory.path().join("batch-c.sqlite3");
    let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
    let request = rows
        .iter()
        .find_map(|row| match row.event() {
            GatewayEvent::Request(request) if request.request_id().as_str() == id => Some(request),
            _ => None,
        })
        .ok_or("authenticated Messages Request")?;
    assert_eq!(
        request.protocol(),
        gateway_core::GatewayProtocol::AnthropicMessages
    );
    let reader = SqliteRequestMetadataReader::open(&database, test_secret_store()?)?;
    let record = reader
        .get_owned(
            request.client_key_id(),
            request.request_id(),
            None,
            system_now_ms_runtime()?,
        )?
        .ok_or("M4 retained")?;
    assert_eq!(record.record.version, 2);
    assert_eq!(record.record.request, *request);
    assert_eq!(record.record.evidence.raw_json(), raw);
    assert_eq!(
        serde_json::to_value(&record.record.evidence)?["source"],
        "anthropic_messages_metadata"
    );
    assert_eq!(record.attempts.len(), attempts);
    for attempt in &record.attempts {
        assert!(
            reader
                .get_owned(
                    request.client_key_id(),
                    request.request_id(),
                    Some(attempt.attempt_id()),
                    system_now_ms_runtime()?
                )?
                .is_some()
        );
    }
    assert!(
        reader
            .get_owned(
                &ClientKeyId::try_new("M4-foreign")?,
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
                Some(&gateway_core::AttemptId::try_new("M4-wrong-attempt")?),
                system_now_ms_runtime()?
            )?
            .is_none()
    );
    assert!(
        rows.iter()
            .all(|row| !serde_json::to_string(row.event()).is_ok_and(|s| s.contains("M4_PRIVATE")))
    );
    Ok(())
}

async fn single(m3: bool, m4: bool) -> TestResult {
    for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
        let fixture = lifecycle_fixture(MessagesTransport::new(&peer, size)?)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()?
            .post(url.replace("/v1/responses", "/v1/messages"))
            .bearer_auth(&fixture.key)
            .header("content-type", "application/json")
            .body(body(streaming, m3, m4))
            .send()
            .await?;
        let status = response.status().as_u16();
        let id = response
            .headers()
            .get("x-request-id")
            .ok_or("request id")?
            .to_str()?
            .to_owned();
        let bytes = response.bytes().await?;
        eprintln!(
            "CPAR64_LOCAL_STATUS {}",
            json!({"m3":m3,"m4":m4,"streaming":streaming,"status":status,"accepts":peer.accepted.load(Ordering::SeqCst),"receives":peer.requests.lock().map_err(|_|"receiver")?.len()})
        );
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
        let public = tool_rounds::client_result(&bytes, streaming)?;
        assert_eq!(public["model"], "grok-4.5");
        assert_eq!(public["usage"]["input_tokens"], 20);
        assert_eq!(public["usage"]["output_tokens"], 7);
        let context = if streaming {
            let frames = public_frames(&bytes)?;
            frames
                .iter()
                .find(|f| f["type"] == "message_delta")
                .ok_or("message_delta")?
                .get("context_management")
                .cloned()
        } else {
            public.get("context_management").cloned()
        };
        assert_eq!(context, m3.then(|| json!({"applied_edits":[]})));
        assert!(!String::from_utf8_lossy(&bytes).contains("M4_PRIVATE"));
        {
            let received = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(received.len(), 1);
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
            assert!(received[0].body.get("metadata").is_none());
            assert!(received[0].body.get("context_management").is_none());
            assert!(!String::from_utf8_lossy(&received[0].wire).contains("M4_PRIVATE"));
        }
        if m4 {
            assert_metadata(&fixture, &id, METADATA, 1)?;
        }
        assert_eq!(
            retained(&recording.database, &id)?.evidence.items().len(),
            2
        );
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        accounting(&database, 1).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_m3_identity_is_local() -> TestResult {
    single(true, false).await
}

#[actix_web::test]
async fn public_build_messages_m4_metadata_is_private() -> TestResult {
    single(false, true).await
}

#[actix_web::test]
async fn public_build_messages_local_controls_keep_two_tool_cycles() -> TestResult {
    tool_rounds::run_cycles(true, false).await
}

pub(super) fn expected_input(round: usize) -> Value {
    // Existing Build encoding uses the exact compact string form for a sole user text.
    if round == 0 {
        return json!("question");
    }
    let text = |role: &str, value: &str| {
        json!({"type":"message","role":role,"content":[{
        "type":if role=="assistant" {"output_text"} else {"input_text"},"text":value}]})
    };
    let mut input = vec![text("user", "question")];
    if round > 0 {
        input.push(json!({"type":"function_call","call_id":"call-0","name":"lookup","arguments":"{\"city\":\"上海\",\"n\":9007199254740993}"}));
        input.push(json!({"type":"function_call_output","call_id":"call-0","output":"result-0"}));
    }
    if round > 1 {
        input.extend([text("assistant", "回答 1"), text("user", "next-question")]);
    }
    if round > 2 {
        input.push(json!({"type":"function_call","call_id":"call-2","name":"lookup","arguments":"{\"city\":\"上海\",\"n\":9007199254740993,\"prior_result\":\"result-0\"}"}));
        input.push(json!({"type":"function_call_output","call_id":"call-2","output":"result-0 -> result-2"}));
    }
    if round > 3 {
        input.extend([text("assistant", "回答 3"), text("user", "follow-up")]);
    }
    json!(input)
}
