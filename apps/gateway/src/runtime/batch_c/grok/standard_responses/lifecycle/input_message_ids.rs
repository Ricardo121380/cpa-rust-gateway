//! C6 native input IDs through the actual public listener and independent receiver.
use super::*;
mod history;
pub(super) mod negatives;

fn fixture_for(
    transport: Arc<dyn GrokBuildTransport>,
    stored: bool,
    configure: impl FnOnce(&mut P12TransportProfiles),
) -> Result<Fixture, Box<dyn Error>> {
    let (channel, secret) = native_channels()[0];
    let mut fixture = fixture_with_configuration(
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
            spare.id = gateway_core::RouteCandidateId::try_new("c6-spare-candidate")?;
            spare.upstream_model = "unselected-spare-model".into();
            spare.priority = 1;
            configuration.route_candidates.push(spare);
            Ok(())
        },
        |endpoints| {
            let mut profiles = P12TransportProfiles::try_new()?;
            profiles.build = Some(transport);
            configure(&mut profiles);
            endpoints.values_mut().next().ok_or("endpoint")?.transports = Arc::new(profiles);
            Ok(())
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

fn inputs() -> Value {
    json!([
        {"type":"message","id":"dev-C6:Original_01","role":"developer","content":[{"type":"input_text","text":"Retain roles and order."}]},
        {"type":"message","id":"user-C6~Original_02","role":"user","content":[{"type":"input_text","text":"seed 41"},{"type":"input_text","text":"第二段原文"}]},
        {"type":"message","id":"sys-C6!Original_03","role":"system","content":[{"type":"input_text","text":"exact identity"}]}
    ])
}

#[actix_web::test]
async fn public_build_c6_native_input_message_ids_reach_receiver() -> TestResult {
    for (streaming, size) in [(false, 31), (true, 1), (true, 7), (true, 31)] {
        let peer = ModelPeer::start_wire(streaming, tool_rounds::wire_response(1, streaming)?)?;
        let fixture = fixture_for(namespaces::transport(&peer, size)?, false, |p| {
            p.build_input_message_ids_local_controlled = true;
        })?;
        let recording = Recording::start(&fixture, false)?;
        let (url, server) = namespaces::listener(&fixture)?;
        // The same IDs in separate self-contained requests are legal, not idempotency keys.
        for _ in 0..2 {
            let body = json!({"model":"p12-test-model","input":inputs(),"stream":streaming});
            let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
            assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
            assert_eq!(
                namespaces::terminal(&bytes, streaming)?["output"],
                tool_rounds::output(1)
            );
        }
        server.stop(false).await;
        let database = recording.finish(&fixture, 0).await?;
        namespaces::assert_recorded(&database, 2).await?;
        negatives::assert_counts(&database, 2, 2).await?;
        let sent = peer.requests.lock().map_err(|_| "receiver")?;
        assert_eq!(sent.len(), 2);
        for row in sent.iter() {
            assert_eq!(row.body["input"], inputs());
        }
        println!(
            "CPAR62_C6_RECEIPT {}",
            json!({"streaming":streaming,"fragment_bytes":size,"received":sent.iter().map(|r|r.body.clone()).collect::<Vec<_>>(),
            "requests":2,"receiver":2,"attempts":fixture.events.attempts()?,"usage":2,"ledger":2,"lease_after":active_leases(&fixture),"cost":"UNKNOWN"})
        );
    }
    Ok(())
}

#[actix_web::test]
async fn public_build_c6_absence_empty_text_and_exact_id_boundaries() -> TestResult {
    let peer = ModelPeer::start_wire(false, tool_rounds::wire_response(1, false)?)?;
    let fixture = fixture_for(namespaces::transport(&peer, 7)?, false, |p| {
        p.build_input_message_ids_local_controlled = true;
    })?;
    let recording = Recording::start(&fixture, false)?;
    let (url, server) = namespaces::listener(&fixture)?;
    let graphic: String = (0x21_u8..=0x7e).map(char::from).collect();
    let mut boundary = inputs();
    boundary[0]["id"] = json!("!");
    boundary[1]["id"] = json!(graphic.repeat(6).chars().take(512).collect::<String>());
    boundary[2].as_object_mut().ok_or("message")?.remove("id");
    boundary[2]["content"][0]["text"] = json!("");
    let (status, bytes) = namespaces::request(
        &fixture,
        &url,
        &json!({"model":"p12-test-model","input":boundary}),
    )
    .await?;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
    server.stop(false).await;
    let database = recording.finish(&fixture, 0).await?;
    namespaces::assert_recorded(&database, 1).await?;
    assert_eq!(
        peer.requests.lock().map_err(|_| "receiver")?[0].body["input"],
        boundary
    );
    Ok(())
}
