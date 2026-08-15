//! Gateway-side projections computed by driving this repository's real code.
//!
//! Every function here executes production types. None of them opens a socket, reads a file,
//! reads an environment variable, or touches a reference implementation. A probe that cannot
//! complete returns [`ProbeError`], which the fixture gate turns into a hard failure instead of a
//! silent pass.

use std::{collections::BTreeSet, error::Error, fmt};

use gateway_core::{
    CanonicalEvent, CanonicalEventState, CanonicalResponse, MessageEnd, MessageRole, MessageStart,
    RawExtensions, ResponseEnd, ResponseId, ResponseStart, TextDelta,
};
use gateway_store::{
    control_plane::{
        ConfigVersion, ConfigVersionId, ConfigVersionStatus, ControlPlaneConfiguration,
        SqliteControlPlaneRepository,
    },
    secret_store::{KeyVersion, MasterKey, MasterKeyRing, SecretStore},
};
use gateway_upstream::UpstreamProxy;
use provider_grok::{
    GrokBuildCredentialKey, GrokBuildCredentialPersistence, GrokBuildCredentialSqliteStore,
    GrokWebBrowserEgressSession, GrokWebBrowserUserAgent, GrokWebConversationAvailability,
    GrokWebConversationId, GrokWebConversationState, GrokWebCredential, GrokWebCredentialSlot,
    GrokWebEgressSessionId, GrokWebTlsProfile, GrokWebToolCapability, GrokWebToolEmulation,
};
use provider_kiro::{
    credential::KiroCredentialKind,
    endpoint_policy::{KiroApiRegion, KiroEndpointKind, KiroEndpointPolicy, KiroRequestOrigin},
    event_semantics::KiroEventSemanticMapper,
    event_stream::{KiroEventStreamDecoder, KiroEventStreamError},
};

use crate::vocabulary::{ProjectionMarker, Subject};

const PROBE_OBSERVED_AT_MS: i64 = 1_000_000;
const PROBE_EXPIRES_AT_MS: i64 = 2_000_000;

/// A value-free reason that a gateway projection could not be computed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeError {
    /// Driving `CanonicalEventState` did not reproduce the declared lifecycle.
    CanonicalLifecycle,
    /// The versioned control-plane repository did not behave as an activation authority.
    ConfigurationAuthority,
    /// The Build and Web credential pools could not be constructed or stayed coupled.
    ProviderPoolIsolation,
    /// The Grok Web Tool-Emulation default could not be read consistently.
    WebToolDefault,
    /// The Kiro CLI/IDE endpoint policy could not be derived.
    EndpointPolicy,
    /// The Kiro `EventStream` decoder did not validate CRCs or stayed chunk-dependent.
    EventStreamIntegrity,
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CanonicalLifecycle => "canonical_lifecycle_probe_failed",
            Self::ConfigurationAuthority => "configuration_authority_probe_failed",
            Self::ProviderPoolIsolation => "provider_pool_isolation_probe_failed",
            Self::WebToolDefault => "web_tool_default_probe_failed",
            Self::EndpointPolicy => "endpoint_policy_probe_failed",
            Self::EventStreamIntegrity => "event_stream_integrity_probe_failed",
        })
    }
}

impl Error for ProbeError {}

/// Computes the gateway projection for one subject by executing gateway code.
///
/// # Errors
///
/// Returns [`ProbeError`] when the observed behavior does not satisfy the invariant the marker
/// stands for. A subject can never fall through to an empty projection.
pub fn observe(subject: Subject) -> Result<Vec<ProjectionMarker>, ProbeError> {
    let markers = match subject {
        Subject::CanonicalLifecycle => canonical_lifecycle()?,
        Subject::ConfigurationAuthority => configuration_authority()?,
        Subject::ProviderPoolIsolation => provider_pool_isolation()?,
        Subject::WebToolDefault => web_tool_default()?,
        Subject::EndpointPolicy => endpoint_policy()?,
        Subject::EventStreamIntegrity => event_stream_integrity()?,
    };
    Ok(markers.into_iter().collect())
}

