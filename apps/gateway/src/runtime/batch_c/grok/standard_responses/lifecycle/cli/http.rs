//! Passive test-only request/stream observation and public rejection probes.
use super::*;
use actix_web::{
    HttpMessage,
    body::{BodySize, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
};
use futures_util::StreamExt;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
pub(in super::super) struct CaptureBody<B> {
    inner: Pin<Box<B>>,
    records: web::Data<Ingress>,
    index: usize,
}
impl<B: MessageBody> MessageBody for CaptureBody<B> {
    type Error = B::Error;
    fn size(&self) -> BodySize {
        self.inner.size()
    }
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<web::Bytes, Self::Error>>> {
        let this = self.get_mut();
        let result = this.inner.as_mut().poll_next(cx);
        if let Poll::Ready(Some(Ok(bytes))) = &result
            && let Ok(mut rows) = this.records.lock()
        {
            let row = &mut rows[this.index];
            row.chunks += 1;
            if let Some(started) = row.started {
                row.chunk_elapsed_ms.push(started.elapsed().as_millis());
            }
            if row.wire.len() + bytes.len() <= 8 * 1024 * 1024 {
                row.wire.extend_from_slice(bytes);
            } else {
                row.wire_truncated = true;
            }
        }
        if matches!(result, Poll::Ready(None))
            && let Ok(mut rows) = this.records.lock()
        {
            rows[this.index].body_complete = true;
        }
        result
    }
}

pub(in super::super) async fn capture<B: MessageBody>(
    mut request: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<CaptureBody<B>>, actix_web::Error> {
    let mut payload = request.take_payload();
    let mut bytes = Vec::new();
    while let Some(chunk) = payload.next().await {
        bytes.extend_from_slice(&chunk?);
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(actix_web::error::ErrorPayloadTooLarge(
                "controlled capture bound",
            ));
        }
    }
    let body = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
    let path = request.path().to_owned();
    let probe = request.headers().contains_key("x-cpar62-probe")
        || request.headers().contains_key("x-cpar64-probe");
    let mut headers = serde_json::Map::new();
    for name in [
        "user-agent",
        "accept",
        "content-type",
        "content-encoding",
        "openai-beta",
        "anthropic-version",
        "anthropic-beta",
        "anthropic-dangerous-direct-browser-access",
        "x-app",
        "x-stainless-lang",
        "x-stainless-package-version",
        "x-stainless-retry-count",
        "x-stainless-runtime",
        "x-stainless-runtime-version",
    ] {
        let values: Vec<_> = request
            .headers()
            .get_all(name)
            .map(|v| v.as_bytes().to_vec())
            .collect();
        if !values.is_empty() {
            headers.insert(name.into(), json!(values));
        }
    }
    // Authentication values never enter even synthetic receipts.
    for name in ["authorization", "x-api-key"] {
        headers.insert(
            format!("{name}_count"),
            json!(request.headers().get_all(name).count()),
        );
    }
    let method = request.method().to_string();
    let uri = request.uri().to_string();
    let request_bytes = bytes.clone();
    // A passive, test-only observer restores the exact bytes, including field presence/order.
    request.set_payload(web::Bytes::from(bytes).into());
    let records = request
        .app_data::<web::Data<Ingress>>()
        .cloned()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("capture"))?;
    let index = {
        let mut rows = records
            .lock()
            .map_err(|_| actix_web::error::ErrorInternalServerError("capture"))?;
        let index = rows.len();
        rows.push(Captured {
            started: Some(std::time::Instant::now()),
            path,
            method,
            uri,
            request_bytes,
            probe,
            request: body,
            headers: Value::Object(headers),
            ..Captured::default()
        });
        index
    };
    let reply = next.call(request).await?;
    {
        let mut rows = records
            .lock()
            .map_err(|_| actix_web::error::ErrorInternalServerError("capture"))?;
        rows[index].status = reply.status().as_u16();
        rows[index].response_headers = json!(
            reply
                .headers()
                .iter()
                .filter(|(name, _)| matches!(
                    name.as_str(),
                    "content-type" | "x-request-id" | "request-id"
                ))
                .map(|(name, value)| (name.as_str(), value.as_bytes().to_vec()))
                .collect::<Vec<_>>()
        );
    }
    Ok(reply.map_body(move |_, body| CaptureBody {
        inner: Box::pin(body),
        records,
        index,
    }))
}

pub(super) async fn assert_presend_rejections(
    fixture: &Fixture,
    address: std::net::SocketAddr,
    ingress: &Ingress,
    peer: &SocketPeer,
    client: &str,
) -> TestResult {
    let http = reqwest::Client::builder().no_proxy().build()?;
    let count = peer.received.lock().map_err(|_| "receiver")?.len();
    let attempts = fixture.events.attempts()?;
    let reply = http
        .post(format!("http://{address}/v1/responses"))
        .header("x-cpar62-probe", "auth")
        .body("invalid JSON must still be rejected at authentication first")
        .send()
        .await?;
    assert_eq!(reply.status().as_u16(), 401);
    if client == "pi" {
        let mut request = ingress
            .lock()
            .map_err(|_| "ingress")?
            .first()
            .ok_or("actual Pi request")?
            .request
            .clone();
        request["background"] = json!(true);
        let reply = http
            .post(format!("http://{address}/v1/responses"))
            .header("x-cpar62-probe", "background")
            .bearer_auth(&fixture.key)
            .header("content-type", "application/json")
            .body(request.to_string())
            .send()
            .await?;
        assert_eq!(reply.status().as_u16(), 400);
    }
    assert_eq!(
        peer.received.lock().map_err(|_| "receiver")?.len(),
        count,
        "independent receive delta is zero"
    );
    assert_eq!(fixture.events.attempts()?, attempts);
    Ok(())
}
