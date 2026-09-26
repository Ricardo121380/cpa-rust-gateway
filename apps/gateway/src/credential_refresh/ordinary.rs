//! Fixed-channel OAuth refresh with durable admission before network I/O.
use super::{
    Arc, BTreeMap, CODEX_REFRESH_SKEW_MS, Cell, ConfigVersionId, CredentialId, CredentialStatus,
    Duration, EndpointCredentialPools, GrokAccountWorkerError, KIMI_OAUTH_TOKEN_URL,
    OAUTH_CONNECT_TIMEOUT, OAUTH_REQUEST_TIMEOUT, OAuthRefreshSummary,
    OpenAiCompatibleRuntimeCredential, Path, REFRESH_CLAIM_LEASE_MS, Read, RuntimeHealthRegistry,
    SecretStore, SqliteControlPlaneRepository, UpstreamProxy, Zeroizing,
    credential_associated_data, now_ms, sync_runtime_material,
};
use gateway_store::credential_refresh::{CredentialRefreshStore, RefreshFailure};
use provider_anthropic_compatible::ClaudeRuntimeCredential;
use provider_kiro::credential::{
    KiroCredential, KiroCredentialError, KiroRefreshRequest, KiroRefreshResponse,
    KiroRefreshTransport,
};
use provider_openai_compatible::CodexCredentialExportFormat;
use std::fmt::Write as _;
use zeroize::Zeroize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Channel {
    Codex,
    Kimi,
    Claude,
    Kiro,
}

pub(super) enum Material {
    OpenAi(OpenAiCompatibleRuntimeCredential),
    Claude(ClaudeRuntimeCredential),
    Kiro(KiroCredential),
}
impl Material {
    fn read(channel: Channel, bytes: &[u8], now: i64) -> Result<Self, RefreshFailure> {
        let invalid = |_| RefreshFailure::InvalidResponse;
        match channel {
            Channel::Codex | Channel::Kimi => {
                let value = OpenAiCompatibleRuntimeCredential::import_compatible(bytes, now)
                    .map_err(invalid)?;
                if !matches!(
                    (&value, channel),
                    (
                        OpenAiCompatibleRuntimeCredential::CodexOAuth(_),
                        Channel::Codex
                    ) | (
                        OpenAiCompatibleRuntimeCredential::KimiOAuth(_),
                        Channel::Kimi
                    )
                ) {
                    return Err(RefreshFailure::InvalidResponse);
                }
                Ok(Self::OpenAi(value))
            }
            Channel::Claude => ClaudeRuntimeCredential::import_at(bytes, now)
                .map(Self::Claude)
                .map_err(|_| RefreshFailure::InvalidResponse),
            Channel::Kiro if bytes.starts_with(b"ksk_") => {
                KiroCredential::import_runtime_secret(bytes, now)
                    .map(Self::Kiro)
                    .map_err(|_| RefreshFailure::InvalidResponse)
            }
            Channel::Kiro => {
                // API-key envelopes have no expiry and must not become refresh failures.
                KiroCredential::import_for_refresh(bytes)
                    .or_else(|_| KiroCredential::import_runtime_secret(bytes, now))
                    .map(Self::Kiro)
                    .map_err(|_| RefreshFailure::InvalidResponse)
            }
        }
    }
    fn expiry(&self) -> Option<i64> {
        match self {
            Self::OpenAi(v) => v.expires_at_ms(),
            Self::Claude(v) => v.expires_at_ms(),
            Self::Kiro(v) => v.expires_at_ms(),
        }
    }
}

trait Exchange {
    fn exchange(
        &self,
        channel: Channel,
        material: &mut Material,
        now: i64,
    ) -> Result<Zeroizing<Vec<u8>>, RefreshFailure>;
}
struct HttpExchange {
    codex_proxy: UpstreamProxy,
}

