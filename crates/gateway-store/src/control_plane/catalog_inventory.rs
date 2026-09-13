//! Target-local model enumeration from saved discovery evidence, never a Provider call.
use super::{
    ConfigVersion, ConfigVersionId, ResourceInventoryReader, load_config_version, malformed,
};
use crate::{StoreError, StoreResult};
use rusqlite::{OptionalExtension, params};

/// Filters and snapshot continuation for an exact catalog target.
#[derive(Clone, Copy)]
pub struct CatalogModelQuery<'a> {
    /// Configuration scope, including drafts and history.
    pub version: &'a ConfigVersionId,
    /// Exact managed interface.
    pub endpoint_id: &'a str,
    /// Exact ordinary or native catalog owner; never its secret.
    pub credential_id: &'a str,
    /// Literal model substring.
    pub search: &'a str,
    /// Between one and one hundred rows.
    pub limit: u16,
    /// Configuration revision, target version and observation timestamp.
    pub expected: Option<(i64, i64, i64)>,
    /// Exclusive model key.
    pub after: Option<&'a str>,
}

/// Persisted evidence clock, independent of configured routes and authorization.
#[derive(Debug)]
pub struct CatalogTargetHeader {
    /// Monotonic version within this target.
    pub snapshot_version: i64,
    /// Last successful discovery.
    pub observed_at_ms: i64,
    /// Beginning of stale evidence.
    pub stale_at_ms: i64,
    /// Hard eligibility deadline.
    pub expires_at_ms: i64,
}

/// A retained exact model, including isolated successful omissions.
#[derive(Debug)]
pub struct CatalogModelRow {
    /// Source-provided exact identity.
    pub model: String,
    /// Whether the latest successful discovery still contained it.
    pub present_in_last_success: bool,
}

/// A bounded read under one `SQLite` transaction.
#[derive(Debug)]
pub struct CatalogModelPage {
    /// Configuration source.
    pub version: ConfigVersion,
    /// Target-local source clock.
    pub target: CatalogTargetHeader,
    /// Exact last-success model count, independent of filtering and pagination.
    pub current_model_count: i64,
    /// Matching saved entries, including entries absent in the last success.
    pub total_count: i64,
    /// Stable model-ordered page.
    pub items: Vec<CatalogModelRow>,
    /// Continuation key, when another matching row exists.
    pub next_after: Option<String>,
}

impl ResourceInventoryReader {
    /// Lists saved models even when no public route has been configured for them.
    /// # Errors
    /// Rejects invalid bounds, missing targets and changed configuration/catalog snapshots.
    pub fn catalog_models(&self, query: CatalogModelQuery<'_>) -> StoreResult<CatalogModelPage> {
        if !(1..=100).contains(&query.limit)
            || [query.endpoint_id, query.credential_id]
                .iter()
                .any(|id| id.is_empty() || id.len() > 128)
            || query.search.len() > 256
            || query
                .after
                .is_some_and(|id| id.is_empty() || id.len() > 4096 || query.expected.is_none())
        {
            return Err(malformed("catalog_model_query"));
        }
        let mut repository = self.repository()?;
        let tx = repository.connection.transaction()?;
        let version =
            load_config_version(&tx, query.version)?.ok_or(StoreError::ConfigVersionNotFound)?;
        let target = tx.query_row(
            "SELECT c.snapshot_version,c.observed_at_ms,c.stale_at_ms,c.expires_at_ms
             FROM model_catalog_targets c JOIN upstream_endpoints e ON e.config_version_id=c.config_version_id AND e.id=c.endpoint_id
             WHERE c.config_version_id=?1 AND c.endpoint_id=?2 AND c.credential_id=?3",
            params![query.version.as_str(), query.endpoint_id, query.credential_id],
            |r| Ok(CatalogTargetHeader {snapshot_version:r.get(0)?,observed_at_ms:r.get(1)?,stale_at_ms:r.get(2)?,expires_at_ms:r.get(3)?}),
        ).optional()?.ok_or(if query.expected.is_some() {StoreError::ConfigVersionRevisionConflict} else {StoreError::ControlPlaneResourceNotFound})?;
        if query.expected.is_some_and(|expected| {
            expected
                != (
                    version.revision,
                    target.snapshot_version,
                    target.observed_at_ms,
                )
        }) {
            return Err(StoreError::ConfigVersionRevisionConflict);
        }
        let (current_model_count, total_count) = tx.query_row(
            "SELECT COALESCE(SUM(CASE WHEN present_in_last_success THEN 1 ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN ?4='' OR instr(lower(model),lower(?4))>0 THEN 1 ELSE 0 END),0)
             FROM model_catalog_models WHERE config_version_id=?1 AND endpoint_id=?2 AND credential_id=?3",
            params![query.version.as_str(), query.endpoint_id, query.credential_id, query.search],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        let mut stmt = tx.prepare(
            "SELECT model,present_in_last_success FROM model_catalog_models
             WHERE config_version_id=?1 AND endpoint_id=?2 AND credential_id=?3
               AND model > ?4 AND (?5='' OR instr(lower(model),lower(?5))>0)
             ORDER BY model LIMIT ?6",
        )?;
        let mut items = stmt
            .query_map(
                params![
                    query.version.as_str(),
                    query.endpoint_id,
                    query.credential_id,
                    query.after.unwrap_or(""),
                    query.search,
                    i64::from(query.limit) + 1
                ],
                |r| {
                    Ok(CatalogModelRow {
                        model: r.get(0)?,
                        present_in_last_success: r.get(1)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let next_after = if items.len() > usize::from(query.limit) {
            items.truncate(usize::from(query.limit));
            items.last().map(|row| row.model.clone())
        } else {
            None
        };
        Ok(CatalogModelPage {
            version,
            target,
            items,
            next_after,
            current_model_count,
            total_count,
        })
    }
}
