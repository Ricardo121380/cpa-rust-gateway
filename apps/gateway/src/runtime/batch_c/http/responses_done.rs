//! B-02 traverses the public handler, leased factory and real loopback SSE transport.
use super::*;

fn text_stream(contradictory: bool) -> Vec<Vec<u8>> {
    let item = json!({"id":"msg-b02","type":"message","role":"assistant","status":"completed",
        "content":[{"type":"output_text","text":"answer","annotations":[],"logprobs":[]}]});
    let frames = vec![
        json!({"type":"response.created","response":{"id":"resp-b02","status":"in_progress"}}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"msg-b02","type":"message","role":"assistant","status":"in_progress","content":[]}}),
        json!({"type":"response.content_part.added","item_id":"msg-b02","output_index":0,"content_index":0,"part":{"type":"output_text","text":"","annotations":[],"logprobs":[]}}),
        json!({"type":"response.output_text.delta","item_id":"msg-b02","output_index":0,"content_index":0,"delta":"answer","logprobs":[]}),
        json!({"type":"response.output_text.done","item_id":"msg-b02","output_index":0,"content_index":0,"text":if contradictory {"contradiction"} else {"answer"},"logprobs":[]}),
        json!({"type":"response.content_part.done","item_id":"msg-b02","output_index":0,"content_index":0,"part":item["content"][0]}),
        json!({"type":"response.output_item.done","output_index":0,"item":item}),
        json!({"type":"response.completed","response":{"id":"resp-b02","status":"completed","output":[item]}}),
    ];
    let frames = frames
        .into_iter()
        .enumerate()
        .map(|(sequence, mut value)| {
            value["sequence_number"] = json!(sequence);
            value
        })
        .collect::<Vec<_>>();
    // Bootstrap and content precede a single read containing both conflict and completion.
    [
        wire(&frames[..4]).into_bytes(),
        wire(&frames[4..]).into_bytes(),
    ]
    .into()
}

fn done_fixture(peer: &Arc<Peer>) -> Result<Fixture, Box<dyn Error>> {
    fixture_with_storage(
        Channel {
            adapter: "openai-compatible.responses",
            format: "openai/responses",
            base: "https://gateway.example.test/v1",
            path: "/responses",
            model: "upstream-c",
        },
        b"synthetic-key",
        true,
        |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.http = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            endpoint.resolver = Arc::new(PublicResolver);
            Ok(())
        },
    )
}

#[actix_web::test]
async fn contradictory_text_done_fails_through_public_gateway_without_replay() -> TestResult {
    let peer = Peer::start_scripted(false, None, Some(text_stream(true)))?;
    let fixture = done_fixture(&peer)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(
                json!({"model":"p12-test-model","input":"question","stream":true,"store":true}),
            )
            .to_request(),
    )
    .await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    let body = String::from_utf8(bytes.to_vec())?;
    let events = fixture.events.0.lock().map_err(|_| "events")?.clone();
    eprintln!(
        "B-02 public status={status}; received={}; events={events:?}; body={body}",
        peer.received.lock().map_err(|_| "received")?.len()
    );
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("response.output_text.delta"));
    assert!(body.contains("answer"));
    assert!(
        !body.contains("response.completed"),
        "contradictory done became success"
    );
    assert!(body.contains("response.failed"));
    assert!(body.contains("UpstreamProtocolError"));
    assert_eq!(peer.requests.lock().map_err(|_| "requests")?.len(), 1);
    assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), 1);
    assert_eq!(fixture.events.attempts()?, 1);
    assert_persisted_outcome(&fixture, true)?;
    let get = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/responses/resp-b02")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .to_request(),
    )
    .await;
    assert_eq!(get.status(), StatusCode::NOT_FOUND);
    let continuation = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(
                json!({"model":"p12-test-model","previous_response_id":"resp-b02","input":"next"}),
            )
            .to_request(),
    )
    .await;
    assert!(!continuation.status().is_success());
    assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), 1);
    Ok(())
}