impl HttpExchange {
    fn post(
        &self,
        url: &str,
        content_type: &str,
        body: &[u8],
        device: Option<&str>,
        codex: bool,
    ) -> Result<Zeroizing<Vec<u8>>, RefreshFailure> {
        let mut builder = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(OAUTH_CONNECT_TIMEOUT)
            .timeout(OAUTH_REQUEST_TIMEOUT);
        if codex && let Some(url) = self.codex_proxy.canonical_url() {
            builder = builder.proxy(reqwest::Proxy::all(url).map_err(|_| RefreshFailure::Network)?);
        }
        let mut request = builder
            .build()
            .map_err(|_| RefreshFailure::Network)?
            .post(url)
            .header("Content-Type", content_type)
            .header("Accept", "application/json")
            .body(body.to_vec());
        if let Some(device) = device {
            request = request
                .header("User-Agent", "cpa-rust-gateway/kimi-oauth")
                .header("X-Msh-Platform", "CPAR")
                .header("X-Msh-Device-Name", "CPAR Gateway")
                .header("X-Msh-Device-Model", "gateway")
                .header("X-Msh-Device-Id", device);
        }
        let response = request.send().map_err(|_| RefreshFailure::Network)?;
        let status = response.status().as_u16();
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .take(65_537)
            .read_to_end(&mut bytes)
            .map_err(|_| RefreshFailure::Network)?;
        if bytes.len() > 65_536 {
            return Err(RefreshFailure::InvalidResponse);
        }
        if !(200..300).contains(&status) {
            return Err(classify_rejection(status, &bytes));
        }
        Ok(bytes)
    }
}

pub(super) fn classify_rejection(status: u16, body: &[u8]) -> RefreshFailure {
    // Only an explicit grant error revokes. A WAF 403, HTML, 429 or 5xx is not revocation proof.
    #[derive(serde::Deserialize)]
    struct Failure {
        error: Option<String>,
        #[serde(rename = "__type")]
        error_type: Option<String>,
    }
    let error = serde_json::from_slice::<Failure>(body)
        .ok()
        .and_then(|v| v.error.or(v.error_type));
    if matches!(status, 400 | 401 | 403)
        && matches!(
            error.as_deref(),
            Some(
                "invalid_grant"
                    | "invalid_client"
                    | "expired_token"
                    | "token_revoked"
                    | "invalid_token"
                    | "InvalidGrantException"
                    | "InvalidClientException"
                    | "ExpiredTokenException"
            )
        )
    {
        RefreshFailure::ReauthRequired
    } else {
        RefreshFailure::Network
    }
}

impl Exchange for HttpExchange {
    fn exchange(
        &self,
        channel: Channel,
        material: &mut Material,
        now: i64,
    ) -> Result<Zeroizing<Vec<u8>>, RefreshFailure> {
        let invalid = |_| RefreshFailure::InvalidResponse;
        match (channel, material) {
            (Channel::Codex, Material::OpenAi(value)) => {
                let request = value.refresh_request().map_err(invalid)?;
                let body = self.post(
                    provider_openai_compatible::CODEX_OAUTH_TOKEN_URL,
                    "application/x-www-form-urlencoded",
                    request.form_body().as_bytes(),
                    None,
                    true,
                )?;
                value.apply_refresh_response(&body, now).map_err(invalid)?;
                value
                    .export_json(CodexCredentialExportFormat::Cpa)
                    .map_err(invalid)
            }
            (Channel::Kimi, Material::OpenAi(value)) => {
                let request = value.kimi_refresh_request().map_err(invalid)?;
                let body = self.post(
                    KIMI_OAUTH_TOKEN_URL,
                    "application/x-www-form-urlencoded",
                    request.form_body().as_bytes(),
                    Some(request.device_id()),
                    false,
                )?;
                value
                    .apply_kimi_refresh_response(&body, now)
                    .map_err(invalid)?;
                value.export_kimi_oauth_json().map_err(invalid)
            }
            (Channel::Claude, Material::Claude(value)) => {
                let request = value
                    .refresh_request()
                    .map_err(|_| RefreshFailure::InvalidResponse)?;
                let body = self.post(
                    provider_anthropic_compatible::CLAUDE_OAUTH_TOKEN_URL,
                    "application/json",
                    &request
                        .json_body()
                        .map_err(|_| RefreshFailure::InvalidResponse)?,
                    None,
                    false,
                )?;
                value
                    .apply_refresh_response(&body, now)
                    .map_err(|_| RefreshFailure::InvalidResponse)?;
                value
                    .export_oauth_json()
                    .map_err(|_| RefreshFailure::InvalidResponse)
            }
            (Channel::Kiro, Material::Kiro(value)) => {
                let transport = KiroHttp {
                    http: self,
                    failure: Cell::new(None),
                };
                *value = value.refresh(&transport, now).map_err(|_| {
                    transport
                        .failure
                        .get()
                        .unwrap_or(RefreshFailure::InvalidResponse)
                })?;
                value
                    .export_oauth_json()
                    .map_err(|_| RefreshFailure::InvalidResponse)
            }
            _ => Err(RefreshFailure::InvalidResponse),
        }
    }
}

