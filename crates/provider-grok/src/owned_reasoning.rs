//! Stateless, authenticated Build reasoning continuation. No history is written for store:false.
use std::fmt;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_core::{
    CanonicalRequest, ClientKeyId, CredentialId, EndpointId, ErrorScope, GatewayError,
    GatewayErrorCode, MessageContent, ProviderId, RouteCandidateId, RouteId, UpstreamId,
};
use gateway_router::{ResponsesExecutionLineage, SnapshotVersion};
use gateway_store::secret_store::{EncryptedSecret, KeyVersion, SecretStore};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::{Zeroize, Zeroizing};

const PREFIX: &str = "cpar_reasoning_v1.";
const MAX_TOKEN: usize = 128 * 1024;
const MAX_CIPHER: usize = 64 * 1024;
const TTL_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// Deployment-key-backed codec; returned tokens survive restart with the same master key.
#[derive(Clone)]
pub struct GrokBuildReasoningCodec(SecretStore);

/// One already-selected Build attempt. Debug never reveals tenant, lineage or ciphertext.
#[derive(Clone)]
pub struct GrokBuildReasoningOwner {
    codec: GrokBuildReasoningCodec,
    client: ClientKeyId,
    public_model: String,
    upstream_model: String,
    lineage: ResponsesExecutionLineage,
    now_ms: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    issued_ms: u64,
    expires_ms: u64,
    upstream_model: String,
    binding: [String; 7],
    revision: u64,
    cipher: String,
}
impl Drop for Payload {
    fn drop(&mut self) {
        self.cipher.zeroize();
    }
}

impl fmt::Debug for GrokBuildReasoningCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GrokBuildReasoningCodec(<redacted>)")
    }
}
impl fmt::Debug for GrokBuildReasoningOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GrokBuildReasoningOwner(<redacted>)")
    }
}

impl GrokBuildReasoningCodec {
    /// Reuses the deployment AEAD key without introducing another file or persistence path.
    #[must_use]
    pub const fn new(secret_store: SecretStore) -> Self {
        Self(secret_store)
    }

    /// Authenticates all opaque history before scheduling; mixed owners fail closed.
    /// # Errors
    /// Rejects forged, expired, foreign, mixed-binding or otherwise invalid history.
    pub fn continuation(
        &self,
        request: &CanonicalRequest,
        client: &ClientKeyId,
        now_ms: u64,
    ) -> Result<Option<ResponsesExecutionLineage>, GatewayError> {
        let mut retained: Option<Payload> = None;
        for message in &request.messages {
            for part in &message.content {
                let MessageContent::Reasoning(history) = part else {
                    continue;
                };
                let item: Value =
                    serde_json::from_str(history.raw().get()).map_err(|_| invalid())?;
                let Some(token) = item.get("encrypted_content").and_then(Value::as_str) else {
                    continue;
                };
                let payload = self.open(
                    token,
                    client,
                    &request.requested_model,
                    item["id"].as_str().ok_or_else(invalid)?,
                    now_ms,
                )?;
                if let Some(previous) = &retained {
                    if previous.binding != payload.binding
                        || previous.upstream_model != payload.upstream_model
                    {
                        return Err(invalid());
                    }
                    // The scheduler must prove rotation continuity from the oldest supplied revision.
                    if previous.revision <= payload.revision {
                        continue;
                    }
                }
                retained = Some(payload);
            }
        }
        retained.as_ref().map(lineage).transpose()
    }

    /// Authenticates history and preserves any additional stored/compaction/WebSocket requirement.
    /// # Errors
    /// Rejects opaque history that disagrees with an existing exact continuation.
    pub fn continuation_pin(
        &self,
        request: &CanonicalRequest,
        client: &ClientKeyId,
        now_ms: u64,
        prior: Option<gateway_router::ResponsesContinuationPin>,
    ) -> Result<Option<gateway_router::ResponsesContinuationPin>, GatewayError> {
        let Some(lineage) = self.continuation(request, client, now_ms)? else {
            return Ok(prior);
        };
        if let Some(prior) = prior {
            if prior.lineage() != &lineage {
                return Err(invalid());
            }
            return Ok(Some(prior));
        }
        Ok(Some(gateway_router::ResponsesContinuationPin::new(
            lineage,
            gateway_router::ResponsesContinuationKind::OwnedReasoning,
        )))
    }