fn canonical_lifecycle() -> Result<BTreeSet<ProjectionMarker>, ProbeError> {
    let response_id = ResponseId::try_new("differential-canonical-lifecycle")
        .map_err(|_| ProbeError::CanonicalLifecycle)?;
    let mut markers = BTreeSet::new();
    let mut state = CanonicalEventState::default();

    if state.apply(&text_delta()).is_ok() {
        return Err(ProbeError::CanonicalLifecycle);
    }

    state
        .apply(&response_start(&response_id))
        .map_err(|_| ProbeError::CanonicalLifecycle)?;
    markers.insert(ProjectionMarker::ResponseStart);

    state
        .apply(&message_start())
        .map_err(|_| ProbeError::CanonicalLifecycle)?;
    state
        .apply(&text_delta())
        .map_err(|_| ProbeError::CanonicalLifecycle)?;
    markers.insert(ProjectionMarker::TextDelta);

    if state.clone().apply(&response_end()).is_ok() || state.finish().is_ok() {
        return Err(ProbeError::CanonicalLifecycle);
    }

    state
        .apply(&message_end())
        .map_err(|_| ProbeError::CanonicalLifecycle)?;
    state
        .apply(&response_end())
        .map_err(|_| ProbeError::CanonicalLifecycle)?;
    state.finish().map_err(|_| ProbeError::CanonicalLifecycle)?;
    if !state.is_success() {
        return Err(ProbeError::CanonicalLifecycle);
    }

    let response = CanonicalResponse::try_new(vec![
        response_start(&response_id),
        message_start(),
        text_delta(),
        message_end(),
        response_end(),
    ])
    .map_err(|_| ProbeError::CanonicalLifecycle)?;
    if response.events().len() != 5 {
        return Err(ProbeError::CanonicalLifecycle);
    }
    markers.insert(ProjectionMarker::ResponseEnd);
    Ok(markers)
}

fn response_start(response_id: &ResponseId) -> CanonicalEvent {
    CanonicalEvent::ResponseStart(ResponseStart {
        response_id: response_id.clone(),
        extensions: RawExtensions::default(),
    })
}

fn message_start() -> CanonicalEvent {
    CanonicalEvent::MessageStart(MessageStart {
        role: MessageRole("assistant".to_owned()),
        extensions: RawExtensions::default(),
    })
}

fn text_delta() -> CanonicalEvent {
    CanonicalEvent::TextDelta(TextDelta {
        text: "differential".to_owned(),
        extensions: RawExtensions::default(),
    })
}

fn message_end() -> CanonicalEvent {
    CanonicalEvent::MessageEnd(MessageEnd {
        extensions: RawExtensions::default(),
    })
}

fn response_end() -> CanonicalEvent {
    CanonicalEvent::ResponseEnd(ResponseEnd {
        stop_reason: None,
        stop_sequence: None,
        extensions: RawExtensions::default(),
    })
}