fn assert_persisted_outcome(fixture: &Fixture, failed: bool) -> TestResult {
    use gateway_core::{AttemptOutcome, RequestOutcome};
    use gateway_store::event_store::SqliteEventStore;
    let events = fixture.events.0.lock().map_err(|_| "events")?.clone();
    let database = fixture.directory.path().join("b02-events.sqlite3");
    SqliteEventStore::open(&database)?.append_batch(&events)?;
    let store = SqliteEventStore::open(&database)?;
    let rows = store.list_events()?;
    let attempt = rows
        .iter()
        .find_map(|row| match row.event() {
            GatewayEvent::Attempt(attempt) => Some(attempt),
            _ => None,
        })
        .ok_or("persisted Attempt missing")?;
    // Existing AttemptOutcome describes the driver/bootstrap result, before source consumption.
    assert!(matches!(attempt.outcome(), AttemptOutcome::Succeeded));
    let terminal = rows
        .iter()
        .find_map(|row| match row.event() {
            GatewayEvent::RequestFinished(request) => Some(request),
            _ => None,
        })
        .ok_or("persisted Request result missing")?;
    assert_eq!(&terminal.request_id, attempt.request_id());
    assert_eq!(
        terminal.outcome,
        if failed {
            RequestOutcome::Failed
        } else {
            RequestOutcome::Succeeded
        }
    );
    assert_eq!(
        terminal.error_code,
        if failed {
            Some(GatewayErrorCode::UpstreamProtocolError)
        } else {
            None
        }
    );
    Ok(())
}

fn native_frames() -> Vec<Value> {
    let text = json!({"id":"msg-b02","type":"message","role":"assistant","status":"completed","content":[
        {"type":"output_text","text":"first answer","annotations":[],"logprobs":null},
        {"type":"output_text","text":"second answer","annotations":[],"logprobs":[]}]});
    let reason = json!({"id":"rs-b02","type":"reasoning","status":"completed",
        "summary":[{"type":"summary_text","text":"short summary"}],
        "content":[{"type":"reasoning_text","text":"reasoning body"}]});
    let mut frames = vec![
        json!({"type":"response.created","response":{"id":"resp-b02","status":"in_progress"}}),
    ];
    for (output_index, item) in [&text, &reason].into_iter().enumerate() {
        let mut empty = item.clone();
        empty["status"] = json!("in_progress");
        empty["content"] = json!([]);
        if output_index == 1 {
            empty["summary"] = json!([]);
        }
        frames.push(
            json!({"type":"response.output_item.added","output_index":output_index,"item":empty}),
        );
        for field in if output_index == 0 {
            vec!["content"]
        } else {
            vec!["summary", "content"]
        } {
            for (index, part) in item[field].as_array().into_iter().flatten().enumerate() {
                let index_field = if field == "summary" {
                    "summary_index"
                } else {
                    "content_index"
                };
                let stem = if output_index == 0 {
                    "response.output_text"
                } else if field == "summary" {
                    "response.reasoning_summary_text"
                } else {
                    "response.reasoning_text"
                };
                let part_stem = if field == "summary" {
                    "response.reasoning_summary_part"
                } else {
                    "response.content_part"
                };
                let mut initial = part.clone();
                initial["text"] = json!("");
                frames.push(json!({"type":format!("{part_stem}.added"),"item_id":item["id"],"output_index":output_index,index_field:index,"part":initial}));
                // Literal expected output above is independent of this transport segmentation.
                for delta in part["text"]
                    .as_str()
                    .unwrap_or_default()
                    .split_inclusive(' ')
                {
                    frames.push(json!({"type":format!("{stem}.delta"),"item_id":item["id"],"output_index":output_index,index_field:index,"delta":delta}));
                }
                let mut done = json!({"type":format!("{stem}.done"),"item_id":item["id"],"output_index":output_index,index_field:index,"text":part["text"]});
                if output_index == 0 {
                    done["logprobs"] = json!([]);
                }
                frames.push(done);
                frames.push(json!({"type":format!("{part_stem}.done"),"item_id":item["id"],"output_index":output_index,index_field:index,"part":part}));
            }
        }
        frames.push(
            json!({"type":"response.output_item.done","output_index":output_index,"item":item}),
        );
    }
    frames.push(json!({"type":"response.completed","response":{"id":"resp-b02","status":"completed","output":[text,reason]}}));
    for (sequence, value) in frames.iter_mut().enumerate() {
        value["sequence_number"] = json!(sequence);
    }
    frames
}

