//! Real `SQLite` request/terminal aggregation and history-compatible migration coverage.
use gateway_core::{
    ClientKeyId, GatewayEvent, GatewayProtocol, RequestEvent, RequestFinishedEvent, RequestId,
    RequestOutcome,
};
use gateway_store::{
    control_plane::{RequestHistoryQuery, SqliteControlPlaneRepository},
    event_store::SqliteEventStore,
};
use std::{
    error::Error,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
struct File(PathBuf);
impl Drop for File {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
#[test]
#[allow(clippy::too_many_lines)] // One persisted scenario brackets indexed activity, snapshots and unknown history.
fn request_counts_snapshot_percentiles_and_unknown_history_are_distinct()
-> Result<(), Box<dyn Error>> {
    let file = File(std::env::temp_dir().join(format!(
        "prism-request-history-{}-{}.sqlite",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    )));
    let mut store = SqliteEventStore::open(&file.0)?;
    let reader = SqliteControlPlaneRepository::open(&file.0)?
        .resource_inventory_reader()
        .ok_or("reader")?;
    let request = |id: &str| -> Result<GatewayEvent, Box<dyn Error>> {
        Ok(GatewayEvent::Request(RequestEvent::new(
            RequestId::try_new(id)?,
            ClientKeyId::try_new("key")?,
            None,
            GatewayProtocol::OpenAiResponses,
            "exact-model".into(),
            "exact-model".into(),
            None,
            true,
        )))
    };
    let terminal = |id: &str, duration: u64, outcome| -> Result<GatewayEvent, Box<dyn Error>> {
        Ok(GatewayEvent::RequestFinished(RequestFinishedEvent {
            request_id: RequestId::try_new(id)?,
            started_at_ms: 120_000,
            finished_at_ms: 121_000,
            duration_ms: duration,
            first_content_ms: Some(duration / 2),
            outcome,
            error_code: None,
        }))
    };
    store.append_batch(&[
        request("history")?,
        request("first")?,
        terminal("first", 100, RequestOutcome::Succeeded)?,
        request("second")?,
        terminal("second", 300, RequestOutcome::Failed)?,
    ])?;
    let activity = reader.client_key_activity(&["key".into(), "unused".into()])?;
    assert_eq!(activity.get("key"), Some(&120_000));
    assert!(!activity.contains_key("unused"));
    let query = RequestHistoryQuery {
        from_ms: Some(120_000),
        to_ms: Some(122_000),
        limit: 1,
        summary: true,
        bucket_ms: 60_000,
        ..Default::default()
    };
    let page = reader.requests(&query)?;
    let stats = page.summary.as_ref().ok_or("summary")?;
    assert_eq!(stats["requests"], 2);
    assert_eq!(stats["succeeded"], 1);
    assert_eq!(stats["failed"], 1);
    assert_eq!(stats["success_rate"], 0.5);
    assert_eq!(stats["p95_duration_ms"], 300);
    assert_eq!(page.series.len(), 1);
    store.append_batch(&[
        request("later")?,
        terminal("later", 500, RequestOutcome::Succeeded)?,
    ])?;
    let next = reader.requests(&RequestHistoryQuery {
        snapshot: Some(page.snapshot),
        after: page.next_after,
        ..query.clone()
    })?;
    assert_eq!(next.items.len(), 1);
    assert_eq!(next.items[0]["request_id"], "first");
    assert_eq!(next.summary.ok_or("summary")?["requests"], 2);
    let history = reader.requests(&RequestHistoryQuery {
        include_unknown: true,
        request_id: Some("history".into()),
        limit: 100,
        summary: true,
        bucket_ms: 60_000,
        ..Default::default()
    })?;
    assert_eq!(history.items[0]["outcome"], "unknown");
    assert!(history.items[0]["duration_ms"].is_null());
    assert!(history.summary.ok_or("summary")?["success_rate"].is_null());
    // Refuse data-destructive rollback while terminal records exist; all originals survive.
    let mut db = gateway_store::open(&file.0)?;
    assert!(gateway_store::rollback_to_version(&mut db, 26).is_err());
    assert_eq!(gateway_store::schema_version(&db)?, Some(27));
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM gateway_event_log", [], |r| r
            .get::<_, i64>(0))?,
        7
    );
    // A narrow window remains complete above the old 100,000-row global ceiling.
    db.execute_batch("BEGIN;
      WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<100001)
      INSERT INTO gateway_event_log(event_type,event_id,request_id,occurred_at_ms,payload_json)
      SELECT 'request','bulk-'||x,'bulk-'||x,NULL,json_object('request',json_object('request_id','bulk-'||x,'client_key_id','key','access_group_id',NULL,'protocol','openai_responses','requested_model','exact-model','public_model','exact-model','route_alias',NULL,'streaming',json('false'))) FROM n;
      WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<100001)
      INSERT INTO gateway_event_log(event_type,event_id,request_id,occurred_at_ms,payload_json)
      SELECT 'request_finished','bulk-'||x,'bulk-'||x,1,json_object('request_finished',json_object('request_id','bulk-'||x,'started_at_ms',0,'finished_at_ms',1,'duration_ms',1,'first_content_ms',NULL,'outcome','succeeded','error_code',NULL)) FROM n;
      COMMIT;")?;
    let narrow = reader.requests(&query)?;
    assert_eq!(narrow.summary.ok_or("summary")?["requests"], 3);
    assert_eq!(narrow.items.len(), 1);
    Ok(())
}
