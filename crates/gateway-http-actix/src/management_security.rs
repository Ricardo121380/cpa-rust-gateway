//! Fail-closed management HTTP admission before P10 CRUD or UI work.
//!
//! The only management credential accepted here is the separate `X-Management-Key` header.
//! Actual peer addresses, not forwarded headers, select the loopback/private admission policy.
//! Browser origins are denied by default; an explicitly configured same-origin UI also needs a
//! dedicated CSRF value for unsafe methods. This module stores no management data and sends no
//! Provider request.

use std::{
    fmt,
    future::{Ready, ready},
    net::IpAddr,
    task::{Context, Poll},
};

use actix_web::{
    Error, HttpMessage, HttpRequest, HttpResponse,
    body::{EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::{Method, header},
    web,
};
use futures_util::future::LocalBoxFuture;
use gateway_control::management_service::ManagementActor;
use subtle::ConstantTimeEq;
use url::Url;
use zeroize::Zeroizing;

/// Header reserved for the separate management credential namespace.
pub const MANAGEMENT_KEY_HEADER: &str = "x-management-key";
const CSRF_HEADER: &str = "x-management-csrf-token";
const MANAGEMENT_ACTOR_LABEL: &str = "management-key";
const MANAGEMENT_DENIED_BODY: &str =
    r#"{"error":{"code":"management_access_denied","message":"Management access is unavailable"}}"#;

/// A validated management credential retained only by the HTTP admission state.
#[derive(Clone, Eq, PartialEq)]
pub struct ManagementKey(Zeroizing<String>);

impl ManagementKey {
    /// Accepts an explicit, namespaced management key without exposing it on error.
    ///
    /// The prefix keeps this configuration visibly distinct from Client Key and Provider
    /// credential namespaces. The full value is never returned or rendered by this type.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementKeyError::InvalidFormat`] for an unsafe or wrong-namespace value.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ManagementKeyError> {
        let value = value.into();
        if !valid_secret(&value, "mgmt_") {
            return Err(ManagementKeyError::InvalidFormat);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    fn matches(&self, candidate: &str) -> bool {
        bool::from(self.0.as_bytes().ct_eq(candidate.as_bytes()))
    }
}

impl fmt::Debug for ManagementKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ManagementKey(<redacted>)")
    }
}

/// Management-key configuration failure without a rejected secret value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementKeyError {
    /// The value did not have the required namespace, length, or safe ASCII form.
    InvalidFormat,
}

impl fmt::Display for ManagementKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("management key has an invalid format")
    }
}

impl std::error::Error for ManagementKeyError {}

/// A validated CSRF token used only when the optional same-origin browser policy is enabled.
#[derive(Clone, Eq, PartialEq)]
pub struct ManagementCsrfToken(Zeroizing<String>);

impl ManagementCsrfToken {
    /// Accepts an explicit, independent CSRF token without exposing it on error.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementCsrfTokenError::InvalidFormat`] for an unsafe or wrong-namespace
    /// value.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ManagementCsrfTokenError> {
        let value = value.into();
        if !valid_secret(&value, "csrf_") {
            return Err(ManagementCsrfTokenError::InvalidFormat);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    fn matches(&self, candidate: &str) -> bool {
        bool::from(self.0.as_bytes().ct_eq(candidate.as_bytes()))
    }
}

impl fmt::Debug for ManagementCsrfToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ManagementCsrfToken(<redacted>)")
    }
}

/// CSRF-token configuration failure without a rejected token value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementCsrfTokenError {
    /// The value did not have the required namespace, length, or safe ASCII form.
    InvalidFormat,
}

impl fmt::Display for ManagementCsrfTokenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("management CSRF token has an invalid format")
    }
}

impl std::error::Error for ManagementCsrfTokenError {}