fn configuration_authority() -> Result<BTreeSet<ProjectionMarker>, ProbeError> {
    let first = ConfigVersionId::try_new("differential-config-version-1")
        .map_err(|_| ProbeError::ConfigurationAuthority)?;
    let second = ConfigVersionId::try_new("differential-config-version-2")
        .map_err(|_| ProbeError::ConfigurationAuthority)?;
    let mut repository = SqliteControlPlaneRepository::open_in_memory()
        .map_err(|_| ProbeError::ConfigurationAuthority)?;

    repository
        .write_configuration(&draft_configuration(&first, 1))
        .map_err(|_| ProbeError::ConfigurationAuthority)?;
    repository
        .write_configuration(&draft_configuration(&second, 2))
        .map_err(|_| ProbeError::ConfigurationAuthority)?;

    if repository
        .load_active_configuration()
        .map_err(|_| ProbeError::ConfigurationAuthority)?
        .is_some()
    {
        return Err(ProbeError::ConfigurationAuthority);
    }

    let first_activation = repository
        .activate_version(&first)
        .map_err(|_| ProbeError::ConfigurationAuthority)?;
    if first_activation.activated_version_id() != &first
        || first_activation.replaced_active_version_id().is_some()
    {
        return Err(ProbeError::ConfigurationAuthority);
    }
    let active = repository
        .load_active_configuration()
        .map_err(|_| ProbeError::ConfigurationAuthority)?
        .ok_or(ProbeError::ConfigurationAuthority)?;
    if active.version.id != first || active.version.status != ConfigVersionStatus::Active {
        return Err(ProbeError::ConfigurationAuthority);
    }

    let second_activation = repository
        .activate_version(&second)
        .map_err(|_| ProbeError::ConfigurationAuthority)?;
    if second_activation.replaced_active_version_id() != Some(&first) {
        return Err(ProbeError::ConfigurationAuthority);
    }
    let active = repository
        .load_active_configuration()
        .map_err(|_| ProbeError::ConfigurationAuthority)?
        .ok_or(ProbeError::ConfigurationAuthority)?;
    if active.version.id != second {
        return Err(ProbeError::ConfigurationAuthority);
    }

    let archived = repository
        .load_config_version(&first)
        .map_err(|_| ProbeError::ConfigurationAuthority)?
        .ok_or(ProbeError::ConfigurationAuthority)?;
    if archived.status != ConfigVersionStatus::Archived {
        return Err(ProbeError::ConfigurationAuthority);
    }
    if repository.activate_version(&second).is_ok() {
        return Err(ProbeError::ConfigurationAuthority);
    }

    Ok(BTreeSet::from([ProjectionMarker::VersionedSqliteSnapshot]))
}

fn draft_configuration(id: &ConfigVersionId, created_at_ms: i64) -> ControlPlaneConfiguration {
    ControlPlaneConfiguration {
        version: ConfigVersion {
            id: id.clone(),
            parent_id: None,
            status: ConfigVersionStatus::Draft,
            revision: 0,
            created_at_ms,
            description: "differential probe".to_owned(),
        },
        egress_policies: Vec::new(),
        upstreams: Vec::new(),
        endpoints: Vec::new(),
        credentials: Vec::new(),
        endpoint_credential_bindings: Vec::new(),
        public_models: Vec::new(),
        model_aliases: Vec::new(),
        model_routes: Vec::new(),
        route_candidates: Vec::new(),
        access_groups: Vec::new(),
        access_group_routes: Vec::new(),
        client_keys: Vec::new(),
        routing_price_policy: None,
    }
}

fn provider_pool_isolation() -> Result<BTreeSet<ProjectionMarker>, ProbeError> {
    let mut markers = BTreeSet::new();
    let key_version = KeyVersion::try_new(1).map_err(|_| ProbeError::ProviderPoolIsolation)?;
    let master_key =
        MasterKey::try_from_bytes([0x31_u8; 32]).map_err(|_| ProbeError::ProviderPoolIsolation)?;
    let key_ring = MasterKeyRing::try_new(key_version, [(key_version, master_key)])
        .map_err(|_| ProbeError::ProviderPoolIsolation)?;
    let build_store = GrokBuildCredentialSqliteStore::open_in_memory(SecretStore::new(key_ring))
        .map_err(|_| ProbeError::ProviderPoolIsolation)?;
    let build_key = GrokBuildCredentialKey::try_new(
        gateway_store::control_plane::ConfigVersionId::try_new("differential-config-version-1")
            .map_err(|_| ProbeError::ProviderPoolIsolation)?,
        gateway_core::CredentialId::try_new("differential-build-credential")
            .map_err(|_| ProbeError::ProviderPoolIsolation)?,
    )
    .map_err(|_| ProbeError::ProviderPoolIsolation)?;

    if build_store
        .load(&build_key)
        .map_err(|_| ProbeError::ProviderPoolIsolation)?
        .is_some()
    {
        return Err(ProbeError::ProviderPoolIsolation);
    }

    let credential = web_credential(3)?;
    let slot = GrokWebCredentialSlot::new(credential.clone());
    if slot
        .load()
        .map_err(|_| ProbeError::ProviderPoolIsolation)?
        .revision()
        != 3
    {
        return Err(ProbeError::ProviderPoolIsolation);
    }
    if GrokWebCredential::provider_id() == "grok.build"
        || GrokWebBrowserEgressSession::provider_id() != GrokWebCredential::provider_id()
    {
        return Err(ProbeError::ProviderPoolIsolation);
    }
    if build_store
        .load(&build_key)
        .map_err(|_| ProbeError::ProviderPoolIsolation)?
        .is_some()
    {
        return Err(ProbeError::ProviderPoolIsolation);
    }
    markers.insert(ProjectionMarker::BuildWebPoolSeparation);

    let session = egress_session("differential-egress-1", credential)?;
    let other_session = egress_session("differential-egress-2", web_credential(3)?)?;
    let conversation_id = GrokWebConversationId::try_new("differential-conversation")
        .map_err(|_| ProbeError::ProviderPoolIsolation)?;
    let state = GrokWebConversationState::try_new(conversation_id, &session, PROBE_OBSERVED_AT_MS)
        .map_err(|_| ProbeError::ProviderPoolIsolation)?;
    if state.availability() != GrokWebConversationAvailability::Available {
        return Err(ProbeError::ProviderPoolIsolation);
    }
    state
        .prepare_turn(&session, PROBE_OBSERVED_AT_MS)
        .map_err(|_| ProbeError::ProviderPoolIsolation)?;
    if state
        .prepare_turn(&other_session, PROBE_OBSERVED_AT_MS)
        .is_ok()
    {
        return Err(ProbeError::ProviderPoolIsolation);
    }
    if state.prepare_turn(&session, PROBE_EXPIRES_AT_MS).is_ok() {
        return Err(ProbeError::ProviderPoolIsolation);
    }
    markers.insert(ProjectionMarker::BrowserEgressBoundConversation);
    Ok(markers)
}

