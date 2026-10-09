//! Native deletion is reviewed against an exact graph and account generation, without secrets.
use super::*;

#[actix_web::test]
async fn native_deletion_preview_is_concrete_revision_guarded_and_preserves_shared_graph()
-> TestResult {
    let (file, resources) = fixture(false)?;
    let db = gateway_store::open(&file.0)?;
    db.execute(
        "UPDATE upstreams SET kind='grok-console-native' WHERE id='owner-a'",
        [],
    )?;
    db.execute("UPDATE upstream_endpoints SET adapter_id='grok.console.responses',base_url='https://api.x.ai/v1',inference_path='/responses',enabled=0 WHERE id='endpoint-owner-a'", [])?;
    db.execute("INSERT INTO upstream_endpoints SELECT config_version_id,'wrong-adapter',upstream_id,'openai-compatible.responses',api_format,base_url,inference_path,models_path,transport,1 FROM upstream_endpoints WHERE id='endpoint-owner-a'", [])?;
    db.execute("INSERT INTO public_models VALUES (?1,'delete-model','public-grok-model','active','Reviewed Grok model','{}')", [VERSION])?;
    db.execute("INSERT INTO model_routes (config_version_id,id,public_model_id,policy,max_attempts,bootstrap_timeout_ms) VALUES (?1,'delete-route','delete-model','round_robin',1,1000)", [VERSION])?;
    db.execute("INSERT INTO route_candidates (config_version_id,id,route_id,endpoint_id,upstream_model,credential_scope,transform_mode,enabled,priority,weight) VALUES (?1,'delete-candidate','delete-route','endpoint-owner-a','upstream-grok-model','endpoint_bindings','canonical',0,0,1)", [VERSION])?;
    let version = KeyVersion::try_new(1)?;
    let secrets = SecretStore::new(MasterKeyRing::try_new(
        version,
        [(version, MasterKey::try_from_bytes([0x51; 32])?)],
    )?);
    let pool = std::sync::Arc::new(provider_grok::GrokAccountPoolStore::try_open(
        &file.0, secrets,
    )?);
    let native =
        gateway_http_actix::management_resources::native_accounts::NativeAccountManagement::new(
            pool.clone(),
        )?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(resources.with_native_accounts(native)))
            .configure(configure_management_resources),
    )
    .await;
    let mut ids = Vec::new();
    for (name, secret) in [
        ("delete-target", "synthetic-target"),
        ("retained-sibling", "synthetic-sibling"),
    ] {
        let response = test::call_service(
            &app,
            authorized(test::TestRequest::post().uri("/admin/native-accounts/import"))
                .set_json(serde_json::json!({"id":name,"channel":"grok.console","secret":secret}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let _: Value = test::read_body_json(response).await;
        ids.push(pool.single_import_account(name)?);
    }
    let path = format!(
        "/admin/native-accounts/{}/deletion-impact?revision=0",
        ids[0]
    );
    assert_eq!(
        test::call_service(&app, test::TestRequest::get().uri(&path).to_request())
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::get().uri(&path)).to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    let impact: Value = test::read_body_json(response).await;
    assert_eq!(impact["account_id"], ids[0]);
    assert_eq!(impact["connections"][0]["upstream_id"], "owner-a");
    assert_eq!(impact["connections"][0]["endpoint_id"], "endpoint-owner-a");
    assert_eq!(impact["connections"][0]["endpoint_enabled"], false);
    assert_eq!(
        impact["connections"][0]["models"][0]["model"],
        "public-grok-model"
    );
    assert_eq!(
        impact["connections"][0]["models"][0]["upstream_model"],
        "upstream-grok-model"
    );
    assert_eq!(
        impact["connections"][0]["models"][0]["candidate_enabled"],
        false
    );
    assert_eq!(
        impact["connections"].as_array().ok_or("connections")?.len(),
        1
    );
    assert_eq!(impact["remaining_accounts"][0]["id"], ids[1]);
    assert_eq!(impact["replacement_guaranteed"], false);
    assert_eq!(impact["history_retained"], true);
    let encoded = impact.to_string();
    assert!(
        !encoded.contains("synthetic-target")
            && !encoded.contains("ciphertext")
            && !encoded.contains("base_url")
    );
    let token = impact["review_token"].as_str().ok_or("review token")?;
    // A changed sibling invalidates the captured review, even when target revision is unchanged.
    let disabled = test::call_service(
        &app,
        authorized(test::TestRequest::patch().uri(&format!("/admin/native-accounts/{}", ids[1])))
            .set_json(serde_json::json!({"revision":0,"enabled":false}))
            .to_request(),
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    assert!(matches!(
        pool.remove_reviewed_account(
            &ids[0],
            0,
            impact["inventory_generation"]
                .as_i64()
                .ok_or("generation")?,
            "admin",
            3
        ),
        Err(provider_grok::GrokAccountPoolError::ExistingAccountConflict)
    ));
    let removed = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete()
                .uri(&format!("/admin/native-accounts/{}?revision=0", ids[0])),
        )
        .insert_header(("X-Deletion-Impact-Review", token))
        .to_request(),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::CONFLICT);
    let fresh: Value = test::read_body_json(
        test::call_service(
            &app,
            authorized(test::TestRequest::get().uri(&path)).to_request(),
        )
        .await,
    )
    .await;
    assert_eq!(fresh["remaining_accounts"][0]["enabled"], false);
    let removed = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete()
                .uri(&format!("/admin/native-accounts/{}?revision=0", ids[0])),
        )
        .insert_header((
            "X-Deletion-Impact-Review",
            fresh["review_token"].as_str().ok_or("fresh token")?,
        ))
        .to_request(),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::OK);
    assert_eq!(
        db.query_row("SELECT count(*) FROM upstreams", [], |r| r.get::<_, i64>(0))?,
        2
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM upstream_endpoints", [], |r| r
            .get::<_, i64>(0))?,
        3
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM grok_accounts", [], |r| r
            .get::<_, i64>(0))?,
        1
    );
    assert!(
        db.query_row(
            "SELECT count(*) FROM native_account_management_events WHERE account_id=?1",
            [&ids[0]],
            |r| r.get::<_, i64>(0)
        )? > 0
    );
    Ok(())
}
