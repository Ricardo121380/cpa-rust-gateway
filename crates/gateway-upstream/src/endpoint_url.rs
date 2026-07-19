//! Safe deterministic composition of one configured upstream endpoint URL.

use std::{error::Error, fmt};

use url::Url;

/// A validated URL obtained by appending one configured inference path to one Base URL.
///
/// The value intentionally hides its concrete URL in `Debug` output. It is a request-target
/// shape only; P2-09's [`crate::EgressPolicy`] remains responsible for per-dial DNS and address
/// admission.
#[derive(Clone, Eq, PartialEq)]
pub struct EndpointUrl(Url);

impl EndpointUrl {
    /// Composes an HTTP(S) Base URL and an absolute, path-only inference path.
    ///
    /// The Base URL path is retained even when `inference_path` begins with `/`: for example,
    /// `https://relay.example/v1` plus `/responses` becomes
    /// `https://relay.example/v1/responses`. Queries, fragments, user-info, path traversal, and
    /// ambiguous path separators are rejected rather than normalized or forwarded.
    ///
    /// # Errors
    ///
    /// Returns [`EndpointUrlError::InvalidBaseUrl`] or
    /// [`EndpointUrlError::InvalidInferencePath`] for a shape that cannot safely form one
    /// configured endpoint target.
    pub fn compose(base_url: &str, inference_path: &str) -> Result<Self, EndpointUrlError> {
        let mut url = Url::parse(base_url).map_err(|_| EndpointUrlError::InvalidBaseUrl)?;
        if !is_valid_base_url(base_url, &url) {
            return Err(EndpointUrlError::InvalidBaseUrl);
        }
        if !is_valid_inference_path(inference_path) {
            return Err(EndpointUrlError::InvalidInferencePath);
        }

        let base_path = url.path().trim_end_matches('/');
        let joined_path = format!("{base_path}{inference_path}");
        url.set_path(&joined_path);

        Ok(Self(url))
    }

    /// Returns the complete target URL for the later transport layer.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns the parsed URL for a later HTTP transport without re-parsing it.
    #[must_use]
    pub fn as_url(&self) -> &Url {
        &self.0
    }
}

impl fmt::Debug for EndpointUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EndpointUrl(<redacted>)")
    }
}

/// A safe classification for configured endpoint URL composition failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointUrlError {
    /// The Base URL is not a supported, origin-bearing HTTP(S) URL without sensitive components.
    InvalidBaseUrl,
    /// The inference path is not one unambiguous absolute path below the configured Base URL.
    InvalidInferencePath,
}

impl fmt::Display for EndpointUrlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBaseUrl => formatter.write_str("invalid configured endpoint base URL"),
            Self::InvalidInferencePath => {
                formatter.write_str("invalid configured endpoint inference path")
            }
        }
    }
}

impl Error for EndpointUrlError {}

fn is_valid_base_url(raw_url: &str, url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && !url.cannot_be_a_base()
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        // `Url::username()` is empty for both absent and explicitly empty user-info.
        && matches!(raw_authority(raw_url), Some(authority) if !authority.contains('@'))
        // WHATWG URL parsing normalizes literal and encoded dot segments. Inspect the original
        // configured value as well so a route cannot be hidden by that normalization.
        && !raw_url.contains(['%', '\\'])
        && is_valid_path(url.path())
        && raw_base_path(raw_url).is_none_or(is_valid_path)
}

fn is_valid_inference_path(path: &str) -> bool {
    is_valid_path(path)
}

fn is_valid_path(path: &str) -> bool {
    if !path.starts_with('/') || path.contains("//") {
        return false;
    }
    if !path.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~')
    }) {
        return false;
    }

    path.split('/')
        .all(|segment| !matches!(segment, "." | ".."))
}

fn raw_base_path(raw_url: &str) -> Option<&str> {
    let (_, after_scheme) = raw_url.split_once("://")?;
    let path_start = after_scheme.find(['/', '?', '#'])?;
    let path_with_suffix = &after_scheme[path_start..];
    if !path_with_suffix.starts_with('/') {
        return None;
    }

    let path_end = path_with_suffix
        .find(['?', '#'])
        .unwrap_or(path_with_suffix.len());
    Some(&path_with_suffix[..path_end])
}

fn raw_authority(raw_url: &str) -> Option<&str> {
    let (_, after_scheme) = raw_url.split_once("://")?;
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    Some(&after_scheme[..authority_end])
}

#[cfg(test)]
mod tests {
    use super::{EndpointUrl, EndpointUrlError};

    #[test]
    fn composition_retains_the_base_path_for_absolute_inference_paths()
    -> Result<(), EndpointUrlError> {
        let without_trailing_slash =
            EndpointUrl::compose("https://relay.example/v1", "/responses")?;
        let with_trailing_slash = EndpointUrl::compose("https://relay.example/v1/", "/responses")?;

        assert_eq!(
            without_trailing_slash.as_str(),
            "https://relay.example/v1/responses"
        );
        assert_eq!(
            with_trailing_slash.as_str(),
            "https://relay.example/v1/responses"
        );
        assert_eq!(without_trailing_slash.as_url().path(), "/v1/responses");
        Ok(())
    }

    #[test]
    fn composition_rejects_ambiguous_or_sensitive_target_shapes() {
        for base_url in [
            "mailto:operator@example.test",
            "ftp://relay.example/v1",
            "https://user:password@relay.example/v1",
            "https://@relay.example/v1",
            "https://relay.example/v1?token=secret",
            "https://relay.example/v1#fragment",
            "https://relay.example/v1/../admin",
            "https://relay.example/v1/./admin",
            "https://relay.example/v1/%2e%2e/admin",
            "https://relay.example/v1/%2E%2E/admin",
            "https://relay.example/v1/%252e%252e/admin",
            r"https://relay.example/v1\..\admin",
        ] {
            assert_eq!(
                EndpointUrl::compose(base_url, "/responses"),
                Err(EndpointUrlError::InvalidBaseUrl)
            );
        }

        for inference_path in ["responses", "/v1//responses", "/v1/../responses", "/v1?x=1"] {
            assert_eq!(
                EndpointUrl::compose("https://relay.example/v1", inference_path),
                Err(EndpointUrlError::InvalidInferencePath)
            );
        }
    }

    #[test]
    fn debug_redacts_the_configured_target() -> Result<(), EndpointUrlError> {
        let endpoint = EndpointUrl::compose("https://private-relay.example/v1", "/responses")?;
        let debug = format!("{endpoint:?}");

        assert!(!debug.contains("private-relay.example"));
        assert_eq!(debug, "EndpointUrl(<redacted>)");
        Ok(())
    }
}
