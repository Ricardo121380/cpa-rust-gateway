//! Grok Build OAuth credential import and Device Authorization Grant boundary.
//!
//! This module is intentionally transport-injectable. It validates and retains only the
//! short-lived credential material needed by a later request path; it neither opens a socket nor
//! interprets an `id_token` claim as an account identity.

use std::{collections::BTreeMap, error::Error, fmt};

use serde::{
    Deserialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::Number;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::{Url, form_urlencoded};
use zeroize::Zeroizing;

/// Fixed issuer for Grok Build's public OAuth client.
pub const GROK_BUILD_OAUTH_ISSUER: &str = "https://auth.x.ai";
/// Public OAuth client identifier used by Grok Build's Device Authorization Grant.
pub const GROK_BUILD_PUBLIC_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
/// Fixed least-privilege scope set requested by the Grok Build OAuth flow.
pub const GROK_BUILD_OAUTH_SCOPE: &str =
    "openid profile email offline_access grok-cli:access api:access";
/// Fixed Device Authorization endpoint for Grok Build.
pub const GROK_BUILD_DEVICE_AUTHORIZATION_URL: &str = "https://auth.x.ai/oauth2/device/code";
/// Fixed OAuth token endpoint for Grok Build.
pub const GROK_BUILD_TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";

const MAX_OAUTH_JSON_BYTES: usize = 64 * 1024;
const MAX_OAUTH_FIELD_BYTES: usize = 16 * 1024;
const MAX_TOKEN_LIFETIME_SECONDS: u64 = 366 * 24 * 60 * 60;
const ABSOLUTE_EXPIRY_FIELD_ALIASES: &[&str] = &[
    "expired",
    "expires_at",
    "expires_at_ms",
    "expiration",
    "expires",
];
const DEFAULT_DEVICE_POLL_INTERVAL_SECONDS: u64 = 5;
const DEVICE_SLOW_DOWN_SECONDS: u64 = 5;
const PERSISTED_CREDENTIAL_FORMAT_VERSION: u8 = 1;

/// Non-secret origin for one Grok Build OAuth credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildCredentialSource {
    /// The credential was decoded from one explicitly supplied OAuth JSON export.
    ImportedJson,
    /// The credential was granted after a Device Authorization poll.
    DeviceCode,
    /// The credential was granted by a later refresh grant.
    Refresh,
    /// The credential was decoded from a CPA xAI OAuth auth file.
    CpaXaiAuthFile,
    /// The credential was decoded from a Grok account credential object.
    GrokAccountJson,
    /// The credential was decoded from the official Grok CLI's indexed auth cache.
    OfficialCliAuthCache,
}

/// One validated, short-lived Grok Build OAuth credential.
///
/// Access and refresh tokens are redacted from diagnostics and zeroized when dropped. Callers may
/// borrow them only while constructing one request or refresh grant; they must never log or persist
/// the returned strings outside the dedicated P6-02 persistence boundary.
#[derive(Clone)]
pub struct GrokBuildCredential {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
    expires_at_ms: i64,
    client_id: String,
    scope: String,
    source: GrokBuildCredentialSource,
}

impl GrokBuildCredential {
    /// Imports one strict OAuth JSON object at the supplied Unix-millisecond observation time.
    ///
    /// The importer accepts only the documented token fields, rejects duplicate names at every JSON
    /// nesting level, and accepts exactly one unambiguous `expires_in` lifetime. `id_token` is
    /// accepted as a compatibility field but is discarded without inspecting any claim.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthError`] for malformed, oversized, ambiguous, or unsafe input.
    pub fn import_json(input: &[u8], observed_at_ms: i64) -> Result<Self, GrokBuildOAuthError> {
        let object = parse_strict_json_object(input)?;
        credential_from_object(
            &object,
            observed_at_ms,
            GrokBuildCredentialSource::ImportedJson,
            None,
            None,
        )
    }

    /// Imports one CPA xAI OAuth auth file without writing, renaming, or exporting it.
    ///
    /// CPA stores the absolute expiry as `expired`, while retaining the original relative
    /// `expires_in` value as non-authoritative metadata. This importer derives the P6 credential's
    /// exact absolute expiry from the former and only accepts the known `type=xai` OAuth shape.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthError`] when the source is not a known CPA xAI OAuth shape, is
    /// expired, or contains malformed/unsafe credential fields.
    pub fn import_cpa_xai_auth_file(
        input: &[u8],
        observed_at_ms: i64,
    ) -> Result<Self, GrokBuildOAuthError> {
        let object = parse_strict_json_object(input)?;
        if required_nonsecret_field(&object, "type")? != "xai" {
            return Err(GrokBuildOAuthError::InvalidField);
        }
        if let Some(auth_kind) = optional_nonsecret_field(&object, "auth_kind")?
            && auth_kind != "oauth"
        {
            return Err(GrokBuildOAuthError::InvalidField);
        }
        validate_optional_issuer(&object)?;
        credential_from_absolute_expiry_object(
            &object,
            "access_token",
            "expired",
            observed_at_ms,
            GrokBuildCredentialSource::CpaXaiAuthFile,
        )
    }

    /// Imports a Grok account OAuth credential object with an absolute `expires_at` timestamp.
    ///
    /// This covers the clean-room-compatible account credential shape used by grok2api and
    /// `Sub2API`. The input is data only: callers retain ownership of any file or database access.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthError`] when the source is malformed, expired, has another public
    /// client identity, or contains unsafe credential fields.
    pub fn import_grok_account_json(
        input: &[u8],
        observed_at_ms: i64,
    ) -> Result<Self, GrokBuildOAuthError> {
        let object = parse_strict_json_object(input)?;
        validate_optional_issuer(&object)?;
        credential_from_absolute_expiry_object(
            &object,
            "access_token",
            "expires_at",
            observed_at_ms,
            GrokBuildCredentialSource::GrokAccountJson,
        )
    }

