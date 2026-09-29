//! Value-free account restoration review, checked again inside the activation transaction.
use super::{
    ConfigVersionId, Connection, OptionalExtension, SqliteControlPlaneRepository, StoreError,
    StoreResult,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Exact identity that historical activation would restore.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RestoredAccount {
    /// Local credential identity; no material is returned.
    pub credential_id: String,
    /// Original owning provider identity.
    pub upstream_id: String,
    /// Revision of the historical material being reviewed.
    pub credential_revision: i64,
}
/// Complete bounded review of historical account restoration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AccountRestorationReview {
    /// Target configuration identity.
    pub target_id: String,
    /// Exact target graph revision.
    pub target_revision: i64,
    /// Current active identity, if present.
    pub active_id: Option<String>,
    /// Current active graph revision, if present.
    pub active_revision: Option<i64>,
    /// Latest deletion audit clock used by this review.
    pub deletion_event_id: i64,
    /// Accounts absent from active that were previously deleted locally.
    pub accounts: Vec<RestoredAccount>,
    /// Digest of all reviewed identities and clocks; not a secret or independent authorization.
    pub review_token: String,
}
impl SqliteControlPlaneRepository {
    /// Reads a complete restoration review without publishing or contacting a Provider.
    /// # Errors
    /// Returns storage errors or fails closed when the target is absent or the review exceeds 256 accounts.
    pub fn account_restoration_review(
        &mut self,
        target: &ConfigVersionId,
    ) -> StoreResult<AccountRestorationReview> {
        let transaction = self.begin_transaction()?;
        review(&transaction.transaction, target)
    }
}
pub(super) fn review(
    connection: &Connection,
    target: &ConfigVersionId,
) -> StoreResult<AccountRestorationReview> {
    let target_revision = connection
        .query_row(
            "SELECT revision FROM config_versions WHERE id=?1",
            [target.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::ConfigVersionRevisionConflict)?;
    let active: Option<(String, i64)> = connection
        .query_row(
            "SELECT id,revision FROM config_versions WHERE status='active'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let deletion_event_id=connection.query_row("SELECT COALESCE(MAX(id),0) FROM management_resource_audit_events WHERE action='credential_deleted'",[],|row|row.get(0))?;
    let mut query=connection.prepare("SELECT c.id,c.upstream_id,c.revision FROM upstream_credentials c WHERE c.config_version_id=?1 AND NOT EXISTS(SELECT 1 FROM upstream_credentials a JOIN config_versions v ON a.config_version_id=v.id WHERE v.status='active' AND a.id=c.id AND a.upstream_id=c.upstream_id) AND EXISTS(SELECT 1 FROM management_resource_audit_events d WHERE d.action='credential_deleted' AND d.resource_kind='credential' AND d.resource_id=c.id) ORDER BY c.upstream_id,c.id LIMIT 257")?;
    let accounts = query
        .query_map([target.as_str()], |row| {
            Ok(RestoredAccount {
                credential_id: row.get(0)?,
                upstream_id: row.get(1)?,
                credential_revision: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if accounts.len() > 256 {
        return Err(StoreError::ConfigVersionRevisionConflict);
    }
    let mut result = AccountRestorationReview {
        target_id: target.as_str().to_owned(),
        target_revision,
        active_id: active.as_ref().map(|(id, _)| id.clone()),
        active_revision: active.map(|(_, revision)| revision),
        deletion_event_id,
        accounts,
        review_token: String::new(),
    };
    let bytes =
        serde_json::to_vec(&result).map_err(|_| StoreError::ConfigVersionRevisionConflict)?;
    result.review_token = format!("{:x}", Sha256::digest(bytes));
    Ok(result)
}
