//! Snapshot-bound request history. Attempts and ledger records are never counted as requests.
use super::ResourceInventoryReader;
use crate::{StoreError, StoreResult};
use rusqlite::params_from_iter;
use serde_json::{Value, json};

/// Literal filters for external requests; no query strings become SQL identifiers.
#[derive(Clone, Debug, Default)]
#[allow(missing_docs)]
pub struct RequestHistoryQuery {
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
    pub model: Option<String>,
    pub upstream_id: Option<String>,
    pub credential_id: Option<String>,
    pub client_key_id: Option<String>,
    pub outcome: Option<String>,
    pub request_id: Option<String>,
    pub ledger_snapshot: Option<i64>,
    pub snapshot: Option<i64>,
    pub after: Option<i64>,
    pub limit: u16,
    pub include_unknown: bool,
    pub summary: bool,
    pub bucket_ms: i64,
}
/// Bounded page plus optional complete aggregation over the same immutable event snapshot.
#[derive(serde::Serialize)]
#[allow(missing_docs)]
pub struct RequestHistoryPage {
    pub ledger_snapshot: i64,
    pub snapshot: i64,
    pub next_after: Option<i64>,
    pub items: Vec<Value>,
    pub summary: Option<Value>,
    pub series: Vec<Value>,
}

impl ResourceInventoryReader {
    /// Latest observed start time of a completed request for each logical key ID.
    /// Old records without terminal timestamps and currently in-flight requests stay unknown.
    /// This is not a credential-secret-specific last-use claim: IDs survive configuration forks.
    /// # Errors
    /// Rejects unbounded key sets, malformed IDs and inaccessible persisted observations.
    pub fn client_key_activity(
        &self,
        keys: &[String],
    ) -> StoreResult<std::collections::BTreeMap<String, i64>> {
        if keys.len() > 10_000 || keys.iter().any(|key| key.is_empty() || key.len() > 128) {
            return Err(StoreError::InvalidPersistedGatewayEvent);
        }
        let mut repository = self.repository()?;
        let tx = repository.connection.transaction()?;
        let encoded =
            serde_json::to_string(keys).map_err(|_| StoreError::InvalidPersistedGatewayEvent)?;
        let mut statement = tx.prepare("SELECT k.value, MAX(json_extract(t.payload_json,'$.request_finished.started_at_ms'))
          FROM json_each(?1) k
          JOIN gateway_event_log r INDEXED BY gateway_request_client_key ON json_extract(r.payload_json,'$.request.client_key_id')=k.value
          JOIN gateway_event_log t ON t.request_id=r.request_id AND t.event_type='request_finished'
          WHERE r.event_type='request' AND json_valid(r.payload_json) AND json_valid(t.payload_json)
          GROUP BY k.value")?;
        let rows = statement.query_map([encoded], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
        })?;
        let mut result = std::collections::BTreeMap::new();
        for row in rows {
            let (key, timestamp) = row?;
            if let Some(timestamp) = timestamp.filter(|v| *v >= 0) {
                result.insert(key, timestamp);
            }
        }
        Ok(result)
    }