    /// Imports the one expected official Grok CLI OAuth cache entry in memory.
    ///
    /// The official CLI stores its Bearer access token as `key` under an indexed
    /// `issuer::client_id` object. This method accepts only the fixed Grok Build issuer and public
    /// client identity; it never opens a path, writes the cache, or exposes its values.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthError`] when the expected indexed entry is absent, malformed,
    /// expired, or belongs to a different issuer/client identity.
    pub fn import_official_cli_auth_cache(
        input: &[u8],
        observed_at_ms: i64,
    ) -> Result<Self, GrokBuildOAuthError> {
        let cache = parse_strict_json_object(input)?;
        let cache_key = format!("{GROK_BUILD_OAUTH_ISSUER}::{GROK_BUILD_PUBLIC_CLIENT_ID}");
        let entry = match cache.get(&cache_key) {
            Some(entry) => entry.as_object().ok_or(GrokBuildOAuthError::InvalidField)?,
            None => match cache_identity_mismatch(&cache) {
                Some(error) => return Err(error),
                None => return Err(GrokBuildOAuthError::MissingField),
            },
        };
        validate_optional_issuer(entry)?;
        credential_from_absolute_expiry_object(
            entry,
            "key",
            "expires_at",
            observed_at_ms,
            GrokBuildCredentialSource::OfficialCliAuthCache,
        )
    }

    /// Borrows the access token solely for an immediate Authorization header or local token flow.
    #[must_use]
    pub fn access_token(&self) -> &str {
        self.access_token.as_str()
    }

    /// Borrows the refresh token solely for an immediate OAuth refresh grant.
    #[must_use]
    pub fn refresh_token(&self) -> &str {
        self.refresh_token.as_str()
    }

    /// Returns the exact Unix-millisecond instant at which this access token is no longer usable.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }

    /// Returns whether the credential is expired at the supplied Unix-millisecond instant.
    #[must_use]
    pub const fn is_expired_at(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }

    /// Returns the non-secret OAuth client identifier associated with this credential.
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Returns the validated OAuth scope string associated with this credential.
    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// Returns the credential's acquisition origin.
    #[must_use]
    pub const fn source(&self) -> GrokBuildCredentialSource {
        self.source
    }

    pub(crate) fn persisted_bytes(&self) -> Result<Zeroizing<Vec<u8>>, GrokBuildOAuthError> {
        validate_secret_field(self.access_token())?;
        validate_secret_field(self.refresh_token())?;
        validate_nonsecret_field(&self.client_id)?;
        validate_scope(&self.scope)?;

        let mut output = Zeroizing::new(Vec::new());
        output.push(PERSISTED_CREDENTIAL_FORMAT_VERSION);
        output.push(match self.source {
            GrokBuildCredentialSource::ImportedJson => 0,
            GrokBuildCredentialSource::DeviceCode => 1,
            GrokBuildCredentialSource::Refresh => 2,
            GrokBuildCredentialSource::CpaXaiAuthFile => 3,
            GrokBuildCredentialSource::GrokAccountJson => 4,
            GrokBuildCredentialSource::OfficialCliAuthCache => 5,
        });
        output.extend_from_slice(&self.expires_at_ms.to_be_bytes());
        write_persisted_segment(&mut output, self.access_token())?;
        write_persisted_segment(&mut output, self.refresh_token())?;
        write_persisted_segment(&mut output, &self.client_id)?;
        write_persisted_segment(&mut output, &self.scope)?;
        Ok(output)
    }

    pub(crate) fn from_persisted_bytes(input: &[u8]) -> Result<Self, GrokBuildOAuthError> {
        if input.len() > MAX_OAUTH_JSON_BYTES {
            return Err(GrokBuildOAuthError::InvalidPersistedCredential);
        }
        let mut cursor = 0;
        let format_version = read_persisted_byte(input, &mut cursor)?;
        if format_version != PERSISTED_CREDENTIAL_FORMAT_VERSION {
            return Err(GrokBuildOAuthError::InvalidPersistedCredential);
        }
        let source = match read_persisted_byte(input, &mut cursor)? {
            0 => GrokBuildCredentialSource::ImportedJson,
            1 => GrokBuildCredentialSource::DeviceCode,
            2 => GrokBuildCredentialSource::Refresh,
            3 => GrokBuildCredentialSource::CpaXaiAuthFile,
            4 => GrokBuildCredentialSource::GrokAccountJson,
            5 => GrokBuildCredentialSource::OfficialCliAuthCache,
            _ => return Err(GrokBuildOAuthError::InvalidPersistedCredential),
        };
        let expires_at_ms = i64::from_be_bytes(
            read_persisted_bytes(input, &mut cursor, std::mem::size_of::<i64>())?
                .try_into()
                .map_err(|_| GrokBuildOAuthError::InvalidPersistedCredential)?,
        );
        let access = read_persisted_segment(input, &mut cursor)?;
        let refresh = read_persisted_segment(input, &mut cursor)?;
        let client_id = read_persisted_segment(input, &mut cursor)?;
        let scope = read_persisted_segment(input, &mut cursor)?;
        if cursor != input.len() {
            return Err(GrokBuildOAuthError::InvalidPersistedCredential);
        }
        validate_secret_field(access)?;
        validate_secret_field(refresh)?;
        validate_nonsecret_field(client_id)?;
        validate_scope(scope)?;

        Ok(Self {
            access_token: Zeroizing::new(access.to_owned()),
            refresh_token: Zeroizing::new(refresh.to_owned()),
            expires_at_ms,
            client_id: client_id.to_owned(),
            scope: scope.to_owned(),
            source,
        })
    }
}

impl fmt::Debug for GrokBuildCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildCredential")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("client_id", &self.client_id)
            .field("scope", &self.scope)
            .field("source", &self.source)
            .finish()
    }
}

/// Fixed Grok Build OAuth endpoint selected for one local mock transport request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildOAuthEndpoint {
    /// Starts the Device Authorization Grant.
    DeviceAuthorization,
    /// Polls or refreshes an OAuth token.
    Token,
}

impl GrokBuildOAuthEndpoint {
    /// Returns the fixed HTTPS endpoint URL.
    #[must_use]
    pub const fn url(self) -> &'static str {
        match self {
            Self::DeviceAuthorization => GROK_BUILD_DEVICE_AUTHORIZATION_URL,
            Self::Token => GROK_BUILD_TOKEN_URL,
        }
    }
}

/// Public, non-secret request kind observable by an OAuth transport mock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildOAuthRequestKind {
    /// Form request that starts the Device Authorization Grant.
    DeviceAuthorization,
    /// Form request that polls a device code for a token.
    DevicePoll,
    /// Form request that exchanges a refresh token for a new token.
    Refresh,
}

