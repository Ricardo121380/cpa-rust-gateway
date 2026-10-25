//! Opt-in integration helper: real protected price HTTP and private stored-payload inspection.
#![deny(unsafe_code)]
use actix_web::{App, HttpServer, web};
use gateway_control::{
    management_mutation_service::{ManagementMutationService, SqliteControlPlaneRepository},
    management_operations_service::ManagementOperationsError,
};
use gateway_http_actix::{
    management_resources::{
        ManagementResourceHttpState, configure_management_resources, price_source::PriceSource,
    },
    management_security::{
        ManagementBrowserPolicy, ManagementHttpState, ManagementKey, ManagementNetworkPolicy,
    },
};
use gateway_store::{
    secret_store::{KeyVersion, MasterKeyRing, SecretStore},
    stored_response::SqliteStoredResponseStore,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{error::Error, fs, path::PathBuf, sync::Arc};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    database: PathBuf,
    keys: PathBuf,
    input: PathBuf,
    output: PathBuf,
    action: String,
}
struct Source;
impl PriceSource for Source {
    fn fetch(&self) -> std::result::Result<Vec<u8>, ManagementOperationsError> {
        Ok(br#"{"synthetic":{"models":{"public":{"cost":{"input":3,"output":3}},"decoy":{"cost":{"input":99,"output":99}}}}}"#.to_vec())
    }
}

#[actix_web::test]
#[ignore = "private synthetic operator job, called by the wave2 process driver"]
async fn wave2_operator_job() -> Result {
    let job: Job = serde_json::from_slice(&fs::read(std::env::var("CPAR_WAVE2_JOB")?)?)?;
    let input: Value = serde_json::from_slice(&fs::read(&job.input)?)?;
    let secrets = SecretStore::new(MasterKeyRing::load_from_directory(
        &job.keys,
        KeyVersion::try_new(1)?,
    )?);
    let output = match job.action.as_str() {
        "review" => reviewed_price(&job, &input, secrets).await?,
        "thinking" => {
            let store = SqliteStoredResponseStore::open(&job.database, secrets)?;
            let id = gateway_core::ResponseId::try_new(
                input["response_id"].as_str().ok_or("response id")?,
            )?;
            let now = i64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis(),
            )?;
            let owner = gateway_core::ClientKeyId::try_new("owner")?;
            let record = store.get_owned(&owner, &id, now)?.ok_or("owned record")?;
            let thinking = record.payload().request().model_thinking.as_deref();
            assert_eq!(
                thinking,
                Some(&gateway_core::ModelThinking::Effort(
                    gateway_core::ThinkingEffort::try_new("high")?
                ))
            );
            assert!(
                store
                    .get_owned(&gateway_core::ClientKeyId::try_new("sibling")?, &id, now)?
                    .is_none()
            );
            json!({"thinking":thinking,"public_model":record.payload().public_model(),"foreign_owner":"unknown"})
        }
        _ => return Err("unsupported wave2 job".into()),
    };
    fs::write(job.output, serde_json::to_vec(&output)?)?;
    Ok(())
}

async fn reviewed_price(job: &Job, input: &Value, secrets: SecretStore) -> Result<Value> {
    let key = input["management_key"].as_str().ok_or("management key")?;
    let resources = web::Data::new(
        ManagementResourceHttpState::new(ManagementMutationService::new(
            SqliteControlPlaneRepository::open(&job.database)?,
            secrets,
        ))
        .with_price_source(Arc::new(Source)),
    );
    let security = web::Data::new(ManagementHttpState::new(
        ManagementKey::try_new(key)?,
        ManagementNetworkPolicy::LoopbackOnly,
        ManagementBrowserPolicy::DenyBrowserOrigins,
    )?);
    let listener = std::net::TcpListener::bind((
        "127.0.0.1",
        u16::try_from(input["port"].as_u64().ok_or("port")?)?,
    ))?;
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
    let scope = input["config_version"].as_str().ok_or("config version")?;
    let revision = input["revision"].as_str().ok_or("revision")?;
    let effective = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )? + 2_000;
    let mappings = ["public-wave2", "team/public-wave2(high)", "gpt-5.5"].into_iter().map(|model| json!({
        "source_provider":"synthetic", "source_model":if model == "public-wave2" {"public"} else {"decoy"},
        "provider_id":"ordinary", "channel_id":"ordinary-endpoint", "model":model,
        "missing_rates":{"reasoning":0,"cached":0,"cache_read":0,"cache_creation":0}
    })).collect::<Vec<_>>();
    let response = client.post(format!("{base}/admin/billing/price-source/preview"))
        .header("X-Management-Key", key).header("X-Config-Version", scope).header("If-Match", revision)
        .body(json!({"catalog_version_id":"wave2-reviewed", "effective_at_ms":effective,"mappings":mappings}).to_string()).send().await?;
    assert_eq!(response.status().as_u16(), 200);
    let preview: Value = serde_json::from_slice(&response.bytes().await?)?;
    assert_eq!(preview["can_apply"], true);
    let response = client
        .post(format!("{base}/admin/billing/price-source/apply"))
        .header("X-Management-Key", key)
        .header("X-Config-Version", scope)
        .header("If-Match", revision)
        .body(
            json!({"preview_id":preview["preview_id"],"content_digest":preview["content_digest"]})
                .to_string(),
        )
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 201);
    let receipt: Value = serde_json::from_slice(&response.bytes().await?)?;
    let response = client
        .get(format!(
            "{base}/admin/billing/catalogs/wave2-reviewed/review"
        ))
        .header("X-Management-Key", key)
        .header("X-Config-Version", scope)
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 200);
    let review: Value = serde_json::from_slice(&response.bytes().await?)?;
    assert_eq!(review["source_digest"], preview["source_digest"]);
    assert_eq!(review["content_digest"], preview["content_digest"]);
    handle.stop(true).await;
    Ok(json!({"review":review,"receipt":receipt,"effective_at_ms":effective}))
}
