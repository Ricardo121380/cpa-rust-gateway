//! Opt-in, source-bound capture for one synthetic Chat JSON diagnostic request.
//!
//! Only classified response headers and body bytes cross this decorator. Outbound credentials,
//! authentication headers and the client payload are never retained. The directory descriptor
//! anchors exclusive 0600 files inside an existing canonical 0700 directory.

use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use super::BoxFuture as ProviderFuture;
use gateway_core::{GatewayError, RequestId};
use gateway_router::ProtocolFormat;
use gateway_router::ResponsesResponseMode as ResponseMode;
use provider_grok::{
    GrokBuildResponseBody, GrokBuildResponsesOutboundRequest, GrokBuildTransport,
    GrokBuildTransportResponse, GrokNativeEgressAttempt,
    MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES,
};
use rustix::fs::{Mode, OFlags, openat};
use sha2::{Digest, Sha256};

const DIRECTORY_ENV: &str = "CPAR_BUILD_DIAGNOSTIC_DIR";
const REQUEST_ENV: &str = "CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256";
const SOURCE_ENV: &str = "CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION";

pub(super) struct BuildDiagnostic {
    directory: Arc<File>,
    request_sha256: String,
}

impl BuildDiagnostic {
    pub(super) fn from_environment() -> Result<Option<Arc<Self>>, GatewayError> {
        let values = [DIRECTORY_ENV, REQUEST_ENV, SOURCE_ENV].map(std::env::var_os);
        if values.iter().all(Option::is_none) {
            return Ok(None);
        }
        let [Some(directory), Some(request), Some(source)] = values else {
            return Err(super::internal_error());
        };
        let request = request.into_string().map_err(|_| super::internal_error())?;
        let source = source.into_string().map_err(|_| super::internal_error())?;
        Self::open(PathBuf::from(directory).as_path(), &request, &source)
            .map(|capture| Some(Arc::new(capture)))
    }

    pub(super) fn open(
        directory: &Path,
        request_sha256: &str,
        source_revision: &str,
    ) -> Result<Self, GatewayError> {
        if source_revision != env!("GATEWAY_RELEASE_REVISION")
            || request_sha256.len() != 64
            || !request_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || !directory.is_absolute()
            || directory
                .canonicalize()
                .map_err(|_| super::internal_error())?
                != directory
        {
            return Err(super::internal_error());
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(directory)
            .map_err(|_| super::internal_error())?;
        let metadata = directory.metadata().map_err(|_| super::internal_error())?;
        if !metadata.is_dir() || metadata.mode() & 0o777 != 0o700 {
            return Err(super::internal_error());
        }
        Ok(Self {
            directory: Arc::new(directory),
            request_sha256: request_sha256.to_owned(),
        })
    }

    pub(super) fn wrap_selected(
        &self,
        inner: Arc<dyn GrokBuildTransport>,
        request_id: &RequestId,
        protocol: ProtocolFormat,
        mode: ResponseMode,
        public_payload: Option<&[u8]>,
    ) -> Result<Arc<dyn GrokBuildTransport>, GatewayError> {
        if protocol != ProtocolFormat::OpenAiChatCompletions
            || mode != ResponseMode::NonStreaming
            || public_payload
                .is_none_or(|body| format!("{:x}", Sha256::digest(body)) != self.request_sha256)
        {
            return Ok(inner);
        }
        // create_new is the persistent one-shot reservation, including failed transports.
        let file = private_file(&self.directory, "native-response.bin")?;
        private_json(
            &self.directory,
            "selection.json",
            &serde_json::json!({
                "source_revision":env!("GATEWAY_RELEASE_REVISION"),
                "request_id":request_id.as_str(), "public_request_sha256":self.request_sha256,
                "protocol":"chat", "mode":"json", "maximum_bytes":MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES,
                "credentials_or_authentication_headers_captured":false,
            }),
        )?;
        Ok(Arc::new(CapturingTransport {
            inner,
            sent: AtomicBool::new(false),
            capture: Arc::new(Capture {
                directory: Arc::clone(&self.directory),
                state: Mutex::new(CaptureState {
                    file,
                    bytes: 0,
                    digest: Sha256::new(),
                    ended: false,
                }),
            }),
        }))
    }
}

fn private_file(directory: &File, name: &str) -> Result<File, GatewayError> {
    let fd = openat(
        directory,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| super::internal_error())?;
    let file = File::from(fd);
    let metadata = file.metadata().map_err(|_| super::internal_error())?;
    let owner = directory
        .metadata()
        .map_err(|_| super::internal_error())?
        .uid();
    if !metadata.is_file()
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.uid() != owner
    {
        return Err(super::internal_error());
    }
    Ok(file)
}

fn private_json(
    directory: &File,
    name: &str,
    value: &serde_json::Value,
) -> Result<(), GatewayError> {
    let mut file = private_file(directory, name)?;
    serde_json::to_writer(&mut file, value).map_err(|_| super::internal_error())?;
    file.sync_all().map_err(|_| super::internal_error())
}

struct CaptureState {
    file: File,
    bytes: usize,
    digest: Sha256,
    ended: bool,
}
struct Capture {
    directory: Arc<File>,
    state: Mutex<CaptureState>,
}
impl Capture {
    async fn append(self: &Arc<Self>, chunk: Vec<u8>) -> Result<Vec<u8>, GatewayError> {
        let capture = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            let mut state = capture.state.lock().map_err(|_| super::internal_error())?;
            let remaining = MAX_GROK_BUILD_NON_STREAMING_RESPONSE_BYTES.saturating_sub(state.bytes);
            let keep = chunk.len().min(remaining);
            state
                .file
                .write_all(&chunk[..keep])
                .map_err(|_| super::internal_error())?;
            state.digest.update(&chunk[..keep]);
            state.bytes += keep;
            if keep != chunk.len() {
                return Err(super::upstream_protocol_error());
            }
            Ok(chunk)
        })
        .await
        .map_err(|_| super::internal_error())?
    }

    async fn finish(self: &Arc<Self>, complete: bool) -> Result<(), GatewayError> {
        let capture = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            let mut state = capture.state.lock().map_err(|_| super::internal_error())?;
            if state.ended {
                return Ok(());
            }
            state.file.sync_all().map_err(|_| super::internal_error())?;
            private_json(
                &capture.directory,
                "body-receipt.json",
                &serde_json::json!({
                    "stage":"provider_transport_body", "complete":complete,
                    "bytes":state.bytes, "sha256":format!("{:x}", state.digest.clone().finalize()),
                }),
            )?;
            state.ended = true;
            Ok(())
        })
        .await
        .map_err(|_| super::internal_error())?
    }
}