/// One OAuth form request containing potentially secret request fields.
///
/// Its private payload prevents external mock implementations from destructuring token, refresh,
/// or device-code fields. The only public observations are [`Self::endpoint`] and [`Self::kind`].
pub struct GrokBuildOAuthRequest {
    kind: GrokBuildOAuthRequestKind,
    payload: GrokBuildOAuthRequestPayload,
}

enum GrokBuildOAuthRequestPayload {
    DeviceAuthorization {
        client_id: String,
        scope: String,
    },
    DevicePoll {
        client_id: String,
        device_code: Zeroizing<String>,
    },
    Refresh {
        client_id: String,
        refresh_token: Zeroizing<String>,
    },
}

impl GrokBuildOAuthRequest {
    /// Returns the fixed endpoint selected by this request.
    #[must_use]
    pub const fn endpoint(&self) -> GrokBuildOAuthEndpoint {
        match self.kind {
            GrokBuildOAuthRequestKind::DeviceAuthorization => {
                GrokBuildOAuthEndpoint::DeviceAuthorization
            }
            GrokBuildOAuthRequestKind::DevicePoll | GrokBuildOAuthRequestKind::Refresh => {
                GrokBuildOAuthEndpoint::Token
            }
        }
    }

    /// Returns the non-secret form operation kind.
    #[must_use]
    pub const fn kind(&self) -> GrokBuildOAuthRequestKind {
        self.kind
    }

    fn device_authorization(client_id: String, scope: String) -> Self {
        Self {
            kind: GrokBuildOAuthRequestKind::DeviceAuthorization,
            payload: GrokBuildOAuthRequestPayload::DeviceAuthorization { client_id, scope },
        }
    }

    fn device_poll(client_id: String, device_code: Zeroizing<String>) -> Self {
        Self {
            kind: GrokBuildOAuthRequestKind::DevicePoll,
            payload: GrokBuildOAuthRequestPayload::DevicePoll {
                client_id,
                device_code,
            },
        }
    }

    fn refresh(client_id: String, refresh_token: Zeroizing<String>) -> Self {
        Self {
            kind: GrokBuildOAuthRequestKind::Refresh,
            payload: GrokBuildOAuthRequestPayload::Refresh {
                client_id,
                refresh_token,
            },
        }
    }

    #[allow(
        dead_code,
        reason = "P6-03 owns the concrete HTTP transport that consumes this secret form body"
    )]
    pub(crate) fn into_form_body(self) -> Zeroizing<Vec<u8>> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        match self.payload {
            GrokBuildOAuthRequestPayload::DeviceAuthorization { client_id, scope } => {
                serializer.append_pair("client_id", &client_id);
                serializer.append_pair("scope", &scope);
            }
            GrokBuildOAuthRequestPayload::DevicePoll {
                client_id,
                device_code,
            } => {
                serializer.append_pair("client_id", &client_id);
                serializer
                    .append_pair("grant_type", "urn:ietf:params:oauth:grant-type:device_code");
                serializer.append_pair("device_code", device_code.as_str());
            }
            GrokBuildOAuthRequestPayload::Refresh {
                client_id,
                refresh_token,
            } => {
                serializer.append_pair("client_id", &client_id);
                serializer.append_pair("grant_type", "refresh_token");
                serializer.append_pair("refresh_token", refresh_token.as_str());
            }
        }
        Zeroizing::new(serializer.finish().into_bytes())
    }
}

impl fmt::Debug for GrokBuildOAuthRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildOAuthRequest")
            .field("endpoint", &self.endpoint())
            .field("kind", &self.kind())
            .field("form_fields", &"<redacted>")
            .finish()
    }
}

/// Bounded raw OAuth HTTP response delivered by a mockable transport boundary.
pub struct GrokBuildOAuthHttpResponse {
    status: u16,
    body: Vec<u8>,
}

impl GrokBuildOAuthHttpResponse {
    /// Validates one bounded response status and body without decoding it.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthError::InvalidHttpResponse`] for a non-HTTP status or a body over
    /// the fixed OAuth parsing limit.
    pub fn try_new(status: u16, body: impl Into<Vec<u8>>) -> Result<Self, GrokBuildOAuthError> {
        let body = body.into();
        if !(100..=599).contains(&status) || body.len() > MAX_OAUTH_JSON_BYTES {
            return Err(GrokBuildOAuthError::InvalidHttpResponse);
        }
        Ok(Self { status, body })
    }

    /// Returns the transport status code.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    fn body(&self) -> &[u8] {
        &self.body
    }
}

impl fmt::Debug for GrokBuildOAuthHttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildOAuthHttpResponse")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Safe failure returned by the injectible OAuth transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildOAuthTransportError {
    /// The injected transport could not complete the local exchange.
    Unavailable,
}

impl fmt::Display for GrokBuildOAuthTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Grok Build OAuth transport is unavailable")
    }
}

impl Error for GrokBuildOAuthTransportError {}

/// Mockable, synchronous OAuth transport seam.
///
/// P6-01 deliberately leaves real socket/TLS/egress behavior outside this interface. Later
/// transport code must use the exact fixed endpoint and may consume the request's private form
/// encoder without adding a second OAuth protocol implementation.
pub trait GrokBuildOAuthTransport {
    /// Sends one OAuth form request and returns its bounded raw HTTP response.
    ///
    /// Implementations must not log the request or response body. The request is consumed so any
    /// secret form material cannot be accidentally reused after one exchange.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthTransportError`] when the local transport cannot complete the
    /// exchange without exposing a provider response body or credential value.
    fn send(
        &self,
        request: GrokBuildOAuthRequest,
    ) -> Result<GrokBuildOAuthHttpResponse, GrokBuildOAuthTransportError>;
}

/// Validated Device Authorization details displayed to a user or local UI.
pub struct GrokBuildDeviceAuthorization {
    device_code: Zeroizing<String>,
    user_code: Zeroizing<String>,
    client_id: String,
    scope: String,
    verification_uri: Url,
    verification_uri_complete: Option<Url>,
    interval_seconds: u64,
    issued_at_ms: i64,
    expires_at_ms: i64,
}

impl GrokBuildDeviceAuthorization {
    /// Returns the one-time user code for a local user-facing authorization prompt.
    #[must_use]
    pub fn user_code(&self) -> &str {
        self.user_code.as_str()
    }

    /// Returns the fixed verification URI without a code embedded in its query.
    #[must_use]
    pub fn verification_uri(&self) -> &Url {
        &self.verification_uri
    }

