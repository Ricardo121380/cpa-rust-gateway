//! B-03 checks whole-output order through the same public path as B-02 confirmations.
use super::*;

pub(in crate::runtime::batch_c::http) fn fallback_fixture(
    first: &Arc<Peer>,
    fallback: &Arc<Peer>,
    channel: Channel,
) -> Result<Fixture, Box<dyn Error>> {
    let fallback_id = EndpointId::try_new("b03-fallback-endpoint")?;
    fixture_with_configuration(
        channel,
        b"synthetic-key",
        channel.format == "openai/responses",
        |configuration| {
            configuration.model_routes[0].max_attempts = 2;
            let mut upstream = configuration.upstreams[0].clone();
            upstream.id = gateway_core::UpstreamId::try_new("b03-fallback-upstream")?;
            upstream.name = "B-03 synthetic fallback upstream".into();
            let mut endpoint = configuration.endpoints[0].clone();
            endpoint.id = fallback_id.clone();
            endpoint.upstream_id = upstream.id.clone();
            configuration.endpoints.push(endpoint);
            let credential_id = gateway_core::CredentialId::try_new("b03-fallback-credential")?;
            let credential = gateway_store::control_plane::CredentialConfiguration {
                id: credential_id.clone(),
                upstream_id: upstream.id.clone(),
                kind: "bearer".into(),
                encrypted_secret: test_secret_store()?.seal(
                    b"synthetic-key",
                    &credential_associated_data(
                        &configuration.version.id,
                        &credential_id,
                        &upstream.id,
                    )?,
                )?,
                status: gateway_store::control_plane::CredentialStatus::Active,
                revision: 1,
            };
            let mut binding = configuration.endpoint_credential_bindings[0].clone();
            binding.endpoint_id = fallback_id.clone();
            binding.upstream_id = upstream.id.clone();
            binding.credential_id = credential.id.clone();
            configuration.endpoint_credential_bindings.push(binding);
            configuration.upstreams.push(upstream);
            configuration.credentials.push(credential);
            let mut candidate = configuration.route_candidates[0].clone();
            candidate.id = gateway_core::RouteCandidateId::try_new("b03-fallback-candidate")?;
            candidate.endpoint_id = fallback_id.clone();
            candidate.priority = 1;
            configuration.route_candidates.push(candidate);
            Ok(())
        },
        |endpoints| {
            for (id, endpoint) in endpoints {
                let mut profiles = P12TransportProfiles::try_new()?;
                profiles.http = Some(if *id == fallback_id {
                    fallback.clone()
                } else {
                    first.clone()
                });
                endpoint.transports = Arc::new(profiles);
                endpoint.resolver = Arc::new(PublicResolver);
            }
            Ok(())
        },
    )
}

#[actix_web::test]
async fn bootstrap_order_conflict_cannot_replay_to_a_usable_fallback() -> TestResult {
    // A transport-only bootstrap EOF proves this second candidate and retry budget are usable.
    for conflict in [false, true] {
        let mut frames = native_frames();
        reverse_terminal(&mut frames)?;
        let first = Peer::start_scripted(
            false,
            None,
            Some(if conflict {
                vec![wire(&frames).into_bytes()]
            } else {
                vec![]
            }),
        )?;
        let fallback = Peer::start_scripted(false, None, Some(chunks(&native_frames())))?;
        let fixture = fallback_fixture(
            &first,
            &fallback,
            Channel {
                adapter: "openai-compatible.responses",
                format: "openai/responses",
                base: "https://gateway.example.test/v1",
                path: "/responses",
                model: "upstream-c",
            },
        )?;
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
                    json!({"model":"upstream-c","input":"question","stream":true,"store":true}),
                )
                .to_request(),
        )
        .await;
        let status = response.status();
        let body = String::from_utf8(test::read_body(response).await.to_vec())?;
        let first_count = first.received.lock().map_err(|_| "received")?.len();
        let fallback_count = fallback.received.lock().map_err(|_| "received")?.len();
        eprintln!(
            "B-03 bootstrap conflict={conflict}; status={status}; first={first_count}; fallback={fallback_count}; attempts={}; body={body}",
            fixture.events.attempts()?
        );
        assert_eq!(first_count, 1);
        assert_eq!(fallback_count, usize::from(!conflict));
        assert_eq!(fixture.events.attempts()?, if conflict { 1 } else { 2 });
        assert_eq!(first.requests.lock().map_err(|_| "requests")?.len(), 1);
        assert_eq!(
            fallback.requests.lock().map_err(|_| "requests")?.len(),
            usize::from(!conflict)
        );
        if conflict {
            assert_eq!(status, StatusCode::BAD_GATEWAY);
            assert!(body.contains("UpstreamProtocolError"), "{body}");
            assert!(!body.contains("response.completed"));
            let events = fixture.events.0.lock().map_err(|_| "events")?.clone();
            let database = fixture
                .directory
                .path()
                .join("b03-bootstrap-events.sqlite3");
            gateway_store::event_store::SqliteEventStore::open(&database)?.append_batch(&events)?;
            let rows =
                gateway_store::event_store::SqliteEventStore::open(&database)?.list_events()?;
            assert!(rows.iter().any(|row| matches!(row.event(), GatewayEvent::RequestFinished(request) if request.outcome == gateway_core::RequestOutcome::Failed && request.error_code == Some(GatewayErrorCode::UpstreamProtocolError))));
            assert!(rows.iter().any(|row| matches!(row.event(), GatewayEvent::Attempt(attempt) if matches!(attempt.outcome(), gateway_core::AttemptOutcome::Failed(error) if error.code() == GatewayErrorCode::UpstreamProtocolError))));
        } else {
            assert_eq!(status, StatusCode::OK);
            assert!(body.contains("response.completed"), "{body}");
            assert!(!body.contains("response.failed"));
        }
        let get = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/responses/resp-b02")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .to_request(),
        )
        .await;
        assert_eq!(
            get.status(),
            if conflict {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::OK
            }
        );
        if conflict {
            let continuation = test::call_service(&app, test::TestRequest::post().uri("/v1/responses")
                .insert_header(("authorization", format!("Bearer {}", fixture.key)))
                .set_json(json!({"model":"upstream-c","input":"next","previous_response_id":"resp-b02"})).to_request()).await;
            assert!(!continuation.status().is_success());
            assert_eq!(first.received.lock().map_err(|_| "received")?.len(), 1);
            assert_eq!(fallback.received.lock().map_err(|_| "received")?.len(), 0);
        }
    }
    Ok(())
}

