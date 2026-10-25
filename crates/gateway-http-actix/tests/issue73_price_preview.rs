//! Controlled management HTTP prices: source failure, review fencing and atomic application.
#![deny(unsafe_code)]
use actix_web::{App, HttpServer, web};
use gateway_control::{
    management_mutation_service::*, management_operations_service::ManagementOperationsError,
};
use gateway_http_actix::{
    management_resources::{
        ManagementResourceHttpState, configure_management_resources, price_source::PriceSource,
    },
    management_security::*,
};
use serde_json::{Value, json};
use std::{
    error::Error,
    path::PathBuf,
    sync::{Arc, Mutex},
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const KEY: &str = "mgmt_issue73_synthetic_0123456789abcdef";
const CSRF: &str = "csrf_issue73_synthetic_0123456789abcdef";
const ORIGIN: &str = "https://price.example.test";
#[derive(Default)]
struct Source(Mutex<Option<Vec<u8>>>);
impl Source {
    fn set(&self, value: Value) -> Result {
        *self.0.lock().map_err(|_| "source lock")? = Some(serde_json::to_vec(&value)?);
        Ok(())
    }
}
impl PriceSource for Source {
    fn fetch(&self) -> std::result::Result<Vec<u8>, ManagementOperationsError> {
        self.0
            .lock()
            .map_err(|_| ManagementOperationsError::SourceUnavailable)?
            .clone()
            .ok_or(ManagementOperationsError::SourceUnavailable)
    }
}
fn source_value() -> Value {
    json!({"vendor":{"models":{"known":{"cost":{"input":1.25,"output":2.5}}}}})
}
struct Http {
    client: reqwest::Client,
    base: String,
    effective: u64,
}
impl Http {
    #[allow(clippy::too_many_arguments)]
    async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Value,
        version: &str,
        revision: u64,
        csrf: bool,
        status: u16,
    ) -> Result<Value> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base))
            .header(MANAGEMENT_KEY_HEADER, KEY)
            .header("Origin", ORIGIN)
            .header("X-Config-Version", version)
            .header("If-Match", format!("\"rev-{revision}\""));
        if csrf {
            request = request.header("X-Management-CSRF-Token", CSRF);
        }
        let response = request.json(&body).send().await?;
        assert_eq!(
            response.status().as_u16(),
            status,
            "{}",
            response.text().await?
        );
        Ok(response.json().await?)
    }
    fn input(&self, id: &str) -> Value {
        json!({"catalog_version_id":id,"effective_at_ms":self.effective,"mappings":[{"source_provider":"vendor","source_model":"known",
            "provider_id":"provider","channel_id":"channel","model":"public",
            "missing_rates":{"reasoning":0,"cached":0,"cache_read":0,"cache_creation":0}}]})
    }
    async fn preview(&self, id: &str, status: u16) -> Result<Value> {
        self.send(
            reqwest::Method::POST,
            "/admin/billing/price-source/preview",
            self.input(id),
            "a",
            0,
            true,
            status,
        )
        .await
    }
    async fn apply(&self, p: &Value, version: &str, revision: u64, status: u16) -> Result<Value> {
        self.send(
            reqwest::Method::POST,
            "/admin/billing/price-source/apply",
            json!({"preview_id":p["preview_id"],"content_digest":p["content_digest"]}),
            version,
            revision,
            true,
            status,
        )
        .await
    }
    async fn get(&self, path: &str, status: u16) -> Result<Value> {
        self.send(
            reqwest::Method::GET,
            path,
            Value::Null,
            "a",
            0,
            true,
            status,
        )
        .await
    }
}
fn import(id: &str, rate: u64) -> Value {
    json!({"catalog_version_id":id,"effective_at_ms":0,"source":"operator","entries":[{
    "provider_id":"provider","channel_id":"channel","model":"public","input_microunits_per_million":rate,
    "output_microunits_per_million":rate,"reasoning_microunits_per_million":0,"cache_read_microunits_per_million":0,
    "cache_creation_microunits_per_million":0,"cached_microunits_per_million":0}]})
}
#[actix_web::test]
#[allow(clippy::too_many_lines)]
async fn price_preview_http_fences_sources_revisions_replays_and_audit_atomically() -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../output/issue-73/tmp")
        .join(format!(
            "http-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
    std::fs::create_dir_all(&root)?;
    let path = root.join("control.sqlite3");
    let mut repository = SqliteControlPlaneRepository::open(&path)?;
    for id in ["a", "b"] {
        repository.write_configuration(&ControlPlaneConfiguration::new(ConfigVersion {
            id: ConfigVersionId::try_new(id)?,
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms: 1,
            description: String::new(),
        }))?;
    }
    let source = Arc::new(Source::default());
    source.set(source_value())?;
    let key_version = KeyVersion::try_new(1)?;
    let resources = web::Data::new(
        ManagementResourceHttpState::new(ManagementMutationService::new(
            repository,
            SecretStore::new(MasterKeyRing::try_new(
                key_version,
                [(key_version, MasterKey::try_from_bytes([73; 32])?)],
            )?),
        ))
        .with_price_source(source.clone()),
    );
    let security = web::Data::new(ManagementHttpState::new(
        ManagementKey::try_new(KEY)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::SameOrigin {
            origin: ManagementOrigin::try_new(ORIGIN)?,
            csrf_token: ManagementCsrfToken::try_new(CSRF)?,
        },
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
    let http = Http {
        client: reqwest::Client::builder().no_proxy().build()?,
        base,
        effective: u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis(),
        )? + 300_000,
    };
    // Management authentication and CSRF are enforced before source I/O or catalog mutation.
    assert_eq!(
        http.client
            .post(format!("{}/admin/billing/price-source/preview", http.base))
            .json(&http.input("new"))
            .send()
            .await?
            .status()
            .as_u16(),
        404
    );
    http.send(
        reqwest::Method::POST,
        "/admin/billing/price-source/preview",
        http.input("new"),
        "a",
        0,
        false,
        404,
    )
    .await?;
    *source.0.lock().map_err(|_| "source lock")? = None;
    http.preview("new", 502).await?;
    assert!(
        http.get("/admin/billing/catalogs", 200)
            .await?
            .as_array()
            .ok_or("catalog array")?
            .is_empty()
    );
    source.set(source_value())?;
    let mut unknown = http.input("new");
    unknown["mappings"][0]["source_model"] = json!("unknown");
    let p = http
        .send(
            reqwest::Method::POST,
            "/admin/billing/price-source/preview",
            unknown,
            "a",
            0,
            true,
            200,
        )
        .await?;
    assert_eq!(p["can_apply"], false);
    assert!(p["preview_id"].is_null());
    assert_eq!(p["changes"][0]["issues"][0], "unknown_model");
    let mut incomplete = http.input("new");
    incomplete["mappings"][0]["missing_rates"] = json!({});
    let p = http
        .send(
            reqwest::Method::POST,
            "/admin/billing/price-source/preview",
            incomplete,
            "a",
            0,
            true,
            200,
        )
        .await?;
    assert_eq!(p["can_apply"], false);
    assert!(p["changes"][0]["rates"]["reasoning"].is_null());
    for (field, value, issue) in [
        ("input", json!(0.0000001), "precision_conflict"),
        ("tiers", json!([]), "unsupported_tier_or_rate"),
        ("input", json!(-1), "invalid_rate"),
        ("input", json!(9007199254.740992_f64), "rate_overflow"),
    ] {
        let mut doc = source_value();
        doc["vendor"]["models"]["known"]["cost"][field] = value;
        source.set(doc)?;
        let p = http.preview("new", 200).await?;
        assert_eq!(p["can_apply"], false, "{p}");
        assert!(
            p["changes"][0]["issues"]
                .as_array()
                .ok_or("issues")?
                .contains(&json!(issue)),
            "{p}"
        );
    }
    let mut unit = source_value();
    unit["vendor"]["models"]["known"]["currency"] = json!("EUR");
    source.set(unit)?;
    assert_eq!(http.preview("new", 200).await?["can_apply"], false);
    source.set(source_value())?;
    let refreshed = http
        .send(
            reqwest::Method::POST,
            "/admin/billing/price-source/refresh",
            json!({"models":["known","unknown"]}),
            "a",
            0,
            true,
            200,
        )
        .await?;
    assert_eq!(
        refreshed["items"][0]["input_microunits_per_million"],
        1_250_000
    );
    assert!(refreshed["items"][0]["reasoning_microunits_per_million"].is_null());
    assert_eq!(refreshed["items"][1]["issues"][0], "unknown_model");
    let p = http.preview("new", 200).await?;
    assert_eq!(p["changes"][0]["change"], "added");
    assert_eq!(p["changes"][0]["source_rates"]["input"], 1_250_000);
    let mut tampered = p.clone();
    tampered["content_digest"] = json!("changed");
    http.apply(&tampered, "a", 0, 409).await?;
    http.apply(&p, "b", 0, 409).await?;
    http.apply(&p, "a", 1, 409).await?;
    *source.0.lock().map_err(|_| "source lock")? = None;
    http.apply(&p, "a", 0, 502).await?;
    source.set(source_value())?;
    let mut changed = source_value();
    changed["vendor"]["models"]["known"]["cost"]["input"] = json!(2);
    source.set(changed)?;
    http.apply(&p, "a", 0, 409).await?;
    source.set(source_value())?;
    // Another configuration's write leaves a's revision intact but invalidates its global preview.
    let mut concurrent = import("concurrent", 2_000_000);
    let mut omitted = concurrent["entries"][0].clone();
    omitted["model"] = json!("omitted-model");
    concurrent["entries"]
        .as_array_mut()
        .ok_or("entries")?
        .push(omitted);
    http.send(
        reqwest::Method::POST,
        "/admin/billing/catalogs",
        concurrent,
        "b",
        0,
        true,
        201,
    )
    .await?;
    http.apply(&p, "a", 0, 409).await?;
    assert!(
        http.get("/admin/resource-audit-events", 200).await?["items"]
            .as_array()
            .ok_or("audit")?
            .is_empty()
    );
    let p = http.preview("new", 200).await?;
    assert_eq!(p["changes"][0]["change"], "changed");
    assert_eq!(p["changes"][1]["change"], "removed_from_proposal");
    assert_eq!(p["changes"][1]["effect"], "retains_prior_price");
    // A storage/audit failure rolls back catalog, review and revision; same confirmation can retry.
    let connection = gateway_store::open(&path)?;
    connection.execute_batch("CREATE TRIGGER reject_review_audit BEFORE INSERT ON management_resource_audit_events WHEN NEW.action='billing_catalog_review_applied' BEGIN SELECT RAISE(ABORT,'controlled audit failure'); END;")?;
    http.apply(&p, "a", 0, 500).await?;
    assert!(
        SqliteControlPlaneRepository::open(&path)?
            .load_billing_catalog_review("new")?
            .is_none()
    );
    assert_eq!(
        http.get("/admin/billing/catalogs", 200)
            .await?
            .as_array()
            .ok_or("catalogs")?
            .len(),
        1
    );
    connection.execute_batch("DROP TRIGGER reject_review_audit;")?;
    let receipt = http.apply(&p, "a", 0, 201).await?;
    assert_eq!(receipt["catalog_version_id"], "new");
    http.apply(&p, "a", 0, 409).await?;
    http.apply(&p, "a", 1, 409).await?;
    let review = http.get("/admin/billing/catalogs/new/review", 200).await?;
    assert_eq!(review["source_digest"], p["source_digest"]);
    assert_eq!(review["changes"], p["changes"]);
    let audit = http.get("/admin/resource-audit-events", 200).await?;
    assert_eq!(audit["items"].as_array().ok_or("audit")?.len(), 1);
    assert_eq!(
        audit["items"][0]["action"],
        "billing_catalog_review_applied"
    );
    http.send(
        reqwest::Method::POST,
        "/admin/billing/catalogs/new/rollback",
        json!({"new_catalog_version_id":"rollback","effective_at_ms":http.effective+1}),
        "a",
        1,
        true,
        201,
    )
    .await?;
    let catalogs = http.get("/admin/billing/catalogs", 200).await?;
    assert_eq!(catalogs.as_array().ok_or("catalogs")?.len(), 3);
    let new = catalogs
        .as_array()
        .ok_or("catalogs")?
        .iter()
        .find(|c| c["catalog_version_id"] == "new")
        .ok_or("new")?;
    let fork = catalogs
        .as_array()
        .ok_or("catalogs")?
        .iter()
        .find(|c| c["catalog_version_id"] == "rollback")
        .ok_or("fork")?;
    assert_eq!(new["entries"], fork["entries"]);
    assert_eq!(
        http.get("/admin/resource-audit-events", 200).await?["items"]
            .as_array()
            .ok_or("audit")?
            .len(),
        2
    );
    let encoded = serde_json::to_string(&review)?;
    for forbidden in [
        KEY,
        CSRF,
        "authorization",
        "cookie",
        "api_key",
        "endpoint_url",
    ] {
        assert!(!encoded.contains(forbidden), "{forbidden}");
    }
    handle.stop(true).await;
    assert!(
        SqliteControlPlaneRepository::open(&path)?
            .load_billing_catalog_review("new")?
            .is_some()
    );
    Ok(())
}