    /// Returns the optional verification URI that embeds the one-time user code.
    #[must_use]
    pub fn verification_uri_complete(&self) -> Option<&Url> {
        self.verification_uri_complete.as_ref()
    }

    /// Returns the current minimum polling interval in seconds.
    #[must_use]
    pub const fn interval_seconds(&self) -> u64 {
        self.interval_seconds
    }

    /// Returns the authorization deadline as a Unix-millisecond instant.
    #[must_use]
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
}

impl fmt::Debug for GrokBuildDeviceAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildDeviceAuthorization")
            .field("device_code", &"<redacted>")
            .field("user_code", &"<redacted>")
            .field("client_id", &self.client_id)
            .field("scope", &self.scope)
            .field("verification_uri", &"<redacted>")
            .field("verification_uri_complete", &"<redacted>")
            .field("interval_seconds", &self.interval_seconds)
            .field("issued_at_ms", &self.issued_at_ms)
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

/// State of a Device Authorization poll after one token response.
#[derive(Debug)]
pub enum GrokBuildDevicePollOutcome {
    /// The user has not yet authorized the device; wait until this instant before polling again.
    Pending {
        /// Earliest permitted next poll as a Unix-millisecond instant.
        retry_at_ms: i64,
    },
    /// The authorization server requested a longer interval; wait until this instant.
    SlowDown {
        /// Earliest permitted next poll as a Unix-millisecond instant.
        retry_at_ms: i64,
    },
    /// The server granted a new OAuth credential and the poller becomes terminal.
    Granted(GrokBuildCredential),
    /// The user denied the Device Authorization request and the poller becomes terminal.
    Denied,
    /// The Device Authorization code expired and the poller becomes terminal.
    Expired,
}

/// Stateful local Device Authorization poller.
pub struct GrokBuildDevicePoller {
    authorization: GrokBuildDeviceAuthorization,
    next_poll_at_ms: i64,
    completed: bool,
}

impl GrokBuildDevicePoller {
    /// Creates a poller that honors the initial server-supplied poll interval.
    #[must_use]
    pub fn new(authorization: GrokBuildDeviceAuthorization) -> Self {
        let next_poll_at_ms = authorization
            .issued_at_ms
            .saturating_add(seconds_to_millis(authorization.interval_seconds));
        Self {
            authorization,
            next_poll_at_ms,
            completed: false,
        }
    }

    /// Returns the earliest permitted next poll as a Unix-millisecond instant.
    #[must_use]
    pub const fn next_poll_at_ms(&self) -> i64 {
        self.next_poll_at_ms
    }

    /// Sends one allowed Device Authorization poll through the injected transport.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthError`] when the code is terminal, the caller polls too early, the
    /// transport fails, or the server returns a malformed OAuth response. A valid pending, denied,
    /// expired, or granted response is returned as an explicit outcome rather than an error.
    pub fn poll<T: GrokBuildOAuthTransport>(
        &mut self,
        transport: &T,
        now_ms: i64,
    ) -> Result<GrokBuildDevicePollOutcome, GrokBuildOAuthError> {
        if self.completed {
            return Err(GrokBuildOAuthError::DeviceFlowCompleted);
        }
        if now_ms >= self.authorization.expires_at_ms {
            self.completed = true;
            return Ok(GrokBuildDevicePollOutcome::Expired);
        }
        if now_ms < self.next_poll_at_ms {
            return Err(GrokBuildOAuthError::PollingTooSoon);
        }

        let response = transport
            .send(GrokBuildOAuthRequest::device_poll(
                self.authorization.client_id.clone(),
                self.authorization.device_code.clone(),
            ))
            .map_err(|_| GrokBuildOAuthError::TransportUnavailable)?;

        match parse_token_result(
            &response,
            now_ms,
            GrokBuildCredentialSource::DeviceCode,
            Some(&self.authorization.client_id),
            Some(&self.authorization.scope),
        )? {
            TokenResult::Granted(credential) => {
                self.completed = true;
                Ok(GrokBuildDevicePollOutcome::Granted(credential))
            }
            TokenResult::Pending => {
                self.schedule_after(now_ms, self.authorization.interval_seconds)?;
                Ok(GrokBuildDevicePollOutcome::Pending {
                    retry_at_ms: self.next_poll_at_ms,
                })
            }
            TokenResult::SlowDown => {
                self.authorization.interval_seconds = self
                    .authorization
                    .interval_seconds
                    .checked_add(DEVICE_SLOW_DOWN_SECONDS)
                    .ok_or(GrokBuildOAuthError::InvalidTimestamp)?;
                self.schedule_after(now_ms, self.authorization.interval_seconds)?;
                Ok(GrokBuildDevicePollOutcome::SlowDown {
                    retry_at_ms: self.next_poll_at_ms,
                })
            }
            TokenResult::Denied => {
                self.completed = true;
                Ok(GrokBuildDevicePollOutcome::Denied)
            }
            TokenResult::Expired => {
                self.completed = true;
                Ok(GrokBuildDevicePollOutcome::Expired)
            }
        }
    }

    fn schedule_after(&mut self, now_ms: i64, seconds: u64) -> Result<(), GrokBuildOAuthError> {
        self.next_poll_at_ms = now_ms
            .checked_add(seconds_to_millis_checked(seconds)?)
            .ok_or(GrokBuildOAuthError::InvalidTimestamp)?;
        Ok(())
    }
}

impl fmt::Debug for GrokBuildDevicePoller {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrokBuildDevicePoller")
            .field("authorization", &self.authorization)
            .field("next_poll_at_ms", &self.next_poll_at_ms)
            .field("completed", &self.completed)
            .finish()
    }
}

/// OAuth flow builder using one explicit public client id and scope set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrokBuildOAuthFlow {
    client_id: String,
    scope: String,
}

impl GrokBuildOAuthFlow {
    /// Creates an OAuth flow with explicitly validated non-secret client metadata.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthError::InvalidField`] for empty, unsafe, or oversized values.
    pub fn try_new(
        client_id: impl Into<String>,
        scope: impl Into<String>,
    ) -> Result<Self, GrokBuildOAuthError> {
        let client_id = client_id.into();
        let scope = scope.into();
        validate_nonsecret_field(&client_id)?;
        validate_scope(&scope)?;
        Ok(Self { client_id, scope })
    }