fn web_credential(revision: u64) -> Result<GrokWebCredential, ProbeError> {
    let export = format!(
        r#"{{
            "kind":"grok_web_sso",
            "account_ref":"differential_account",
            "lineage_ref":"differential_lineage",
            "revision":{revision},
            "expires_at_ms":{PROBE_EXPIRES_AT_MS},
            "cookies":[
                {{"name":"sso_session","value":"differential_opaque_value","domain":"grok.example.test","path":"/","secure":true,"http_only":true}}
            ]
        }}"#
    );
    GrokWebCredential::import_sso_json(export.as_bytes(), PROBE_OBSERVED_AT_MS)
        .map_err(|_| ProbeError::ProviderPoolIsolation)
}

fn egress_session(
    id: &str,
    credential: GrokWebCredential,
) -> Result<GrokWebBrowserEgressSession, ProbeError> {
    GrokWebBrowserEgressSession::try_new(
        GrokWebEgressSessionId::try_new(id).map_err(|_| ProbeError::ProviderPoolIsolation)?,
        credential,
        GrokWebBrowserUserAgent::try_new("Mozilla/5.0 (X11; Linux x86_64) differential-probe")
            .map_err(|_| ProbeError::ProviderPoolIsolation)?,
        GrokWebTlsProfile::try_new("differential_profile")
            .map_err(|_| ProbeError::ProviderPoolIsolation)?,
        UpstreamProxy::Direct,
        PROBE_OBSERVED_AT_MS,
    )
    .map_err(|_| ProbeError::ProviderPoolIsolation)
}

fn web_tool_default() -> Result<BTreeSet<ProjectionMarker>, ProbeError> {
    let default = GrokWebToolEmulation::default();
    let prompt = default
        .prepare(&[])
        .map_err(|_| ProbeError::WebToolDefault)?;
    let capability = default.tool_capability();
    if prompt.capability() != capability || prompt.has_addendum() {
        return Err(ProbeError::WebToolDefault);
    }
    let marker = match (default.is_enabled(), capability) {
        (true, GrokWebToolCapability::Emulated) => ProjectionMarker::ToolEmulationDefaultEnabled,
        (false, GrokWebToolCapability::Disabled) => ProjectionMarker::ToolEmulationDefaultDisabled,
        (true, GrokWebToolCapability::Disabled) | (false, GrokWebToolCapability::Emulated) => {
            return Err(ProbeError::WebToolDefault);
        }
    };
    if GrokWebToolEmulation::new(true).tool_capability() != GrokWebToolCapability::Emulated {
        return Err(ProbeError::WebToolDefault);
    }
    Ok(BTreeSet::from([marker]))
}

