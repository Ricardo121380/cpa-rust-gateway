# B-01 red-json

Baseline product: `ee521c82dad311f8e82e15f0f63f1e4f5189b35f` plus the new public regression. Exit: **101**. First response and its exact output passed; the actual second public request failed with 400 and the handoff count remained 1.

```text
   Compiling gateway v0.1.0 (/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/apps/gateway)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.27s
     Running unittests src/main.rs (target/debug/deps/gateway-eafc5f5031908db2)

running 1 test

thread 'runtime::batch_c::http::responses_history::responses_json_multipart_output_replays_exactly_through_public_http' (41821913) panicked at apps/gateway/src/runtime/batch_c/http/responses_history.rs:227:5:
assertion `left == right` failed: replay; upstream calls 1: {"error":{"code":"ClientRequestError","message":"the client request is invalid","param":null,"type":"invalid_request_error"}}
  left: 400
 right: 200
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test runtime::batch_c::http::responses_history::responses_json_multipart_output_replays_exactly_through_public_http ... FAILED

failures:

failures:
    runtime::batch_c::http::responses_history::responses_json_multipart_output_replays_exactly_through_public_http

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 175 filtered out; finished in 0.19s

error: test failed, to rerun pass `-p gateway --bin gateway`
```