    /// Starts Device Authorization with an injected local transport and parses its response.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthError`] for a failed local transport, non-success response, or an
    /// unsafe/malformed Device Authorization payload.
    pub fn start_device_authorization<T: GrokBuildOAuthTransport>(
        &self,
        transport: &T,
        now_ms: i64,
    ) -> Result<GrokBuildDeviceAuthorization, GrokBuildOAuthError> {
        let response = transport
            .send(GrokBuildOAuthRequest::device_authorization(
                self.client_id.clone(),
                self.scope.clone(),
            ))
            .map_err(|_| GrokBuildOAuthError::TransportUnavailable)?;
        parse_device_authorization_response(&response, now_ms, &self.client_id, &self.scope)
    }

    /// Refreshes one matching credential through the injected OAuth transport.
    ///
    /// This pure P6-01 operation has no concurrency coordination or persistence side effect; P6-02
    /// owns per-credential singleflight, revision comparison, and durable commit.
    ///
    /// # Errors
    ///
    /// Returns [`GrokBuildOAuthError`] for a mismatched client id, failed local transport, or an
    /// invalid refresh response.
    pub fn refresh<T: GrokBuildOAuthTransport>(
        &self,
        transport: &T,
        credential: &GrokBuildCredential,
        now_ms: i64,
    ) -> Result<GrokBuildCredential, GrokBuildOAuthError> {
        if credential.client_id() != self.client_id {
            return Err(GrokBuildOAuthError::CredentialClientMismatch);
        }
        let response = transport
            .send(GrokBuildOAuthRequest::refresh(
                self.client_id.clone(),
                Zeroizing::new(credential.refresh_token().to_owned()),
            ))
            .map_err(|_| GrokBuildOAuthError::TransportUnavailable)?;
        match parse_token_result(
            &response,
            now_ms,
            GrokBuildCredentialSource::Refresh,
            Some(&self.client_id),
            Some(&credential.scope),
        )? {
            TokenResult::Granted(credential) => Ok(credential),
            TokenResult::Pending
            | TokenResult::SlowDown
            | TokenResult::Denied
            | TokenResult::Expired => Err(GrokBuildOAuthError::InvalidTokenResponse),
        }
    }

    /// Returns this flow's public OAuth client identifier.
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Returns this flow's validated scope string.
    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }
}

impl Default for GrokBuildOAuthFlow {
    fn default() -> Self {
        Self {
            client_id: GROK_BUILD_PUBLIC_CLIENT_ID.to_owned(),
            scope: GROK_BUILD_OAUTH_SCOPE.to_owned(),
        }
    }
}

/// Secret-free failure classification for OAuth import and Device Authorization handling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokBuildOAuthError {
    /// A supplied JSON or response body exceeded the fixed bounded parser limit.
    InputTooLarge,
    /// The input was not one strict JSON object without duplicate fields.
    InvalidJson,
    /// The OAuth object carried an unsupported or unknown field.
    UnexpectedField,
    /// A required OAuth field was absent.
    MissingField,
    /// A field was present but empty, unsafe, oversized, or of the wrong JSON type.
    InvalidField,
    /// More than one expiry representation was supplied or no precise lifetime was available.
    AmbiguousExpiration,
    /// The provided `token_type` was not the accepted bearer type.
    UnsupportedTokenType,
    /// A timestamp or duration cannot be represented safely as Unix milliseconds.
    InvalidTimestamp,
    /// An authenticated persisted credential did not match the bounded binary format.
    InvalidPersistedCredential,
    /// The transport response had an invalid HTTP status or exceeded its body bound.
    InvalidHttpResponse,
    /// A non-success Device Authorization response had no supported OAuth failure shape.
    InvalidDeviceAuthorizationResponse,
    /// A token response had no supported OAuth success or failure shape.
    InvalidTokenResponse,
    /// A caller attempted to poll before the server-authorized interval elapsed.
    PollingTooSoon,
    /// A caller tried to poll after a terminal Device Authorization result.
    DeviceFlowCompleted,
    /// A credential belongs to a different public OAuth client id than this flow.
    CredentialClientMismatch,
    /// A credential cache entry belongs to a different OAuth issuer than this flow.
    CredentialIssuerMismatch,
    /// The injected local OAuth transport was unavailable.
    TransportUnavailable,
}

impl fmt::Display for GrokBuildOAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InputTooLarge => "Grok Build OAuth input exceeds the bounded limit",
            Self::InvalidJson => "Grok Build OAuth input is not strict JSON",
            Self::UnexpectedField => "Grok Build OAuth input has an unsupported field",
            Self::MissingField => "Grok Build OAuth input is missing a required field",
            Self::InvalidField => "Grok Build OAuth input has an invalid field",
            Self::AmbiguousExpiration => "Grok Build OAuth expiry is ambiguous",
            Self::UnsupportedTokenType => "Grok Build OAuth token type is unsupported",
            Self::InvalidTimestamp => "Grok Build OAuth timestamp is invalid",
            Self::InvalidPersistedCredential => "Grok Build persisted credential is invalid",
            Self::InvalidHttpResponse => "Grok Build OAuth HTTP response is invalid",
            Self::InvalidDeviceAuthorizationResponse => {
                "Grok Build Device Authorization response is invalid"
            }
            Self::InvalidTokenResponse => "Grok Build OAuth token response is invalid",
            Self::PollingTooSoon => "Grok Build Device Authorization was polled too soon",
            Self::DeviceFlowCompleted => "Grok Build Device Authorization is already terminal",
            Self::CredentialClientMismatch => {
                "Grok Build credential belongs to another OAuth client"
            }
            Self::CredentialIssuerMismatch => {
                "Grok Build credential belongs to another OAuth issuer"
            }
            Self::TransportUnavailable => "Grok Build OAuth transport is unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for GrokBuildOAuthError {}

enum TokenResult {
    Granted(GrokBuildCredential),
    Pending,
    SlowDown,
    Denied,
    Expired,
}

