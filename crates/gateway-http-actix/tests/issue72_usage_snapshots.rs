//! Frozen query/export regression over real loopback management HTTP and isolated `SQLite`.
#![deny(unsafe_code)]
use actix_web::{App, HttpServer, web};
use gateway_control::{
    billing_materializer::{BILLING_MATERIALIZER_ID, materialize_billing_events_with_retention},
    management_mutation_service::{
        KeyVersion, ManagementMutationService, MasterKey, MasterKeyRing, SecretStore,
        SqliteControlPlaneRepository,
    },
    management_operations_service::{
        ManagementOperationsError, OperationalUsagePage, OperationalUsageQuery,
    },
    usage_snapshot::{SnapshotError, UsageSnapshot, UsageSnapshotQuery, read_usage_snapshot},
};
use gateway_core::*;
use gateway_http_actix::{
    management_resources::{
        ManagementResourceHttpState, ManagementUsageFacade, configure_management_resources,
    },
    management_security::{
        ManagementBrowserPolicy, ManagementHttpState, ManagementKey, ManagementNetworkPolicy,
    },
};
use gateway_store::{
    billing_ledger::{
        BillingCatalogSource, BillingPriceCatalog, BillingPriceEntry, SqliteBillingLedger,
    },
    event_store::SqliteEventStore,
};
use serde_json::{Value, json};
use std::{
    error::Error,
    path::{Path, PathBuf},
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const KEY: &str = "mgmt_issue72_synthetic_0123456789abcdef";
struct Source(PathBuf);
impl ManagementUsageFacade for Source {
    fn list_usage(
        &self,
        _: &OperationalUsageQuery,
    ) -> std::result::Result<OperationalUsagePage, ManagementOperationsError> {
        Err(ManagementOperationsError::SourceUnavailable)
    }
    fn create_usage_snapshot(
        &self,
        query: UsageSnapshotQuery,
    ) -> std::result::Result<UsageSnapshot, SnapshotError> {
        let store = SqliteEventStore::open_read_only(&self.0)
            .map_err(|_| SnapshotError::SourceUnavailable)?;
        read_usage_snapshot(&store, query)
    }
}
fn request(id: &str, key: &str, model: &str, time: i64) -> Result<RequestEvent> {
    Ok(RequestEvent::new(
        RequestId::try_new(id)?,
        ClientKeyId::try_new(key)?,
        Some(AccessGroupId::try_new(if key == "a" {
            "group-a"
        } else {
            "group-b"
        })?),
        GatewayProtocol::OpenAiResponses,
        model.to_owned(),
        model.to_owned(),
        None,
        false,
    )
    .with_started_at_ms(time))
}
fn attempt(
    r: &RequestEvent,
    n: u64,
    account: &str,
    time: i64,
    failed: bool,
) -> Result<AttemptEvent> {
    Ok(AttemptEvent::new(
        r.request_id().clone(),
        n,
        RouteId::try_new("route")?,
        RouteCandidateId::try_new("candidate")?,
        CredentialId::try_new(account)?,
        EndpointId::try_new("channel")?,
        UpstreamId::try_new("provider")?,
        "private-upstream-model".to_owned(),
        time - 1,
        time,
        if failed {
            AttemptOutcome::Failed(GatewayError::new(
                GatewayErrorCode::ProviderTransient,
                ErrorScope::Provider,
            ))
        } else {
            AttemptOutcome::Succeeded
        },
        if failed {
            AttemptRetryDecision::RetryEligible
        } else {
            AttemptRetryDecision::Completed
        },
    ))
}
fn usage(
    a: &AttemptEvent,
    input: u64,
    output: Option<u64>,
    provenance: UsageProvenance,
) -> Result<GatewayEvent> {
    Ok(GatewayEvent::Usage(
        UsageEvent::from_usage(
            a.request_id().clone(),
            ResponseId::try_new(format!("response-{}", a.attempt_id()))?,
            &Usage {
                input_tokens: Some(input),
                output_tokens: output,
                reasoning_tokens: Some(0),
                cached_tokens: Some(0),
                cache_read_tokens: None,
                cache_creation_tokens: None,
                provenance,
                input_accounting: InputTokenAccounting::Inclusive,
                ..Default::default()
            },
        )
        .with_attempt_id(a.attempt_id().clone()),
    ))
}
fn finished(r: &RequestEvent, outcome: RequestOutcome) -> GatewayEvent {
    GatewayEvent::RequestFinished(RequestFinishedEvent {
        request_id: r.request_id().clone(),
        started_at_ms: 1000,
        finished_at_ms: 4000,
        duration_ms: 3000,
        first_content_ms: None,
        outcome,
        error_code: None,
    })
}
fn seed(path: &Path) -> Result {
    let mut store = SqliteEventStore::open(path)?;
    let r = request("retry", "a", "priced", 1000)?;
    let a = attempt(&r, 1, "account-a", 1100, true)?;
    let b = attempt(&r, 2, "account-b", 2100, false)?;
    store.append_batch(&[
        GatewayEvent::Request(r.clone()),
        GatewayEvent::Attempt(a.clone()),
        usage(&a, 10, Some(2), UsageProvenance::Measured)?,
        GatewayEvent::Attempt(b.clone()),
        usage(&b, 20, Some(4), UsageProvenance::Measured)?,
        finished(&r, RequestOutcome::Succeeded),
    ])?;
    for (id, key, model, input, output, provenance, outcome) in [
        (
            "cancel",
            "a",
            "priced",
            5,
            None,
            UsageProvenance::Measured,
            RequestOutcome::Cancelled,
        ),
        (
            "zero",
            "a",
            "=unpriced",
            0,
            Some(0),
            UsageProvenance::Measured,
            RequestOutcome::Failed,
        ),
        (
            "estimated",
            "a",
            "priced",
            7,
            Some(0),
            UsageProvenance::Estimated,
            RequestOutcome::Succeeded,
        ),
        (
            "foreign",
            "b",
            "priced",
            999,
            Some(0),
            UsageProvenance::Measured,
            RequestOutcome::Succeeded,
        ),
    ] {
        let r = request(id, key, model, 3000)?;
        let a = attempt(&r, 1, "account-a", 3100, false)?;
        store.append_batch(&[
            GatewayEvent::Request(r.clone()),
            GatewayEvent::Attempt(a.clone()),
            usage(&a, input, output, provenance)?,
            finished(&r, outcome),
        ])?;
    }
    store.append_batch(&[GatewayEvent::Request(request(
        "pending", "a", "priced", 3200,
    )?)])?;
    let mut ledger = SqliteBillingLedger::open(path)?;
    for (id, time, rate) in [("price-v1", 0, 1_000_000), ("price-v2", 2500, 2_000_000)] {
        ledger.insert_catalog(&BillingPriceCatalog {
            catalog_version_id: id.to_owned(),
            effective_at_ms: time,
            created_at_ms: 0,
            source: BillingCatalogSource::Test,
            entries: vec![BillingPriceEntry {
                provider_id: "provider".to_owned(),
                channel_id: "channel".to_owned(),
                model: "priced".to_owned(),
                input_microunits_per_million: rate,
                output_microunits_per_million: rate,
                reasoning_microunits_per_million: 0,
                cache_read_microunits_per_million: 0,
                cache_creation_microunits_per_million: 0,
                cached_microunits_per_million: 0,
            }],
        })?;
    }
    let receipt = materialize_billing_events_with_retention(
        &store,
        &mut ledger,
        BILLING_MATERIALIZER_ID,
        1024,
        None,
        5000,
    )?;
    assert_eq!(receipt.failed_events, 0);
    Ok(())
}
async fn api(
    client: &reqwest::Client,
    base: &str,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
    status: u16,
) -> Result<Value> {
    let mut req = client
        .request(method, format!("{base}{path}"))
        .header("X-Management-Key", KEY)
        .header("Content-Type", "application/json");
    if let Some(body) = body {
        req = req.body(body.to_string());
    }
    let response = req.send().await?;
    let actual = response.status().as_u16();
    let bytes = response.bytes().await?;
    assert_eq!(actual, status, "{}", String::from_utf8_lossy(&bytes));
    if bytes.is_empty() {
        Ok(Value::Null)
    } else {
        Ok(serde_json::from_slice(&bytes)?)
    }
}
fn filters() -> Value {
    json!({"from_ms":1000,"to_ms":5000,"bucket_ms":1000,"client_key_id":"a","access_group_id":"group-a"})
}

#[actix_web::test]
#[allow(clippy::too_many_lines)]
async fn usage_snapshot_http_freezes_totals_details_and_exports() -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../output/issue-72/tmp")
        .join(format!(
            "http-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
    std::fs::create_dir_all(&root)?;
    let path = root.join("control.sqlite3");
    seed(&path)?;
    let version = KeyVersion::try_new(1)?;
    let secrets = SecretStore::new(MasterKeyRing::try_new(
        version,
        [(version, MasterKey::try_from_bytes([7; 32])?)],
    )?);
    let resources = web::Data::new(
        ManagementResourceHttpState::new(ManagementMutationService::new(
            SqliteControlPlaneRepository::open(&path)?,
            secrets,
        ))
        .with_usage(Box::new(Source(path.clone()))),
    );
    let security = web::Data::new(ManagementHttpState::new(
        ManagementKey::try_new(KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?);
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let base = format!("http://{}", listener.local_addr()?);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(resources.clone())
            .app_data(security.clone())
            .configure(configure_management_resources)
    })
    .workers(1)
    .listen(listener)?
    .run();
    let handle = server.handle();
    actix_web::rt::spawn(server);
    let client = reqwest::Client::builder().no_proxy().build()?;
    assert_eq!(
        client
            .post(format!("{base}/admin/operations/usage-snapshots"))
            .body(filters().to_string())
            .send()
            .await?
            .status()
            .as_u16(),
        404
    );
    let snapshot = api(
        &client,
        &base,
        reqwest::Method::POST,
        "/admin/operations/usage-snapshots",
        Some(filters()),
        201,
    )
    .await?;
    let id = snapshot["snapshot_id"].as_str().ok_or("snapshot id")?;
    let prefix = format!("/admin/operations/usage-snapshots/{id}");
    assert_eq!(snapshot["summary"]["requests"], 5);
    assert_eq!(snapshot["summary"]["attempts"], 5);
    assert_eq!(snapshot["summary"]["usage_observations"], 5);
    assert_eq!(snapshot["detail_rows"], 6);
    assert_eq!(snapshot["summary"]["input_tokens"]["total"], 42);
    assert_eq!(snapshot["summary"]["known_cost_microunits"], 46);
    assert_eq!(snapshot["summary"]["cost_status"], "partial");
    assert_eq!(
        snapshot["summary"]["cost_records"],
        json!({"exact":2,"partial":1,"unknown":1,"unpriced":1,"pending":1,"unavailable":0})
    );
    assert_eq!(snapshot["summary"]["failed_usage_observations"], 2);
    assert_eq!(snapshot["summary"]["cancelled_usage_observations"], 1);
    let mut query = filters();
    query["account_id"] = json!("account-a");
    let scoped = api(
        &client,
        &base,
        reqwest::Method::POST,
        "/admin/operations/usage-snapshots",
        Some(query),
        201,
    )
    .await?;
    assert_eq!(scoped["summary"]["requests"], 4);
    assert_eq!(scoped["summary"]["known_cost_microunits"], 22);
    let mut foreign = filters();
    foreign["client_key_id"] = json!("b");
    foreign["access_group_id"] = json!("group-b");
    let other = api(
        &client,
        &base,
        reqwest::Method::POST,
        "/admin/operations/usage-snapshots",
        Some(foreign),
        201,
    )
    .await?;
    assert_eq!(other["summary"]["input_tokens"]["total"], 999);
    // New events and ledger retention run after creation, between export pages.
    let first = api(
        &client,
        &base,
        reqwest::Method::GET,
        &format!("{prefix}/rows?limit=1"),
        None,
        200,
    )
    .await?;
    let mut rows = first["items"].as_array().ok_or("items")?.clone();
    let mut cursor = first["next_cursor"].clone();
    let mut events = SqliteEventStore::open(&path)?;
    events.append_batch(&[GatewayEvent::Request(request(
        "concurrent",
        "a",
        "priced",
        3300,
    )?)])?;
    SqliteBillingLedger::open(&path)?.purge_expired(i64::MAX.cast_unsigned(), 100)?;
    while let Some(c) = cursor.as_str() {
        let page = api(
            &client,
            &base,
            reqwest::Method::GET,
            &format!("{prefix}/rows?limit=1&cursor={c}"),
            None,
            200,
        )
        .await?;
        rows.extend(page["items"].as_array().ok_or("items")?.iter().cloned());
        cursor = page["next_cursor"].clone();
    }
    assert_eq!(rows.len(), 6);
    assert_eq!(
        rows.iter()
            .filter_map(|r| r["input_tokens"].as_u64())
            .sum::<u64>(),
        42
    );
    let mut exported = Vec::new();
    let mut next = None::<String>;
    loop {
        let suffix = next
            .as_ref()
            .map_or(String::new(), |c| format!("&cursor={c}"));
        let response = client
            .get(format!(
                "{base}{prefix}/export?format=jsonl&limit=2{suffix}"
            ))
            .header("X-Management-Key", KEY)
            .send()
            .await?;
        assert_eq!(response.status(), 200);
        next = response
            .headers()
            .get("X-Next-Cursor")
            .map(|v| v.to_str().map(str::to_owned))
            .transpose()?;
        for line in response.text().await?.lines() {
            exported.push(serde_json::from_str::<Value>(line)?);
        }
        if next.is_none() {
            break;
        }
    }
    assert_eq!(exported, rows);
    let csv = client
        .get(format!("{base}{prefix}/export?format=csv"))
        .header("X-Management-Key", KEY)
        .send()
        .await?
        .text()
        .await?;
    assert!(csv.contains("\"'=unpriced\""));
    assert!(!csv.contains("private-upstream-model"));
    assert!(!csv.contains("source_fingerprint"));
    let buckets = api(
        &client,
        &base,
        reqwest::Method::GET,
        &format!("{prefix}/rows?grain=buckets&limit=1"),
        None,
        200,
    )
    .await?;
    assert!(buckets["next_cursor"].is_string());
    let c = buckets["next_cursor"].as_str().ok_or("bucket cursor")?;
    api(
        &client,
        &base,
        reqwest::Method::GET,
        &format!("{prefix}/export?cursor={c}"),
        None,
        409,
    )
    .await?;
    api(
        &client,
        &base,
        reqwest::Method::GET,
        &format!("{prefix}/rows?limit=101"),
        None,
        400,
    )
    .await?;
    api(
        &client,
        &base,
        reqwest::Method::GET,
        &format!("{prefix}/export?client_key_id=b"),
        None,
        400,
    )
    .await?;
    api(
        &client,
        &base,
        reqwest::Method::GET,
        &format!("{prefix}/rows?limit=1&limit=2"),
        None,
        400,
    )
    .await?;
    let mut invalid = filters();
    invalid["bucket_ms"] = json!(1);
    api(
        &client,
        &base,
        reqwest::Method::POST,
        "/admin/operations/usage-snapshots",
        Some(invalid),
        400,
    )
    .await?;
    let stable = api(&client, &base, reqwest::Method::GET, &prefix, None, 200).await?;
    assert_eq!(stable, snapshot);
    api(&client, &base, reqwest::Method::DELETE, &prefix, None, 204).await?;
    api(
        &client,
        &base,
        reqwest::Method::GET,
        &format!("{prefix}/export"),
        None,
        410,
    )
    .await?;
    let mut empty = filters();
    empty["client_key_id"] = json!("absent");
    let empty = api(
        &client,
        &base,
        reqwest::Method::POST,
        "/admin/operations/usage-snapshots",
        Some(empty),
        201,
    )
    .await?;
    assert_eq!(empty["summary"]["requests"], 0);
    assert!(empty["summary"]["known_cost_microunits"].is_null());
    let empty_id = empty["snapshot_id"].as_str().ok_or("empty id")?;
    let empty_csv = client
        .get(format!(
            "{base}/admin/operations/usage-snapshots/{empty_id}/export?format=csv"
        ))
        .header("X-Management-Key", KEY)
        .send()
        .await?
        .text()
        .await?;
    assert!(empty_csv.starts_with("\"row_kind\""));
    assert_eq!(empty_csv.lines().count(), 1);
    let bulk = (0..20_001)
        .map(|i| request(&format!("bulk-{i}"), "bulk", "priced", 3000).map(GatewayEvent::Request))
        .collect::<Result<Vec<_>>>()?;
    events.append_batch(&bulk)?;
    let mut large = filters();
    large["client_key_id"] = json!("bulk");
    large["access_group_id"] = json!("group-b");
    api(
        &client,
        &base,
        reqwest::Method::POST,
        "/admin/operations/usage-snapshots",
        Some(large),
        413,
    )
    .await?;
    // Removing only this isolated source makes a new capture unavailable; it must not fabricate zero totals.
    drop(events);
    std::fs::rename(&path, root.join("unavailable.sqlite3"))?;
    api(
        &client,
        &base,
        reqwest::Method::POST,
        "/admin/operations/usage-snapshots",
        Some(filters()),
        503,
    )
    .await?;
    handle.stop(true).await;
    Ok(())
}