    fn open(
        &self,
        token: &str,
        client: &ClientKeyId,
        model: &str,
        item: &str,
        now_ms: u64,
    ) -> Result<Payload, GatewayError> {
        if token.len() > MAX_TOKEN {
            return Err(invalid());
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(token.strip_prefix(PREFIX).ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        let version: [u8; 4] = bytes
            .get(..4)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?;
        let encrypted = EncryptedSecret::try_from_persisted(
            KeyVersion::try_new(u32::from_be_bytes(version)).map_err(|_| invalid())?,
            bytes.get(4..).ok_or_else(invalid)?.to_vec(),
        )
        .map_err(|_| invalid())?;
        let plaintext = self
            .0
            .open(&encrypted, &aad(client, model, item)?)
            .map_err(|_| invalid())?;
        if plaintext.as_bytes().len() > MAX_CIPHER + 8192 {
            return Err(invalid());
        }
        let payload: Payload =
            serde_json::from_slice(plaintext.as_bytes()).map_err(|_| invalid())?;
        if payload.issued_ms > now_ms
            || payload.expires_ms <= now_ms
            || payload.issued_ms.checked_add(TTL_MS) != Some(payload.expires_ms)
            || payload.cipher.is_empty()
            || payload.cipher.len() > MAX_CIPHER
            || payload
                .binding
                .iter()
                .any(|s| s.is_empty() || s.len() > 512)
            || payload.upstream_model.is_empty()
            || payload.upstream_model.len() > 512
        {
            return Err(invalid());
        }
        Ok(payload)
    }
}

impl GrokBuildReasoningOwner {
    /// Binds the codec to an authenticated client and an already-leased exact attempt.
    #[must_use]
    pub fn new(
        codec: GrokBuildReasoningCodec,
        client: ClientKeyId,
        public_model: String,
        upstream_model: String,
        lineage: ResponsesExecutionLineage,
        now_ms: u64,
    ) -> Self {
        Self {
            codec,
            client,
            public_model,
            upstream_model,
            lineage,
            now_ms,
        }
    }

    /// Wraps provider ciphertext; absence/null is preserved. Raw ciphertext never reaches the client.
    /// # Errors
    /// Rejects non-reasoning ciphertext, malformed or oversized content and failed encryption.
    pub fn seal_item(&self, item: &mut Value) -> Result<(), GatewayError> {
        let Some(cipher) = item.get("encrypted_content").filter(|v| !v.is_null()) else {
            return Ok(());
        };
        if item["type"] != "reasoning" {
            return Err(invalid());
        }
        let cipher = cipher.as_str().ok_or_else(invalid)?;
        if cipher.is_empty() || cipher.len() > MAX_CIPHER {
            return Err(invalid());
        }
        let payload = Payload {
            issued_ms: self.now_ms,
            expires_ms: self.now_ms.checked_add(TTL_MS).ok_or_else(invalid)?,
            upstream_model: self.upstream_model.clone(),
            binding: binding(&self.lineage),
            revision: self.lineage.credential_revision(),
            cipher: cipher.to_owned(),
        };
        let plaintext = Zeroizing::new(serde_json::to_vec(&payload).map_err(|_| invalid())?);
        let encrypted = self
            .codec
            .0
            .seal(
                &plaintext,
                &aad(
                    &self.client,
                    &self.public_model,
                    item["id"].as_str().ok_or_else(invalid)?,
                )?,
            )
            .map_err(|_| invalid())?;
        let mut bytes = encrypted.key_version().get().to_be_bytes().to_vec();
        bytes.extend_from_slice(encrypted.ciphertext());
        let token = format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes));
        if token.len() > MAX_TOKEN {
            return Err(invalid());
        }
        item["encrypted_content"] = Value::String(token);
        Ok(())
    }

