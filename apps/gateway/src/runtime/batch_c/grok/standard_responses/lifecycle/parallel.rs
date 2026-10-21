//! Bounded parallel function batches through the public production Build assembly.
use super::*;
mod failures;
mod history;
mod negatives;

fn controlled_compiler(database: &Path) -> Result<RouteCompiler, RuntimeCompositionError> {
    // Only this private owned fixture supplies the independent local capability evidence.
    // The actual compiler still intersects endpoint/model/candidate policy and emits the snapshot.
    let stock = p12_adapter_capabilities("grok.build.responses")?;
    let parallel = CapabilitySet::try_new(stock.iter().chain([SemanticCapability::ParallelTools]))
        .map_err(|_| RuntimeCompositionError::Unavailable)?;
    Ok(RouteCompiler::with_adapter_capabilities(
        CatalogView::default(),
        BTreeMap::from([("grok.build.responses".into(), parallel)]),
    )
    .with_durable_catalog(database.to_owned()))
}

fn fixture_for(
    transport: Arc<dyn GrokBuildTransport>,
    stored: bool,
    parallel_profile: bool,
    qualify: impl FnOnce(&mut P12TransportProfiles),
) -> Result<Fixture, Box<dyn Error>> {
    let (channel, secret) = native_channels()[0];
    let mut fixture = fixture_with_compiler(
        channel,
        secret.as_bytes(),
        false,
        |configuration| {
            if stored {
                configuration.route_candidates[0].transform_mode = TransformMode::Canonical;
            }
            configuration.public_models[0].model_name = channel.model.into();
            configuration.model_aliases.push(
                gateway_store::control_plane::ModelAliasConfiguration {
                    alias: "p12-test-model".into(),
                    public_model_id: configuration.public_models[0].id.clone(),
                },
            );
            configuration.model_routes[0].max_attempts = 3;
            let mut spare = configuration.route_candidates[0].clone();
            spare.id = gateway_core::RouteCandidateId::try_new("parallel-spare-candidate")?;
            spare.upstream_model = "unselected-spare-model".into();
            spare.priority = 1;
            configuration.route_candidates.push(spare);
            Ok(())
        },
        |endpoints| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(transport);
            qualify(&mut profiles);
            endpoints.values_mut().next().ok_or("endpoint")?.transports = Arc::new(profiles);
            Ok(())
        },
        |_, _| Ok(Arc::from([])),
        |database| {
            if parallel_profile {
                controlled_compiler(database)
            } else {
                deployment_route_compiler(database)
            }
        },
    )?;
    if stored {
        fixture.state = fixture.state.clone().with_stored_response_store(Arc::new(
            gateway_store::stored_response::SqliteStoredResponseStore::open(
                fixture.directory.path().join("batch-c.sqlite3"),
                test_secret_store()?,
            )?,
        ));
    }
    Ok(fixture)
}

fn declarations(namespaced: bool) -> Value {
    let f =
        json!({"type":"function","name":"lookup","parameters":{"type":"object"},"strict":false});
    if namespaced {
        json!([
            {"type":"namespace","name":"left","description":"Independent left seed", "tools":[f.clone()]},
            {"type":"namespace","name":"right","description":"Independent right seed", "tools":[f.clone()]},
            {"type":"function","name":"combine","parameters":{"type":"object"},"strict":false}
        ])
    } else {
        json!([f, {"type":"function","name":"measure","parameters":{"type":"object"},"strict":false},
            {"type":"function","name":"combine","parameters":{"type":"object"},"strict":false}])
    }
}

