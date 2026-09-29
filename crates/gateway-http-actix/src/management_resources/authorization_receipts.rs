//! Bounded, process-local, secret-free readback of an existing authorization operation.
//! This stores receipts only: it cannot exchange, retry, schedule or persist credentials.
use super::{
    BTreeMap, ConfigRevision, ConfigVersionId, CredentialId, HttpRequest, HttpResponse,
    ManagementOperationsError, ManagementResourceHttpState, Serialize, StatusCode, SystemTime,
    UNIX_EPOCH, UpstreamId, WriteContext, error_response, header, internal_error, invalid_input,
    principal, read_context, web,
};
use gateway_control::management_service::ManagementActor;

const RETENTION_MS: i64 = 15 * 60 * 1_000;
const CAPACITY: usize = 512;

#[derive(Clone, Serialize)]
pub(super) struct Receipt {
    session_id: String,
    channel: String,
    config_version: String,
    upstream_id: String,
    credential_id: String,
    started_revision: String,
    revision: String,
    state: String,
    expires_at_ms: i64,
    #[serde(skip)]
    actor: String,
    #[serde(skip)]
    logical: Option<String>,
    #[serde(skip)]
    retain_until_ms: i64,
}

#[derive(Default)]
pub(super) struct Receipts(BTreeMap<String, Receipt>);

fn clock() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|v| i64::try_from(v.as_millis()).ok())
        .unwrap_or(0)
}

impl Receipts {
    #[allow(clippy::too_many_arguments)] // One value-free receipt of a channel-owned operation.
    pub(super) fn begin(
        &mut self,
        key: String,
        logical: Option<String>,
        channel: &str,
        actor: &ManagementActor,
        context: &WriteContext,
        owner: &UpstreamId,
        credential: &CredentialId,
        expires: i64,
    ) -> Result<(), ManagementOperationsError> {
        let now = clock();
        self.0.retain(|_, entry| entry.retain_until_ms > now);
        if self.0.contains_key(&key) {
            return Ok(());
        }
        if self.0.len() >= CAPACITY {
            return Err(ManagementOperationsError::SourceUnavailable);
        }
        // A new challenge for the same logical admission has its own handle; keep old receipts
        // readable, but only the latest challenge can receive that admission's outcome.
        if logical.is_some() {
            for entry in self.0.values_mut() {
                if entry.logical == logical {
                    entry.logical = None;
                }
            }
        }
        self.0.insert(
            key.clone(),
            Receipt {
                session_id: key,
                logical,
                channel: channel.to_owned(),
                actor: actor.as_str().to_owned(),
                config_version: context.version.to_string(),
                upstream_id: owner.to_string(),
                credential_id: credential.to_string(),
                started_revision: context.revision.as_token(),
                revision: context.revision.as_token(),
                state: "pending".to_owned(),
                expires_at_ms: expires,
                retain_until_ms: expires.max(now).saturating_add(RETENTION_MS),
            },
        );
        Ok(())
    }

    pub(super) fn logical_key(&self, logical: &str, actor: &ManagementActor) -> Option<String> {
        self.0
            .values()
            .find(|entry| {
                entry.logical.as_deref() == Some(logical) && entry.actor == actor.as_str()
            })
            .map(|entry| entry.session_id.clone())
    }

    pub(super) fn update(
        &mut self,
        key: &str,
        state: &str,
        saved: Option<(&CredentialId, ConfigRevision)>,
    ) {
        let Some(entry) = self.0.get_mut(key) else {
            return;
        };
        // A replay, stale callback or failed later read cannot erase a successful write receipt.
        if entry.state == "completed" {
            return;
        }
        state.clone_into(&mut entry.state);
        if let Some((id, revision)) = saved {
            entry.credential_id = id.to_string();
            entry.revision = revision.as_token();
        }
        if state != "pending" && state != "in_progress" {
            entry.retain_until_ms = clock().saturating_add(RETENTION_MS);
        }
    }

    pub(super) fn read(
        &self,
        key: &str,
        actor: &ManagementActor,
        version: &ConfigVersionId,
        now: i64,
    ) -> Option<Receipt> {
        let entry = self.0.get(key).filter(|entry| {
            entry.actor == actor.as_str()
                && entry.config_version == version.as_str()
                && entry.retain_until_ms > now
        })?;
        let mut result = entry.clone();
        if result.state == "pending" && result.expires_at_ms <= now {
            "expired".clone_into(&mut result.state);
        }
        Some(result)
    }
}

