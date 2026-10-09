//! B-01 replay uses the actual public handler, native codec and loopback HTTP transport.
use super::*;

mod phase_admission;

fn multipart_output() -> Value {
    json!([{
        "id":"msg-b01", "type":"message", "role":"assistant",
        "status":"completed",
        "content":[
            {"type":"output_text","text":"first segment","logprobs":null,
             "annotations":[{"type":"url_citation","start_index":0,"end_index":5,
                 "url":"https://first.example.test/source","title":"Synthetic first source"}]},
            {"type":"output_text","text":"second segment","logprobs":[],
             "annotations":[{"type":"url_citation","start_index":0,"end_index":6,
                 "url":"https://second.example.test/source","title":"Synthetic second source"}]}
        ]
    }])
}

pub(super) fn reply(body: &Value, output: &Value) -> Result<HttpResponse, actix_web::Error> {
    let response = json!({"id":"resp-b01","object":"response","status":"completed",
        "model":body["model"],"output":output,
        "usage":{"input_tokens":3,"output_tokens":4,"total_tokens":7}});
    if body["stream"] != true {
        return Ok(HttpResponse::Ok().json(response));
    }
    let mut frames = String::new();
    let mut sequence = 0;
    let mut emit = |kind: &str, mut payload: Value| {
        payload["sequence_number"] = json!(sequence);
        sequence += 1;
        frame(&mut frames, kind, payload)
    };
    let start = json!({"response":{"id":"resp-b01","object":"response",
        "status":"in_progress","model":body["model"],"output":[]}});
    emit("response.created", start.clone())?;
    emit("response.in_progress", start)?;
    let item = &output[0];
    let mut empty = item.clone();
    empty["status"] = json!("in_progress");
    empty["content"] = json!([]);
    emit(
        "response.output_item.added",
        json!({"output_index":0,"item":empty}),
    )?;
    let parts = item["content"]
        .as_array()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("fixture parts"))?;
    for (index, part) in parts.iter().enumerate() {
        let mut empty = part.clone();
        empty["text"] = json!("");
        empty["annotations"] = json!([]);
        emit(
            "response.content_part.added",
            json!({"output_index":0,"item_id":item["id"],
            "content_index":index,"part":empty}),
        )?;
        let text = part["text"]
            .as_str()
            .ok_or_else(|| actix_web::error::ErrorInternalServerError("fixture text"))?;
        let (prefix, suffix) = text
            .split_once(' ')
            .ok_or_else(|| actix_web::error::ErrorInternalServerError("fixture split"))?;
        for delta in [format!("{prefix} "), suffix.to_owned()] {
            emit(
                "response.output_text.delta",
                json!({"output_index":0,"item_id":item["id"],
                "content_index":index,"delta":delta,"logprobs":[]}),
            )?;
        }
        emit(
            "response.output_text.annotation.added",
            json!({"output_index":0,
            "item_id":item["id"],"content_index":index,"annotation_index":0,
            "annotation":part["annotations"][0]}),
        )?;
        emit(
            "response.output_text.done",
            json!({"output_index":0,"item_id":item["id"],
            "content_index":index,"text":text,"logprobs":[]}),
        )?;
        emit(
            "response.content_part.done",
            json!({"output_index":0,"item_id":item["id"],
            "content_index":index,"part":part}),
        )?;
    }
    emit(
        "response.output_item.done",
        json!({"output_index":0,"item":item}),
    )?;
    emit("response.completed", json!({"response":response}))?;
    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .body(frames))
}

fn history_fixture(peer: &Arc<Peer>) -> Result<Fixture, Box<dyn Error>> {
    fixture(
        Channel {
            adapter: "openai-compatible.responses",
            format: "openai/responses",
            base: "https://gateway.example.test/v1",
            path: "/responses",
            model: "upstream-c",
        },
        b"synthetic-key",
        |endpoint| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.http = Some(peer.clone());
            endpoint.transports = Arc::new(profiles);
            endpoint.resolver = Arc::new(PublicResolver);
            Ok(())
        },
    )
}