fn parse_device_authorization_response(
    response: &GrokBuildOAuthHttpResponse,
    now_ms: i64,
    client_id: &str,
    scope: &str,
) -> Result<GrokBuildDeviceAuthorization, GrokBuildOAuthError> {
    if !(200..=299).contains(&response.status()) {
        return Err(GrokBuildOAuthError::InvalidDeviceAuthorizationResponse);
    }
    let object = parse_strict_json_object(response.body())?;
    ensure_known_fields(
        &object,
        &[
            "device_code",
            "user_code",
            "verification_uri",
            "verification_uri_complete",
            "expires_in",
            "interval",
        ],
    )?;
    let device_code = Zeroizing::new(required_secret_field(&object, "device_code")?.to_owned());
    let user_code = Zeroizing::new(required_secret_field(&object, "user_code")?.to_owned());
    let verification_uri = parse_https_url(required_nonsecret_field(&object, "verification_uri")?)?;
    let verification_uri_complete = object
        .get("verification_uri_complete")
        .map(optional_https_url)
        .transpose()?;
    let expires_in = required_positive_seconds(&object, "expires_in")?;
    let interval_seconds = match object.get("interval") {
        Some(value) => positive_seconds(value)?,
        None => DEFAULT_DEVICE_POLL_INTERVAL_SECONDS,
    };
    let expires_at_ms = now_ms
        .checked_add(seconds_to_millis_checked(expires_in)?)
        .ok_or(GrokBuildOAuthError::InvalidTimestamp)?;

    Ok(GrokBuildDeviceAuthorization {
        device_code,
        user_code,
        client_id: client_id.to_owned(),
        scope: scope.to_owned(),
        verification_uri,
        verification_uri_complete,
        interval_seconds,
        issued_at_ms: now_ms,
        expires_at_ms,
    })
}

fn parse_token_result(
    response: &GrokBuildOAuthHttpResponse,
    observed_at_ms: i64,
    source: GrokBuildCredentialSource,
    client_id: Option<&str>,
    scope: Option<&str>,
) -> Result<TokenResult, GrokBuildOAuthError> {
    let object = parse_strict_json_object(response.body())?;
    if (200..=299).contains(&response.status()) {
        return credential_from_object(&object, observed_at_ms, source, client_id, scope)
            .map(TokenResult::Granted);
    }

    ensure_known_fields(&object, &["error", "error_description", "error_uri"])?;
    let error = required_nonsecret_field(&object, "error")?;
    match error {
        "authorization_pending" => Ok(TokenResult::Pending),
        "slow_down" => Ok(TokenResult::SlowDown),
        "access_denied" => Ok(TokenResult::Denied),
        "expired_token" => Ok(TokenResult::Expired),
        _ => Err(GrokBuildOAuthError::InvalidTokenResponse),
    }
}

fn credential_from_absolute_expiry_object(
    object: &BTreeMap<String, StrictJsonValue>,
    access_token_field: &str,
    expiry_field: &str,
    observed_at_ms: i64,
    source: GrokBuildCredentialSource,
) -> Result<GrokBuildCredential, GrokBuildOAuthError> {
    ensure_unambiguous_absolute_expiry(object, expiry_field)?;
    let access_token = required_secret_field(object, access_token_field)?;
    let refresh_token = required_secret_field(object, "refresh_token")?;
    let expires_at_ms = required_rfc3339_expiry_at_ms(object, expiry_field)?;
    validate_absolute_expiry(expires_at_ms, observed_at_ms)?;

    if let Some(token_type) = optional_nonsecret_field(object, "token_type")?
        && !token_type.eq_ignore_ascii_case("bearer")
    {
        return Err(GrokBuildOAuthError::UnsupportedTokenType);
    }
    if let Some(id_token) = optional_secret_field(object, "id_token")? {
        let _discarded_id_token = Zeroizing::new(id_token.to_owned());
    }

    let client_id =
        optional_nonsecret_field(object, "client_id")?.unwrap_or(GROK_BUILD_PUBLIC_CLIENT_ID);
    if client_id != GROK_BUILD_PUBLIC_CLIENT_ID {
        return Err(GrokBuildOAuthError::CredentialClientMismatch);
    }
    let scope = optional_scope_field(object, "scope")?.unwrap_or(GROK_BUILD_OAUTH_SCOPE);
    if scope != GROK_BUILD_OAUTH_SCOPE {
        return Err(GrokBuildOAuthError::InvalidTokenResponse);
    }

    Ok(GrokBuildCredential {
        access_token: Zeroizing::new(access_token.to_owned()),
        refresh_token: Zeroizing::new(refresh_token.to_owned()),
        expires_at_ms,
        client_id: client_id.to_owned(),
        scope: scope.to_owned(),
        source,
    })
}

fn cache_identity_mismatch(
    cache: &BTreeMap<String, StrictJsonValue>,
) -> Option<GrokBuildOAuthError> {
    if cache.keys().any(|key| {
        key.rsplit_once("::").is_some_and(|(issuer, client_id)| {
            issuer == GROK_BUILD_OAUTH_ISSUER && client_id != GROK_BUILD_PUBLIC_CLIENT_ID
        })
    }) {
        return Some(GrokBuildOAuthError::CredentialClientMismatch);
    }
    if cache.keys().any(|key| {
        key.rsplit_once("::").is_some_and(|(issuer, client_id)| {
            client_id == GROK_BUILD_PUBLIC_CLIENT_ID && issuer != GROK_BUILD_OAUTH_ISSUER
        })
    }) {
        return Some(GrokBuildOAuthError::CredentialIssuerMismatch);
    }
    None
}

fn ensure_unambiguous_absolute_expiry(
    object: &BTreeMap<String, StrictJsonValue>,
    expected_field: &str,
) -> Result<(), GrokBuildOAuthError> {
    if ABSOLUTE_EXPIRY_FIELD_ALIASES
        .iter()
        .any(|field| *field != expected_field && object.contains_key(*field))
    {
        return Err(GrokBuildOAuthError::AmbiguousExpiration);
    }
    Ok(())
}

fn validate_optional_issuer(
    object: &BTreeMap<String, StrictJsonValue>,
) -> Result<(), GrokBuildOAuthError> {
    if let Some(issuer) = optional_nonsecret_field(object, "issuer")?
        && issuer != GROK_BUILD_OAUTH_ISSUER
    {
        return Err(GrokBuildOAuthError::CredentialIssuerMismatch);
    }
    Ok(())
}

fn required_rfc3339_expiry_at_ms(
    object: &BTreeMap<String, StrictJsonValue>,
    field: &str,
) -> Result<i64, GrokBuildOAuthError> {
    let value = required_nonsecret_field(object, field)?;
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| GrokBuildOAuthError::AmbiguousExpiration)?;
    let seconds = parsed.unix_timestamp();
    if seconds < 0 {
        return Err(GrokBuildOAuthError::InvalidTimestamp);
    }
    let milliseconds = seconds
        .checked_mul(1_000)
        .and_then(|milliseconds| {
            milliseconds.checked_add(i64::from(parsed.nanosecond() / 1_000_000))
        })
        .ok_or(GrokBuildOAuthError::InvalidTimestamp)?;
    Ok(milliseconds)
}

