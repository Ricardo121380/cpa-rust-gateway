//! Independent same-response calls, multiple results, then a dependent later round.
use super::*;

fn inputs() -> Value {
    json!([
        {"type":"message","id":"parallel-dev-original","role":"developer","content":[{"type":"input_text","text":"Keep identities and content order."}]},
        {"type":"message","id":"parallel-user-original","role":"user","content":[{"type":"input_text","text":"Read independent left and right seeds."},{"type":"input_text","text":"之后计算总和的后继。"}]},
        {"type":"message","id":"parallel-system-original","role":"system","content":[{"type":"input_text","text":"Exact history."}]}
    ])
}

fn results(reply: &Value, round: usize, namespaced: bool) -> Result<Vec<Value>, Box<dyn Error>> {
    let mut results = Vec::new();
    if round == 0 {
        // Reverse the results: only call_id, never array order or bare name, correlates them.
        for index in [2, 1] {
            let call = &reply["output"][index];
            let args: Value = serde_json::from_str(call["arguments"].as_str().ok_or("args")?)?;
            assert_eq!(
                call["name"],
                if namespaced || index == 1 {
                    "lookup"
                } else {
                    "measure"
                }
            );
            let seed = args["seed"].as_i64().ok_or("seed")?;
            assert_eq!(seed, if index == 1 { 19 } else { 22 });
            let mut result = json!({"type":"function_call_output","call_id":call["call_id"],"output":seed.to_string()});
            if namespaced {
                assert_eq!(call["namespace"], if index == 1 { "left" } else { "right" });
                result["namespace"] = call["namespace"].clone();
                result["name"] = call["name"].clone();
            }
            results.push(result);
        }
    } else if round == 2 {
        let call = &reply["output"][1];
        assert_eq!(call["name"], "combine");
        let args: Value = serde_json::from_str(call["arguments"].as_str().ok_or("args")?)?;
        assert_eq!(
            args["left"].as_i64().ok_or("left")? + args["right"].as_i64().ok_or("right")?,
            41
        );
        assert_eq!(args["sum"], 41);
        results.push(json!({"type":"function_call_output","call_id":call["call_id"],"output":(args["sum"].as_i64().ok_or("sum")?+1).to_string()}));
    } else {
        results.push(json!({"type":"message","id":format!("parallel-followup-{round}"),"role":"user",
            "content":[{"type":"input_text","text":if round==1 {"Use both returned seeds to compute 42."} else {"What were both seeds and their combined successor?"}}]}));
    }
    Ok(results)
}

