//! Full effective histories, including encrypted owned continuation, retain native input IDs.
use super::*;
use gateway_store::stored_response::{SqliteStoredResponseStore, StoredResponsePayload};

fn tools() -> Value {
    json!([{"type":"function","name":"lookup","parameters":{"type":"object"},"strict":false}])
}

fn output(round: usize) -> Value {
    let mut items = tool_rounds::output(round);
    if round == 0 || round == 2 {
        items[1]["arguments"] = json!(if round == 0 {
            "{}"
        } else {
            r#"{"previous":41}"#
        });
    } else {
        items[1]["content"][0]["text"] = json!(if round == 1 {
            "Seed is 41."
        } else {
            "Result is 42."
        });
    }
    items
}

fn next_input(reply: &Value, round: usize) -> Result<Vec<Value>, Box<dyn Error>> {
    if round == 0 || round == 2 {
        let call = &reply["output"][1];
        assert_eq!(call["name"], "lookup");
        let args: Value = serde_json::from_str(call["arguments"].as_str().ok_or("args")?)?;
        let result = if round == 0 {
            41
        } else {
            args["previous"].as_i64().ok_or("seed")? + 1
        };
        assert_eq!(result, if round == 0 { 41 } else { 42 });
        Ok(vec![
            json!({"type":"function_call_output","call_id":call["call_id"],"output":result.to_string()}),
        ])
    } else {
        Ok(vec![
            json!({"type":"message","id":format!("user-followup-{round}"),"role":"user",
            "content":[{"type":"input_text","text":if round==1 {"Use the seed to compute its successor."} else {"What were both values?"}}]}),
        ])
    }
}

#[actix_web::test]
async fn public_build_c6_explicit_and_stored_history_keep_two_related_tool_cycles() -> TestResult {
    for stored in [false, true] {
        for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
            run_history(stored, streaming, size).await?;
        }
    }
    Ok(())
}

async fn run_history(stored: bool, streaming: bool, size: usize) -> TestResult {
    let peer = ModelPeer::start_sequence(
        streaming,
        (0..5)
            .map(|round| {
                namespaces::wire_output(&output(round), &format!("c6-native-{round}"), streaming)
            })
            .collect::<Result<_, _>>()?,
    )?;
    let fixture = fixture_for(namespaces::transport(&peer, size)?, stored, |p| {
        p.build_input_message_ids_local_controlled = true;
    })?;
    let recording = Recording::start(&fixture, false)?;
    let (url, server) = namespaces::listener(&fixture)?;
    let mut history = inputs().as_array().ok_or("input")?.clone();
    let mut current = history.clone();
    let mut previous = None;
    let mut public = Vec::new();
    let mut lineage = None;
    for round in 0..5 {
        let mut body = json!({"model":"p12-test-model","input":if stored {&current} else {&history},
            "tools":tools(),"parallel_tool_calls":false,"reasoning":{"summary":"auto"},"stream":streaming,"store":stored});
        if let Some(id) = &previous {
            body["previous_response_id"] = json!(id);
        }
        let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
        assert_eq!(
            status,
            200,
            "{stored}/{streaming}/{round}: {}",
            String::from_utf8_lossy(&bytes)
        );
        let reply = namespaces::terminal(&bytes, streaming)?;
        assert_eq!(reply["output"], output(round));
        assert_eq!(reply["usage"]["input_tokens"], 20);
        assert_eq!(reply["usage"]["output_tokens"], 7);
        if streaming {
            let frames = public_frames(&bytes)?;
            assert!(
                frames
                    .iter()
                    .any(|v| v["type"] == "response.reasoning_summary_text.delta")
            );
            assert_eq!(
                frames
                    .iter()
                    .filter(|v| v["type"] == "response.function_call_arguments.done")
                    .count(),
                usize::from(round == 0 || round == 2)
            );
        }
        {
            let received = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(received.len(), round + 1);
            assert_eq!(received[round].body["input"], json!(history));
            assert!(received[round].body.get("previous_response_id").is_none());
            assert_eq!(received[round].model_override, "grok-4.5");
        }
        if stored {
            assert_stored_history(&fixture, &reply, &history, &mut lineage, round)?;
            previous = Some(reply["id"].as_str().ok_or("response id")?.to_owned());
        }
        public.push(reply.clone());
        history.extend(reply["output"].as_array().ok_or("output")?.iter().cloned());
        current = next_input(&reply, round)?;
        history.extend(current.iter().cloned());
    }
    server.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    namespaces::assert_recorded(&database, 5).await?;
    negatives::assert_counts(&database, 5, 5).await?;
    assert_eq!(active_leases(&fixture), 0);
    let received: Vec<_> = peer
        .requests
        .lock()
        .map_err(|_| "receiver")?
        .iter()
        .map(|r| r.body.clone())
        .collect();
    println!(
        "CPAR62_C6_HISTORY {}",
        json!({"stored":stored,"streaming":streaming,"fragment_bytes":size,"receiver":received.len(),
        "requests":5,"attempts":fixture.events.attempts()?,"usage":5,"ledger":5,"cost":"UNKNOWN","native_success_companion":"UNKNOWN","received":received,"public":public,"exact_lineage":stored})
    );
    Ok(())
}