    /// Unwraps only for the same selected attempt binding. Rotation is admitted by the scheduler.
    /// # Errors
    /// Rejects foreign/malformed history, changed model/binding or revision rollback.
    pub fn open_item(&self, item: &mut Value) -> Result<(), GatewayError> {
        let Some(token) = item.get("encrypted_content").filter(|v| !v.is_null()) else {
            return Ok(());
        };
        if item["type"] != "reasoning" {
            return Err(invalid());
        }
        let mut payload = self.codec.open(
            token.as_str().ok_or_else(invalid)?,
            &self.client,
            &self.public_model,
            item["id"].as_str().ok_or_else(invalid)?,
            self.now_ms,
        )?;
        if payload.binding != binding(&self.lineage)
            || payload.upstream_model != self.upstream_model
            || payload.revision > self.lineage.credential_revision()
        {
            return Err(invalid());
        }
        item["encrypted_content"] = Value::String(std::mem::take(&mut payload.cipher));
        Ok(())
    }
}

fn aad(client: &ClientKeyId, model: &str, item: &str) -> Result<Vec<u8>, GatewayError> {
    if [client.as_str(), model, item]
        .iter()
        .any(|v| v.is_empty() || v.len() > 512)
    {
        return Err(invalid());
    }
    serde_json::to_vec(&json!([
        "cpar.grok.build.reasoning.v1",
        client.as_str(),
        model,
        item
    ]))
    .map_err(|_| invalid())
}
fn binding(v: &ResponsesExecutionLineage) -> [String; 7] {
    [
        v.snapshot_version().as_str(),
        v.provider_id().as_str(),
        v.upstream_id().as_str(),
        v.channel_id().as_str(),
        v.route_id().as_str(),
        v.route_candidate_id().as_str(),
        v.credential_id().as_str(),
    ]
    .map(str::to_owned)
}
fn lineage(p: &Payload) -> Result<ResponsesExecutionLineage, GatewayError> {
    Ok(ResponsesExecutionLineage::new(
        SnapshotVersion::try_new(p.binding[0].clone()).map_err(|_| invalid())?,
        ProviderId::try_new(p.binding[1].clone()).map_err(|_| invalid())?,
        UpstreamId::try_new(p.binding[2].clone()).map_err(|_| invalid())?,
        EndpointId::try_new(p.binding[3].clone()).map_err(|_| invalid())?,
        RouteId::try_new(p.binding[4].clone()).map_err(|_| invalid())?,
        RouteCandidateId::try_new(p.binding[5].clone()).map_err(|_| invalid())?,
        CredentialId::try_new(p.binding[6].clone()).map_err(|_| invalid())?,
        p.revision,
    ))
}
const fn invalid() -> GatewayError {
    GatewayError::new(GatewayErrorCode::ClientRequestError, ErrorScope::Request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GrokBuildCredential, GrokBuildResponsesDecoder, GrokBuildResponsesRequestBuilder,
        GrokBuildResponsesStreamDecoder,
    };
    use gateway_store::secret_store::{MasterKey, MasterKeyRing};
    use protocol_openai_responses::{
        OpenAiResponseMetadata, OpenAiResponsesSseEncoder, decode_request, encode_response,
    };
    use std::fmt::Write as _;
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn codec(seed: u8) -> Result<GrokBuildReasoningCodec, Box<dyn std::error::Error>> {
        let version = KeyVersion::try_new(1)?;
        Ok(GrokBuildReasoningCodec::new(SecretStore::new(
            MasterKeyRing::try_new(version, [(version, MasterKey::try_from_bytes([seed; 32])?)])?,
        )))
    }
    fn owner() -> Result<GrokBuildReasoningOwner, Box<dyn std::error::Error>> {
        Ok(GrokBuildReasoningOwner::new(
            codec(23)?,
            ClientKeyId::try_new("owner")?,
            "public-model".into(),
            "upstream-model".into(),
            ResponsesExecutionLineage::new(
                SnapshotVersion::try_new("version")?,
                ProviderId::try_new("provider")?,
                UpstreamId::try_new("upstream")?,
                EndpointId::try_new("endpoint")?,
                RouteId::try_new("route")?,
                RouteCandidateId::try_new("candidate")?,
                CredentialId::try_new("credential")?,
                5,
            ),
            10_000,
        ))
    }
    fn item() -> Value {
        json!({"id":"reason","type":"reasoning","summary":[],"encrypted_content":"synthetic-cipher"})
    }

    #[test]
    fn owned_reasoning_authenticates_identity_scope_expiry_tamper_and_restart() -> TestResult {
        let original = owner()?;
        let mut sealed = item();
        original.seal_item(&mut sealed)?;
        assert_ne!(sealed["encrypted_content"], item()["encrypted_content"]);
        assert!(!format!("{original:?}").contains("credential"));
        let mut restarted = owner()?;
        restarted.now_ms += 1;
        let mut opened = sealed.clone();
        restarted.open_item(&mut opened)?;
        assert_eq!(opened, item());
        for which in 0..7 {
            let mut foreign = owner()?;
            match which {
                0 => foreign.client = ClientKeyId::try_new("other")?,
                1 => foreign.public_model = "other".into(),
                2 => foreign.upstream_model = "other".into(),
                3 => foreign.now_ms += TTL_MS,
                4 => foreign.now_ms -= 1,
                5 => foreign.codec = codec(24)?,
                _ => {
                    foreign.lineage = ResponsesExecutionLineage::new(
                        SnapshotVersion::try_new("version")?,
                        ProviderId::try_new("provider")?,
                        UpstreamId::try_new("upstream")?,
                        EndpointId::try_new("endpoint")?,
                        RouteId::try_new("route")?,
                        RouteCandidateId::try_new("candidate")?,
                        CredentialId::try_new("other-credential")?,
                        5,
                    );
                }
            }
            assert!(
                foreign.open_item(&mut sealed.clone()).is_err(),
                "case {which}"
            );
        }
        let mut wrong_item = sealed.clone();
        wrong_item["id"] = json!("other");
        assert!(original.open_item(&mut wrong_item).is_err());
        let mut tampered = sealed.clone();
        let mut token = tampered["encrypted_content"]
            .as_str()
            .ok_or("token")?
            .as_bytes()
            .to_vec();
        let last = token.len() - 5;
        token[last] = if token[last] == b'A' { b'B' } else { b'A' };
        tampered["encrypted_content"] = json!(String::from_utf8(token)?);
        assert!(original.open_item(&mut tampered).is_err());
        let mut oversized = item();
        oversized["encrypted_content"] = json!("x".repeat(MAX_CIPHER + 1));
        assert!(original.seal_item(&mut oversized).is_err());
        assert!(
            decode_request(&json!({"model":"public-model","input":[item()]}).to_string()).is_err()
        );
        Ok(())
    }

    #[test]
    fn owned_reasoning_mixed_grants_and_rotation_require_exact_lease_proof() -> TestResult {
        use gateway_upstream::{
            CredentialMaterialReplacement, CredentialSecret, EndpointCredentialInput,
            EndpointCredentialPool,
        };
        let original = owner()?;
        let mut first = item();
        original.seal_item(&mut first)?;
        let mut newer = original.clone();
        let mut fields = binding(&original.lineage);
        let make_lineage = |fields: [String; 7], revision| {
            lineage(&Payload {
                issued_ms: 10_000,
                expires_ms: 10_000 + TTL_MS,
                upstream_model: "upstream-model".into(),
                binding: fields,
                revision,
                cipher: "synthetic-cipher".into(),
            })
        };
        newer.lineage = make_lineage(fields.clone(), 6)?;
        let mut second = item();
        second["id"] = json!("second");
        newer.seal_item(&mut second)?;
        let request = decode_request(
            &json!({"model":"public-model","input":[first.clone(),second]}).to_string(),
        )?;
        let retained = original
            .codec
            .continuation(&request.request, &original.client, 10_000)?
            .ok_or("pin")?;
        assert_eq!(retained.credential_revision(), 5);
        for kind in [
            gateway_router::ResponsesContinuationKind::StoredResponse,
            gateway_router::ResponsesContinuationKind::Compaction,
            gateway_router::ResponsesContinuationKind::WebSocketSession,
        ] {
            let prior = gateway_router::ResponsesContinuationPin::new(retained.clone(), kind);
            let resolved = original.codec.continuation_pin(
                &request.request,
                &original.client,
                10_000,
                Some(prior.clone()),
            )?;
            assert_eq!(resolved, Some(prior));
        }

        let pool = EndpointCredentialPool::try_new(
            retained.channel_id().clone(),
            [EndpointCredentialInput {
                credential_id: retained.credential_id().clone(),
                credential_kind: "grok_build_oauth".into(),
                credential_revision: 5,
                priority: 0,
                weight: 1,
                concurrency: 1,
                expires_at_ms: Some(1_000_000),
                secret: CredentialSecret::try_new(b"test-only".to_vec())?,
            }],
        )?;
        pool.replace_credential_if_revision(
            retained.credential_id(),
            5,
            CredentialMaterialReplacement {
                credential_revision: 6,
                expires_at_ms: Some(1_000_000),
                secret: CredentialSecret::try_new(b"test-new".to_vec())?,
            },
        )?;
        assert!(
            pool.try_lease_exact_revision_eligible_at(retained.credential_id(), 5, 10_000, |_| {
                true
            })
            .is_none()
        );
        pool.set_build_continuation_range(retained.credential_id(), 5, 6)?;
        let lease = pool
            .try_lease_exact_revision_eligible_at(retained.credential_id(), 5, 10_000, |_| true)
            .ok_or("proven rotation")?;
        assert_eq!(lease.credential_revision(), 6);
        drop(lease);
        fields[6] = "other-grant".into();
        newer.lineage = make_lineage(fields, 6)?;
        let mut foreign = item();
        newer.seal_item(&mut foreign)?;
        let mixed =
            decode_request(&json!({"model":"public-model","input":[first,foreign]}).to_string())?;
        assert!(
            original
                .codec
                .continuation(&mixed.request, &original.client, 10_000)
                .is_err()
        );
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One round trip spans both upstream and downstream native codecs.
    fn owned_reasoning_json_and_chunked_sse_roundtrip_native_history_and_tools() -> TestResult {
        let owner = owner()?;
        let mut response: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/grok-build/p6-03-non-streaming.json"
        ))?;
        response["output"][0]["encrypted_content"] = json!("synthetic-cipher");
        let bytes = serde_json::to_vec(&response)?;
        assert!(GrokBuildResponsesDecoder::decode_non_streaming(&bytes).is_err());
        let canonical = GrokBuildResponsesDecoder::decode_owned_with_encoding(
            None,
            &bytes,
            Some(owner.clone()),
        )?;
        let downstream = encode_response(
            &canonical,
            OpenAiResponseMetadata::try_new("public-model", 10)?,
        )?;
        let token = downstream["output"][0]["encrypted_content"]
            .as_str()
            .ok_or("missing cipher")?;
        assert!(gateway_core::is_owned_reasoning_token(token));
        assert!(!downstream.to_string().contains("synthetic-cipher"));
        assert_eq!(
            downstream["output"][1]["content"][0]["text"],
            "It is sunny."
        );
        let mut history = downstream["output"].as_array().ok_or("output")?.clone();
        history.push(json!({"type":"function_call_output","call_id":"call-grok-build-01","output":"synthetic tool result"}));
        let request = decode_request(
            &json!({"model":"public-model","store":false,"input":history}).to_string(),
        )?;
        assert!(!request.store);
        assert_eq!(
            owner
                .codec
                .continuation(&request.request, &owner.client, owner.now_ms)?,
            Some(owner.lineage.clone())
        );
        assert!(
            owner
                .codec
                .continuation(
                    &request.request,
                    &ClientKeyId::try_new("other")?,
                    owner.now_ms
                )
                .is_err()
        );
        let credential = GrokBuildCredential::import_json(br#"{"access_token":"synthetic-access","refresh_token":"synthetic-refresh","expires_in":3600,"token_type":"Bearer"}"#, 0)?;
        assert!(
            GrokBuildResponsesRequestBuilder::build(
                &credential,
                "upstream-model",
                &request.request,
                protocol_openai_responses::ResponseMode::NonStreaming
            )
            .is_err()
        );
        let outbound = GrokBuildResponsesRequestBuilder::build_owned(
            &credential,
            "upstream-model",
            &request.request,
            protocol_openai_responses::ResponseMode::NonStreaming,
            None,
            Some(&owner),
        )?;
        let outbound: Value = serde_json::from_slice(outbound.body())?;
        assert_eq!(
            outbound["input"][0]["encrypted_content"],
            "synthetic-cipher"
        );
        assert_eq!(outbound["input"][3]["output"], "synthetic tool result");
        assert!(!outbound.to_string().contains(PREFIX));
        // Same real native parser and downstream encoder, across arbitrary SSE chunk boundaries.
        let fixture = include_str!("../../../tests/fixtures/grok-build/p6-03-stream.sse");
        let mut stream = String::new();
        for record in fixture.split("\n\n").filter(|r| !r.trim().is_empty()) {
            let mut lines = record.lines();
            let event = lines.next().ok_or("event")?;
            let mut data: Value = serde_json::from_str(
                lines
                    .next()
                    .ok_or("data")?
                    .strip_prefix("data: ")
                    .ok_or("prefix")?,
            )?;
            if data["type"] == "response.output_item.done" && data["item"]["type"] == "reasoning" {
                data["item"]["encrypted_content"] = json!("synthetic-cipher");
            }
            if data["type"] == "response.completed" {
                data["response"]["output"][0]["encrypted_content"] = json!("synthetic-cipher");
            }
            write!(stream, "{event}\ndata: {data}\n\n")?;
        }
        for size in [1, 17, 4096] {
            let mut decoder =
                GrokBuildResponsesStreamDecoder::new().with_reasoning_owner(Some(owner.clone()));
            let mut encoder = OpenAiResponsesSseEncoder::new(OpenAiResponseMetadata::try_new(
                "public-model",
                10,
            )?);
            for chunk in stream.as_bytes().chunks(size) {
                for event in decoder.push_bytes(chunk)? {
                    let _ = encoder.encode_event(&event)?;
                }
            }
            decoder.finish()?;
            let completed = encoder.into_completed_response()?;
            let mut reasoning = completed["output"][0].clone();
            owner.open_item(&mut reasoning)?;
            assert_eq!(reasoning["encrypted_content"], "synthetic-cipher");
            assert_eq!(completed["usage"]["output_tokens"], 8);
        }
        let boundary = stream
            .rfind("event: response.completed")
            .ok_or("terminal")?;
        let malformed = format!(
            "{}{}",
            &stream[..boundary],
            stream[boundary..].replace("synthetic-cipher", "different")
        );
        assert!(
            GrokBuildResponsesStreamDecoder::new()
                .with_reasoning_owner(Some(owner))
                .push_bytes(malformed.as_bytes())
                .is_err()
        );
        Ok(())
    }
}