fn endpoint_policy() -> Result<BTreeSet<ProjectionMarker>, ProbeError> {
    let region = KiroApiRegion::try_new("us-east-1").map_err(|_| ProbeError::EndpointPolicy)?;
    let ide = KiroEndpointPolicy::try_new(KiroEndpointKind::Ide, region.clone())
        .map_err(|_| ProbeError::EndpointPolicy)?;
    let cli = KiroEndpointPolicy::try_new(KiroEndpointKind::Cli, region)
        .map_err(|_| ProbeError::EndpointPolicy)?;

    if ide.url().as_str() != "https://q.us-east-1.amazonaws.com/generateAssistantResponse"
        || cli.url().as_str() != "https://runtime.us-east-1.kiro.dev/"
    {
        return Err(ProbeError::EndpointPolicy);
    }
    if ide.origin() != KiroRequestOrigin::AiEditor || cli.origin() != KiroRequestOrigin::KiroCli {
        return Err(ProbeError::EndpointPolicy);
    }
    if ide.thinking_placement() == cli.thinking_placement() {
        return Err(ProbeError::EndpointPolicy);
    }

    let ide_headers = ide.request_headers(KiroCredentialKind::Social);
    let cli_headers = cli.request_headers(KiroCredentialKind::Social);
    if ide_headers.contains_key("x-amz-target") || !cli_headers.contains_key("x-amz-target") {
        return Err(ProbeError::EndpointPolicy);
    }
    if ide_headers.get("content-type").map(String::as_str) != Some("application/json")
        || cli_headers.get("content-type").map(String::as_str) != Some("application/x-amz-json-1.0")
    {
        return Err(ProbeError::EndpointPolicy);
    }
    if !cli
        .request_headers(KiroCredentialKind::ApiKey)
        .contains_key("tokentype")
    {
        return Err(ProbeError::EndpointPolicy);
    }
    if KiroApiRegion::try_new("differential region").is_ok() {
        return Err(ProbeError::EndpointPolicy);
    }
    Ok(BTreeSet::from([ProjectionMarker::CliIdeEndpointPolicy]))
}

fn event_stream_integrity() -> Result<BTreeSet<ProjectionMarker>, ProbeError> {
    let mut markers = BTreeSet::new();
    let first = wire_frame(
        &[
            string_header(":message-type", "event"),
            string_header(":event-type", "assistantResponseEvent"),
        ],
        br#"{"content":"differential"}"#,
    );
    let second = wire_frame(
        &[
            string_header(":message-type", "event"),
            string_header(":event-type", "contextUsageEvent"),
        ],
        b"{}",
    );

    let mut decoder = KiroEventStreamDecoder::new();
    decoder
        .feed(&first)
        .map_err(|_| ProbeError::EventStreamIntegrity)?;
    let accepted = decoder
        .next_frame()
        .map_err(|_| ProbeError::EventStreamIntegrity)?
        .ok_or(ProbeError::EventStreamIntegrity)?;
    if accepted.headers().event_type() != Some("assistantResponseEvent") {
        return Err(ProbeError::EventStreamIntegrity);
    }
    decoder
        .finish()
        .map_err(|_| ProbeError::EventStreamIntegrity)?;

    let mut corrupt_message = first.clone();
    let last = corrupt_message
        .len()
        .checked_sub(1)
        .ok_or(ProbeError::EventStreamIntegrity)?;
    corrupt_message[last] ^= 0xff;
    let mut decoder = KiroEventStreamDecoder::new();
    decoder
        .feed(&corrupt_message)
        .map_err(|_| ProbeError::EventStreamIntegrity)?;
    if decoder.next_frame() != Err(KiroEventStreamError::MessageCrcMismatch) {
        return Err(ProbeError::EventStreamIntegrity);
    }

    let mut corrupt_prelude = first.clone();
    corrupt_prelude[11] ^= 0xff;
    let mut decoder = KiroEventStreamDecoder::new();
    decoder
        .feed(&corrupt_prelude)
        .map_err(|_| ProbeError::EventStreamIntegrity)?;
    if decoder.next_frame() != Err(KiroEventStreamError::PreludeCrcMismatch) {
        return Err(ProbeError::EventStreamIntegrity);
    }
    markers.insert(ProjectionMarker::EventStreamCrcValidation);

    let wire = [first, second].concat();
    let baseline = canonical_events(&wire, wire.len())?;
    for chunk_size in [1_usize, 2, 3, 5, 7, 11, 13, 17, wire.len()] {
        if canonical_events(&wire, chunk_size)? != baseline {
            return Err(ProbeError::EventStreamIntegrity);
        }
    }
    CanonicalResponse::try_new(baseline.clone()).map_err(|_| ProbeError::EventStreamIntegrity)?;
    if !baseline
        .iter()
        .any(|event| matches!(event, CanonicalEvent::TextDelta(_)))
    {
        return Err(ProbeError::EventStreamIntegrity);
    }
    markers.insert(ProjectionMarker::ChunkInvariantCanonicalEvents);
    Ok(markers)
}