fn valid_secret(value: &str, prefix: &str) -> bool {
    (32..=512).contains(&value.len())
        && value.starts_with(prefix)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// Exact HTTP(S) origin admitted for the optional same-origin management UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementOrigin(String);

impl ManagementOrigin {
    /// Validates and canonicalizes one HTTP(S) origin without a path, query, fragment, or userinfo.
    ///
    /// # Errors
    ///
    /// Returns [`ManagementOriginError::Invalid`] for a non-HTTP(S) or non-origin value.
    pub fn try_new(value: impl AsRef<str>) -> Result<Self, ManagementOriginError> {
        let value = value.as_ref();
        let parsed = Url::parse(value).map_err(|_| ManagementOriginError::Invalid)?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
        {
            return Err(ManagementOriginError::Invalid);
        }
        let origin = parsed.origin().ascii_serialization();
        if origin == "null" || value != origin {
            return Err(ManagementOriginError::Invalid);
        }
        Ok(Self(origin))
    }

    /// Returns the canonical origin value that an `Origin` header must match exactly.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Same-origin configuration failure without echoing an unsafe origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementOriginError {
    /// The origin was not a canonical HTTP(S) origin.
    Invalid,
}

impl fmt::Display for ManagementOriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("management origin is invalid")
    }
}

impl std::error::Error for ManagementOriginError {}

/// Peer-address policy for management traffic.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ManagementNetworkPolicy {
    /// Allow only a loopback peer; this is the secure default.
    #[default]
    LoopbackOnly,
    /// Allow loopback plus RFC1918 IPv4 or IPv6 unique-local-address peers.
    LoopbackOrPrivate,
}

impl ManagementNetworkPolicy {
    fn admits(self, address: IpAddr) -> bool {
        address.is_loopback()
            || matches!(self, Self::LoopbackOrPrivate)
                && match address {
                    IpAddr::V4(address) => address.is_private(),
                    IpAddr::V6(address) => address.octets()[0] & 0xfe == 0xfc,
                }
    }
}

/// Browser policy for the management shell.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ManagementBrowserPolicy {
    /// Reject every request carrying `Origin`; this is the secure default until the SPA exists.
    #[default]
    DenyBrowserOrigins,
    /// Permit only one exact UI origin; unsafe methods also require the separate CSRF token.
    SameOrigin {
        /// Canonical browser origin for the embedded management UI.
        origin: ManagementOrigin,
        /// Independent browser-request CSRF value.
        csrf_token: ManagementCsrfToken,
    },
}

/// Actix app state required to admit an `/admin/` request.
#[derive(Clone)]
pub struct ManagementHttpState {
    key: ManagementKey,
    network_policy: ManagementNetworkPolicy,
    browser_policy: ManagementBrowserPolicy,
    actor: ManagementActor,
}

impl ManagementHttpState {
    /// Creates a management admission state with a fixed audit actor that never includes key data.
    ///
    /// # Errors
    ///
    /// Returns only if the fixed, non-secret actor label would violate the durable audit boundary.
    pub fn new(
        key: ManagementKey,
        network_policy: ManagementNetworkPolicy,
        browser_policy: ManagementBrowserPolicy,
    ) -> Result<Self, ManagementHttpStateError> {
        let actor = ManagementActor::try_new(MANAGEMENT_ACTOR_LABEL)
            .map_err(|_| ManagementHttpStateError::AuditActorUnavailable)?;
        Ok(Self {
            key,
            network_policy,
            browser_policy,
            actor,
        })
    }

    /// Returns the configured peer-address policy without returning credential material.
    #[must_use]
    pub const fn network_policy(&self) -> ManagementNetworkPolicy {
        self.network_policy
    }

    fn authenticate(&self, request: &HttpRequest) -> Result<ManagementRequestPrincipal, ()> {
        let Some(peer_address) = request.peer_addr().map(|address| address.ip()) else {
            return Err(());
        };
        if !self.network_policy.admits(peer_address) {
            return Err(());
        }

        let Some(presented_key) = single_header(request, MANAGEMENT_KEY_HEADER)? else {
            return Err(());
        };
        let presented_key = presented_key.to_str().map_err(|_| ())?;
        if !self.key.matches(presented_key) {
            return Err(());
        }

        self.check_browser_boundary(request)?;
        Ok(ManagementRequestPrincipal {
            actor: self.actor.clone(),
        })
    }

