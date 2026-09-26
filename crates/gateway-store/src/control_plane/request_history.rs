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
        let columns="r.event_ordinal ordinal,r.request_id,r.payload_json r,
            (SELECT a.payload_json FROM gateway_event_log a WHERE a.event_type='attempt' AND a.request_id=r.request_id AND a.event_ordinal<=?1 ORDER BY json_extract(a.payload_json,'$.attempt.attempt_number') DESC LIMIT 1) a,
            (SELECT COUNT(*) FROM gateway_event_log a WHERE a.event_type='attempt' AND a.request_id=r.request_id AND a.event_ordinal<=?1) attempts,
            (SELECT u.payload_json FROM gateway_event_log u WHERE u.event_type='usage' AND u.request_id=r.request_id AND u.event_ordinal<=?1 ORDER BY u.event_ordinal DESC LIMIT 1) u";
        let projection = "), projected AS (
          SELECT *,json_extract(r,'$.request.public_model') model,
            json_extract(r,'$.request.client_key_id') client_key,
            json_extract(a,'$.attempt.upstream_id') upstream,
            json_extract(a,'$.attempt.credential_id') credential,
            COALESCE(terminal_outcome,'unknown') outcome
          FROM source
        ), filtered AS (
          SELECT * FROM projected WHERE (?5 IS NULL OR model=?5) AND (?6 IS NULL OR upstream=?6)
            AND (?7 IS NULL OR credential=?7) AND (?8 IS NULL OR client_key=?8)
            AND (?9 IS NULL OR outcome=?9) AND (?10 IS NULL OR request_id=?10)
        ) ";
        // Read narrow indexed terminal facts for aggregation; payloads are loaded only for the page.
        let cte=format!("WITH matches AS (
          SELECT request_id,event_ordinal terminal, json_extract(payload_json,'$.request_finished.outcome') terminal_outcome, json_extract(payload_json,'$.request_finished.duration_ms') duration, json_extract(payload_json,'$.request_finished.first_content_ms') first_content, json_extract(payload_json,'$.request_finished.finished_at_ms') finished FROM gateway_event_log INDEXED BY gateway_terminal_history_cover WHERE event_type='request_finished' AND event_ordinal<=?1 AND occurred_at_ms>=?2 AND occurred_at_ms<=?3
          UNION ALL
          SELECT r.request_id,NULL,NULL,NULL,NULL,NULL FROM gateway_event_log r WHERE ?4 AND r.event_type='request' AND r.event_ordinal<=?1
            AND NOT EXISTS(SELECT 1 FROM gateway_event_log t WHERE t.event_type='request_finished' AND t.event_id=r.request_id AND t.event_ordinal<=?1)
        ), source AS (
          SELECT {columns},m.terminal,m.terminal_outcome,m.duration,m.first_content,m.finished FROM matches m CROSS JOIN gateway_event_log r ON r.event_id=m.request_id
          WHERE r.event_type='request' AND r.event_ordinal<=?1
        {projection}");
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
        // A wide page walks newest Request ordinals directly, stopping at limit+1.
        // Narrow windows keep the terminal-time index so old windows never scan newer history.
        let wide_page_cte;
        let page_cte = if query
            .to_ms
            .unwrap_or(i64::MAX)
            .saturating_sub(query.from_ms.unwrap_or(0))
            > 86_400_000
        {
            wide_page_cte = format!("WITH source AS (
              SELECT {columns},t.event_ordinal terminal, json_extract(t.payload_json,'$.request_finished.outcome') terminal_outcome, json_extract(t.payload_json,'$.request_finished.duration_ms') duration, json_extract(t.payload_json,'$.request_finished.first_content_ms') first_content, json_extract(t.payload_json,'$.request_finished.finished_at_ms') finished FROM gateway_event_log r INDEXED BY gateway_event_log_type_ordinal
              LEFT JOIN gateway_event_log t ON t.event_type='request_finished' AND t.event_id=r.request_id AND t.event_ordinal<=?1
              WHERE r.event_type='request' AND r.event_ordinal<?11
                AND ((t.occurred_at_ms>=?2 AND t.occurred_at_ms<=?3) OR (?4 AND t.event_ordinal IS NULL))
              {projection}");
            &wide_page_cte
        } else {
            &cte
        };
        let sql = format!(
            "{page_cte}, page AS MATERIALIZED (SELECT ordinal,request_id,r,terminal,a,attempts,u FROM filtered WHERE ordinal<?11 ORDER BY ordinal DESC LIMIT ?12),
             billing AS (SELECT l.request_id,COUNT(*) records,SUM(l.cost_microunits) cost,
               CASE WHEN SUM(l.cost_confidence='unpriced')>0 THEN 'unpriced' WHEN SUM(l.cost_confidence='unknown')>0 THEN 'unknown' WHEN SUM(l.cost_confidence='partial')>0 THEN 'partial' ELSE 'exact' END confidence
               FROM page p CROSS JOIN billing_ledger_entries l INDEXED BY billing_ledger_request_idx ON l.request_id=p.request_id AND l.ledger_id<=?13 GROUP BY l.request_id)
             SELECT p.ordinal,p.r,(SELECT payload_json FROM gateway_event_log WHERE event_ordinal=p.terminal),p.a,p.attempts,p.u,COALESCE(b.records,0),b.cost,b.confidence
             FROM page p LEFT JOIN billing b ON b.request_id=p.request_id ORDER BY p.ordinal DESC"
        );
        let mut page_values = values.clone();
        page_values.push(
            query
                .after
                .unwrap_or(i64::MAX)
                .min(snapshot.saturating_add(1))
                .into(),
        );
        page_values.push((i64::from(query.limit) + 1).into());
        page_values.push(ledger_snapshot.into());
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
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = entries.len() > usize::from(query.limit);
        let mut items = Vec::new();
        let mut last = None;
        for (ordinal, r, t, a, attempts, u, records, cost, confidence) in
            entries.into_iter().take(usize::from(query.limit))
        {
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
            items.push(json!({"request_id":r["request_id"],"client_key_id":r["client_key_id"],"model":r["public_model"],"requested_model":r["requested_model"],"protocol":r["protocol"],"streaming":r["streaming"],"upstream_id":a["upstream_id"],"endpoint_id":a["endpoint_id"],"credential_id":a["credential_id"],"attempt_count":attempts,"outcome":t["outcome"].as_str().unwrap_or("unknown"),"started_at_ms":t["started_at_ms"],"finished_at_ms":t["finished_at_ms"],"duration_ms":t["duration_ms"],"first_content_ms":t["first_content_ms"],"error_code":t["error_code"],"usage":usage["usage"]["usage"],"cost_microunits":cost,"cost_confidence":confidence,"ledger_records":records}));
            last = Some(ordinal);
        }
        let mut summary = None;
        let mut series = Vec::new();
        if query.summary {
            // Materialize only narrow scalar columns once, on SQLite's temporary store.
            // Percentiles remain exact and unknown historical durations remain null.
            let sql = format!("{cte}, aggregate_rows AS MATERIALIZED (
                SELECT outcome,attempts,duration,first_content,finished FROM filtered
              ), duration_counts AS (
                SELECT duration,COUNT(*) frequency FROM aggregate_rows WHERE duration IS NOT NULL GROUP BY duration
              ), ranked AS (
                SELECT duration,SUM(frequency) OVER(ORDER BY duration) n,SUM(frequency) OVER() total FROM duration_counts
              ), quantiles AS (
                SELECT MIN(CASE WHEN n>=(total+1)/2 THEN duration END) p50,MIN(CASE WHEN n>=(total*95+99)/100 THEN duration END) p95 FROM ranked
              ), buckets AS (
                SELECT (finished/?11)*?11 at_ms,COUNT(*) requests,SUM(outcome='succeeded') succeeded,SUM(outcome='failed') failed,SUM(outcome='cancelled') cancelled,AVG(duration) average_duration_ms,AVG(first_content) average_first_content_ms
                FROM aggregate_rows WHERE finished IS NOT NULL GROUP BY finished/?11 ORDER BY finished/?11 LIMIT 1001
              ) SELECT json_object('requests',COUNT(*),'succeeded',COALESCE(SUM(outcome='succeeded'),0),'failed',COALESCE(SUM(outcome='failed'),0),'cancelled',COALESCE(SUM(outcome='cancelled'),0),'unknown',COALESCE(SUM(outcome='unknown'),0),'attempts',COALESCE(SUM(attempts),0),'success_rate',SUM(outcome='succeeded')*1.0/NULLIF(SUM(outcome<>'unknown'),0),'average_duration_ms',AVG(duration),'average_first_content_ms',AVG(first_content),'p50_duration_ms',(SELECT p50 FROM quantiles),'p95_duration_ms',(SELECT p95 FROM quantiles)),
                (SELECT json_group_array(json_object('at_ms',at_ms,'requests',requests,'succeeded',succeeded,'failed',failed,'cancelled',cancelled,'average_duration_ms',average_duration_ms,'average_first_content_ms',average_first_content_ms)) FROM buckets)
              FROM aggregate_rows");
            let mut aggregate_values = values;
            aggregate_values.push(query.bucket_ms.into());
            let (stats, trend): (String, String) =
                tx.query_row(&sql, params_from_iter(aggregate_values), |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
            summary = Some(
                serde_json::from_str(&stats)
                    .map_err(|_| StoreError::InvalidPersistedGatewayEvent)?,
            );
            series = serde_json::from_str(&trend)
                .map_err(|_| StoreError::InvalidPersistedGatewayEvent)?;
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