fn assert_stored_history(
    fixture: &Fixture,
    reply: &Value,
    history: &[Value],
    lineage: &mut Option<Value>,
    round: usize,
) -> TestResult {
    let record = read_stored(fixture, reply)?;
    let expected = protocol_openai_responses::decode_request(
        &json!({"model":"p12-test-model","input":history}).to_string(),
    )?;
    assert_eq!(
        record.payload().request().messages,
        expected.request.messages
    );
    let mut stable = serde_json::to_value(record.payload().lineage())?;
    assert_eq!(
        stable["credential"]["upstream_response_id"],
        format!("c6-native-{round}")
    );
    stable["credential"]
        .as_object_mut()
        .ok_or("credential binding")?
        .remove("upstream_response_id");
    if let Some(original) = lineage {
        assert_eq!(&stable, original);
    } else {
        *lineage = Some(stable);
    }
    Ok(())
}

fn owner(fixture: &Fixture) -> Result<ClientKeyId, Box<dyn Error>> {
    fixture
        .events
        .0
        .lock()
        .map_err(|_| "events")?
        .iter()
        .find_map(|e| match e {
            GatewayEvent::Request(v) => Some(v.client_key_id().clone()),
            _ => None,
        })
        .ok_or_else(|| "Request owner".into())
}

fn read_stored(
    fixture: &Fixture,
    reply: &Value,
) -> Result<gateway_store::stored_response::StoredResponseRecord, Box<dyn Error>> {
    let store = SqliteStoredResponseStore::open(
        fixture.directory.path().join("batch-c.sqlite3"),
        test_secret_store()?,
    )?;
    let id = gateway_core::ResponseId::try_new(reply["id"].as_str().ok_or("response id")?)?;
    assert!(
        store
            .get_owned(
                &ClientKeyId::try_new("c6-other-owner")?,
                &id,
                system_now_ms_runtime()?
            )?
            .is_none()
    );
    store
        .get_owned(&owner(fixture)?, &id, system_now_ms_runtime()?)?
        .ok_or_else(|| "owned stored Response".into())
}

#[actix_web::test]
async fn public_build_c6_stored_continuation_rejects_duplicates_foreign_owner_and_lineage()
-> TestResult {
    for streaming in [false, true] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire_response(1, streaming)?)?;
        let fixture = fixture_for(namespaces::transport(&peer, 7)?, true, |p| {
            p.build_input_message_ids_local_controlled = true;
        })?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let body =
            json!({"model":"p12-test-model","input":inputs(),"store":true,"stream":streaming});
        let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
        assert_eq!(status, 200);
        let reply = namespaces::terminal(&bytes, streaming)?;
        let record = read_stored(&fixture, &reply)?;
        let store = SqliteStoredResponseStore::open(
            fixture.directory.path().join("batch-c.sqlite3"),
            test_secret_store()?,
        )?;
        seed_inaccessible(&store, &owner(&fixture)?, record.payload())?;
        for (label, previous, input, expected_status) in [
            (
                "repeat-same",
                reply["id"].clone(),
                json!([inputs()[0]]),
                400,
            ),
            (
                "repeat-changed",
                reply["id"].clone(),
                json!([{"role":"user","id":inputs()[0]["id"],"content":"changed"}]),
                400,
            ),
            (
                "repeat-prior-output",
                reply["id"].clone(),
                json!([{"role":"developer","id":"reason-1","content":"changed kind"}]),
                400,
            ),
            (
                "foreign-owner",
                json!("c6-foreign-response"),
                json!([{"role":"user","content":"probe"}]),
                404,
            ),
            (
                "wrong-lineage",
                json!("c6-wrong-lineage"),
                json!([{"role":"user","content":"probe"}]),
                409,
            ),
            (
                "wrong-credential-revision",
                json!("c6-wrong-credential"),
                json!([{"role":"user","content":"probe"}]),
                503,
            ),
        ] {
            let request = json!({"model":"p12-test-model","input":input,"previous_response_id":previous,"stream":streaming});
            let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
            assert_eq!(
                status,
                expected_status,
                "{label}: {}",
                String::from_utf8_lossy(&bytes)
            );
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
            assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
            assert_eq!(fixture.events.attempts()?, 1);
            println!(
                "CPAR62_C6_STORED_NEGATIVE {}",
                json!({"label":label,"streaming":streaming,"status":status,"receiver_total":1,"attempt_total":1,"cost":"UNKNOWN"})
            );
        }
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        super::negatives::assert_counts(&database, 1, 1).await?;
    }
    Ok(())
}

fn seed_inaccessible(
    store: &SqliteStoredResponseStore,
    client: &ClientKeyId,
    original: &StoredResponsePayload,
) -> TestResult {
    for label in ["foreign", "route", "credential"] {
        let mut events = original.canonical_response()?.into_events();
        let Some(CanonicalEvent::ResponseStart(start)) = events.first_mut() else {
            return Err("response start".into());
        };
        start.response_id = gateway_core::ResponseId::try_new(match label {
            "foreign" => "c6-foreign-response",
            "route" => "c6-wrong-lineage",
            _ => "c6-wrong-credential",
        })?;
        let mut lineage = serde_json::to_value(original.lineage())?;
        if label == "route" {
            lineage["target"]["route"] = json!("c6-unrelated-route");
        } else if label == "credential" {
            lineage["credential"]["credential_revision"] = json!(999);
        }
        let payload = StoredResponsePayload::try_new(
            serde_json::from_value(lineage)?,
            original.public_model(),
            0,
            original.request().clone(),
            gateway_core::CanonicalResponse::try_new(events)?,
        )?;
        let owner = if label == "foreign" {
            ClientKeyId::try_new("c6-other-owner")?
        } else {
            client.clone()
        };
        store.put_owned(&owner, system_now_ms_runtime()?, &payload)?;
    }
    Ok(())
}