pub(super) fn update(
    state: &ManagementResourceHttpState,
    key: &str,
    status: &str,
    saved: Option<(&CredentialId, ConfigRevision)>,
) {
    if let Ok(mut receipts) = state.authorization_receipts.lock() {
        receipts.update(key, status, saved);
    }
}

pub(super) async fn status(
    request: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ManagementResourceHttpState>,
) -> HttpResponse {
    let actor = match principal(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let context = match read_context(&request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let key = path.into_inner();
    if key.is_empty() || key.len() > 128 {
        return invalid_input();
    }
    let Ok(receipts) = state.authorization_receipts.lock() else {
        return internal_error();
    };
    // This lock is independent of Provider I/O and SQLite. No workflow method is invoked here.
    match receipts.read(&key, &actor, &context.version, clock()) {
        Some(receipt) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(receipt),
        None => error_response(
            StatusCode::NOT_FOUND,
            "management_authorization_receipt_missing",
            "授权回执不在本进程保留范围内；结果待确认，请核对账号与版本，不要重放授权",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipts_are_actor_version_bounded_and_cannot_erase_a_saved_result()
    -> Result<(), Box<dyn std::error::Error>> {
        let actor = ManagementActor::try_new("local-a")?;
        let other = ManagementActor::try_new("local-b")?;
        let context = WriteContext {
            version: ConfigVersionId::try_new("draft-a")?,
            revision: ConfigRevision::try_new(7)?,
        };
        let owner = UpstreamId::try_new("provider-a")?;
        let credential = CredentialId::try_new("account-a")?;
        let now = clock();
        let mut receipts = Receipts::default();
        receipts.begin(
            "session-a".to_owned(),
            None,
            "codex",
            &actor,
            &context,
            &owner,
            &credential,
            now + 60_000,
        )?;
        assert!(
            receipts
                .read("session-a", &other, &context.version, now)
                .is_none()
        );
        assert!(
            receipts
                .read(
                    "session-a",
                    &actor,
                    &ConfigVersionId::try_new("draft-b")?,
                    now
                )
                .is_none()
        );
        assert_eq!(
            receipts
                .read("session-a", &actor, &context.version, now + 60_001)
                .map(|entry| entry.state),
            Some("expired".to_owned())
        );
        receipts.update(
            "session-a",
            "completed",
            Some((&credential, ConfigRevision::try_new(8)?)),
        );
        receipts.update("session-a", "failed", None);
        let receipt = receipts
            .read("session-a", &actor, &context.version, clock())
            .ok_or("missing saved receipt")?;
        assert_eq!(receipt.state, "completed");
        assert_eq!(receipt.revision, "rev-8");
        let serialized = serde_json::to_value(&receipt)?;
        assert!(serialized.get("actor").is_none());
        assert!(serialized.get("logical").is_none());
        assert!(
            receipts
                .read(
                    "session-a",
                    &actor,
                    &context.version,
                    receipt.retain_until_ms
                )
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn retained_receipts_have_a_hard_capacity_and_only_new_logical_admission_receives_updates()
    -> Result<(), Box<dyn std::error::Error>> {
        let actor = ManagementActor::try_new("local")?;
        let context = WriteContext {
            version: ConfigVersionId::try_new("draft")?,
            revision: ConfigRevision::try_new(0)?,
        };
        let owner = UpstreamId::try_new("provider")?;
        let credential = CredentialId::try_new("account")?;
        let mut receipts = Receipts::default();
        for index in 0..CAPACITY {
            receipts.begin(
                format!("session-{index}"),
                Some("same-logical-account".to_owned()),
                "claude",
                &actor,
                &context,
                &owner,
                &credential,
                clock() + 60_000,
            )?;
        }
        assert_eq!(
            receipts.logical_key("same-logical-account", &actor),
            Some(format!("session-{}", CAPACITY - 1))
        );
        assert!(
            receipts
                .begin(
                    "overflow".to_owned(),
                    None,
                    "claude",
                    &actor,
                    &context,
                    &owner,
                    &credential,
                    clock() + 60_000
                )
                .is_err()
        );
        assert_eq!(receipts.0.len(), CAPACITY);
        Ok(())
    }
}