struct CapturingTransport {
    inner: Arc<dyn GrokBuildTransport>,
    sent: AtomicBool,
    capture: Arc<Capture>,
}
impl GrokBuildTransport for CapturingTransport {
    fn send(
        &self,
        request: GrokBuildResponsesOutboundRequest,
    ) -> ProviderFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        self.send_with_egress_attempt(request, None)
    }
    fn send_with_egress_attempt(
        &self,
        request: GrokBuildResponsesOutboundRequest,
        attempt: Option<Arc<GrokNativeEgressAttempt>>,
    ) -> ProviderFuture<'_, Result<GrokBuildTransportResponse, GatewayError>> {
        Box::pin(async move {
            if self.sent.swap(true, Ordering::AcqRel) {
                return Err(super::internal_error());
            }
            let response = match self.inner.send_with_egress_attempt(request, attempt).await {
                Ok(response) => response,
                Err(error) => {
                    self.capture.finish(false).await?;
                    return Err(error);
                }
            };
            let (status, content_type, content_encoding, body) = response.into_parts();
            private_json(
                &self.capture.directory,
                "transport.json",
                &serde_json::json!({
                    "stage":"provider_transport_headers", "status":status,
                    "content_type_class":format!("{content_type:?}"), "content_encoding_class":format!("{content_encoding:?}"),
                }),
            )?;
            Ok(GrokBuildTransportResponse::new(
                status,
                content_type,
                content_encoding,
                Box::new(CapturingBody {
                    inner: body,
                    capture: Arc::clone(&self.capture),
                }),
            ))
        })
    }
}
struct CapturingBody {
    inner: Box<dyn GrokBuildResponseBody>,
    capture: Arc<Capture>,
}
impl GrokBuildResponseBody for CapturingBody {
    fn next_chunk(&mut self) -> ProviderFuture<'_, Result<Option<Vec<u8>>, GatewayError>> {
        Box::pin(async move {
            match self.inner.next_chunk().await {
                Ok(Some(chunk)) => match self.capture.append(chunk).await {
                    Ok(chunk) => Ok(Some(chunk)),
                    Err(error) => {
                        self.capture.finish(false).await?;
                        Err(error)
                    }
                },
                Ok(None) => {
                    self.capture.finish(true).await?;
                    Ok(None)
                }
                Err(error) => {
                    self.capture.finish(false).await?;
                    Err(error)
                }
            }
        })
    }
}