    /// Reads real accepted request records and separately correlated final/attempt/usage evidence.
    /// # Errors
    /// Rejects invalid ranges, bounds and inaccessible or malformed persisted observations.
    #[allow(clippy::too_many_lines)] // Keep filtering, pagination and complete aggregation in one read transaction.
    pub fn requests(&self, query: &RequestHistoryQuery) -> StoreResult<RequestHistoryPage> {
        if !(1..=100).contains(&query.limit)
            || query.bucket_ms < 60_000
            || query.from_ms.is_some_and(|v| v < 0)
            || query.to_ms.is_some_and(|v| v < 0)
            || matches!((query.from_ms,query.to_ms),(Some(a),Some(b)) if a>b)
            || query.snapshot.is_some_and(|v| v < 0)
            || query.after.is_some_and(|v| v < 0)
            || [
                &query.model,
                &query.upstream_id,
                &query.credential_id,
                &query.client_key_id,
                &query.request_id,
            ]
            .into_iter()
            .flatten()
            .any(|v| v.is_empty() || v.len() > 512)
            || query
                .outcome
                .as_deref()
                .is_some_and(|v| !matches!(v, "succeeded" | "failed" | "cancelled" | "unknown"))
        {
            return Err(StoreError::InvalidPersistedGatewayEvent);
        }
        let mut repository = self.repository()?;
        let tx = repository.connection.transaction()?;
        let maximum: i64 = tx.query_row(
            "SELECT COALESCE(MAX(event_ordinal),0) FROM gateway_event_log",
            [],
            |r| r.get(0),
        )?;
        let snapshot = query.snapshot.unwrap_or(maximum);
        let ledger_max: i64 = tx.query_row(
            "SELECT COALESCE(MAX(ledger_id),0) FROM billing_ledger_entries",
            [],
            |r| r.get(0),
        )?;
        let ledger_snapshot = query.ledger_snapshot.unwrap_or(ledger_max);
        if ledger_snapshot < 0 || ledger_snapshot > ledger_max {
            return Err(StoreError::ConfigVersionRevisionConflict);
        }
        if snapshot > maximum {
            return Err(StoreError::ConfigVersionRevisionConflict);
        }
        // t is unique per request. Scalar attempt/usage lookups are request-indexed and snapshot-bound.
        let cte="WITH matches AS (
          SELECT request_id FROM gateway_event_log WHERE event_type='request_finished' AND event_ordinal<=?1 AND occurred_at_ms>=?2 AND occurred_at_ms<=?3
          UNION ALL
          SELECT r.request_id FROM gateway_event_log r WHERE ?4 AND r.event_type='request' AND r.event_ordinal<=?1
            AND NOT EXISTS(SELECT 1 FROM gateway_event_log t WHERE t.event_type='request_finished' AND t.request_id=r.request_id AND t.event_ordinal<=?1)
        ), source AS (
          SELECT r.event_ordinal ordinal,r.request_id,r.payload_json r,t.payload_json t,
            (SELECT a.payload_json FROM gateway_event_log a WHERE a.event_type='attempt' AND a.request_id=r.request_id AND a.event_ordinal<=?1 ORDER BY json_extract(a.payload_json,'$.attempt.attempt_number') DESC LIMIT 1) a,
            (SELECT COUNT(*) FROM gateway_event_log a WHERE a.event_type='attempt' AND a.request_id=r.request_id AND a.event_ordinal<=?1) attempts,
            (SELECT u.payload_json FROM gateway_event_log u WHERE u.event_type='usage' AND u.request_id=r.request_id AND u.event_ordinal<=?1 ORDER BY u.event_ordinal DESC LIMIT 1) u
          FROM matches m JOIN gateway_event_log r ON r.request_id=m.request_id LEFT JOIN gateway_event_log t ON t.event_type='request_finished' AND t.request_id=r.request_id AND t.event_ordinal<=?1
          WHERE r.event_type='request' AND r.event_ordinal<=?1
            AND ((t.occurred_at_ms>=?2 AND t.occurred_at_ms<=?3) OR (?4 AND t.event_ordinal IS NULL))
        ), projected AS (
          SELECT *,json_extract(r,'$.request.public_model') model,
            json_extract(r,'$.request.client_key_id') client_key,
            json_extract(a,'$.attempt.upstream_id') upstream,
            json_extract(a,'$.attempt.credential_id') credential,
            COALESCE(json_extract(t,'$.request_finished.outcome'),'unknown') outcome,
            json_extract(t,'$.request_finished.duration_ms') duration,
            json_extract(t,'$.request_finished.first_content_ms') first_content,
            json_extract(t,'$.request_finished.finished_at_ms') finished
          FROM source
        ), filtered AS (
          SELECT * FROM projected WHERE (?5 IS NULL OR model=?5) AND (?6 IS NULL OR upstream=?6)
            AND (?7 IS NULL OR credential=?7) AND (?8 IS NULL OR client_key=?8)
            AND (?9 IS NULL OR outcome=?9) AND (?10 IS NULL OR request_id=?10)
        ) ";
        let values: Vec<rusqlite::types::Value> = vec![
            snapshot.into(),
            query.from_ms.unwrap_or(0).into(),
            query.to_ms.unwrap_or(i64::MAX).into(),
            i64::from(query.include_unknown).into(),
            query.model.clone().into(),
            query.upstream_id.clone().into(),
            query.credential_id.clone().into(),
            query.client_key_id.clone().into(),
            query.outcome.clone().into(),
            query.request_id.clone().into(),
        ];
        let sql = format!(
            "{cte} SELECT ordinal,r,t,a,attempts,u FROM filtered WHERE ordinal<?11 ORDER BY ordinal DESC LIMIT ?12"
        );
        let mut page_values = values.clone();
        page_values.push(query.after.unwrap_or(i64::MAX).into());
        page_values.push((i64::from(query.limit) + 1).into());
        let mut stmt = tx.prepare(&sql)?;
        let entries = stmt
            .query_map(params_from_iter(page_values), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = entries.len() > usize::from(query.limit);
        let mut items = Vec::new();
        let mut last = None;
        for (ordinal, r, t, a, attempts, u) in entries.into_iter().take(usize::from(query.limit)) {
            let parse = |s: &str| {
                serde_json::from_str::<Value>(s)
                    .map_err(|_| StoreError::InvalidPersistedGatewayEvent)
            };
            let request = parse(&r)?;
            let terminal = t.as_deref().map(parse).transpose()?.unwrap_or(Value::Null);
            let attempt = a.as_deref().map(parse).transpose()?.unwrap_or(Value::Null);
            let usage = u.as_deref().map(parse).transpose()?.unwrap_or(Value::Null);
            let r = &request["request"];
            let t = &terminal["request_finished"];
            let a = &attempt["attempt"];
            let billing=tx.query_row("SELECT COUNT(*),SUM(cost_microunits),CASE WHEN SUM(cost_confidence='unpriced')>0 THEN 'unpriced' WHEN SUM(cost_confidence='unknown')>0 THEN 'unknown' WHEN SUM(cost_confidence='partial')>0 THEN 'partial' ELSE 'exact' END FROM billing_ledger_entries WHERE request_id=?1 AND ledger_id<=?2",rusqlite::params![r["request_id"].as_str(),ledger_snapshot],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,Option<i64>>(1)?,row.get::<_,String>(2)?)))?;
            items.push(json!({"request_id":r["request_id"],"client_key_id":r["client_key_id"],"model":r["public_model"],"requested_model":r["requested_model"],"protocol":r["protocol"],"streaming":r["streaming"],"upstream_id":a["upstream_id"],"endpoint_id":a["endpoint_id"],"credential_id":a["credential_id"],"attempt_count":attempts,"outcome":t["outcome"].as_str().unwrap_or("unknown"),"started_at_ms":t["started_at_ms"],"finished_at_ms":t["finished_at_ms"],"duration_ms":t["duration_ms"],"first_content_ms":t["first_content_ms"],"error_code":t["error_code"],"usage":usage["usage"]["usage"],"cost_microunits":billing.1,"cost_confidence":if billing.0==0 {None}else{Some(billing.2)},"ledger_records":billing.0}));
            last = Some(ordinal);
        }
        let mut summary = None;
        let mut series = Vec::new();
        if query.summary {
            let sql = format!(
                "{cte} SELECT COUNT(*),COALESCE(SUM(outcome='succeeded'),0),COALESCE(SUM(outcome='failed'),0),COALESCE(SUM(outcome='cancelled'),0),COALESCE(SUM(outcome='unknown'),0),COALESCE(SUM(attempts),0),AVG(duration),AVG(first_content),SUM(outcome='succeeded')*1.0/NULLIF(SUM(outcome<>'unknown'),0) FROM filtered"
            );
            let (
                count,
                success,
                failed,
                cancelled,
                unknown,
                attempts,
                average,
                first,
                success_rate,
            ) = tx.query_row(&sql, params_from_iter(values.clone()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, Option<f64>>(6)?,
                    r.get::<_, Option<f64>>(7)?,
                    r.get::<_, Option<f64>>(8)?,
                ))
            })?;
            let quantiles = format!(
                "{cte}, ranked AS (SELECT duration,ROW_NUMBER() OVER(ORDER BY duration) n,COUNT(*) OVER() total FROM filtered WHERE duration IS NOT NULL) SELECT MAX(CASE WHEN n=(total+1)/2 THEN duration END),MAX(CASE WHEN n=(total*95+99)/100 THEN duration END) FROM ranked"
            );
            let (p50, p95) = tx.query_row(&quantiles, params_from_iter(values.clone()), |r| {
                Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?))
            })?;
            summary = Some(
                json!({"requests":count,"succeeded":success,"failed":failed,"cancelled":cancelled,"unknown":unknown,"attempts":attempts,"success_rate":success_rate,"average_duration_ms":average,"average_first_content_ms":first,"p50_duration_ms":p50,"p95_duration_ms":p95}),
            );
            let sql = format!(
                "{cte} SELECT (finished/?11)*?11,COUNT(*),SUM(outcome='succeeded'),SUM(outcome='failed'),SUM(outcome='cancelled'),AVG(duration),AVG(first_content) FROM filtered WHERE finished IS NOT NULL GROUP BY finished/?11 ORDER BY finished/?11 LIMIT 1001"
            );
            let mut trend_values = values;
            trend_values.push(query.bucket_ms.into());
            let mut stmt = tx.prepare(&sql)?;
            series=stmt.query_map(params_from_iter(trend_values),|r|Ok(json!({"at_ms":r.get::<_,i64>(0)?,"requests":r.get::<_,i64>(1)?,"succeeded":r.get::<_,i64>(2)?,"failed":r.get::<_,i64>(3)?,"cancelled":r.get::<_,i64>(4)?,"average_duration_ms":r.get::<_,Option<f64>>(5)?,"average_first_content_ms":r.get::<_,Option<f64>>(6)?})))?.collect::<Result<Vec<_>,_>>()?;
            if series.len() > 1000 {
                return Err(StoreError::InvalidPersistedGatewayEvent);
            }
        }
        Ok(RequestHistoryPage {
            ledger_snapshot,
            snapshot,
            next_after: if has_more { last } else { None },
            items,
            summary,
            series,
        })
    }
}