fn validate_absolute_expiry(
    expires_at_ms: i64,
    observed_at_ms: i64,
) -> Result<(), GrokBuildOAuthError> {
    let remaining_ms = expires_at_ms
        .checked_sub(observed_at_ms)
        .ok_or(GrokBuildOAuthError::InvalidTimestamp)?;
    let maximum_ms = seconds_to_millis_checked(MAX_TOKEN_LIFETIME_SECONDS)?;
    if remaining_ms <= 0 || remaining_ms > maximum_ms {
        return Err(GrokBuildOAuthError::AmbiguousExpiration);
    }
    Ok(())
}

fn credential_from_object(
    object: &BTreeMap<String, StrictJsonValue>,
    observed_at_ms: i64,
    source: GrokBuildCredentialSource,
    required_client_id: Option<&str>,
    required_scope: Option<&str>,
) -> Result<GrokBuildCredential, GrokBuildOAuthError> {
    if ["expires_at", "expires_at_ms", "expiration", "expires"]
        .iter()
        .any(|field| object.contains_key(*field))
    {
        return Err(GrokBuildOAuthError::AmbiguousExpiration);
    }
    ensure_known_fields(
        object,
        &[
            "access_token",
            "refresh_token",
            "expires_in",
            "token_type",
            "client_id",
            "scope",
            "id_token",
        ],
    )?;
    let access_token = Zeroizing::new(required_secret_field(object, "access_token")?.to_owned());
    let refresh_token = Zeroizing::new(required_secret_field(object, "refresh_token")?.to_owned());
    let expires_in = required_positive_seconds(object, "expires_in")?;
    let expires_at_ms = observed_at_ms
        .checked_add(seconds_to_millis_checked(expires_in)?)
        .ok_or(GrokBuildOAuthError::InvalidTimestamp)?;

    if let Some(token_type) = optional_nonsecret_field(object, "token_type")?
        && !token_type.eq_ignore_ascii_case("bearer")
    {
        return Err(GrokBuildOAuthError::UnsupportedTokenType);
    }
    if let Some(id_token) = optional_secret_field(object, "id_token")? {
        let _discarded_id_token = Zeroizing::new(id_token.to_owned());
    }

    let client_id = optional_nonsecret_field(object, "client_id")?
        .or(required_client_id)
        .unwrap_or(GROK_BUILD_PUBLIC_CLIENT_ID)
        .to_owned();
    validate_nonsecret_field(&client_id)?;
    if required_client_id.is_some_and(|expected| expected != client_id) {
        return Err(GrokBuildOAuthError::CredentialClientMismatch);
    }
    let scope = optional_scope_field(object, "scope")?
        .or(required_scope)
        .unwrap_or(GROK_BUILD_OAUTH_SCOPE)
        .to_owned();
    validate_scope(&scope)?;
    if required_scope.is_some_and(|expected| expected != scope) {
        return Err(GrokBuildOAuthError::InvalidTokenResponse);
    }

    Ok(GrokBuildCredential {
        access_token,
        refresh_token,
        expires_at_ms,
        client_id,
        scope,
        source,
    })
}

fn ensure_known_fields(
    object: &BTreeMap<String, StrictJsonValue>,
    known_fields: &[&str],
) -> Result<(), GrokBuildOAuthError> {
    if object
        .keys()
        .any(|field| !known_fields.contains(&field.as_str()))
    {
        return Err(GrokBuildOAuthError::UnexpectedField);
    }
    Ok(())
}

fn required_secret_field<'a>(
    object: &'a BTreeMap<String, StrictJsonValue>,
    field: &str,
) -> Result<&'a str, GrokBuildOAuthError> {
    let value = object.get(field).ok_or(GrokBuildOAuthError::MissingField)?;
    let value = value.as_str().ok_or(GrokBuildOAuthError::InvalidField)?;
    validate_secret_field(value)?;
    Ok(value)
}

fn optional_secret_field<'a>(
    object: &'a BTreeMap<String, StrictJsonValue>,
    field: &str,
) -> Result<Option<&'a str>, GrokBuildOAuthError> {
    object
        .get(field)
        .map(|value| {
            let value = value.as_str().ok_or(GrokBuildOAuthError::InvalidField)?;
            validate_secret_field(value)?;
            Ok(value)
        })
        .transpose()
}

fn required_nonsecret_field<'a>(
    object: &'a BTreeMap<String, StrictJsonValue>,
    field: &str,
) -> Result<&'a str, GrokBuildOAuthError> {
    let value = object.get(field).ok_or(GrokBuildOAuthError::MissingField)?;
    let value = value.as_str().ok_or(GrokBuildOAuthError::InvalidField)?;
    validate_nonsecret_field(value)?;
    Ok(value)
}

fn optional_nonsecret_field<'a>(
    object: &'a BTreeMap<String, StrictJsonValue>,
    field: &str,
) -> Result<Option<&'a str>, GrokBuildOAuthError> {
    object
        .get(field)
        .map(|value| {
            let value = value.as_str().ok_or(GrokBuildOAuthError::InvalidField)?;
            validate_nonsecret_field(value)?;
            Ok(value)
        })
        .transpose()
}

fn optional_scope_field<'a>(
    object: &'a BTreeMap<String, StrictJsonValue>,
    field: &str,
) -> Result<Option<&'a str>, GrokBuildOAuthError> {
    object
        .get(field)
        .map(|value| {
            let value = value.as_str().ok_or(GrokBuildOAuthError::InvalidField)?;
            validate_scope(value)?;
            Ok(value)
        })
        .transpose()
}

fn required_positive_seconds(
    object: &BTreeMap<String, StrictJsonValue>,
    field: &str,
) -> Result<u64, GrokBuildOAuthError> {
    object
        .get(field)
        .ok_or(GrokBuildOAuthError::MissingField)
        .and_then(positive_seconds)
}