struct KiroHttp<'a> {
    http: &'a HttpExchange,
    failure: Cell<Option<RefreshFailure>>,
}
impl KiroRefreshTransport for KiroHttp<'_> {
    fn refresh(
        &self,
        request: KiroRefreshRequest,
    ) -> Result<KiroRefreshResponse, KiroCredentialError> {
        // AWS/Social wire shape is camelCase; provider boundary accepts strict normalized fields.
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            access_token: String,
            refresh_token: Option<String>,
            expires_in: i64,
            token_type: Option<String>,
        }
        #[derive(serde::Serialize)]
        struct Normalized<'a> {
            access_token: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            refresh_token: Option<&'a str>,
            expires_in: i64,
            #[serde(skip_serializing_if = "Option::is_none")]
            token_type: Option<&'a str>,
        }
        let result = self.http.post(
            &request.token_url(),
            "application/json",
            &request.json_body()?,
            None,
            false,
        );
        let body = result.map_err(|failure| {
            self.failure.set(Some(failure));
            KiroCredentialError::TransportUnavailable
        })?;
        let mut v: Response = serde_json::from_slice(&body)
            .map_err(|_| KiroCredentialError::InvalidRefreshResponse)?;
        let refresh = v.refresh_token.as_deref();
        let normalized = serde_json::to_vec(&Normalized {
            access_token: &v.access_token,
            refresh_token: refresh,
            expires_in: v.expires_in,
            token_type: v.token_type.as_deref(),
        })
        .map_err(|_| KiroCredentialError::InvalidRefreshResponse)?;
        v.access_token.zeroize();
        v.refresh_token.zeroize();
        Ok(KiroRefreshResponse::new(normalized))
    }
}