fn output(round: usize, namespaced: bool) -> Value {
    let mut items = tool_rounds::output(round)
        .as_array()
        .cloned()
        .unwrap_or_default();
    if round == 0 {
        items.truncate(1);
        for (index, seed) in [(0, 19), (1, 22)] {
            let mut call = json!({"id":format!("parallel-item-{index}"),"type":"function_call","status":"completed",
                "call_id":format!("parallel-call-{index}"),"name":"lookup",
                "arguments":json!({"seed":seed,"label":if index==0 {"左"} else {"右"}}).to_string()});
            if namespaced {
                call["namespace"] = json!(if index == 0 { "left" } else { "right" });
            } else if index == 1 {
                call["name"] = json!("measure");
            }
            items.push(call);
        }
    } else if round == 2 {
        items[1]["name"] = json!("combine");
        items[1]["arguments"] = json!(r#"{"left":19,"right":22,"sum":41}"#);
    } else {
        items[1]["content"][0]["text"] = json!(if round == 1 {
            "Both independent seeds total 41."
        } else {
            "The later combined result is 42."
        });
    }
    json!(items)
}

fn event(kind: &str, mut value: Value) -> String {
    value["type"] = json!(kind);
    format!("event: {kind}\ndata: {value}\n\n")
}

fn wire(
    round: usize,
    namespaced: bool,
    streaming: bool,
    reverse_done: bool,
) -> Result<String, Box<dyn Error>> {
    if round != 0 || !streaming {
        return namespaces::wire_output(
            &output(round, namespaced),
            &format!("parallel-native-{round}"),
            streaming,
        );
    }
    let items = output(round, namespaced);
    let summary = json!([items[0]]);
    let prefix = namespaces::wire_output(&summary, "parallel-native-0", true)?;
    let mut bytes = prefix
        .split("event: response.completed")
        .next()
        .ok_or("prefix")?
        .to_owned();
    for index in [1, 2] {
        let mut added = items[index].clone();
        added["status"] = json!("in_progress");
        added["arguments"] = json!("");
        bytes += &event(
            "response.output_item.added",
            json!({"output_index":index,"item":added}),
        );
    }
    let a: Vec<_> = items[1]["arguments"]
        .as_str()
        .ok_or("args")?
        .chars()
        .collect();
    let b: Vec<_> = items[2]["arguments"]
        .as_str()
        .ok_or("args")?
        .chars()
        .collect();
    for offset in 0..a.len().max(b.len()) {
        for (index, chars) in [(1, &a), (2, &b)] {
            if let Some(ch) = chars.get(offset) {
                bytes += &event(
                    "response.function_call_arguments.delta",
                    json!({"item_id":items[index]["id"],
                    "output_index":index,"call_id":items[index]["call_id"],"delta":ch.to_string()}),
                );
            }
        }
    }
    for index in if reverse_done { [2, 1] } else { [1, 2] } {
        bytes += &event(
            "response.function_call_arguments.done",
            json!({"item_id":items[index]["id"],"output_index":index,
            "call_id":items[index]["call_id"],"name":items[index]["name"],"arguments":items[index]["arguments"]}),
        );
        bytes += &event(
            "response.output_item.done",
            json!({"output_index":index,"item":items[index]}),
        );
    }
    let mut end = response();
    end["id"] = json!("parallel-native-0");
    end["output"] = items;
    number_events(&(bytes + &frame("response.completed", &end)))
}

fn body(namespaced: bool, streaming: bool) -> Value {
    json!({"model":"p12-test-model","input":"Read two independent seeds.","tools":declarations(namespaced),
        "parallel_tool_calls":true,"tool_choice":"auto","reasoning":{"summary":"auto"},"stream":streaming})
}

#[actix_web::test]
async fn public_build_parallel_flat_functions_reach_receiver() -> TestResult {
    let peer = ModelPeer::start_wire(false, wire(0, false, false, false)?)?;
    let fixture = fixture_for(namespaces::transport(&peer, 7)?, false, true, |p| {
        p.build_parallel_local_controlled = true;
    })?;
    let recording = Recording::start(&fixture, false)?;
    let (url, server) = namespaces::listener(&fixture)?;
    let (status, bytes) = namespaces::request(&fixture, &url, &body(false, false)).await?;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
    assert_eq!(
        namespaces::terminal(&bytes, false)?["output"],
        output(0, false)
    );
    assert_eq!(peer.accepted.load(Ordering::SeqCst), 1);
    assert_eq!(peer.requests.lock().map_err(|_| "receiver")?.len(), 1);
    assert_eq!(fixture.events.attempts()?, 1);
    server.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    namespaces::assert_recorded(&database, 1).await?;
    println!(
        "CPAR62_PARALLEL_RECEIPT {}",
        json!({"case":"two-flat-functions","status":status,"receiver":1,"attempts":1,"usage":1,"ledger":1,"cost":"UNKNOWN"})
    );
    Ok(())
}

fn number_events(wire: &str) -> Result<String, Box<dyn Error>> {
    let mut numbered = String::new();
    for (index, record) in wire.split("\n\n").filter(|s| !s.is_empty()).enumerate() {
        let data = record
            .lines()
            .find_map(|s| s.strip_prefix("data: "))
            .ok_or("data")?;
        let mut value: Value = serde_json::from_str(data)?;
        value["sequence_number"] = json!(11 + index * 3);
        let kind = value["type"].as_str().ok_or("event type")?.to_owned();
        numbered += &event(&kind, value);
    }
    Ok(numbered)
}

fn owner(fixture: &Fixture) -> Result<ClientKeyId, Box<dyn Error>> {
    fixture
        .events
        .0
        .lock()
        .map_err(|_| "events")?
        .iter()
        .find_map(|v| match v {
            GatewayEvent::Request(request) => Some(request.client_key_id().clone()),
            _ => None,
        })
        .ok_or_else(|| "request owner".into())
}

fn read_stored(
    fixture: &Fixture,
    id: &str,
) -> Result<Option<gateway_store::stored_response::StoredResponseRecord>, Box<dyn Error>> {
    gateway_store::stored_response::SqliteStoredResponseStore::open(
        fixture.directory.path().join("batch-c.sqlite3"),
        test_secret_store()?,
    )?
    .get_owned(
        &owner(fixture)?,
        &gateway_core::ResponseId::try_new(id)?,
        system_now_ms_runtime()?,
    )
    .map_err(Into::into)
}

async fn assert_counts(database: &Path, attempts: usize, usage: usize) -> TestResult {
    input_message_ids::negatives::assert_counts(database, attempts, usage).await?;
    let rows = SqliteEventStore::open_read_only(database)?.list_events()?;
    let mut seen = BTreeSet::new();
    for row in &rows {
        if let GatewayEvent::Usage(value) = row.event() {
            assert!(seen.insert(value.attempt_id()));
            assert_eq!(
                value.usage().provenance,
                gateway_core::UsageProvenance::Measured
            );
            assert_eq!(value.usage().input_tokens, Some(20));
            assert_eq!(value.usage().output_tokens, Some(7));
            assert_eq!(
                value.usage().input_accounting,
                gateway_core::InputTokenAccounting::Inclusive
            );
        }
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_parallel_coexisting_c1_c6_fields_keep_independent_recipients() -> TestResult {
    for streaming in [false, true] {
        let peer = ModelPeer::start_wire(streaming, wire(0, true, streaming, true)?)?;
        let fixture = fixture_for(namespaces::transport(&peer, 7)?, false, true, |p| {
            p.build_parallel_local_controlled = true;
            p.build_namespaces_local_controlled = true;
            p.build_search_local_controlled = true;
            p.build_input_message_ids_local_controlled = true;
        })?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        let mut request = body(true, streaming);
        request["tools"]
            .as_array_mut()
            .ok_or("tools")?
            .push(json!({"type":"web_search","external_web_access":false}));
        request["input"] = json!([{"type":"message","role":"user","id":"parallel-combined-input","content":[{"type":"input_text","text":"two independent functions; search declaration remains separate"}]}]);
        request["client_metadata"] = client_metadata::metadata();
        let (status, bytes) = namespaces::request(&fixture, &url, &request).await?;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
        assert_eq!(
            namespaces::terminal(&bytes, streaming)?["output"],
            output(0, true)
        );
        {
            let sent = peer.requests.lock().map_err(|_| "receiver")?;
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0].body["tools"], request["tools"]);
            assert_eq!(sent[0].body["input"], request["input"]);
            assert_eq!(sent[0].body["parallel_tool_calls"], true);
            assert!(sent[0].body.get("client_metadata").is_none());
            assert!(!String::from_utf8_lossy(&sent[0].wire).contains("C4_PRIVATE"));
        }
        client_metadata::assert_capture(&fixture, &request["client_metadata"].to_string(), 1)?;
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        assert_counts(&database, 1, 1).await?;
        println!(
            "CPAR62_PARALLEL_COMBINED {}",
            json!({"streaming":streaming,"status":200,"receiver":1,"attempts":1,"usage":1,"ledger":1,
            "namespace_identity":"EXACT","c4_local_encrypted_readback":"EXACT","c5_declaration":"SEPARATE_HOSTED","c6_native_input_id":"EXACT","cost":"UNKNOWN","actual_codex":"NOT_THIS_TEST"})
        );
    }
    Ok(())
}
