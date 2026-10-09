//! E-02: real management HTTP + committed SQLite, then a loopback proxy drops the reply.
use super::*;
use actix_web::HttpServer;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn request(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
) -> reqwest::RequestBuilder {
    client
        .request(method, url)
        .header(MANAGEMENT_KEY_HEADER, KEY)
        .header("X-Config-Version", VERSION)
}
async fn read_json(response: reqwest::Response) -> Result<Value, Box<dyn Error>> {
    assert!(response.status().is_success(), "HTTP {}", response.status());
    Ok(serde_json::from_str(&response.text().await?)?)
}

#[actix_web::test]
async fn kiro_committed_device_authorization_recovers_after_real_connection_loss() -> TestResult {
    assert_commit_loss(false).await
}
#[actix_web::test]
async fn kimi_committed_device_authorization_recovers_after_real_connection_loss() -> TestResult {
    assert_commit_loss(true).await
}

#[allow(clippy::too_many_lines)] // One ordered commit/loss/recovery scenario with a shared channel setup.
async fn assert_commit_loss(kimi: bool) -> TestResult {
    use gateway_http_actix::management_resources::{
        kimi_device::{KimiDeviceHttpResponse, KimiDeviceWorkflow},
        kiro_device::KiroDeviceWorkflow,
    };
    let (file, state) = fixture(false)?;
    let db = gateway_store::open(&file.0)?;
    let kind = if kimi { "kimi-coding" } else { "kiro" };
    db.execute(
        "UPDATE upstreams SET kind=?1 WHERE id='owner-a' AND config_version_id=?2",
        (kind, VERSION),
    )?;
    let exchanges = Arc::new(AtomicUsize::new(0));
    let state = if kimi {
        let count = Arc::clone(&exchanges);
        state.with_kimi_workflow(KimiDeviceWorkflow::with_transport(Box::new(move|url,_,_|{
            if url.ends_with("/device_authorization") {return Ok(KimiDeviceHttpResponse::new(200,serde_json::json!({"device_code":"synthetic-device","user_code":"SYNTHETIC","verification_uri":"https://auth.kimi.com/device","expires_in":300,"interval":5})));}
            if url.ends_with("/token") {count.fetch_add(1,Ordering::SeqCst);return Ok(KimiDeviceHttpResponse::new(200,serde_json::json!({"access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_in":3600})));}
            Err(())
        })))
    } else {
        let count = Arc::clone(&exchanges);
        state.with_kiro_workflow(KiroDeviceWorkflow::with_transport(Box::new(move|url,_|{
            if url.ends_with("/client/register") {return Ok(serde_json::json!({"clientId":"synthetic-client","clientSecret":"synthetic-secret"}));}
            if url.ends_with("/device_authorization") {return Ok(serde_json::json!({"deviceCode":"synthetic-device","userCode":"SYNTHETIC","verificationUri":"https://view.awsapps.com/start/#/device","expiresIn":300,"interval":5}));}
            if url.ends_with("/token") {count.fetch_add(1,Ordering::SeqCst);return Ok(serde_json::json!({"accessToken":"synthetic-access","refreshToken":"synthetic-refresh","expiresIn":3600}));}
            Err(())
        })))
    };
    let guard = security()?;
    let state = web::Data::new(state);
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(guard.clone()))
            .app_data(state.clone())
            .configure(configure_management_resources)
    })
    .workers(1)
    .listen(listener)?
    .run();
    let handle = server.handle();
    let running = actix_web::rt::spawn(server);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let channel = if kimi { "kimi" } else { "kiro" };
    let base = format!("http://{address}");
    let path = format!("{base}/admin/upstreams/owner-a/{channel}-authorization");
    let start = read_json(
        request(&client, reqwest::Method::POST, &format!("{path}/start"))
            .header("If-Match", "rev-0")
            .header("Content-Type", "application/json")
            .body(r#"{"id":"seo-device-account"}"#)
            .send()
            .await?,
    )
    .await?;
    let session = start["session_id"].as_str().ok_or("session")?.to_owned();
    assert_eq!(exchanges.load(Ordering::SeqCst), 0);
    let count: i64 = db.query_row(
        "SELECT count(*) FROM upstream_credentials WHERE id='seo-device-account'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(count, 0);
    tokio::time::sleep(std::time::Duration::from_millis(5100)).await;
    let revision = if kimi { "rev-1" } else { "rev-0" };
    let payload = serde_json::json!({"id":"seo-device-account","session_id":session}).to_string();
    let proxy = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let proxy_address = proxy.local_addr()?;
    let forward = client.clone();
    let forward_path = format!("{path}/poll");
    let forward_body = payload.clone();
    let database = file.0.clone();
    let dropped = tokio::spawn(async move {
        let (mut socket, _) = proxy.accept().await?;
        let mut incoming = Vec::new();
        let forwarded_body = loop {
            let mut chunk = [0; 1024];
            let received = socket.read(&mut chunk).await?;
            assert!(
                received > 0 && incoming.len() + received <= 8192,
                "bounded client poll"
            );
            incoming.extend_from_slice(&chunk[..received]);
            if let Some(split) = incoming.windows(4).position(|value| value == b"\r\n\r\n") {
                let header = std::str::from_utf8(&incoming[..split])?;
                assert!(header.starts_with("POST /poll HTTP/1.1\r\n"));
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .map(str::parse::<usize>)
                    })
                    .ok_or("client body length")??;
                if incoming.len() >= split + 4 + length {
                    break std::str::from_utf8(&incoming[split + 4..split + 4 + length])?
                        .to_owned();
                }
            }
        };
        assert_eq!(
            forwarded_body, forward_body,
            "forward the actual original request body"
        );
        let response = request(&forward, reqwest::Method::POST, &forward_path)
            .header("If-Match", revision)
            .header("Content-Type", "application/json")
            .body(forwarded_body)
            .send()
            .await?;
        assert_eq!(response.status(), reqwest::StatusCode::CREATED);
        let response_revision = response
            .headers()
            .get("ETag")
            .ok_or("etag")?
            .to_str()?
            .to_owned();
        let complete: Value = serde_json::from_str(&response.text().await?)?;
        assert_eq!(complete["state"], "completed");
        let db = gateway_store::open(&database)?;
        let saved:(i64,i64)=db.query_row("SELECT count(*),revision FROM upstream_credentials WHERE config_version_id=?1 AND id='seo-device-account'",[VERSION],|row|Ok((row.get(0)?,row.get(1)?)))?;
        assert_eq!(
            saved,
            (1, 0),
            "SQLite commit is visible before cutting the client connection"
        );
        // No success bytes are sent to the original client.
        socket.shutdown().await?;
        Ok::<_, Box<dyn Error + Send + Sync>>((complete, response_revision, saved))
    });
    let lost = request(
        &client,
        reqwest::Method::POST,
        &format!("http://{proxy_address}/poll"),
    )
    .header("If-Match", revision)
    .header("Content-Type", "application/json")
    .body(payload.clone())
    .send()
    .await;
    assert!(
        lost.is_err(),
        "original client must observe loss, not success"
    );
    let (complete, committed_revision, saved) = dropped
        .await?
        .map_err(|error| -> Box<dyn Error> { error })?;
    assert_eq!(exchanges.load(Ordering::SeqCst), 1);
    for _ in 0..2 {
        let receipt = read_json(
            request(
                &client,
                reqwest::Method::GET,
                &format!("{base}/admin/account-authorization/{session}"),
            )
            .send()
            .await?,
        )
        .await?;
        assert_eq!(receipt["session_id"], session);
        assert_eq!(receipt["state"], "completed");
        assert_eq!(receipt["credential_id"], complete["credential_id"]);
        assert_eq!(receipt["revision"], committed_revision.trim_matches('"'));
        assert_eq!(receipt["config_version"], VERSION);
        assert_eq!(receipt["upstream_id"], "owner-a");
        assert_eq!(exchanges.load(Ordering::SeqCst), 1);
    }
    let replay = request(&client, reqwest::Method::POST, &format!("{path}/poll"))
        .header("If-Match", &committed_revision)
        .header("Content-Type", "application/json")
        .body(payload)
        .send()
        .await?;
    assert_eq!(replay.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(exchanges.load(Ordering::SeqCst), 1);
    let denied = client
        .get(format!("{base}/admin/account-authorization/{session}"))
        .header("X-Config-Version", VERSION)
        .send()
        .await?;
    assert_eq!(denied.status(), reqwest::StatusCode::NOT_FOUND);
    let current:(i64,i64)=db.query_row("SELECT count(*),revision FROM upstream_credentials WHERE config_version_id=?1 AND id='seo-device-account'",[VERSION],|row|Ok((row.get(0)?,row.get(1)?)))?;
    assert_eq!(
        current, saved,
        "receipt/replay cannot create or replace an account"
    );
    eprintln!(
        "E-02 {kind}: real HTTP port={address}, original_session={session}, committed={committed_revision}, account=seo-device-account, rows=1, credential_revision=0, client_EOF, receipt_reads=2, token_exchanges=1, replay=409, unauthenticated=404"
    );
    handle.stop(true).await;
    running.await??;
    Ok(())
}
