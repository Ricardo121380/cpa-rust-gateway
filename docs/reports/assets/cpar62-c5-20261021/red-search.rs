//! C5 hosted search through the public listener and an independent owned receiver.
use super::*;

fn declarations() -> Value {
    json!([{"type":"web_search","external_web_access":false}])
}

fn output() -> Value {
    json!([
        {"id":"ws-1","type":"web_search_call","status":"completed","action":{
            "type":"search","queries":["seed successor"],"sources":[{"type":"url","url":"https://example.invalid/seed"}]}},
        {"id":"answer-1","type":"message","status":"completed","role":"assistant","content":[{
            "type":"output_text","text":"The seed is 41.","annotations":[{"type":"url_citation","start_index":0,"end_index":14,
                "url":"https://example.invalid/seed","title":"Seed reference"}]}]}
    ])
}

fn wire() -> String {
    let mut reply = response();
    reply["output"] = output();
    reply.to_string()
}

#[actix_web::test]
async fn public_build_search_json_and_fragmented_sse_keep_explicit_history() -> TestResult {
    let peer = ModelPeer::start_sequence(false, vec![wire()])?;
    let fixture = lifecycle_fixture_qualified(namespaces::transport(&peer, 31)?, true)?;
    let recording = Recording::start(&fixture, false)?;
    let (url, server) = namespaces::listener(&fixture)?;
    let body = json!({"model":"p12-test-model","input":"Find the seed and cite it.",
        "tools":declarations(),"tool_choice":"required","parallel_tool_calls":false});
    let (status, bytes) = namespaces::request(&fixture, &url, &body).await?;
    println!("CPAR62_SEARCH_BASELINE {}", json!({"status":status,"receiver_count":peer.accepted.load(Ordering::SeqCst),
        "attempts":fixture.events.attempts()?,"lease_after":active_leases(&fixture),"body":String::from_utf8_lossy(&bytes)}));
    server.stop(false).await;
    let _database = recording.finish(&fixture, 0).await?;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
    let reply: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(reply["output"], output());
    Ok(())
}
