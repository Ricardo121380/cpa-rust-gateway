# O-01 initial phase trial

Synthetic local run; not B-01 RED. Raw log SHA256: `cfb2ad185d960c6ad528522ce3d335617de2ed9e49fc7366620ff53ca5e54e3b`.

```text
   Compiling gateway v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/apps/gateway)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.96s
     Running unittests src/main.rs (target/debug/deps/gateway-eafc5f5031908db2)

running 1 test

thread 'runtime::batch_c::http::responses_history::responses_json_multipart_output_replays_exactly_through_public_http' (41817575) panicked at apps/gateway/src/runtime/batch_c/http/responses_history.rs:196:5:
assertion `left == right` failed: {"error":{"code":"StreamTruncated","message":"the stream ended before completion","param":null,"type":"server_error"}}; events [GatewayEvent::Request(RequestEvent { request_id: RequestId("p1-request-9b609a9f33829b18a14d527f12483b52-0"), client_key_id: ClientKeyId("p12-runtime-client-key"), access_group_id: Some(AccessGroupId("p12-runtime-group")), protocol: OpenAiResponses, requested_model: "<redacted>", public_model: "<redacted>", route_alias_present: false, streaming: false, started_at_ms: Some(1791379067057) }), GatewayEvent::Attempt(AttemptEvent { request_id: RequestId("p1-request-9b609a9f33829b18a14d527f12483b52-0"), attempt_id: AttemptId("attempt:45:p1-request-9b609a9f33829b18a14d527f12483b52-0:1"), attempt_number: 1, route_id: RouteId("p12-runtime-route"), route_candidate_id: RouteCandidateId("p12-runtime-candidate"), credential_id: CredentialId("p12-runtime-credential"), endpoint_id: EndpointId("p12-krill-endpoint"), upstream_id: UpstreamId("p12-runtime-upstream"), upstream_model: "<redacted>", started_at_ms: 1791379067058, ended_at_ms: 1791379067171, outcome: Failed(GatewayError { code: StreamTruncated, scope: Stream }), retry_decision: RetryEligible, execution_identity: Some(AttemptExecutionIdentity(<protected identifiers>)) }), GatewayEvent::RequestFinished(RequestFinishedEvent { request_id: RequestId("p1-request-9b609a9f33829b18a14d527f12483b52-0"), started_at_ms: 1791379067057, finished_at_ms: 1791379067171, duration_ms: 114, first_content_ms: None, outcome: Failed, error_code: Some(StreamTruncated) })]; upstream stream Some(Bool(false))
  left: 502
 right: 200
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test runtime::batch_c::http::responses_history::responses_json_multipart_output_replays_exactly_through_public_http ... FAILED

failures:

failures:
    runtime::batch_c::http::responses_history::responses_json_multipart_output_replays_exactly_through_public_http

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 175 filtered out; finished in 0.18s

error: test failed, to rerun pass `-p gateway --bin gateway`
```