fn reverse_terminal(frames: &mut [Value]) -> TestResult {
    frames.last_mut().ok_or("completed")?["response"]["output"]
        .as_array_mut()
        .ok_or("output")?
        .swap(0, 1);
    Ok(())
}

fn interleaved_frames(reasoning_finishes_first: bool) -> Result<Vec<Value>, Box<dyn Error>> {
    use std::collections::VecDeque;
    let mut original = native_frames();
    let completed = original.pop().ok_or("completed")?;
    let created = original.remove(0);
    let mut text = original
        .iter()
        .filter(|value| value["output_index"] == 0)
        .cloned()
        .collect::<VecDeque<_>>();
    let mut reason = original
        .into_iter()
        .filter(|value| value["output_index"] == 1)
        .collect::<VecDeque<_>>();
    let mut frames = vec![
        created,
        text.pop_front().ok_or("text start")?,
        reason.pop_front().ok_or("reason start")?,
    ];
    let reason_done = reason.pop_back().ok_or("reason done")?;
    while !text.is_empty() || !reason.is_empty() {
        if let Some(value) = reason.pop_front() {
            frames.push(value);
        }
        if reasoning_finishes_first && reason.is_empty() && !text.is_empty() {
            frames.push(reason_done.clone());
            frames.extend(text.drain(..));
        } else if let Some(value) = text.pop_front() {
            frames.push(value);
        }
    }
    if !reasoning_finishes_first {
        frames.push(reason_done);
    }
    frames.push(completed);
    for (sequence, frame) in frames.iter_mut().enumerate() {
        frame["sequence_number"] = json!(sequence);
    }
    Ok(frames)
}

#[actix_web::test]
async fn reversed_terminal_output_fails_publicly_without_storage_or_replay() -> TestResult {
    let mut frames = native_frames();
    reverse_terminal(&mut frames)?;
    eprintln!(
        "B-03 upstream terminal={}",
        frames.last().ok_or("completed")?
    );
    assert_public_failure(frames, "B-03 reversed final output").await
}

#[actix_web::test]
async fn terminal_order_rejection_cannot_be_repaired_by_later_completion() -> TestResult {
    for same_read in [true, false] {
        let mut frames = native_frames();
        let valid = frames.last().ok_or("completed")?.clone();
        reverse_terminal(&mut frames)?;
        let mut stream = chunks(&frames);
        let recovery = wire(&[valid]).into_bytes();
        if same_read {
            stream.last_mut().ok_or("tail")?.extend(recovery);
        } else {
            stream.push(recovery);
        }
        assert_public_failure_chunks(stream, "B-03 later completed").await?;
    }
    Ok(())
}