fn positive_seconds(value: &StrictJsonValue) -> Result<u64, GrokBuildOAuthError> {
    let seconds = value
        .as_number()
        .and_then(Number::as_u64)
        .ok_or(GrokBuildOAuthError::AmbiguousExpiration)?;
    if seconds == 0 || seconds > MAX_TOKEN_LIFETIME_SECONDS {
        return Err(GrokBuildOAuthError::AmbiguousExpiration);
    }
    Ok(seconds)
}

fn validate_secret_field(value: &str) -> Result<(), GrokBuildOAuthError> {
    if value.is_empty()
        || value.len() > MAX_OAUTH_FIELD_BYTES
        || value.trim() != value
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(GrokBuildOAuthError::InvalidField);
    }
    Ok(())
}

fn validate_nonsecret_field(value: &str) -> Result<(), GrokBuildOAuthError> {
    if value.is_empty()
        || value.len() > MAX_OAUTH_FIELD_BYTES
        || value.trim() != value
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(GrokBuildOAuthError::InvalidField);
    }
    Ok(())
}

fn validate_scope(scope: &str) -> Result<(), GrokBuildOAuthError> {
    if scope.is_empty() || scope.len() > MAX_OAUTH_FIELD_BYTES || scope.trim() != scope {
        return Err(GrokBuildOAuthError::InvalidField);
    }
    if scope
        .split_ascii_whitespace()
        .any(|item| item.is_empty() || !item.bytes().all(|byte| byte.is_ascii_graphic()))
    {
        return Err(GrokBuildOAuthError::InvalidField);
    }
    Ok(())
}

fn parse_https_url(value: &str) -> Result<Url, GrokBuildOAuthError> {
    let url = Url::parse(value).map_err(|_| GrokBuildOAuthError::InvalidField)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(GrokBuildOAuthError::InvalidField);
    }
    Ok(url)
}

fn optional_https_url(value: &StrictJsonValue) -> Result<Url, GrokBuildOAuthError> {
    let value = value.as_str().ok_or(GrokBuildOAuthError::InvalidField)?;
    parse_https_url(value)
}

fn seconds_to_millis(seconds: u64) -> i64 {
    i64::try_from(seconds.saturating_mul(1_000)).unwrap_or(i64::MAX)
}

fn seconds_to_millis_checked(seconds: u64) -> Result<i64, GrokBuildOAuthError> {
    let milliseconds = seconds
        .checked_mul(1_000)
        .ok_or(GrokBuildOAuthError::InvalidTimestamp)?;
    i64::try_from(milliseconds).map_err(|_| GrokBuildOAuthError::InvalidTimestamp)
}

fn write_persisted_segment(output: &mut Vec<u8>, value: &str) -> Result<(), GrokBuildOAuthError> {
    let length =
        u32::try_from(value.len()).map_err(|_| GrokBuildOAuthError::InvalidPersistedCredential)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_persisted_byte(input: &[u8], cursor: &mut usize) -> Result<u8, GrokBuildOAuthError> {
    let byte = *input
        .get(*cursor)
        .ok_or(GrokBuildOAuthError::InvalidPersistedCredential)?;
    *cursor = cursor
        .checked_add(1)
        .ok_or(GrokBuildOAuthError::InvalidPersistedCredential)?;
    Ok(byte)
}

fn read_persisted_segment<'a>(
    input: &'a [u8],
    cursor: &mut usize,
) -> Result<&'a str, GrokBuildOAuthError> {
    let length = u32::from_be_bytes(
        read_persisted_bytes(input, cursor, std::mem::size_of::<u32>())?
            .try_into()
            .map_err(|_| GrokBuildOAuthError::InvalidPersistedCredential)?,
    );
    let length =
        usize::try_from(length).map_err(|_| GrokBuildOAuthError::InvalidPersistedCredential)?;
    if length > MAX_OAUTH_FIELD_BYTES {
        return Err(GrokBuildOAuthError::InvalidPersistedCredential);
    }
    let value = read_persisted_bytes(input, cursor, length)?;
    std::str::from_utf8(value).map_err(|_| GrokBuildOAuthError::InvalidPersistedCredential)
}

fn read_persisted_bytes<'a>(
    input: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], GrokBuildOAuthError> {
    let end = cursor
        .checked_add(length)
        .ok_or(GrokBuildOAuthError::InvalidPersistedCredential)?;
    let value = input
        .get(*cursor..end)
        .ok_or(GrokBuildOAuthError::InvalidPersistedCredential)?;
    *cursor = end;
    Ok(value)
}

fn parse_strict_json_object(
    input: &[u8],
) -> Result<BTreeMap<String, StrictJsonValue>, GrokBuildOAuthError> {
    if input.len() > MAX_OAUTH_JSON_BYTES {
        return Err(GrokBuildOAuthError::InputTooLarge);
    }
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let value = StrictJsonValue::deserialize(&mut deserializer)
        .map_err(|_| GrokBuildOAuthError::InvalidJson)?;
    deserializer
        .end()
        .map_err(|_| GrokBuildOAuthError::InvalidJson)?;
    match value {
        StrictJsonValue::Object(object) => Ok(object),
        _ => Err(GrokBuildOAuthError::InvalidJson),
    }
}

enum StrictJsonValue {
    Null,
    Bool,
    Number(Number),
    String(String),
    Array,
    Object(BTreeMap<String, StrictJsonValue>),
}

impl StrictJsonValue {
    fn as_object(&self) -> Option<&BTreeMap<String, StrictJsonValue>> {
        match self {
            Self::Object(value) => Some(value),
            Self::Null | Self::Bool | Self::Number(_) | Self::String(_) | Self::Array => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::Null | Self::Bool | Self::Number(_) | Self::Array | Self::Object(_) => None,
        }
    }

    fn as_number(&self) -> Option<&Number> {
        match self {
            Self::Number(value) => Some(value),
            Self::Null | Self::Bool | Self::String(_) | Self::Array | Self::Object(_) => None,
        }
    }
}

impl<'de> Deserialize<'de> for StrictJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictJsonVisitor)
    }
}

struct StrictJsonVisitor;

impl<'de> Visitor<'de> for StrictJsonVisitor {
    type Value = StrictJsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("strict JSON without duplicate object fields")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue::Bool)
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue::Number(Number::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(StrictJsonValue::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(StrictJsonValue::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<StrictJsonValue>()?.is_some() {}
        Ok(StrictJsonValue::Array)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON object field"));
            }
            let value = map.next_value()?;
            values.insert(key, value);
        }
        Ok(StrictJsonValue::Object(values))
    }
}