pub(super) struct Pass<'a> {
    pub database: &'a Path,
    pub secrets: &'a SecretStore,
    pub version: &'a ConfigVersionId,
    pub channels: &'a BTreeMap<CredentialId, Channel>,
    pub pools: Option<&'a EndpointCredentialPools>,
    pub health: Option<&'a RuntimeHealthRegistry>,
    pub guard: Option<&'a (
        Arc<tokio::sync::Mutex<()>>,
        Arc<std::sync::atomic::AtomicBool>,
    )>,
    pub stopped: Option<&'a std::sync::atomic::AtomicBool>,
}
impl Pass<'_> {
    pub fn run(&self, proxy: UpstreamProxy) -> Result<OAuthRefreshSummary, GrokAccountWorkerError> {
        self.run_with(&HttpExchange { codex_proxy: proxy }, &now_ms)
    }

    #[allow(clippy::too_many_lines)]
    fn run_with(
        &self,
        exchange: &dyn Exchange,
        clock: &dyn Fn() -> Result<i64, GrokAccountWorkerError>,
    ) -> Result<OAuthRefreshSummary, GrokAccountWorkerError> {
        let started = std::time::Instant::now();
        let mut exchanges = 0usize;
        let store_error = |_| GrokAccountWorkerError::StoreUnavailable;
        let mut repo = SqliteControlPlaneRepository::open(self.database).map_err(store_error)?;
        let Some(config) = repo.load_configuration(self.version).map_err(store_error)? else {
            return Ok(OAuthRefreshSummary::default());
        };
        let mut store = CredentialRefreshStore::open(self.database).map_err(store_error)?;
        let mut summary = OAuthRefreshSummary::default();
        for row in config
            .credentials
            .iter()
            .filter(|c| c.status == CredentialStatus::Active)
        {
            if exchanges >= 8 || started.elapsed() >= Duration::from_secs(30) {
                break;
            }
            if self
                .stopped
                .is_some_and(|s| s.load(std::sync::atomic::Ordering::Acquire))
            {
                break;
            }
            let Some(channel) = self.channels.get(&row.id) else {
                continue;
            };
            let _guard = match self.guard {
                Some((gate, active)) => {
                    let lock = gate.blocking_lock();
                    if !active.load(std::sync::atomic::Ordering::Acquire) {
                        break;
                    }
                    Some(lock)
                }
                None => None,
            };
            let now = clock()?;
            if self
                .stopped
                .is_some_and(|s| s.load(std::sync::atomic::Ordering::Acquire))
            {
                break;
            }
            let aad = credential_associated_data(self.version, &row.id, &row.upstream_id)
                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?;
            let bytes = self.secrets.open(&row.encrypted_secret, &aad);
            let material = bytes
                .as_ref()
                .map_err(|_| RefreshFailure::InvalidResponse)
                .and_then(|bytes| Material::read(*channel, bytes.as_bytes(), now));
            if let (Ok(material), Ok(bytes)) = (&material, &bytes)
                && material
                    .expiry()
                    .is_none_or(|expiry| expiry > now.saturating_add(CODEX_REFRESH_SKEW_MS))
            {
                summary.runtime_replaced += sync_runtime_material(
                    &config,
                    &row.id,
                    bytes.as_bytes(),
                    material.expiry(),
                    u64::try_from(row.revision)
                        .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                    self.pools,
                    self.health,
                    now,
                )?;
                continue;
            }
            summary.due += 1;
            let mut random = [0u8; 16];
            getrandom::fill(&mut random).map_err(|_| GrokAccountWorkerError::InvalidRequest)?;
            let nonce = random.iter().fold(String::new(), |mut s, b| {
                let _ = write!(s, "{b:02x}");
                s
            });
            let Some(claim) = store
                .claim(
                    self.version,
                    &row.id,
                    row.revision,
                    &nonce,
                    now,
                    REFRESH_CLAIM_LEASE_MS,
                )
                .map_err(store_error)?
            else {
                summary.backed_off += 1;
                continue;
            };
            let result = material.and_then(|mut value| {
                exchanges += 1;
                exchange
                    .exchange(*channel, &mut value, now)
                    .map(|bytes| (bytes, value.expiry()))
            });
            let completed = clock()?;
            match result {
                Ok((replacement, expiry)) => {
                    let sealed = self
                        .secrets
                        .seal(&replacement, &aad)
                        .map_err(|_| GrokAccountWorkerError::SecretStoreFailure)?;
                    if store
                        .succeed(&claim, &sealed, completed)
                        .map_err(store_error)?
                    {
                        summary.succeeded += 1;
                        summary.runtime_replaced += sync_runtime_material(
                            &config,
                            &row.id,
                            &replacement,
                            expiry,
                            u64::try_from(row.revision + 1)
                                .map_err(|_| GrokAccountWorkerError::InvalidPersistedState)?,
                            self.pools,
                            self.health,
                            completed,
                        )?;
                    } else {
                        summary.conflicted += 1;
                    }
                }
                Err(failure) => {
                    if store
                        .fail(&claim, failure, completed)
                        .map_err(store_error)?
                    {
                        if failure == RefreshFailure::ReauthRequired {
                            summary.reauth_required += 1;
                            if let Some(health) = self.health {
                                for binding in config
                                    .endpoint_credential_bindings
                                    .iter()
                                    .filter(|b| b.credential_id == row.id && b.enabled)
                                {
                                    health
                                        .mark_credential_unauthorized(
                                            binding.endpoint_id.clone(),
                                            row.id.clone(),
                                        )
                                        .map_err(|_| {
                                            GrokAccountWorkerError::InvalidPersistedState
                                        })?;
                                }
                            }
                        } else {
                            summary.backed_off += 1;
                        }
                    } else {
                        summary.conflicted += 1;
                    }
                }
            }
        }
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_store::{
        control_plane::{
            ConfigVersion, ConfigVersionStatus, ControlPlaneConfiguration, CredentialConfiguration,
            UpstreamConfiguration,
        },
        secret_store::{KeyVersion, MasterKey, MasterKeyRing},
    };
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    static FIXTURE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    const CLAUDE: &[u8] = br#"{"kind":"claude_oauth","access_token":"old","refresh_token":"refresh","expires_at_ms":1000,"account_id":"subject","email":"sample@example.test","plan":"pro"}"#;
    const KIRO: &[u8] = br#"{"kind":"enterprise","access_token":"old","refresh_token":"refresh","expires_at_ms":1000,"client_id":"app","client_secret":"secret","auth_region":"us-east-1"}"#;

    struct Fixture {
        database: PathBuf,
        secrets: SecretStore,
        version: ConfigVersionId,
        channels: BTreeMap<CredentialId, Channel>,
    }
    impl Fixture {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            Self::with_extra_credentials(0)
        }
        fn with_extra_credentials(extra: usize) -> Result<Self, Box<dyn std::error::Error>> {
            let key = KeyVersion::try_new(1)?;
            let secrets = SecretStore::new(MasterKeyRing::try_new(
                key,
                [(key, MasterKey::try_from_bytes([19; 32])?)],
            )?);
            let version = ConfigVersionId::try_new("refresh-test")?;
            let mut config = ControlPlaneConfiguration::new(ConfigVersion {
                id: version.clone(),
                parent_id: None,
                status: ConfigVersionStatus::Draft,
                revision: 1,
                created_at_ms: 0,
                description: String::new(),
            });
            let owner = gateway_core::UpstreamId::try_new("owner")?;
            config.upstreams.push(UpstreamConfiguration {
                id: owner.clone(),
                name: "test".into(),
                kind: "test".into(),
                enabled: true,
                tags_json: "[]".into(),
                egress_policy_id: None,
            });
            let mut channels = BTreeMap::new();
            for (id, channel, bytes) in [
                ("claude", Channel::Claude, CLAUDE),
                ("kiro", Channel::Kiro, KIRO),
            ] {
                let id = CredentialId::try_new(id)?;
                let aad = credential_associated_data(&version, &id, &owner)?;
                config.credentials.push(CredentialConfiguration {
                    id: id.clone(),
                    upstream_id: owner.clone(),
                    kind: "bearer".into(),
                    encrypted_secret: secrets.seal(bytes, &aad)?,
                    status: CredentialStatus::Active,
                    revision: 1,
                });
                channels.insert(id, channel);
            }
            for n in 0..extra {
                let id = CredentialId::try_new(format!("extra-{n}"))?;
                let aad = credential_associated_data(&version, &id, &owner)?;
                config.credentials.push(CredentialConfiguration {
                    id: id.clone(),
                    upstream_id: owner.clone(),
                    kind: "bearer".into(),
                    encrypted_secret: secrets.seal(CLAUDE, &aad)?,
                    status: CredentialStatus::Active,
                    revision: 1,
                });
                channels.insert(id, Channel::Claude);
            }
            let database = std::env::temp_dir().join(format!(
                "cpar-oauth-{}-{}-{}.sqlite",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
                FIXTURE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let mut repo = SqliteControlPlaneRepository::open(&database)?;
            repo.write_configuration(&config)?;
            repo.activate_version(&version)?;
            Ok(Self {
                database,
                secrets,
                version,
                channels,
            })
        }
        fn pass(&self) -> Pass<'_> {
            Pass {
                database: &self.database,
                secrets: &self.secrets,
                version: &self.version,
                channels: &self.channels,
                pools: None,
                health: None,
                guard: None,
                stopped: None,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.database);
        }
    }
    struct Good(Cell<usize>);
    impl Exchange for Good {
        fn exchange(
            &self,
            _: Channel,
            material: &mut Material,
            now: i64,
        ) -> Result<Zeroizing<Vec<u8>>, RefreshFailure> {
            self.0.set(self.0.get() + 1);
            let body = br#"{"access_token":"new","refresh_token":"rotated","expires_in":3600}"#;
            match material {
                Material::Claude(value) => {
                    value
                        .apply_refresh_response(body, now)
                        .map_err(|_| RefreshFailure::InvalidResponse)?;
                    value
                        .export_oauth_json()
                        .map_err(|_| RefreshFailure::InvalidResponse)
                }
                Material::Kiro(value) => {
                    struct Transport;
                    impl KiroRefreshTransport for Transport {
                        fn refresh(
                            &self,
                            request: KiroRefreshRequest,
                        ) -> Result<KiroRefreshResponse, KiroCredentialError>
                        {
                            assert_eq!(
                                request.token_url(),
                                "https://oidc.us-east-1.amazonaws.com/token"
                            );
                            let body = request.json_body()?;
                            let parsed: serde_json::Value = serde_json::from_slice(&body)
                                .map_err(|_| KiroCredentialError::InvalidField)?;
                            assert_eq!(parsed["grantType"], "refresh_token");
                            assert_eq!(parsed["clientId"], "app");
                            Ok(KiroRefreshResponse::new(br#"{"access_token":"new","refresh_token":"rotated","expires_in":3600}"#.to_vec()))
                        }
                    }
                    *value = value
                        .refresh(&Transport, now)
                        .map_err(|_| RefreshFailure::InvalidResponse)?;
                    value
                        .export_oauth_json()
                        .map_err(|_| RefreshFailure::InvalidResponse)
                }
                Material::OpenAi(_) => Err(RefreshFailure::InvalidResponse),
            }
        }
    }
    #[test]
    fn startup_refreshes_expired_claude_and_kiro_and_preserves_identity_and_kind() -> TestResult {
        let fixture = Fixture::new()?;
        let exchange = Good(Cell::new(0));
        assert!(KiroCredential::import_runtime_secret(KIRO, 2_000).is_err());
        let summary = fixture.pass().run_with(&exchange, &|| Ok(2_000))?;
        assert_eq!(summary.succeeded, 2);
        assert_eq!(exchange.0.get(), 2);
        assert_eq!(fixture.pass().run_with(&exchange, &|| Ok(3_000))?.due, 0);
        let mut repo = SqliteControlPlaneRepository::open(&fixture.database)?;
        let config = repo.load_configuration(&fixture.version)?.ok_or("config")?;
        for row in config.credentials {
            assert_eq!(row.kind, "bearer");
            assert_eq!(row.revision, 2);
            let aad = credential_associated_data(&fixture.version, &row.id, &row.upstream_id)?;
            let bytes = fixture.secrets.open(&row.encrypted_secret, &aad)?;
            let body: serde_json::Value = serde_json::from_slice(bytes.as_bytes())?;
            if row.id.as_str() == "claude" {
                assert_eq!(body["email"], "sample@example.test");
                assert_eq!(body["account_id"], "subject");
            } else {
                assert_eq!(body["auth_region"], "us-east-1");
                assert_eq!(body["client_id"], "app");
            }
        }
        Ok(())
    }
    #[test]
    fn failures_backoff_across_passes_and_revoked_grants_stop_refreshing() -> TestResult {
        struct Failed(RefreshFailure, Cell<usize>);
        impl Exchange for Failed {
            fn exchange(
                &self,
                _: Channel,
                _: &mut Material,
                _: i64,
            ) -> Result<Zeroizing<Vec<u8>>, RefreshFailure> {
                self.1.set(self.1.get() + 1);
                Err(self.0)
            }
        }
        let fixture = Fixture::new()?;
        let exchange = Failed(RefreshFailure::Network, Cell::new(0));
        assert_eq!(
            fixture
                .pass()
                .run_with(&exchange, &|| Ok(2_000))?
                .backed_off,
            2
        );
        assert_eq!(
            fixture
                .pass()
                .run_with(&exchange, &|| Ok(3_000))?
                .backed_off,
            2
        );
        assert_eq!(exchange.1.get(), 2);
        let rejected = Failed(RefreshFailure::ReauthRequired, Cell::new(0));
        assert_eq!(
            fixture
                .pass()
                .run_with(&rejected, &|| Ok(62_000))?
                .reauth_required,
            2
        );
        assert_eq!(
            fixture.pass().run_with(&rejected, &|| Ok(9_000_000))?.due,
            0
        );
        assert_eq!(rejected.1.get(), 2);
        Ok(())
    }
    #[test]
    fn unknown_rejections_are_not_revocation_and_stop_prevents_new_exchanges() -> TestResult {
        for status in [400, 403, 429, 503] {
            assert_eq!(
                classify_rejection(status, b"<html>denied</html>"),
                RefreshFailure::Network
            );
        }
        assert_eq!(
            classify_rejection(400, br#"{"error":"invalid_grant"}"#),
            RefreshFailure::ReauthRequired
        );
        assert_eq!(
            classify_rejection(503, br#"{"error":"invalid_grant"}"#),
            RefreshFailure::Network
        );
        let fixture = Fixture::new()?;
        let exchange = Good(Cell::new(0));
        let stopped = std::sync::atomic::AtomicBool::new(true);
        let mut pass = fixture.pass();
        pass.stopped = Some(&stopped);
        assert_eq!(pass.run_with(&exchange, &|| Ok(2_000))?.due, 0);
        assert_eq!(exchange.0.get(), 0);
        assert_eq!(
            Material::read(Channel::Claude, b"api-key", 2_000).map(|m| m.expiry()),
            Ok(None)
        );
        assert_eq!(
            Material::read(Channel::Kiro, b"ksk_example", 2_000).map(|m| m.expiry()),
            Ok(None)
        );
        Ok(())
    }
    #[test]
    fn damaged_ciphertext_is_isolated_from_a_healthy_sibling() -> TestResult {
        let fixture = Fixture::new()?;
        let db = gateway_store::open(&fixture.database)?;
        let mut damaged: Vec<u8> = db.query_row(
            "SELECT ciphertext FROM upstream_credentials WHERE id='claude'",
            [],
            |r| r.get(0),
        )?;
        *damaged.last_mut().ok_or("envelope")? ^= 1;
        db.execute(
            "UPDATE upstream_credentials SET ciphertext=?1 WHERE id='claude'",
            [damaged],
        )?;
        let exchange = Good(Cell::new(0));
        let result = fixture.pass().run_with(&exchange, &|| Ok(2000))?;
        assert_eq!(result.succeeded, 1);
        assert_eq!(result.backed_off, 1);
        assert_eq!(exchange.0.get(), 1);
        Ok(())
    }

    #[test]
    fn bounded_pass_leaves_remaining_credentials_for_the_next_pass() -> TestResult {
        let fixture = Fixture::with_extra_credentials(10)?;
        let exchange = Good(Cell::new(0));
        assert_eq!(
            fixture.pass().run_with(&exchange, &|| Ok(2000))?.succeeded,
            8
        );
        assert_eq!(exchange.0.get(), 8);
        assert_eq!(
            fixture.pass().run_with(&exchange, &|| Ok(2000))?.succeeded,
            4
        );
        assert_eq!(exchange.0.get(), 12);
        Ok(())
    }

    #[test]
    fn stop_during_exchange_commits_current_result_but_never_starts_the_next() -> TestResult {
        struct Stop<'a>(&'a std::sync::atomic::AtomicBool);
        impl Exchange for Stop<'_> {
            fn exchange(
                &self,
                channel: Channel,
                material: &mut Material,
                now: i64,
            ) -> Result<Zeroizing<Vec<u8>>, RefreshFailure> {
                self.0.store(true, std::sync::atomic::Ordering::Release);
                Good(Cell::new(0)).exchange(channel, material, now)
            }
        }
        let fixture = Fixture::new()?;
        let stopped = std::sync::atomic::AtomicBool::new(false);
        let mut pass = fixture.pass();
        pass.stopped = Some(&stopped);
        assert_eq!(pass.run_with(&Stop(&stopped), &|| Ok(2000))?.succeeded, 1);
        Ok(())
    }

    #[test]
    fn http_exchange_bounds_bodies_and_does_not_follow_redirects() -> TestResult {
        use std::{io::Write, net::TcpListener};
        for (status, body, expected) in [
            (
                403,
                b"<html>denied</html>".to_vec(),
                RefreshFailure::Network,
            ),
            (
                400,
                br#"{"__type":"InvalidGrantException"}"#.to_vec(),
                RefreshFailure::ReauthRequired,
            ),
            (302, Vec::new(), RefreshFailure::Network),
            (200, vec![b'x'; 65537], RefreshFailure::InvalidResponse),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let address = listener.local_addr()?;
            let server = std::thread::spawn(move || -> std::io::Result<()> {
                let (mut stream, _) = listener.accept()?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf)?;
                write!(
                    stream,
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nLocation: http://127.0.0.1:1/must-not-follow\r\nConnection: close\r\n\r\n",
                    body.len()
                )?;
                stream.write_all(&body)
            });
            let http = HttpExchange {
                codex_proxy: UpstreamProxy::Direct,
            };
            assert_eq!(
                http.post(
                    &format!("http://{address}/token"),
                    "application/json",
                    b"{}",
                    None,
                    false
                )
                .err(),
                Some(expected)
            );
            server.join().map_err(|_| "server panicked")??;
        }
        Ok(())
    }
}