#[actix_web::test]
async fn public_build_parallel_json_and_interleaved_sse_keep_explicit_and_stored_cycles()
-> TestResult {
    for namespaced in [false, true] {
        for stored in [false, true] {
            for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
                for reverse_done in [false, true] {
                    run_history(
                        namespaced,
                        stored,
                        streaming,
                        size,
                        if reverse_done { [2, 1] } else { [1, 2] },
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}

async fn run_history(
    namespaced: bool,
    stored: bool,
    streaming: bool,
    size: usize,
    completion_order: [usize; 2],
) -> TestResult {
    let reverse_done = completion_order == [2, 1];
    let peer = ModelPeer::start_sequence(
        streaming,
        (0..5)
            .map(|round| wire(round, namespaced, streaming, reverse_done))
            .collect::<Result<_, _>>()?,
    )?;
    let fixture = fixture_for(namespaces::transport(&peer, size)?, stored, true, |p| {
        p.build_parallel_local_controlled = true;
        p.build_namespaces_local_controlled = namespaced;
        p.build_input_message_ids_local_controlled = true;
    })?;
    let recording = Recording::start(&fixture, false)?;
    let (url, server) = namespaces::listener(&fixture)?;
    let mut history = inputs().as_array().ok_or("inputs")?.clone();
    let mut current = history.clone();
    let mut previous = None;
    let mut lineage = None;
    let mut public = Vec::new();
    let mut returned_seeds = None;
    for round in 0..5 {
        let mut request = body(namespaced, streaming);
        request["input"] = json!(if stored { &current } else { &history });
        request["store"] = json!(stored);
        if let Some(id) = &previous {
            request["previous_response_id"] = json!(id);
        }
        let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
        assert_eq!(
            status,
            200,
            "{namespaced}/{stored}/{streaming}/{size}/{round}: {}",
            String::from_utf8_lossy(&bytes)
        );
        let reply = namespaces::terminal(&bytes, streaming)?;
        assert_eq!(reply["output"], output(round, namespaced));
        if streaming {
            assert_stream(&bytes, round, reverse_done)?;
        }
        {
            let sent = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(sent.len(), round + 1);
            assert_eq!(sent[round].body["input"], json!(history));
            assert_eq!(sent[round].body["tools"], declarations(namespaced));
            assert_eq!(sent[round].body["parallel_tool_calls"], true);
            assert!(sent[round].body.get("previous_response_id").is_none());
            assert_eq!(sent[round].model_override, "grok-4.5");
            if round == 2 {
                let seeds: Vec<_> = sent[round].body["input"]
                    .as_array()
                    .ok_or("input")?
                    .iter()
                    .filter(|v| v["type"] == "function_call_output")
                    .cloned()
                    .collect();
                assert_eq!(Some(&seeds), returned_seeds.as_ref());
            }
        }
        if stored {
            assert_stored(&fixture, &reply, &history, &mut lineage, round)?;
            previous = Some(reply["id"].clone());
        }
        history.extend(reply["output"].as_array().ok_or("output")?.iter().cloned());
        current = results(&reply, round, namespaced)?;
        if round == 0 {
            returned_seeds = Some(current.clone());
        }
        history.extend(current.iter().cloned());
        public.push(reply);
    }
    assert_eq!(active_leases(&fixture), 0);
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 5);
    server.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    namespaces::assert_recorded(&database, 5).await?;
    assert_counts(&database, 5, 5).await?;
    let received: Vec<_> = peer
        .requests
        .lock()
        .map_err(|_| "receiver")?
        .iter()
        .map(|r| r.body.clone())
        .collect();
    println!(
        "CPAR62_PARALLEL_HISTORY {}",
        json!({"namespaced":namespaced,"stored":stored,"streaming":streaming,"fragment_bytes":size,
        "reverse_done":reverse_done,"receiver":5,"attempts":5,"usage":5,"ledger":5,"cost":"UNKNOWN","native_success_companion":"UNKNOWN",
        "source_ids_and_order":"EXACT","lineage":stored,"received":received,"public":public})
    );
    Ok(())
}

fn assert_stored(
    fixture: &Fixture,
    reply: &Value,
    history: &[Value],
    lineage: &mut Option<Value>,
    round: usize,
) -> TestResult {
    let record =
        read_stored(fixture, reply["id"].as_str().ok_or("response id")?)?.ok_or("stored root")?;
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
        format!("parallel-native-{round}")
    );
    stable["credential"]
        .as_object_mut()
        .ok_or("credential")?
        .remove("upstream_response_id");
    if let Some(original) = lineage {
        assert_eq!(&stable, original);
    } else {
        *lineage = Some(stable);
    }
    let store = gateway_store::stored_response::SqliteStoredResponseStore::open(
        fixture.directory.path().join("batch-c.sqlite3"),
        test_secret_store()?,
    )?;
    assert!(
        store
            .get_owned(
                &ClientKeyId::try_new("parallel-foreign-owner")?,
                record.response_id(),
                system_now_ms_runtime()?
            )?
            .is_none()
    );
    Ok(())
}

fn assert_stream(bytes: &[u8], round: usize, reverse_done: bool) -> TestResult {
    let frames = public_frames(bytes)?;
    let done: Vec<_> = frames
        .iter()
        .filter(|v| v["type"] == "response.function_call_arguments.done")
        .collect();
    assert_eq!(
        done.len(),
        match round {
            0 => 2,
            2 => 1,
            _ => 0,
        }
    );
    if round == 0 {
        assert_eq!(
            done[0]["item_id"],
            if reverse_done {
                "parallel-item-1"
            } else {
                "parallel-item-0"
            }
        );
        let deltas: Vec<_> = frames
            .iter()
            .filter(|v| v["type"] == "response.function_call_arguments.delta")
            .collect();
        assert!(
            deltas
                .windows(2)
                .filter(|p| p[0]["item_id"] != p[1]["item_id"])
                .count()
                > 4
        );
        for index in [1, 2] {
            let args: String = deltas
                .iter()
                .filter(|v| v["output_index"] == index)
                .filter_map(|v| v["delta"].as_str())
                .collect();
            assert_eq!(
                args,
                output(0, false)[index]["arguments"]
                    .as_str()
                    .ok_or("args")?
            );
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_parallel_stored_multi_results_reject_missing_duplicate_and_wrong_identity()
-> TestResult {
    let peer = ModelPeer::start_wire(false, wire(0, true, false, false)?)?;
    let fixture = fixture_for(namespaces::transport(&peer, 7)?, true, true, |p| {
        p.build_parallel_local_controlled = true;
        p.build_namespaces_local_controlled = true;
    })?;
    let recording = Recording::start(&fixture, false)?;
    let (url, server) = namespaces::listener(&fixture)?;
    let mut initial = body(true, false);
    initial["store"] = json!(true);
    let (status, bytes) = namespaces::request(&fixture, &url, &initial).await?;
    assert_eq!(status, 200);
    let reply = namespaces::terminal(&bytes, false)?;
    let valid = results(&reply, 0, true)?;
    for label in [
        "wrong-namespace",
        "wrong-name",
        "missing",
        "duplicate",
        "wrong-call",
    ] {
        let mut input = valid.clone();
        match label {
            "wrong-namespace" => input[0]["namespace"] = json!("left"),
            "wrong-name" => input[0]["name"] = json!("combine"),
            "missing" => {
                input.pop();
            }
            "duplicate" => input[1] = input[0].clone(),
            _ => input[0]["call_id"] = json!("not-the-stored-call"),
        }
        let mut request = body(true, false);
        request["input"] = json!(input);
        request["previous_response_id"] = reply["id"].clone();
        let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
        println!(
            "CPAR62_PARALLEL_STORED_RESULT {}",
            json!({"label":label,"status":status,"receiver_total":peer.requests.lock().map_err(|_|"receiver")?.len(),"attempt_total":fixture.events.attempts()?})
        );
        assert_eq!(status, 400, "{label}: {}", String::from_utf8_lossy(&bytes));
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.events.attempts()?, 1);
    }
    server.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    assert_counts(&database, 1, 1).await?;
    Ok(())
}