fn completed_response(
    bytes: &[u8],
    streaming: bool,
    expected: &Value,
) -> Result<Value, Box<dyn Error>> {
    if !streaming {
        return Ok(serde_json::from_slice(bytes)?);
    }
    let frames = std::str::from_utf8(bytes)?
        .split("\n\n")
        .filter_map(|frame| frame.lines().find_map(|line| line.strip_prefix("data: ")))
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert!(
        !frames
            .iter()
            .any(|frame| frame["type"] == "response.failed")
    );
    let deltas = frames
        .iter()
        .filter(|frame| frame["type"] == "response.output_text.delta")
        .map(|frame| (frame["content_index"].clone(), frame["delta"].clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        deltas,
        vec![
            (json!(0), json!("first ")),
            (json!(0), json!("segment")),
            (json!(1), json!("second ")),
            (json!(1), json!("segment"))
        ]
    );
    let parts = frames
        .iter()
        .filter(|frame| frame["type"] == "response.content_part.done")
        .map(|frame| (frame["content_index"].clone(), frame["part"].clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        parts,
        vec![
            (json!(0), expected[0]["content"][0].clone()),
            (json!(1), expected[0]["content"][1].clone())
        ]
    );
    let completed = frames
        .iter()
        .filter(|frame| frame["type"] == "response.completed")
        .collect::<Vec<_>>();
    assert_eq!(completed.len(), 1);
    Ok(completed[0]["response"].clone())
}

async fn assert_multipart_replay(streaming: bool) -> TestResult {
    let expected = multipart_output();
    let peer = Peer::start(false, Some(expected.clone()))?;
    let fixture = history_fixture(&peer)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let initial = json!({"type":"message","role":"user",
        "content":[{"type":"input_text","text":"question"}]});
    let next = json!({"type":"message","role":"user",
        "content":[{"type":"input_text","text":"next-question"}]});
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(
                json!({"model":"p12-test-model","input":[initial.clone()],"stream":streaming}),
            )
            .to_request(),
    )
    .await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let terminal = completed_response(&bytes, streaming, &expected)?;
    assert_eq!(terminal["status"], "completed");
    assert_eq!(terminal["output"], expected);
    assert_eq!(peer.requests.lock().map_err(|_| "requests")?.len(), 1);
    assert_eq!(peer.received.lock().map_err(|_| "received")?.len(), 1);
    // Only the returned output items belong in input; response-level model/usage/error do not.
    let mut history = vec![initial.clone()];
    history.extend(
        terminal["output"]
            .as_array()
            .ok_or("returned output")?
            .iter()
            .cloned(),
    );
    history.push(next.clone());
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("Bearer {}", fixture.key)))
            .set_json(json!({"model":"p12-test-model","input":history,"stream":streaming}))
            .to_request(),
    )
    .await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "replay; upstream calls {}: {}",
        peer.requests.lock().map_err(|_| "requests")?.len(),
        String::from_utf8_lossy(&bytes)
    );
    let terminal = completed_response(&bytes, streaming, &expected)?;
    assert_eq!(terminal["output"], expected);
    let received = peer.received.lock().map_err(|_| "received")?;
    assert_eq!(received.len(), 2);
    assert_eq!(received[1]["input"], json!([initial, expected[0], next]));
    assert_eq!(received[1]["model"], "upstream-c");
    assert_eq!(received[1]["stream"], streaming);
    let requests = peer.requests.lock().map_err(|_| "requests")?;
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].body["input"],
        json!([initial, expected[0], next])
    );
    assert_eq!(requests[1].body["model"], "upstream-c");
    assert_eq!(requests[1].url, "https://gateway.example.test/v1/responses");
    Ok(())
}

#[actix_web::test]
async fn responses_json_multipart_output_replays_exactly_through_public_http() -> TestResult {
    assert_multipart_replay(false).await
}

#[actix_web::test]
async fn responses_sse_multipart_output_replays_exactly_through_public_http() -> TestResult {
    assert_multipart_replay(true).await
}

#[actix_web::test]
async fn responses_multipart_history_rejects_invalid_semantics_before_upstream_http() -> TestResult
{
    let output = multipart_output();
    let peer = Peer::start(false, Some(output.clone()))?;
    let fixture = history_fixture(&peer)?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(fixture.state.clone()))
            .configure(gateway_http_actix::configure),
    )
    .await;
    let mut cases = Vec::new();
    for (name, field, value) in [
        ("illegal message role", "role", json!("tool")),
        ("unknown message field", "unreviewed", json!(true)),
        ("incomplete history item", "status", json!("incomplete")),
        ("unsupported phase", "phase", json!("analysis")),
    ] {
        let mut item = output[0].clone();
        item[field] = value;
        cases.push((name, json!([item])));
    }
    for (name, field, value) in [
        ("unknown second-part field", "unreviewed", json!(true)),
        (
            "unrepresentable logprobs",
            "logprobs",
            json!([{
                "token":"synthetic","bytes":[115],"logprob":-1.0,"top_logprobs":[]
            }]),
        ),
        (
            "malformed second-part citation",
            "annotations",
            json!([{
                "type":"url_citation","start_index":6,"end_index":0,
                "url":"https://example.test/invalid","title":"Synthetic invalid range"
            }]),
        ),
    ] {
        let mut item = output[0].clone();
        item["content"][1][field] = value;
        cases.push((name, json!([item])));
    }
    let mut unsupported = output[0].clone();
    unsupported["content"][1] = json!({"type":"refusal","refusal":"synthetic refusal"});
    cases.push(("unsupported second part", json!([unsupported])));
    cases.push((
        "orphan tool result",
        json!([output[0], {
            "type":"function_call_output","call_id":"orphan","output":"synthetic result"
        }]),
    ));
    cases.push((
        "multipart assistant overtakes pending tool",
        json!([{
        "type":"function_call","id":"pending-item","status":"completed",
        "call_id":"pending-call","name":"lookup","arguments":"{}"
    }, output[0]]),
    ));
    for (name, input) in cases {
        for streaming in [false, true] {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                    .set_json(json!({"model":"p12-test-model","input":input,"stream":streaming,
                        "tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]}))
                    .to_request(),
            )
            .await;
            let status = response.status();
            let bytes = test::read_body(response).await;
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "{name} streaming={streaming}"
            );
            let error: Value = serde_json::from_slice(&bytes)?;
            assert_eq!(error["error"]["code"], "ClientRequestError", "{name}");
            assert_eq!(
                peer.requests.lock().map_err(|_| "requests")?.len(),
                0,
                "{name}"
            );
            assert_eq!(fixture.events.attempts()?, 0, "{name}");
            assert_eq!(
                peer.received.lock().map_err(|_| "received")?.len(),
                0,
                "{name}"
            );
        }
    }
    Ok(())
}
