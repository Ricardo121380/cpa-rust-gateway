use super::*;
use gateway_http_actix::management_resources::{
    CodexOAuthManagementWorkflow, ManagementCodexOAuthExchange,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Exchange {
    channel: &'static str,
    condition: &'static str,
    calls: Arc<AtomicUsize>,
}
impl ManagementCodexOAuthExchange for Exchange {
    fn exchange(
        &mut self,
        _: &CredentialId,
        _: zeroize::Zeroizing<String>,
        _: zeroize::Zeroizing<Vec<u8>>,
    ) -> Option<zeroize::Zeroizing<Vec<u8>>> {
        let ordinal = self.calls.fetch_add(1, Ordering::SeqCst);
        let mut material = serde_json::json!({"kind":format!("{}_oauth",self.channel),"access_token":format!("synthetic-access-{ordinal}"),"refresh_token":"synthetic-refresh","expires_at_ms":4_102_444_800_000_i64,"account_id":"observed-owner","email":"same-display@example.test"});
        if ordinal > 0 {
            match self.condition {
                "different" => material["account_id"] = serde_json::json!("foreign-owner"),
                "unknown" => {
                    material.as_object_mut()?.remove("account_id");
                }
                _ => {}
            }
        }
        Some(zeroize::Zeroizing::new(serde_json::to_vec(&material).ok()?))
    }
}

#[actix_web::test]
async fn codex_claude_public_identity_matrix_preserves_disabled_intent_and_relationships()
-> TestResult {
    for channel in ["codex", "claude"] {
        for condition in ["same", "different", "unknown"] {
            for disabled in [false, true] {
                let calls = Arc::new(AtomicUsize::new(0));
                let workflow = Box::new(CodexOAuthManagementWorkflow::with_exchange(Box::new(
                    Exchange {
                        channel,
                        condition,
                        calls: calls.clone(),
                    },
                )));
                let (file, state) = if channel == "codex" {
                    fixture_with_workflow(false, workflow)?
                } else {
                    let (file, state) = fixture(false)?;
                    (file, state.with_claude_workflow(workflow))
                };
                let db = gateway_store::open(&file.0)?;
                db.execute(
                    "UPDATE upstreams SET kind=?1 WHERE config_version_id=?2 AND id='owner-a'",
                    [channel, VERSION],
                )?;
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(security()?))
                        .app_data(web::Data::new(state))
                        .configure(configure_management_resources),
                )
                .await;
                let path = format!("/admin/upstreams/owner-a/{channel}-authorization");
                let mut before = None;
                for round in 0..2 {
                    let revision = if round == 0 { "rev-0" } else { "rev-1" };
                    let response = test::call_service(&app,authorized(test::TestRequest::post().uri(&format!("{path}/start"))).insert_header(("If-Match",revision)).set_json(serde_json::json!({"id":"matrix-account","replace_existing":round>0})).to_request()).await;
                    assert_eq!(response.status(), StatusCode::ACCEPTED);
                    let started: Value = test::read_body_json(response).await;
                    let url = url::Url::parse(
                        started["authorization_url"]
                            .as_str()
                            .ok_or("authorization URL")?,
                    )?;
                    let state = url
                        .query_pairs()
                        .find(|(key, _)| key == "state")
                        .ok_or("state")?
                        .1
                        .into_owned();
                    let response=test::call_service(&app,authorized(test::TestRequest::post().uri(&format!("{path}/callback"))).insert_header(("If-Match",revision)).set_json(serde_json::json!({"id":"matrix-account","replace_existing":round>0,"callback":{"state":state,"code":"synthetic-local-code"}})).to_request()).await;
                    let accepted = round == 0 || condition == "same";
                    assert_eq!(
                        response.status(),
                        if round == 0 {
                            StatusCode::CREATED
                        } else if accepted {
                            StatusCode::OK
                        } else {
                            StatusCode::CONFLICT
                        },
                        "{channel}/{condition}/disabled={disabled}"
                    );
                    let body: Value = test::read_body_json(response).await;
                    if accepted {
                        assert_eq!(body["id"], "matrix-account");
                        assert_eq!(body["revision"], round);
                    } else {
                        assert_eq!(
                            body["error"]["code"],
                            "management_credential_identity_unverified"
                        );
                    }
                    assert!(!body.to_string().contains("synthetic-refresh"));
                    if round == 0 {
                        if disabled {
                            db.execute("UPDATE upstream_credentials SET status='disabled' WHERE config_version_id=?1 AND id='matrix-account'",[VERSION])?;
                        }
                        before = SqliteControlPlaneRepository::open(&file.0)?
                            .load_configuration(&ConfigVersionId::try_new(VERSION)?)?;
                    }
                }
                let before = before.ok_or("before configuration")?;
                let after = SqliteControlPlaneRepository::open(&file.0)?
                    .load_configuration(&ConfigVersionId::try_new(VERSION)?)?
                    .ok_or("after configuration")?;
                assert_reauthorization_relationships_preserved(&before, &after);
                assert_eq!(before.credentials.len(), after.credentials.len());
                let old = before
                    .credentials
                    .iter()
                    .find(|row| row.id.as_str() == "matrix-account")
                    .ok_or("old account")?;
                let new = after
                    .credentials
                    .iter()
                    .find(|row| row.id.as_str() == "matrix-account")
                    .ok_or("new account")?;
                assert_eq!(
                    new.status,
                    if disabled {
                        CredentialStatus::Disabled
                    } else {
                        CredentialStatus::Active
                    }
                );
                assert_eq!(new.revision, i64::from(condition == "same"));
                assert_eq!(
                    old.encrypted_secret.ciphertext() == new.encrypted_secret.ciphertext(),
                    condition != "same",
                    "sealed credential preservation"
                );
                assert_eq!(
                    calls.load(Ordering::SeqCst),
                    2,
                    "exactly one exchange per submitted callback"
                );
                println!(
                    "CPAR-05 HTTP {channel}/{condition}/disabled={disabled}: first=1 maintenance=1 retained_accounts={} revision={}",
                    after.credentials.len(),
                    new.revision
                );
            }
        }
    }
    Ok(())
}
