use super::*;
use gateway_http_actix::management_resources::{
    account_quota::{AccountQuotaObservation, AccountQuotaWindow},
    native_accounts::usage::{NativeUsageError, NativeUsageTransport},
};
use gateway_store::quota_history::{AccountScope, QuotaHistoryStore, Sample};
use std::sync::{Arc, Mutex};

struct Sequence(
    Mutex<std::collections::VecDeque<Result<AccountQuotaObservation, NativeUsageError>>>,
);
impl NativeUsageTransport for Sequence {
    fn read(
        &self,
        _: provider_grok::GrokAccountIdentitySnapshot,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<AccountQuotaObservation, NativeUsageError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            self.0
                .lock()
                .map_err(|_| NativeUsageError::Unavailable)?
                .pop_front()
                .ok_or(NativeUsageError::Unavailable)?
        })
    }
}
fn sample(now: i64, percent: f64, reset: i64) -> AccountQuotaObservation {
    AccountQuotaObservation {
        source: "grok_console".into(),
        observed_at_ms: now,
        email: Some("private@example.test".into()),
        plan: None,
        partial: false,
        windows: vec![
            AccountQuotaWindow {
                name: "console.chat".into(),
                used_percent: Some(percent),
                used: None,
                limit: None,
                unit: Some("percent".into()),
                reset_at: None,
                reset_at_ms: Some(reset),
                duration_seconds: None,
            },
            AccountQuotaWindow {
                name: "console.image".into(),
                used_percent: Some(0.0),
                used: None,
                limit: None,
                unit: Some("percent".into()),
                reset_at: None,
                reset_at_ms: None,
                duration_seconds: None,
            },
        ],
    }
}