fn wire(frames: &[Value]) -> String {
    let mut output = String::new();
    for value in frames {
        output.push_str("data: ");
        output.push_str(&value.to_string());
        output.push_str("\n\n");
    }
    output
}

fn chunks(frames: &[Value]) -> Vec<Vec<u8>> {
    vec![
        wire(&frames[..5]).into_bytes(),
        wire(&frames[5..]).into_bytes(),
    ]
}

async fn assert_public_failure(frames: Vec<Value>, label: &str) -> TestResult {
    let peer = Peer::start_scripted(false, None, Some(chunks(&frames)))?;
    let fixture = done_fixture(&peer)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(
                json!({"model":"p12-test-model","input":"question","stream":true,"store":true}),
            )
            .to_request(),
    )
    .await;
    let body = String::from_utf8(test::read_body(response).await.to_vec())?;
    assert!(!body.contains("response.completed"), "{label}: {body}");
    assert_eq!(
        body.matches("event: response.failed").count(),
        1,
        "{label}: {body}"
    );
    assert!(body.contains("UpstreamProtocolError"), "{label}: {body}");
    assert_persisted_outcome(&fixture, true)?;
    let get = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/responses/resp-b02")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .to_request(),
    )
    .await;
    assert_eq!(get.status(), StatusCode::NOT_FOUND, "{label}");
    let continuation = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(
                json!({"model":"p12-test-model","input":"next","previous_response_id":"resp-b02"}),
            )
            .to_request(),
    )
    .await;
    assert!(!continuation.status().is_success(), "{label}");
    assert_eq!(
        peer.received.lock().map_err(|_| "received")?.len(),
        1,
        "{label}"
    );
    assert_eq!(
        peer.requests.lock().map_err(|_| "requests")?.len(),
        1,
        "{label}"
    );
    Ok(())
}