fn canonical_events(wire: &[u8], chunk_size: usize) -> Result<Vec<CanonicalEvent>, ProbeError> {
    if chunk_size == 0 {
        return Err(ProbeError::EventStreamIntegrity);
    }
    let response_id = ResponseId::try_new("differential-event-stream")
        .map_err(|_| ProbeError::EventStreamIntegrity)?;
    let mut mapper = KiroEventSemanticMapper::new(response_id);
    let mut decoder = KiroEventStreamDecoder::new();
    let mut events = mapper
        .start()
        .map_err(|_| ProbeError::EventStreamIntegrity)?;

    for chunk in wire.chunks(chunk_size) {
        decoder
            .feed(chunk)
            .map_err(|_| ProbeError::EventStreamIntegrity)?;
        drain(&mut decoder, &mut mapper, &mut events)?;
    }
    decoder
        .finish()
        .map_err(|_| ProbeError::EventStreamIntegrity)?;
    events.extend(
        mapper
            .finish()
            .map_err(|_| ProbeError::EventStreamIntegrity)?,
    );
    Ok(events)
}

fn drain(
    decoder: &mut KiroEventStreamDecoder,
    mapper: &mut KiroEventSemanticMapper,
    events: &mut Vec<CanonicalEvent>,
) -> Result<(), ProbeError> {
    loop {
        let frame = decoder
            .next_frame()
            .map_err(|_| ProbeError::EventStreamIntegrity)?;
        let Some(frame) = frame else {
            return Ok(());
        };
        events.extend(
            mapper
                .push_frame(&frame)
                .map_err(|_| ProbeError::EventStreamIntegrity)?,
        );
    }
}

fn wire_frame(headers: &[Vec<u8>], payload: &[u8]) -> Vec<u8> {
    let headers = headers.concat();
    let total_length = 12 + headers.len() + payload.len() + 4;
    let mut wire = Vec::with_capacity(total_length);
    wire.extend_from_slice(
        &u32::try_from(total_length)
            .unwrap_or_default()
            .to_be_bytes(),
    );
    wire.extend_from_slice(
        &u32::try_from(headers.len())
            .unwrap_or_default()
            .to_be_bytes(),
    );
    wire.extend_from_slice(&crc32(&wire).to_be_bytes());
    wire.extend_from_slice(&headers);
    wire.extend_from_slice(payload);
    wire.extend_from_slice(&crc32(&wire).to_be_bytes());
    wire
}

fn string_header(name: &str, value: &str) -> Vec<u8> {
    let value = value.as_bytes();
    let mut header = Vec::with_capacity(4 + name.len() + value.len());
    header.push(u8::try_from(name.len()).unwrap_or_default());
    header.extend_from_slice(name.as_bytes());
    header.push(7);
    header.extend_from_slice(&u16::try_from(value.len()).unwrap_or_default().to_be_bytes());
    header.extend_from_slice(value);
    header
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}