#[actix_web::test]
#[allow(clippy::too_many_lines)] // One management workflow verifies bounded history semantics.
async fn quota_history_http_filters_gaps_resets_failures_and_duplicates() -> TestResult {
    let (file, resources) = fixture(false)?;
    let key = KeyVersion::try_new(1)?;
    let secrets = SecretStore::new(MasterKeyRing::try_new(
        key,
        [(key, MasterKey::try_from_bytes([0x51; 32])?)],
    )?);
    let store = Arc::new(provider_grok::GrokAccountPoolStore::try_open(
        &file.0, secrets,
    )?);
    let history = Arc::new(QuotaHistoryStore::open(&file.0)?);
    let mut partial = sample(800_000, 40.0, 1_200_000);
    partial.partial = true;
    let transport = Sequence(Mutex::new(
        [
            Ok(sample(1_000, 100.0, 2_000)),
            Ok(sample(1_000, 100.0, 2_000)),
            Ok(sample(700_000, 10.0, 900_000)),
            Ok(partial),
            Err(NativeUsageError::Forbidden),
        ]
        .into(),
    ));
    let native =
        gateway_http_actix::management_resources::native_accounts::NativeAccountManagement::new(
            store,
        )?
        .with_usage_transport(Arc::new(transport));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(
                resources
                    .with_native_accounts(native)
                    .with_quota_history(history.clone()),
            ))
            .configure(configure_management_resources),
    )
    .await;
    for (id, secret) in [
        ("history-a", "synthetic-history-a"),
        ("history-b", "synthetic-history-b"),
    ] {
        let r = test::call_service(
            &app,
            authorized(test::TestRequest::post().uri("/admin/native-accounts/import"))
                .set_json(serde_json::json!({"id":id,"channel":"grok.console","secret":secret}))
                .to_request(),
        )
        .await;
        assert_eq!(r.status(), StatusCode::CREATED);
    }
    let rows: Value = test::call_and_read_body_json(
        &app,
        authorized(test::TestRequest::get().uri("/admin/native-accounts")).to_request(),
    )
    .await;
    let a = rows["items"][0]["id"].as_str().ok_or("a")?;
    let b = rows["items"][1]["id"].as_str().ok_or("b")?;
    for id in [a, a, a, b, a] {
        let r = test::call_service(
            &app,
            authorized(
                test::TestRequest::get()
                    .uri(&format!("/admin/native-accounts/{id}/usage?revision=0")),
            )
            .to_request(),
        )
        .await;
        assert_eq!(r.status(), StatusCode::OK);
    }
    let now = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    // The small historical fixture interval exposes gaps without sleeping or changing production clocks.
    let path = format!(
        "/admin/quota-history?from_ms=0&to_ms=1000000&limit=200&account_id={a}&window=console.chat&config_version_id="
    );
    let denied = test::call_service(&app, test::TestRequest::get().uri(&path).to_request()).await;
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
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
    let page: Value = test::read_body_json(response).await;
    assert_eq!(page["entries"].as_array().ok_or("entries")?.len(), 2);
    assert_eq!(page["entries"][0]["status"], "exhausted");
    assert_eq!(page["entries"][0]["confidence"], "observed");
    assert_eq!(page["entries"][0]["stale"], true);
    assert_eq!(
        page["entries"][1]["gap_before"],
        serde_json::json!([2_000, 700_000])
    );
    assert_eq!(page["entries"][1]["reported_reset_changed"], true);
    assert!(!page.to_string().contains("private@example.test"));
    let all: Value = test::call_and_read_body_json(
        &app,
        authorized(
            test::TestRequest::get().uri("/admin/quota-history?from_ms=0&to_ms=1000000&limit=1"),
        )
        .to_request(),
    )
    .await;
    assert_eq!(all["has_more"], true);
    let next = all["next_after_id"].as_i64().ok_or("cursor")?;
    let second: Value = test::call_and_read_body_json(
        &app,
        authorized(test::TestRequest::get().uri(&format!(
            "/admin/quota-history?from_ms=0&to_ms=1000000&limit=200&after_id={next}"
        )))
        .to_request(),
    )
    .await;
    assert_eq!(second["entries"].as_array().ok_or("page")?.len(), 5);
    assert!(
        second["entries"]
            .as_array()
            .ok_or("page")?
            .iter()
            .any(|e| e["account_id"] == b && e["completeness"] == "partial")
    );
    let failed: Value = test::call_and_read_body_json(
        &app,
        authorized(test::TestRequest::get().uri(&format!(
            "/admin/quota-history?from_ms={}&to_ms={}&limit=200&account_id={a}&window=console.chat",
            now - 10000,
            now + 10000
        )))
        .to_request(),
    )
    .await;
    assert_eq!(failed["entries"][0]["status"], "failed");
    assert_eq!(failed["entries"][0]["used_percent"], Value::Null);
    for suffix in [
        "from_ms=0&to_ms=1&limit=201",
        "from_ms=0&to_ms=9999999999999&limit=1",
        "from_ms=2&to_ms=1&limit=1",
        "from_ms=0&to_ms=1&limit=1&unknown=1",
    ] {
        assert_eq!(
            test::call_service(
                &app,
                authorized(test::TestRequest::get().uri(&format!("/admin/quota-history?{suffix}")))
                    .to_request()
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
    // Existing history survives deletion; a captured in-flight append cannot recreate an account.
    let fence = history.capture(
        AccountScope {
            account_kind: "native".into(),
            config_version_id: String::new(),
            account_id: a.into(),
        },
        Some(0),
    )?;
    let r = test::call_service(
        &app,
        authorized(
            test::TestRequest::delete().uri(&format!("/admin/native-accounts/{a}?revision=0")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(r.status(), StatusCode::OK);
    assert!(matches!(
        history.append(&fence, &[Sample::missing("grok_console", now + 1, false)]),
        Err(gateway_store::quota_history::Error::Conflict)
    ));
    let retained: Value = test::call_and_read_body_json(
        &app,
        authorized(test::TestRequest::get().uri(&path)).to_request(),
    )
    .await;
    assert_eq!(retained["entries"], page["entries"]);
    Ok(())
}

#[actix_web::test]
async fn quota_history_ordinary_revision_fence_unknown_and_reopen() -> TestResult {
    let (file, resources) = fixture(false)?;
    let history = Arc::new(QuotaHistoryStore::open(&file.0)?);
    let account = AccountScope {
        account_kind: "credential".into(),
        config_version_id: VERSION.into(),
        account_id: "account-000".into(),
    };
    let fence = history.capture(account.clone(), Some(0))?;
    let unknown = Sample::missing("codex", 1000, false);
    history.append(&fence, std::slice::from_ref(&unknown))?;
    history.append(&fence, std::slice::from_ref(&unknown))?;
    let mut changed = unknown.clone();
    changed.status = "failed".into();
    assert!(matches!(
        history.append(&fence, &[changed]),
        Err(gateway_store::quota_history::Error::Conflict)
    ));
    history.append(&fence, &[Sample::missing("codex", 999, false)])?;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(
                resources.with_quota_history(history.clone()),
            ))
            .configure(configure_management_resources),
    )
    .await;
    let response = test::call_service(
        &app,
        authorized(test::TestRequest::patch().uri("/admin/credentials/account-000/status"))
            .insert_header(("If-Match", "rev-0"))
            .set_json(serde_json::json!({"credential_revision":0,"status":"disabled"}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(matches!(
        history.append(&fence, &[Sample::missing("codex", 2000, false)]),
        Err(gateway_store::quota_history::Error::Conflict)
    ));
    let reopened = QuotaHistoryStore::open(&file.0)?;
    let query = gateway_store::quota_history::Query {
        from_ms: 0,
        to_ms: 3000,
        limit: 10,
        after_id: 0,
        account_kind: Some("credential".into()),
        config_version_id: Some(VERSION.into()),
        account_id: Some("account-000".into()),
        provider: None,
        window: None,
    };
    let result = reopened.query(&query, 3000)?;
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].sample.status, "unknown");
    assert_eq!(result.entries[0].sample.used_percent, None);
    Ok(())
}

#[actix_web::test]
async fn quota_history_kimi_partial_projection_and_source_isolation() -> TestResult {
    let kimi = provider_openai_compatible::KimiAccountObservation::from_payloads(
        None,
        Some(
            &serde_json::json!({"usages":{"limit_5h":{"used_ratio":0},"limit_7d":{"used_ratio":-1}}}),
        ),
        1_000,
    );
    assert!(kimi.quota_available);
    let samples = AccountQuotaObservation::from_kimi(&kimi).history_samples();
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].completeness, "partial");
    assert_eq!(samples[0].used_percent, Some(0.0));
    let mut web = sample(1_000, 10.0, 2_000);
    web.source = "grok_web".into();
    web.windows[0].name = "web.auto".into();
    web.windows[1].name = "web.credits".into();
    let samples = web.history_samples();
    assert_eq!(samples[0].source, "rest");
    assert_eq!(samples[1].source, "grpc");
    Ok(())
}

#[actix_web::test]
async fn quota_history_missing_store_fails_before_native_transport() -> TestResult {
    let (file, _resources) = fixture(false)?;
    let key = KeyVersion::try_new(1)?;
    let secrets = SecretStore::new(MasterKeyRing::try_new(
        key,
        [(key, MasterKey::try_from_bytes([0x51; 32])?)],
    )?);
    let transport = Arc::new(Sequence(Mutex::new([Ok(sample(1000, 0.0, 2000))].into())));
    let native =
        gateway_http_actix::management_resources::native_accounts::NativeAccountManagement::new(
            Arc::new(provider_grok::GrokAccountPoolStore::try_open(
                &file.0,
                secrets.clone(),
            )?),
        )?
        .with_usage_transport(transport.clone());
    let resources = ManagementResourceHttpState::with_workflow(
        ManagementMutationService::new(SqliteControlPlaneRepository::open(&file.0)?, secrets),
        Box::new(gateway_http_actix::management_resources::CodexOAuthManagementWorkflow::new()),
    )
    .with_native_accounts(native);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(security()?))
            .app_data(web::Data::new(resources))
            .configure(configure_management_resources),
    )
    .await;
    let imported=test::call_service(&app,authorized(test::TestRequest::post().uri("/admin/native-accounts/import")).set_json(serde_json::json!({"id":"no-history","channel":"grok.console","secret":"synthetic-no-history"})).to_request()).await;
    assert_eq!(imported.status(), StatusCode::CREATED);
    let rows: Value = test::call_and_read_body_json(
        &app,
        authorized(test::TestRequest::get().uri("/admin/native-accounts")).to_request(),
    )
    .await;
    let id = rows["items"][0]["id"].as_str().ok_or("id")?;
    let r = test::call_service(
        &app,
        authorized(
            test::TestRequest::get().uri(&format!("/admin/native-accounts/{id}/usage?revision=0")),
        )
        .to_request(),
    )
    .await;
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(transport.0.lock().map_err(|_| "lock")?.len(), 1);
    Ok(())
}