#[actix_web::test]
async fn done_field_and_correlation_errors_fail_through_public_gateway() -> TestResult {
    for kind in [
        "response.output_text.done",
        "response.content_part.done",
        "response.reasoning_summary_part.done",
        "response.reasoning_summary_text.done",
        "response.reasoning_text.done",
    ] {
        let field = if kind.contains("summary") {
            "summary_index"
        } else {
            "content_index"
        };
        let payload = if kind.ends_with("part.done") {
            "part"
        } else {
            "text"
        };
        for missing in ["item_id", "output_index", field, "sequence_number", payload] {
            let mut frames = native_frames();
            let done = frames
                .iter_mut()
                .find(|value| value["type"] == kind)
                .ok_or("done")?;
            done.as_object_mut().ok_or("object")?.remove(missing);
            assert_public_failure(frames, &format!("{kind} missing {missing}")).await?;
        }
        for (key, value) in [
            ("item_id", json!("unknown-item")),
            ("item_id", json!(4)),
            ("output_index", json!(42)),
            ("output_index", json!("0")),
            (field, json!(9)),
            (field, json!(-1)),
            (field, json!(64)),
            ("sequence_number", json!("bad")),
            ("illegal_extension", json!(true)),
            (payload, json!(12)),
        ] {
            let mut frames = native_frames();
            let done = frames
                .iter_mut()
                .find(|value| value["type"] == kind)
                .ok_or("done")?;
            done[key] = value;
            assert_public_failure(frames, &format!("{kind} bad {key}")).await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn native_done_lifecycle_and_metadata_conflicts_fail_publicly() -> TestResult {
    for case in [
        "item-done-wrong-index",
        "delta-after-text-done",
        "annotation-after-part-done",
        "unknown-part-type",
        "extra-part-field",
        "nonempty-logprobs",
        "changed-annotations",
        "item-changed-confirmed-part",
        "item-added-suffix",
        "done-after-item",
        "done-after-response",
        "early-done",
        "changed-repeat",
        "bad-summary-status",
    ] {
        let mut frames = native_frames();
        let index = frames
            .iter()
            .position(|value| value["type"] == "response.output_text.done")
            .ok_or("done")?;
        let part_done = index + 1;
        match case {
            "item-done-wrong-index" => {
                frames.iter_mut().find(|value| value["type"] == "response.output_item.done").ok_or("item")?["output_index"] = json!(9);
            }
            "delta-after-text-done" => {let delta=frames[index-1].clone();frames.insert(index+1,delta);},
            "annotation-after-part-done" => frames.insert(part_done+1,json!({"type":"response.output_text.annotation.added","item_id":"msg-b02","content_index":0,"annotation_index":0,"annotation":{"type":"url_citation","start_index":0,"end_index":1,"url":"https://synthetic.example.test","title":"Synthetic"}})),
            "unknown-part-type" => frames[part_done]["part"]["type"]=json!("refusal"),
            "extra-part-field" => frames[part_done]["part"]["unknown"]=json!(true),
            "nonempty-logprobs" => frames[index]["logprobs"]=json!([{"token":"fake"}]),
            "changed-annotations" => {frames[part_done]["part"]["annotations"]=json!([{"type":"url_citation","start_index":0,"end_index":1,"url":"https://synthetic.example.test","title":"Synthetic"}]);},
            "item-changed-confirmed-part" | "item-added-suffix" => {
                let item_done=frames.iter_mut().find(|value|value["type"]=="response.output_item.done").ok_or("item")?;
                item_done["item"]["content"][0]["text"]=json!("first answer suffix");
            },
            "done-after-item" => {let done=frames.remove(index);let end=frames.iter().position(|value|value["type"]=="response.output_item.done").ok_or("item")?;frames.insert(end+1,done);},
            "done-after-response" => {let done=frames[index].clone();frames.push(done);},
            "early-done" => {let mut done=frames[index].clone();done["content_index"]=json!(12);frames.insert(index,done);},
            "changed-repeat" => {let mut done=frames[index].clone();done["text"]=json!("changed");frames.insert(index+1,done);},
            "bad-summary-status" => frames.iter_mut().find(|value|value["type"]=="response.reasoning_summary_part.done").ok_or("summary")?["status"]=json!("completed"),
            _ => return Err("unknown test case".into()),
        }
        assert_public_failure(frames, case).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn valid_native_done_preserves_parts_and_allows_owned_stored_continuation() -> TestResult {
    for variant in [
        "complete",
        "identical-repeat",
        "without-confirmations",
        "without-part-added",
        "initial-text",
    ] {
        let mut frames = native_frames();
        let expected = frames.last().ok_or("completed")?["response"]["output"].clone();
        if variant == "identical-repeat" {
            let index = frames
                .iter()
                .position(|value| value["type"] == "response.output_text.done")
                .ok_or("done")?;
            let done = frames[index].clone();
            frames.insert(index + 1, done);
        } else if variant == "without-confirmations" {
            frames.retain(|value| {
                !matches!(
                    value["type"].as_str(),
                    Some(
                        "response.output_text.done"
                            | "response.content_part.done"
                            | "response.reasoning_summary_part.done"
                            | "response.reasoning_summary_text.done"
                            | "response.reasoning_text.done"
                    )
                )
            });
        } else if variant == "without-part-added" {
            frames.retain(|value| {
                !matches!(
                    value["type"].as_str(),
                    Some("response.content_part.added" | "response.reasoning_summary_part.added")
                )
            });
        } else if variant == "initial-text" {
            let added = frames
                .iter_mut()
                .find(|value| value["type"] == "response.content_part.added")
                .ok_or("added")?;
            added["part"]["text"] = json!("first ");
            let delta = frames
                .iter()
                .position(|value| value["type"] == "response.output_text.delta")
                .ok_or("delta")?;
            frames.remove(delta);
        }
        let peer = Peer::start_scripted(false, None, Some(chunks(&frames)))?;
        let fixture = done_fixture(&peer)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(
                    json!({"model":"p12-test-model","input":"question","stream":true,"store":true}),
                )
                .to_request(),
        )
        .await;
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        let terminal = body
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .find(|value| value["type"] == "response.completed")
            .ok_or_else(|| format!("{variant}: {body}"))?;
        assert_eq!(terminal["response"]["output"], expected, "{variant}");
        assert_eq!(body.matches("event: response.completed").count(), 1);
        assert!(!body.contains("response.failed"));
        assert_persisted_outcome(&fixture, false)?;
        let get = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/responses/resp-b02")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .to_request(),
        )
        .await;
        assert_eq!(get.status(), StatusCode::OK);
        let saved: Value = serde_json::from_slice(&test::read_body(get).await)?;
        assert_eq!(saved["output"], expected);
        let continuation=test::call_service(&app,test::TestRequest::post().uri("/v1/responses")
            .insert_header(("authorization",format!("Bearer {}",fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":"next","previous_response_id":"resp-b02","stream":true,"store":true})).to_request()).await;
        let continued = String::from_utf8(test::read_body(continuation).await.to_vec())?;
        assert!(
            continued.contains("response.completed"),
            "{variant}: {continued}"
        );
        assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), 2);
        assert_eq!(peer.requests.lock().map_err(|_| "requests")?.len(), 2);
    }
    Ok(())
}

#[actix_web::test]
async fn done_conflict_never_registers_a_websocket_root_or_stored_response() -> TestResult {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    for invalid in [false, true] {
        let peer = Peer::start_scripted(false, None, Some(text_stream(invalid)))?;
        let fixture = done_fixture(&peer)?;
        let state = fixture.state.clone();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(gateway_http_actix::configure)
        })
        .workers(1)
        .disable_signals()
        .listen(listener)?
        .run();
        let handle = server.handle();
        actix_web::rt::spawn(server);
        let mut request = format!("ws://{address}/v1/responses").into_client_request()?;
        request
            .headers_mut()
            .insert("authorization", format!("Bearer {}", fixture.key).parse()?);
        let (mut socket, upgrade) = tokio_tungstenite::connect_async(request).await?;
        assert_eq!(upgrade.status(), 101);
        socket.send(Message::Text(json!({"type":"response.create","model":"p12-test-model","input":"question","store":true}).to_string().into())).await?;
        let mut first = Vec::new();
        loop {
            let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
                .await?
                .ok_or("socket closed")??;
            if let Message::Text(text) = message {
                let value: Value = serde_json::from_str(&text)?;
                let ended = matches!(
                    value["type"].as_str(),
                    Some("response.completed" | "response.failed" | "error")
                );
                first.push(value);
                if ended {
                    break;
                }
            }
        }
        assert_eq!(
            first.last().ok_or("terminal")?["type"],
            if invalid {
                "response.failed"
            } else {
                "response.completed"
            }
        );
        assert_persisted_outcome(&fixture, invalid)?;
        let get = reqwest::Client::new()
            .get(format!("http://{address}/v1/responses/resp-b02"))
            .bearer_auth(&fixture.key)
            .send()
            .await?;
        assert_eq!(get.status().as_u16(), if invalid { 404 } else { 200 });
        socket.send(Message::Text(json!({"type":"response.create","model":"p12-test-model","previous_response_id":"resp-b02","input":"next","store":true}).to_string().into())).await?;
        let terminal = loop {
            let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
                .await?
                .ok_or("socket closed")??;
            if let Message::Text(text) = message {
                let value: Value = serde_json::from_str(&text)?;
                if matches!(
                    value["type"].as_str(),
                    Some("response.completed" | "response.failed" | "error")
                ) {
                    break value;
                }
            }
        };
        assert_eq!(
            terminal["type"],
            if invalid {
                "error"
            } else {
                "response.completed"
            }
        );
        assert_eq!(
            peer.received.lock().map_err(|_| "received")?.len(),
            if invalid { 1 } else { 2 }
        );
        assert_eq!(
            peer.requests.lock().map_err(|_| "requests")?.len(),
            if invalid { 1 } else { 2 }
        );
        socket.close(None).await?;
        handle.stop(false).await;
    }
    Ok(())
}

#[actix_web::test]
async fn done_frames_are_read_boundary_invariant_and_rejection_is_permanent() -> TestResult {
    use protocol_openai_responses::OpenAiResponsesSseDecoder;
    let frames = native_frames();
    let bytes = wire(&frames);
    let mut complete = OpenAiResponsesSseDecoder::new();
    let expected = complete.push(bytes.as_bytes())?;
    for size in [1, 2, 7, 31, 128, bytes.len()] {
        let mut decoder = OpenAiResponsesSseDecoder::new();
        let mut actual = Vec::new();
        for chunk in bytes.as_bytes().chunks(size) {
            actual.extend(decoder.push(chunk)?);
        }
        actual.extend(decoder.finish()?);
        assert_eq!(actual, expected, "read size {size}");
    }
    for kind in [
        "response.output_text.done",
        "response.content_part.done",
        "response.reasoning_summary_part.done",
        "response.reasoning_summary_text.done",
        "response.reasoning_text.done",
    ] {
        let mut conflict = frames.clone();
        let position = conflict
            .iter()
            .position(|value| value["type"] == kind)
            .ok_or("done")?;
        if kind.ends_with("part.done") {
            conflict[position]["part"]["text"] = json!("conflict");
        } else {
            conflict[position]["text"] = json!("conflict");
        }
        let prefix = wire(&conflict[..position]);
        let tail = wire(&conflict[position..]);
        for size in [1, 7, 31, tail.len()] {
            let mut decoder = OpenAiResponsesSseDecoder::new();
            let _ = decoder.push(prefix.as_bytes())?;
            let mut rejected = false;
            for chunk in tail.as_bytes().chunks(size) {
                if decoder.push(chunk).is_err() {
                    rejected = true;
                    break;
                }
            }
            assert!(rejected, "{kind}, read {size}");
            assert!(decoder.push(b"\n").is_err());
            assert!(decoder.finish().is_err());
            assert!(!decoder.is_finished());
        }
    }
    Ok(())
}

#[actix_web::test]
async fn bootstrap_done_conflict_records_failed_attempt_and_no_owned_result() -> TestResult {
    use gateway_core::{AttemptOutcome, RequestOutcome};
    let peer = Peer::start_scripted(false, None, Some(vec![text_stream(true).concat()]))?;
    let fixture = done_fixture(&peer)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(
                json!({"model":"p12-test-model","input":"question","stream":true,"store":true}),
            )
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = String::from_utf8(test::read_body(response).await.to_vec())?;
    assert!(body.contains("StreamTruncated"), "{body}");
    let events = fixture.events.0.lock().map_err(|_| "events")?.clone();
    let database = fixture.directory.path().join("bootstrap-events.sqlite3");
    gateway_store::event_store::SqliteEventStore::open(&database)?.append_batch(&events)?;
    let rows = gateway_store::event_store::SqliteEventStore::open(&database)?.list_events()?;
    let attempt = rows
        .iter()
        .find_map(|row| match row.event() {
            GatewayEvent::Attempt(a) => Some(a),
            _ => None,
        })
        .ok_or("Attempt")?;
    assert!(
        matches!(attempt.outcome(),AttemptOutcome::Failed(error) if error.code()==GatewayErrorCode::StreamTruncated)
    );
    let request = rows
        .iter()
        .find_map(|row| match row.event() {
            GatewayEvent::RequestFinished(r) => Some(r),
            _ => None,
        })
        .ok_or("Request")?;
    assert_eq!(request.outcome, RequestOutcome::Failed);
    assert_eq!(&request.request_id, attempt.request_id());
    assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), 1);
    let get = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/v1/responses/resp-b02")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .to_request(),
    )
    .await;
    assert_eq!(get.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[actix_web::test]
async fn contradictory_native_done_confirmations_fail_public_storage_and_continuation() -> TestResult
{
    for kind in [
        "response.content_part.done",
        "response.reasoning_summary_part.done",
        "response.reasoning_summary_text.done",
        "response.reasoning_text.done",
    ] {
        let mut frames = native_frames();
        let done = frames
            .iter_mut()
            .find(|value| value["type"] == kind)
            .ok_or("done")?;
        if kind.ends_with("part.done") {
            done["part"]["text"] = json!("conflict");
        } else {
            done["text"] = json!("conflict");
        }
        let peer = Peer::start_scripted(false, None, Some(chunks(&frames)))?;
        let fixture = done_fixture(&peer)?;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(fixture.state.clone()))
                .configure(gateway_http_actix::configure),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(
                    json!({"model":"p12-test-model","input":"question","stream":true,"store":true}),
                )
                .to_request(),
        )
        .await;
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        assert!(!body.contains("response.completed"), "{kind}: {body}");
        assert!(body.contains("response.failed"), "{kind}: {body}");
        assert!(body.contains("UpstreamProtocolError"), "{kind}: {body}");
        assert_persisted_outcome(&fixture, true)?;
        let get = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/responses/resp-b02")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .to_request(),
        )
        .await;
        assert_eq!(get.status(), StatusCode::NOT_FOUND);
        assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), 1);
    }
    Ok(())
}