#[actix_web::test]
async fn terminal_item_identity_and_index_conflicts_fail_publicly() -> TestResult {
    for variant in [
        "duplicate-final",
        "missing-final",
        "unknown-final",
        "duplicate-index",
        "conflicting-done-index",
        "gap-index",
        "reversed-indices",
        "reversed-incomplete",
    ] {
        let mut frames = native_frames();
        let terminal = frames.last_mut().ok_or("completed")?;
        match variant {
            "duplicate-final" => {
                terminal["response"]["output"][1] = terminal["response"]["output"][0].clone();
            }
            "missing-final" => {
                terminal["response"]["output"]
                    .as_array_mut()
                    .ok_or("output")?
                    .pop();
            }
            "unknown-final" => terminal["response"]["output"][1]["id"] = json!("unknown-item"),
            "reversed-incomplete" => {
                terminal["type"] = json!("response.incomplete");
                terminal["response"]["status"] = json!("incomplete");
                terminal["response"]["incomplete_details"] = json!({"reason":"max_output_tokens"});
                reverse_terminal(&mut frames)?;
            }
            _ => {
                for frame in &mut frames {
                    if frame["output_index"] == 1 {
                        frame["output_index"] = json!(if variant == "gap-index" { 2 } else { 0 });
                        if variant == "duplicate-index"
                            && frame["type"] != "response.output_item.added"
                            || variant == "conflicting-done-index"
                                && frame["type"] != "response.output_item.done"
                        {
                            frame["output_index"] = json!(1);
                        }
                    } else if variant == "reversed-indices" && frame["output_index"] == 0 {
                        frame["output_index"] = json!(1);
                    }
                }
            }
        }
        if variant == "reversed-indices" {
            assert_bootstrap_failure(chunks(&frames)).await?;
        } else {
            assert_public_failure(frames, variant).await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn same_read_terminal_order_conflict_fails_during_bootstrap_without_owned_result()
-> TestResult {
    let mut frames = native_frames();
    reverse_terminal(&mut frames)?;
    assert_bootstrap_failure(vec![wire(&frames).into_bytes()]).await
}

#[actix_web::test]
async fn interleaved_native_output_preserves_order_metadata_storage_and_continuation() -> TestResult
{
    let expected = native_frames().last().ok_or("completed")?["response"]["output"].clone();
    for variant in [
        "sequential",
        "text-finishes-first",
        "reasoning-finishes-first",
        "without-final-output",
        "legacy-indices",
        "snapshot-content",
    ] {
        let mut frames = if variant == "sequential" {
            native_frames()
        } else {
            interleaved_frames(true)?
        };
        if variant == "text-finishes-first" {
            frames = interleaved_frames(false)?;
        } else if variant == "without-final-output" {
            frames.last_mut().ok_or("completed")?["response"]
                .as_object_mut()
                .ok_or("response")?
                .remove("output");
        } else if variant == "legacy-indices" || variant == "snapshot-content" {
            frames.retain(|value| {
                matches!(
                    value["type"].as_str(),
                    Some(
                        "response.created"
                            | "response.output_item.added"
                            | "response.output_item.done"
                            | "response.completed"
                    )
                ) || variant == "legacy-indices"
                    && matches!(
                        value["type"].as_str(),
                        Some(
                            "response.output_text.delta"
                                | "response.reasoning_summary_text.delta"
                                | "response.reasoning_text.delta"
                        )
                    )
            });
            for frame in &mut frames {
                frame.as_object_mut().ok_or("frame")?.remove("output_index");
            }
        }
        assert_public_success(frames, &expected, variant).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn terminal_order_conflict_has_no_websocket_root_and_valid_interleaving_can_continue()
-> TestResult {
    for invalid in [false, true] {
        let mut frames = interleaved_frames(true)?;
        let expected = frames.last().ok_or("completed")?["response"]["output"].clone();
        if invalid {
            reverse_terminal(&mut frames)?;
        }
        assert_websocket_result(chunks(&frames), invalid, Some(&expected)).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn output_order_checks_are_read_boundary_invariant_and_permanent() -> TestResult {
    use protocol_openai_responses::OpenAiResponsesSseDecoder;
    let frames = interleaved_frames(true)?;
    let bytes = wire(&frames);
    let expected = OpenAiResponsesSseDecoder::new().push(bytes.as_bytes())?;
    for size in [1, 7, 31, bytes.len()] {
        let mut decoder = OpenAiResponsesSseDecoder::new();
        let mut events = Vec::new();
        for chunk in bytes.as_bytes().chunks(size) {
            events.extend(decoder.push(chunk)?);
        }
        events.extend(decoder.finish()?);
        assert_eq!(events, expected, "legal size {size}");
    }
    let mut conflict = frames.clone();
    reverse_terminal(&mut conflict)?;
    let prefix = wire(&conflict[..conflict.len() - 1]);
    let tail = wire(&conflict[conflict.len() - 1..]);
    for size in [1, 7, 31, tail.len()] {
        let mut decoder = OpenAiResponsesSseDecoder::new();
        let mut events = decoder.push(prefix.as_bytes())?;
        let mut rejected = false;
        for chunk in tail.as_bytes().chunks(size) {
            if let Ok(batch) = decoder.push(chunk) {
                events.extend(batch);
            } else {
                rejected = true;
                break;
            }
        }
        assert!(rejected, "conflict size {size}");
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, gateway_core::CanonicalEvent::ResponseEnd(_)))
        );
        assert!(
            decoder
                .push(wire(&frames[frames.len() - 1..]).as_bytes())
                .is_err()
        );
        assert!(decoder.finish().is_err());
        assert!(!decoder.is_finished());
    }
    Ok(())
}
