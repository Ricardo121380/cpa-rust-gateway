//! Closed fields/history and independent zero-receiver rejections.
use super::*;

async fn rejected(streaming: bool, request: String, capture: Option<&str>) -> TestResult {
    let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
    let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
    let recording = Recording::start(&fixture, false)?;
    let (url, server) = namespaces::listener(&fixture)?;
    let (status, id, bytes) = send(&fixture, &url, request).await?;
    assert_eq!(status, 400, "{}", String::from_utf8_lossy(&bytes));
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
    assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
    assert_eq!(active_leases(&fixture), 0);
    if let Some(raw) = capture {
        assert_metadata(&fixture, &id, raw, 0)?;
    }
    assert!(!String::from_utf8_lossy(&bytes).contains("M4_PRIVATE"));
    server.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
    assert_eq!(
        rows.iter()
            .filter(|row| matches!(row.event(), GatewayEvent::Request(_)))
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|row| matches!(row.event(), GatewayEvent::RequestFinished(_)))
            .count(),
        1
    );
    assert!(rows.iter().all(|row| !matches!(
        row.event(),
        GatewayEvent::Attempt(_) | GatewayEvent::Usage(_)
    )));
    let captured: i64 = gateway_store::open(&database)?.query_row(
        "SELECT count(*) FROM request_metadata_evidence",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(captured, i64::from(capture.is_some()));
    let worker = crate::billing_worker::BillingWorker::start(
        database.clone(),
        Arc::new(gateway_control::billing_processing::BillingProcessingMonitor::default()),
    );
    worker.stop().await.map_err(|()| "billing")?;
    assert!(
        SqliteBillingLedger::open_read_only(database)?
            .list_bounded(10)?
            .is_empty()
    );
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_m4_rejects_unreviewed_shapes_without_truncation() -> TestResult {
    let oversized = format!(
        "{{{}}}",
        " ".repeat(gateway_core::MAX_REQUEST_METADATA_BYTES - 1)
    );
    let long = json!({"user_id":"🐦".repeat(513)}).to_string();
    for streaming in [false, true] {
        for raw in [
            "null",
            "[]",
            "\"id\"",
            "{\"user_id\":null}",
            "{\"user_id\":1}",
            "{\"user_id\":{}}",
            "{\"user_id\":\"a\",\"extra\":\"b\"}",
            "{\"user_id\":\"a\",\"user_id\":\"b\"}",
            r#"{"user_id":"a","user\u005fid":"b"}"#,
            &long,
            &oversized,
        ] {
            let request = json!({"model":"grok-4.5","max_tokens":19,"stream":streaming,"messages":[{"role":"user","content":"question"}]});
            rejected(streaming, with_metadata(&request, raw), None).await?;
        }
        let raw = body(streaming, false, true);
        let repeated = format!("{},\"metadata\":{{}}}}", &raw[..raw.len() - 1]);
        rejected(streaming, repeated, None).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_m4_retains_empty_unicode_opaque_and_exact_boundary() -> TestResult {
    let exact = format!(
        "{{{}}}",
        " ".repeat(gateway_core::MAX_REQUEST_METADATA_BYTES - 2)
    );
    let unicode = json!({"user_id":"🐦".repeat(512)}).to_string();
    for raw in [
        "{}",
        r#"{ "user_id" : "" }"#,
        r#"{"user_id":"{\"device_id\":1,\"device_id\":2}"}"#,
        r#"{ "user_id":"\u4e2d文 e\u0301\n\u0000" }"#,
        &unicode,
        &exact,
    ] {
        let peer = ModelPeer::start_wire(false, tool_rounds::wire(1, false)?)?;
        let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let request = json!({"model":"grok-4.5","max_tokens":19,"messages":[{"role":"user","content":"question"}]});
        let (status, id, _) = send(&fixture, &url, with_metadata(&request, raw)).await?;
        assert_eq!(status, 200);
        assert_metadata(&fixture, &id, raw, 1)?;
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
        server.stop(false).await;
        recording.finish(&fixture, 0).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_m4_request_only_preserves_m1_m2_m5_blockers() -> TestResult {
    for streaming in [false, true] {
        for mask in 1..8 {
            let mut request: Value = serde_json::from_str(&body(streaming, true, false))?;
            if mask & 1 != 0 {
                request["thinking"] = json!({"type":"adaptive","display":"updates"});
            }
            if mask & 2 != 0 {
                request["output_config"] = json!({"effort":"high"});
            }
            if mask & 4 != 0 {
                request["messages"][0]["content"] =
                    json!([{"type":"text","text":"question","cache_control":{"type":"ephemeral"}}]);
            }
            rejected(streaming, with_metadata(&request, METADATA), Some(METADATA)).await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_m3_invalid_policy_and_history_never_send() -> TestResult {
    for streaming in [false, true] {
        for policy in [
            json!(null),
            json!({}),
            json!({"edits":[]}),
            json!({"edits":[{"type":"unknown","keep":"all"}]}),
            json!({"edits":[{"type":"clear_thinking_20251015"}]}),
            json!({"edits":[{"type":"clear_thinking_20251015","keep":null}]}),
            json!({"edits":[{"type":"clear_thinking_20251015","keep":1}]}),
            json!({"edits":[{"type":"clear_thinking_20251015","keep":{"type":"thinking_turns","value":1}}]}),
            json!({"edits":[{"type":"clear_thinking_20251015","keep":"all","extra":true}]}),
            json!({"edits":[{"type":"clear_thinking_20251015","keep":"all"},{"type":"clear_thinking_20251015","keep":"all"}]}),
            json!({"edits":[{"type":"clear_thinking_20251015","keep":"all"}],"extra":true}),
        ] {
            let mut request: Value = serde_json::from_str(&body(streaming, false, false))?;
            request["context_management"] = policy;
            rejected(streaming, with_metadata(&request, METADATA), Some(METADATA)).await?;
        }
        for content in [
            json!({"type":"thinking","thinking":"private","signature":"native"}),
            json!({"type":"redacted_thinking","data":"opaque"}),
            json!({"type":"reasoning","summary":[]}),
            json!({"type":"unknown_native","body":"opaque"}),
        ] {
            let mut request: Value = serde_json::from_str(&body(streaming, true, false))?;
            request["messages"] = json!([{"role":"assistant","content":[content]},{"role":"user","content":"question"}]);
            rejected(streaming, with_metadata(&request, METADATA), Some(METADATA)).await?;
        }
        for content in [
            json!([{"type":"thinking","thinking":"hidden","signature":"native"}]),
            json!([{"type":"text","text":"visible"},{"type":"redacted_thinking","data":"opaque"}]),
            json!([{"type":"text","text":"visible","extra":"unrepresented"}]),
        ] {
            let mut request: Value = serde_json::from_str(&body(streaming, true, false))?;
            request["tools"] = json!([{"name":"lookup","input_schema":{"type":"object"}}]);
            request["messages"] = json!([{"role":"assistant","content":[{"type":"tool_use","id":"call","name":"lookup","input":{}}]},
                {"role":"user","content":[{"type":"tool_result","tool_use_id":"call","content":content}]}]);
            rejected(streaming, with_metadata(&request, METADATA), Some(METADATA)).await?;
        }
        let request = body(streaming, true, false)
            .replace("\"keep\":\"all\"", "\"keep\":\"all\",\"keep\":\"all\"");
        rejected(streaming, request, None).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_m3_rechecks_later_history() -> TestResult {
    for streaming in [false, true] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire(1, streaming)?)?;
        let fixture = lifecycle_fixture(MessagesTransport::new(&peer, 7)?)?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let (status, first, bytes) = send(&fixture, &url, body(streaming, true, true)).await?;
        assert_eq!(status, 200);
        let public = tool_rounds::client_result(&bytes, streaming)?;
        let mut later: Value = serde_json::from_str(&body(streaming, true, false))?;
        later["messages"] = json!([{"role":"user","content":"question"},{"role":"assistant","content":public["content"]},
            {"role":"assistant","content":[{"type":"redacted_thinking","data":"unrepresented"}]},{"role":"user","content":"follow-up"}]);
        let (status, second, _) = send(&fixture, &url, with_metadata(&later, METADATA)).await?;
        assert_eq!(status, 400);
        assert_metadata(&fixture, &first, METADATA, 1)?;
        assert_metadata(&fixture, &second, METADATA, 0)?;
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
        assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
        assert_eq!(active_leases(&fixture), 0);
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        let rows = SqliteEventStore::open_read_only(&database)?.list_events()?;
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r.event(), GatewayEvent::Request(_)))
                .count(),
            2
        );
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r.event(), GatewayEvent::Attempt(_)))
                .count(),
            1
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_messages_local_controls_require_actual_candidates_and_mode() -> TestResult {
    for kind in [
        "empty",
        "canonical",
        "passthrough",
        "lossless",
        "mixed-mode",
        "mixed-adapter",
    ] {
        let peer = ModelPeer::start_wire(false, tool_rounds::wire(1, false)?)?;
        let (channel, secret) = native_channels()[0];
        let transport = MessagesTransport::new(&peer, 7)?;
        let configured = fixture_with_configuration(
            channel,
            secret.as_bytes(),
            false,
            |config| {
                config.public_models[0].model_name = "m4-local-control-model".into();
                if kind == "empty" {
                    config.route_candidates.clear();
                } else if kind.starts_with("mixed-") {
                    let mut other = config.route_candidates[0].clone();
                    other.id = gateway_core::RouteCandidateId::try_new("other-mode")?;
                    other.priority = 1;
                    if kind == "mixed-mode" {
                        other.upstream_model = "m4-other-model".into();
                        other.transform_mode = TransformMode::LosslessBridge;
                    } else {
                        other.endpoint_id = add_compatible_candidate(config, &peer)?;
                    }
                    config.route_candidates.push(other);
                } else {
                    config.route_candidates[0].transform_mode = match kind {
                        "canonical" => TransformMode::Canonical,
                        "passthrough" => TransformMode::Passthrough,
                        _ => TransformMode::LosslessBridge,
                    };
                }
                Ok(())
            },
            |endpoints| {
                let mut profiles = P12TransportProfiles::try_new()?;
                profiles.build = Some(transport);
                let profiles = Arc::new(profiles);
                for endpoint in endpoints.values_mut() {
                    endpoint.transports = profiles.clone();
                }
                Ok(())
            },
        );
        if kind == "empty" {
            assert!(configured.as_ref().err().is_some_and(|error| {
                format!("{error:?}").contains("RouteHasNoHardEligibleCandidate")
            }));
            assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
            assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
            continue;
        }
        let fixture = configured?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let mut request: Value = serde_json::from_str(&body(false, true, false))?;
        request["model"] = json!("m4-local-control-model");
        let (status, _, _) = send(&fixture, &url, with_metadata(&request, METADATA)).await?;
        assert_ne!(status, 200, "{kind}");
        assert_eq!(peer.accepted.load(Ordering::SeqCst), 0);
        assert!(peer.requests.lock().map_err(|_| "receiver")?.is_empty());
        assert_eq!(fixture.events.attempts()?, 0);
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        assert_eq!(
            gateway_store::open(database)?.query_row(
                "SELECT count(*) FROM request_metadata_evidence",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
    }
    for (channel, secret) in native_channels().into_iter().skip(1) {
        let independent = ModelPeer::start_wire(false, tool_rounds::wire(1, false)?)?;
        let peer = Arc::new(Peer::default());
        let fixture = fixture(channel, secret.as_bytes(), |endpoint| {
            install_peer(endpoint, channel.adapter, &peer)
        })?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let mut request: Value = serde_json::from_str(&body(false, true, false))?;
        request["model"] = json!("p12-test-model");
        let (status, _, _) = send(&fixture, &url, with_metadata(&request, METADATA)).await?;
        assert_eq!(status, 400);
        assert!(peer.bodies.lock().map_err(|_| "adapter")?.is_empty());
        assert_eq!(independent.accepted.load(Ordering::SeqCst), 0);
        assert!(
            independent
                .requests
                .lock()
                .map_err(|_| "receiver")?
                .is_empty()
        );
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        assert_eq!(
            gateway_store::open(database)?.query_row(
                "SELECT count(*) FROM request_metadata_evidence",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            0
        );
    }
    Ok(())
}

fn add_compatible_candidate(
    config: &mut ControlPlaneConfiguration,
    peer: &ModelPeer,
) -> Result<EndpointId, Box<dyn Error>> {
    let mut upstream = config.upstreams[0].clone();
    upstream.id = gateway_core::UpstreamId::try_new("m4-compatible-upstream")?;
    upstream.name = "M4 compatible candidate".into();
    upstream.kind = "openai-compatible".into();
    let mut policy = config.egress_policies[0].clone();
    policy.id = EgressPolicyId::try_new("m4-compatible-policy")?;
    policy.name = "M4 owned compatible receiver".into();
    policy.allowed_schemes_json = json!(["https"]).to_string();
    policy.allowed_hosts_json = json!(["127.0.0.1"]).to_string();
    policy.allowed_ports_json =
        json!([reqwest::Url::parse(&peer.url)?.port().ok_or("port")?]).to_string();
    policy.allowed_cidrs_json = json!(["127.0.0.1/32"]).to_string();
    upstream.egress_policy_id = Some(policy.id.clone());
    let mut endpoint = config.endpoints[0].clone();
    endpoint.id = EndpointId::try_new("m4-compatible-endpoint")?;
    endpoint.upstream_id = upstream.id.clone();
    endpoint.adapter_id = "openai-compatible.responses".into();
    endpoint.base_url = peer
        .url
        .trim_end_matches("/responses")
        .replacen("http://", "https://", 1);
    let secrets = test_secret_store()?;
    let mut template = p12_configuration_for(&secrets, "m4-template")?;
    let mut credential = template.credentials.remove(0);
    credential.id = CredentialId::try_new("m4-compatible-credential")?;
    credential.upstream_id = upstream.id.clone();
    credential.encrypted_secret = secrets.seal(
        b"m4-controlled",
        &credential_associated_data(&config.version.id, &credential.id, &credential.upstream_id)?,
    )?;
    let mut binding = template.endpoint_credential_bindings.remove(0);
    binding.endpoint_id = endpoint.id.clone();
    binding.credential_id = credential.id.clone();
    binding.upstream_id = upstream.id.clone();
    let id = endpoint.id.clone();
    config.egress_policies.push(policy);
    config.upstreams.push(upstream);
    config.endpoints.push(endpoint);
    config.credentials.push(credential);
    config.endpoint_credential_bindings.push(binding);
    Ok(id)
}