    fn check_browser_boundary(&self, request: &HttpRequest) -> Result<(), ()> {
        let Some(origin) = single_header(request, "origin")? else {
            return Ok(());
        };
        let origin = origin.to_str().map_err(|_| ())?;
        let origin = ManagementOrigin::try_new(origin).map_err(|_| ())?;
        let ManagementBrowserPolicy::SameOrigin {
            origin: admitted_origin,
            csrf_token,
        } = &self.browser_policy
        else {
            return Err(());
        };
        if &origin != admitted_origin || request.method() == Method::OPTIONS {
            return Err(());
        }
        if unsafe_method(request.method()) {
            let Some(presented_token) = single_header(request, CSRF_HEADER)? else {
                return Err(());
            };
            let presented_token = presented_token.to_str().map_err(|_| ())?;
            if !csrf_token.matches(presented_token) {
                return Err(());
            }
        }
        Ok(())
    }
}

impl fmt::Debug for ManagementHttpState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagementHttpState")
            .field("key", &"<redacted>")
            .field("network_policy", &self.network_policy)
            .field("browser_policy", &self.browser_policy)
            .field("actor", &self.actor)
            .finish()
    }
}

/// Management admission-state construction failure without secret material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagementHttpStateError {
    /// The fixed internal actor could not satisfy the shared durable audit constraints.
    AuditActorUnavailable,
}

impl fmt::Display for ManagementHttpStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("management audit identity is unavailable")
    }
}

impl std::error::Error for ManagementHttpStateError {}

/// Verified management actor identity attached to a guarded handler request for durable auditing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagementRequestPrincipal {
    actor: ManagementActor,
}

impl ManagementRequestPrincipal {
    /// Returns the non-secret actor to use for the later management mutation audit event.
    #[must_use]
    pub fn actor(&self) -> &ManagementActor {
        &self.actor
    }
}

/// Registers future `/admin/` routes behind the P10-02 security middleware.
///
/// P10-02 itself intentionally passes no resource routes: P10-04 onward supply their declared
/// handlers through this closure. A missing [`ManagementHttpState`] rejects each protected route.
pub fn configure_management<F>(config: &mut web::ServiceConfig, configure_routes: F)
where
    F: FnOnce(&mut web::ServiceConfig),
{
    config.service(
        web::scope("/admin")
            .wrap(ManagementSecurity)
            .configure(configure_routes),
    );
}

#[derive(Clone, Copy, Debug, Default)]
struct ManagementSecurity;

impl<S, B> Transform<S, ServiceRequest> for ManagementSecurity
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = ManagementSecurityMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ManagementSecurityMiddleware { service }))
    }
}

struct ManagementSecurityMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for ManagementSecurityMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(context)
    }

    fn call(&self, request: ServiceRequest) -> Self::Future {
        let principal = request
            .app_data::<web::Data<ManagementHttpState>>()
            .and_then(|state| state.authenticate(request.request()).ok());
        let Some(principal) = principal else {
            return Box::pin(async move {
                Ok(request.into_response(management_denied_response().map_into_right_body()))
            });
        };
        request.extensions_mut().insert(principal);
        let future = self.service.call(request);
        Box::pin(async move { future.await.map(ServiceResponse::map_into_left_body) })
    }
}

fn single_header<'a>(
    request: &'a HttpRequest,
    name: &'static str,
) -> Result<Option<&'a header::HeaderValue>, ()> {
    let mut values = request
        .headers()
        .get_all(header::HeaderName::from_static(name));
    let value = values.next();
    if values.next().is_some() {
        return Err(());
    }
    Ok(value)
}

const fn unsafe_method(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD)
}

fn management_denied_response() -> HttpResponse {
    HttpResponse::NotFound()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .content_type("application/json")
        .body(MANAGEMENT_DENIED_BODY)
}
