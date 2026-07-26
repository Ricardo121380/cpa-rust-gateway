//! Append-only durable storage for bounded gateway lifecycle observations.
//!
//! The request path only calls the synchronous, non-blocking event sink in `gateway-core`.
//! This module consumes already-admitted observations after that boundary. It never gives the
//! queue, `SQLite`, or its retry loop back to Router or HTTP callers.

use std::{
    collections::VecDeque,
    fmt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use gateway_core::{GatewayEvent, GatewayEventPriority, RequestId};
use gateway_observability::{EventQueueReceiver, TelemetryPipeline};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{StoreError, StoreResult, migrate, open, open_in_memory};

/// The largest finite event batch accepted by the asynchronous writer.
pub const MAX_EVENT_WRITER_BATCH_SIZE: usize = 1_024;
/// Default number of Required observations included in one durable transaction when ready.
pub const DEFAULT_EVENT_WRITER_BATCH_SIZE: usize = 64;
/// Default bounded delay before a failed `SQLite` batch is retried.
pub const DEFAULT_EVENT_WRITER_RETRY_DELAY: Duration = Duration::from_millis(25);

/// Durable category for one [`GatewayEvent`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GatewayEventLogKind {
    /// One accepted request observation.
    Request,
    /// One terminal upstream attempt observation.
    Attempt,
    /// One final canonical usage observation.
    Usage,
    /// One sanitized runtime-health transition.
    Health,
}

impl GatewayEventLogKind {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Attempt => "attempt",
            Self::Usage => "usage",
            Self::Health => "health",
        }
    }

    fn from_sql(value: &str) -> StoreResult<Self> {
        match value {
            "request" => Ok(Self::Request),
            "attempt" => Ok(Self::Attempt),
            "usage" => Ok(Self::Usage),
            "health" => Ok(Self::Health),
            _ => Err(StoreError::InvalidPersistedGatewayEvent),
        }
    }
}

/// One validated append-only event row loaded from `gateway_event_log`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredGatewayEvent {
    ordinal: i64,
    kind: GatewayEventLogKind,
    event_id: String,
    request_id: Option<RequestId>,
    occurred_at_ms: Option<i64>,
    event: GatewayEvent,
}

impl StoredGatewayEvent {
    /// Returns the append order allocated by `SQLite` after a successful transaction.
    #[must_use]
    pub const fn ordinal(&self) -> i64 {
        self.ordinal
    }

    /// Returns the stable durable event category.
    #[must_use]
    pub const fn kind(&self) -> GatewayEventLogKind {
        self.kind
    }

    /// Returns the stable `(event_type, event_id)` idempotence component.
    #[must_use]
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Returns the Request correlation when this event belongs to a client request.
    #[must_use]
    pub fn request_id(&self) -> Option<&RequestId> {
        self.request_id.as_ref()
    }

    /// Returns the explicit event time when the event category carries one.
    #[must_use]
    pub const fn occurred_at_ms(&self) -> Option<i64> {
        self.occurred_at_ms
    }

    /// Returns the fully typed, access-controlled durable event.
    #[must_use]
    pub const fn event(&self) -> &GatewayEvent {
        &self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EventRecord {
    kind: GatewayEventLogKind,
    event_id: String,
    request_id: Option<String>,
    occurred_at_ms: Option<i64>,
    payload_json: String,
}

impl EventRecord {
    fn from_event(event: &GatewayEvent) -> StoreResult<Self> {
        let (kind, event_id, request_id, occurred_at_ms) = match event {
            GatewayEvent::Request(value) => (
                GatewayEventLogKind::Request,
                value.request_id().as_str().to_owned(),
                Some(value.request_id().as_str().to_owned()),
                None,
            ),
            GatewayEvent::Attempt(value) => (
                GatewayEventLogKind::Attempt,
                value.attempt_id().as_str().to_owned(),
                Some(value.request_id().as_str().to_owned()),
                Some(value.ended_at_ms()),
            ),
            GatewayEvent::Usage(value) => (
                GatewayEventLogKind::Usage,
                value.response_id().as_str().to_owned(),
                Some(value.request_id().as_str().to_owned()),
                None,
            ),
            GatewayEvent::Health(value) => (
                GatewayEventLogKind::Health,
                value.health_event_id().as_str().to_owned(),
                None,
                Some(value.occurred_at_ms()),
            ),
            GatewayEvent::Diagnostic(_) => return Err(StoreError::DiagnosticEventNotPersistable),
        };
        let payload_json =
            serde_json::to_string(event).map_err(|_| StoreError::InvalidPersistedGatewayEvent)?;
        // The durable schema bounds both identifier columns. Enforcing that bound here, before the
        // insert, is what makes an over-long identifier a record-level poison instead of a raw
        // SQLite CHECK violation: the latter is indistinguishable from a transient store failure
        // and would retry forever. Usage event ids are upstream-supplied response ids, so this is
        // the one poison class an external party can actually trigger.
        if !is_persistable_identifier(&event_id)
            || request_id
                .as_deref()
                .is_some_and(|request_id| !is_persistable_identifier(request_id))
        {
            return Err(StoreError::InvalidPersistedGatewayEvent);
        }

        Ok(Self {
            kind,
            event_id,
            request_id,
            occurred_at_ms,
            payload_json,
        })
    }
}

/// The durable identifier bound enforced by `gateway_event_log`'s schema CHECK constraints.
const MAX_EVENT_LOG_IDENTIFIER_BYTES: usize = 512;

/// Reports whether one identifier can satisfy the durable schema's length constraint.
fn is_persistable_identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_EVENT_LOG_IDENTIFIER_BYTES
}

/// File- or memory-backed append-only event store.
pub struct SqliteEventStore {
    connection: Connection,
}

impl SqliteEventStore {
    /// Opens and migrates one file-backed event store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot open or migrate.
    pub fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        Self::from_connection(open(path)?)
    }

    /// Opens and migrates one isolated in-memory event store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the in-memory database cannot migrate.
    pub fn open_in_memory() -> StoreResult<Self> {
        Self::from_connection(open_in_memory()?)
    }

    /// Takes an already-open connection, applies migrations, and owns it.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when migration cannot complete.
    pub fn from_connection(mut connection: Connection) -> StoreResult<Self> {
        migrate(&mut connection)?;
        Ok(Self { connection })
    }

    /// Atomically appends a bounded batch of Required events.
    ///
    /// Replaying an identical `(event_type, event_id, payload)` is a no-op. Reusing the same
    /// durable identity with different contents fails the whole transaction rather than hiding a
    /// correlation collision. Diagnostic events remain explicitly non-persistable.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] and leaves no partial rows when any record is invalid, conflicts,
    /// or `SQLite` rejects the transaction.
    pub fn append_batch(&mut self, events: &[GatewayEvent]) -> StoreResult<usize> {
        let records = events
            .iter()
            .map(EventRecord::from_event)
            .collect::<StoreResult<Vec<_>>>()?;
        if records.is_empty() {
            return Ok(0);
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut inserted = 0;
        for record in &records {
            let existing: Option<String> = transaction
                .query_row(
                    "SELECT payload_json FROM gateway_event_log \
                     WHERE event_type = ?1 AND event_id = ?2",
                    params![record.kind.as_sql(), record.event_id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(existing) = existing {
                if existing != record.payload_json {
                    return Err(StoreError::ConflictingGatewayEventReplay);
                }
                continue;
            }

            transaction.execute(
                "INSERT INTO gateway_event_log \
                 (event_type, event_id, request_id, occurred_at_ms, payload_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    record.kind.as_sql(),
                    record.event_id,
                    record.request_id,
                    record.occurred_at_ms,
                    record.payload_json,
                ],
            )?;
            inserted += 1;
        }
        transaction.commit()?;
        Ok(inserted)
    }

    /// Returns one request-correlated Request/Attempt/Usage timeline in durable append order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidPersistedGatewayEvent`] if a row cannot be safely decoded or
    /// does not structurally match its indexed metadata.
    pub fn events_for_request(
        &self,
        request_id: &RequestId,
    ) -> StoreResult<Vec<StoredGatewayEvent>> {
        self.load_events(
            "SELECT event_ordinal, event_type, event_id, request_id, occurred_at_ms, payload_json \
             FROM gateway_event_log WHERE request_id = ?1 ORDER BY event_ordinal",
            [request_id.as_str()],
        )
    }

    /// Returns all durable Health events in append order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidPersistedGatewayEvent`] if a stored row is malformed.
    pub fn health_events(&self) -> StoreResult<Vec<StoredGatewayEvent>> {
        self.load_events(
            "SELECT event_ordinal, event_type, event_id, request_id, occurred_at_ms, payload_json \
             FROM gateway_event_log WHERE event_type = 'health' ORDER BY event_ordinal",
            [],
        )
    }

    /// Returns every event in append order for operator-only recovery and verification.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidPersistedGatewayEvent`] if a stored row is malformed.
    pub fn list_events(&self) -> StoreResult<Vec<StoredGatewayEvent>> {
        self.load_events(
            "SELECT event_ordinal, event_type, event_id, request_id, occurred_at_ms, payload_json \
             FROM gateway_event_log ORDER BY event_ordinal",
            [],
        )
    }

    /// Runs `PRAGMA quick_check` and accepts only `SQLite`'s exact `ok` result.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::GatewayEventLogIntegrityCheckFailed`] for any non-`ok` result.
    pub fn quick_check(&self) -> StoreResult<()> {
        let result: String = self
            .connection
            .query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if result == "ok" {
            Ok(())
        } else {
            Err(StoreError::GatewayEventLogIntegrityCheckFailed)
        }
    }

    fn load_events<P>(&self, sql: &str, parameters: P) -> StoreResult<Vec<StoredGatewayEvent>>
    where
        P: rusqlite::Params,
    {
        let mut statement = self.connection.prepare(sql)?;
        let mut rows = statement.query(parameters)?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            let ordinal = row.get(0)?;
            let event_type: String = row.get(1)?;
            let event_id: String = row.get(2)?;
            let request_id: Option<String> = row.get(3)?;
            let occurred_at_ms = row.get(4)?;
            let payload_json: String = row.get(5)?;
            events.push(decode_stored_event(
                ordinal,
                &event_type,
                event_id,
                request_id,
                occurred_at_ms,
                &payload_json,
            )?);
        }
        Ok(events)
    }
}

impl fmt::Debug for SqliteEventStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SqliteEventStore(<connection redacted>)")
    }
}

fn decode_stored_event(
    ordinal: i64,
    event_type: &str,
    event_id: String,
    request_id: Option<String>,
    occurred_at_ms: Option<i64>,
    payload_json: &str,
) -> StoreResult<StoredGatewayEvent> {
    let kind = GatewayEventLogKind::from_sql(event_type)?;
    let event: GatewayEvent =
        serde_json::from_str(payload_json).map_err(|_| StoreError::InvalidPersistedGatewayEvent)?;
    let expected = EventRecord::from_event(&event)?;
    if expected.kind != kind
        || expected.event_id != event_id
        || expected.request_id != request_id
        || expected.occurred_at_ms != occurred_at_ms
        || expected.payload_json != payload_json
    {
        return Err(StoreError::InvalidPersistedGatewayEvent);
    }
    let request_id = request_id
        .map(RequestId::try_new)
        .transpose()
        .map_err(|_| StoreError::InvalidPersistedGatewayEvent)?;

    Ok(StoredGatewayEvent {
        ordinal,
        kind,
        event_id,
        request_id,
        occurred_at_ms,
        event,
    })
}

/// Validated bounded configuration for [`AsyncSqliteEventWriter`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventWriterConfig {
    batch_size: usize,
    retry_delay: Duration,
}

impl EventWriterConfig {
    /// Validates bounded batch and retry values before a writer is started.
    ///
    /// # Errors
    ///
    /// Returns [`EventWriterConfigError`] when a value could cause a busy loop or unbounded batch.
    pub const fn try_new(
        batch_size: usize,
        retry_delay: Duration,
    ) -> Result<Self, EventWriterConfigError> {
        if batch_size == 0 {
            return Err(EventWriterConfigError::ZeroBatchSize);
        }
        if batch_size > MAX_EVENT_WRITER_BATCH_SIZE {
            return Err(EventWriterConfigError::BatchSizeTooLarge);
        }
        if retry_delay.is_zero() {
            return Err(EventWriterConfigError::ZeroRetryDelay);
        }
        Ok(Self {
            batch_size,
            retry_delay,
        })
    }

    /// Returns the maximum Required events retained in one pending transaction.
    #[must_use]
    pub const fn batch_size(self) -> usize {
        self.batch_size
    }

    /// Returns the delay after a visible durable-write failure.
    #[must_use]
    pub const fn retry_delay(self) -> Duration {
        self.retry_delay
    }
}

impl Default for EventWriterConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_EVENT_WRITER_BATCH_SIZE,
            retry_delay: DEFAULT_EVENT_WRITER_RETRY_DELAY,
        }
    }
}

/// Configuration error for [`EventWriterConfig`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventWriterConfigError {
    /// A writer must retain at least one Required event per transaction.
    ZeroBatchSize,
    /// A writer batch exceeded the frozen finite upper bound.
    BatchSizeTooLarge,
    /// A zero retry delay would busy-loop while `SQLite` is unavailable.
    ZeroRetryDelay,
}

impl fmt::Display for EventWriterConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBatchSize => formatter.write_str("event writer batch size must be positive"),
            Self::BatchSizeTooLarge => {
                formatter.write_str("event writer batch size exceeds the finite maximum")
            }
            Self::ZeroRetryDelay => {
                formatter.write_str("event writer retry delay must be positive")
            }
        }
    }
}

impl std::error::Error for EventWriterConfigError {}

/// Snapshot of durable-writer outcomes that never claims Diagnostics were persisted.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventWriterMetrics {
    /// Required events whose transaction completed, including idempotent replays.
    pub required_events_committed: u64,
    /// New durable rows inserted by successful Required-event transactions.
    pub rows_inserted: u64,
    /// Low-priority diagnostics intentionally consumed without a durable Required-event row.
    pub diagnostics_not_persisted: u64,
    /// Retryable `SQLite` open, migration, transaction, or blocking-worker failures.
    pub sqlite_write_failures: u64,
    /// Required events dropped because their stable durable identity can never append.
    pub required_events_quarantined: u64,
    /// Required events retained in the writer's one bounded pending transaction.
    pub pending_required: u64,
}

#[derive(Default)]
struct EventWriterMetricsState {
    required_events_committed: AtomicU64,
    rows_inserted: AtomicU64,
    diagnostics_not_persisted: AtomicU64,
    sqlite_write_failures: AtomicU64,
    required_events_quarantined: AtomicU64,
    pending_required: AtomicU64,
}

/// Cloneable observation handle for one running [`AsyncSqliteEventWriter`].
#[derive(Clone, Default)]
pub struct EventWriterMetricsHandle {
    state: Arc<EventWriterMetricsState>,
}

impl EventWriterMetricsHandle {
    /// Returns one non-blocking counter snapshot.
    #[must_use]
    pub fn snapshot(&self) -> EventWriterMetrics {
        EventWriterMetrics {
            required_events_committed: self.required_events_committed(),
            rows_inserted: self.rows_inserted(),
            diagnostics_not_persisted: self.diagnostics_not_persisted(),
            sqlite_write_failures: self.sqlite_write_failures(),
            required_events_quarantined: self.required_events_quarantined(),
            pending_required: self.pending_required(),
        }
    }

    fn required_events_committed(&self) -> u64 {
        self.state.required_events_committed.load(Ordering::Relaxed)
    }

    fn rows_inserted(&self) -> u64 {
        self.state.rows_inserted.load(Ordering::Relaxed)
    }

    fn diagnostics_not_persisted(&self) -> u64 {
        self.state.diagnostics_not_persisted.load(Ordering::Relaxed)
    }

    fn sqlite_write_failures(&self) -> u64 {
        self.state.sqlite_write_failures.load(Ordering::Relaxed)
    }

    fn required_events_quarantined(&self) -> u64 {
        self.state
            .required_events_quarantined
            .load(Ordering::Relaxed)
    }

    fn pending_required(&self) -> u64 {
        self.state.pending_required.load(Ordering::Relaxed)
    }
}

/// Writer-internal classification of one failed durable write attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EventWriteFailure {
    /// The store open, migration, transaction, or blocking worker failed in a retryable way.
    Transient,
    /// At least one submitted record can never append under its stable durable identity.
    PoisonedRecord,
}

/// Returns whether `error` deterministically rejects a specific record rather than the store.
fn is_record_poison(error: &StoreError) -> bool {
    matches!(
        error,
        StoreError::ConflictingGatewayEventReplay
            | StoreError::InvalidPersistedGatewayEvent
            | StoreError::DiagnosticEventNotPersistable
    )
}

/// Asynchronous consumer that batches already-admitted events onto a blocking `SQLite` worker.
///
/// Transient `SQLite` failures retain the one finite pending Required batch and retry it after the
/// configured delay. Deterministic record-level failures instead replay the batch one event per
/// transaction, keeping every healthy event durable and quarantining only the poisoned records.
/// Neither path creates an unbounded overflow queue, blocks the event producer, or increments a
/// persistence-success counter. Diagnostics are explicitly counted as non-persisted.
pub struct AsyncSqliteEventWriter {
    database_path: PathBuf,
    receiver: EventQueueReceiver,
    config: EventWriterConfig,
    store: Option<SqliteEventStore>,
    pending: Vec<GatewayEvent>,
    metrics: EventWriterMetricsHandle,
    telemetry: Option<Arc<TelemetryPipeline>>,
    #[cfg(test)]
    test_max_page_count: Option<Arc<std::sync::atomic::AtomicI64>>,
}

impl AsyncSqliteEventWriter {
    /// Creates one asynchronous consumer for a file-backed durable event log.
    #[must_use]
    pub fn new(
        database_path: impl AsRef<Path>,
        receiver: EventQueueReceiver,
        config: EventWriterConfig,
    ) -> Self {
        Self {
            database_path: database_path.as_ref().to_path_buf(),
            receiver,
            config,
            store: None,
            pending: Vec::with_capacity(config.batch_size()),
            metrics: EventWriterMetricsHandle::default(),
            telemetry: None,
            #[cfg(test)]
            test_max_page_count: None,
        }
    }

    /// Adds one non-blocking telemetry fan-out to this writer's existing single-consumer path.
    ///
    /// The pipeline observes every admitted event exactly once before durable batching. It is not
    /// invoked again when a pending `SQLite` batch retries, and it does not create another receiver
    /// that could compete for Required or Diagnostic events.
    #[must_use]
    pub fn with_telemetry_pipeline(mut self, telemetry: Arc<TelemetryPipeline>) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    /// Constrains the database page count at each test-only write attempt.
    #[cfg(test)]
    fn with_test_max_page_count(
        mut self,
        max_page_count: Arc<std::sync::atomic::AtomicI64>,
    ) -> Self {
        self.test_max_page_count = Some(max_page_count);
        self
    }

    /// Returns a cloneable non-blocking metrics handle for the writer lifecycle.
    #[must_use]
    pub fn metrics_handle(&self) -> EventWriterMetricsHandle {
        self.metrics.clone()
    }

    /// Consumes the queue until all senders close and the final Required batch is durable.
    ///
    /// A persistent transient store failure intentionally keeps this future alive with the bounded
    /// pending batch. A deterministic record-level failure quarantines only the poisoned events,
    /// so one unappendable record cannot wedge durable persistence for later Required events.
    /// Shutdown code must therefore observe the metrics or cancel the task explicitly; the writer
    /// never fabricates a successful flush.
    pub async fn run(mut self) -> EventWriterMetrics {
        loop {
            self.drain_ready_events();
            if self.pending.is_empty() {
                let Some(event) = self.receiver.recv().await else {
                    return self.metrics.snapshot();
                };
                self.accept_event(event);
                continue;
            }

            match self.write_events(self.pending.clone()).await {
                Ok(inserted) => {
                    let committed = u64::try_from(self.pending.len()).unwrap_or(u64::MAX);
                    self.metrics
                        .state
                        .required_events_committed
                        .fetch_add(committed, Ordering::Relaxed);
                    self.metrics
                        .state
                        .rows_inserted
                        .fetch_add(inserted, Ordering::Relaxed);
                    self.pending.clear();
                    self.update_pending_metric();
                }
                Err(EventWriteFailure::Transient) => {
                    self.metrics
                        .state
                        .sqlite_write_failures
                        .fetch_add(1, Ordering::Relaxed);
                    tokio::time::sleep(self.config.retry_delay()).await;
                }
                Err(EventWriteFailure::PoisonedRecord) => {
                    self.quarantine_poisoned_pending().await;
                }
            }
        }
    }

    /// Replays the pending batch one event per transaction after a deterministic record failure.
    ///
    /// Healthy events commit durably and leave the batch; a record that can never append is
    /// dropped and counted in `required_events_quarantined`. A transient failure stops this pass
    /// with the interrupted event and the unprocessed suffix retained as the pending batch, so a
    /// store outage during the replay never drops a healthy event.
    async fn quarantine_poisoned_pending(&mut self) {
        let mut remaining = VecDeque::from(std::mem::take(&mut self.pending));
        while let Some(event) = remaining.pop_front() {
            // The gauge must track what is still owed while the replay runs: an event already
            // committed or quarantined below is no longer pending, and leaving the pre-pass value
            // in place would double-count it as both durable and outstanding.
            self.metrics
                .state
                .pending_required
                .store(remaining.len() as u64 + 1, Ordering::Relaxed);
            match self.write_events(vec![event.clone()]).await {
                Ok(inserted) => {
                    self.metrics
                        .state
                        .required_events_committed
                        .fetch_add(1, Ordering::Relaxed);
                    self.metrics
                        .state
                        .rows_inserted
                        .fetch_add(inserted, Ordering::Relaxed);
                }
                Err(EventWriteFailure::PoisonedRecord) => {
                    self.metrics
                        .state
                        .required_events_quarantined
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(EventWriteFailure::Transient) => {
                    self.pending.push(event);
                    self.pending.extend(remaining);
                    self.update_pending_metric();
                    self.metrics
                        .state
                        .sqlite_write_failures
                        .fetch_add(1, Ordering::Relaxed);
                    tokio::time::sleep(self.config.retry_delay()).await;
                    return;
                }
            }
        }
        self.update_pending_metric();
    }

    fn drain_ready_events(&mut self) {
        while self.pending.len() < self.config.batch_size() {
            let Some(event) = self.receiver.try_recv() else {
                break;
            };
            self.accept_event(event);
        }
    }

    fn accept_event(&mut self, event: GatewayEvent) {
        if let Some(telemetry) = &self.telemetry {
            let _ = telemetry.observe_event(&event);
        }
        match event.priority() {
            GatewayEventPriority::Required => {
                self.pending.push(event);
                self.update_pending_metric();
            }
            GatewayEventPriority::Diagnostic => {
                self.metrics
                    .state
                    .diagnostics_not_persisted
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn update_pending_metric(&self) {
        let pending = u64::try_from(self.pending.len()).unwrap_or(u64::MAX);
        self.metrics
            .state
            .pending_required
            .store(pending, Ordering::Relaxed);
    }

    async fn write_events(&mut self, events: Vec<GatewayEvent>) -> Result<u64, EventWriteFailure> {
        let database_path = self.database_path.clone();
        let store = self.store.take();
        #[cfg(test)]
        let test_max_page_count = self.test_max_page_count.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut store = match store {
                Some(store) => store,
                None => match SqliteEventStore::open(database_path) {
                    Ok(store) => store,
                    Err(error) => return (None, Err(error)),
                },
            };
            #[cfg(test)]
            if let Some(max_page_count) = test_max_page_count {
                let limit = max_page_count.load(Ordering::Relaxed);
                if let Err(error) = store
                    .connection
                    .pragma_update(None, "max_page_count", limit)
                {
                    return (Some(store), Err(StoreError::from(error)));
                }
            }
            let result = store.append_batch(&events);
            (Some(store), result)
        })
        .await;
        match result {
            Ok((store, Ok(inserted))) => {
                self.store = store;
                Ok(u64::try_from(inserted).unwrap_or(u64::MAX))
            }
            Ok((store, Err(error))) => {
                self.store = store;
                if is_record_poison(&error) {
                    Err(EventWriteFailure::PoisonedRecord)
                } else {
                    Err(EventWriteFailure::Transient)
                }
            }
            Err(_) => {
                self.store = None;
                Err(EventWriteFailure::Transient)
            }
        }
    }
}

impl fmt::Debug for AsyncSqliteEventWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AsyncSqliteEventWriter")
            .field("database_path", &"<redacted>")
            .field("config", &self.config)
            .field("pending_required", &self.pending.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };

    use gateway_core::{
        AttemptEvent, AttemptOutcome, AttemptRetryDecision, ClientKeyId, CredentialId,
        DiagnosticEvent, EndpointId, GatewayError, GatewayErrorCode, GatewayEvent,
        GatewayEventSink, GatewayProtocol, HealthEvent, HealthEventId, HealthEventKind,
        RequestEvent, RequestId, ResponseId, RouteCandidateId, RouteId, UpstreamId, Usage,
        UsageEvent,
    };
    use gateway_observability::{
        BoundedEventQueue, EventQueueConfig, NoopOpenTelemetryExporter, OpenTelemetryExportOutcome,
        PrometheusMetrics, StructuredJsonExporter, StructuredJsonRecord, TelemetryPipeline,
    };

    use super::{
        AsyncSqliteEventWriter, EventWriterConfig, GatewayEventLogKind,
        MAX_EVENT_LOG_IDENTIFIER_BYTES, SqliteEventStore,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[derive(Default)]
    struct CollectingJsonExporter {
        records: Mutex<Vec<StructuredJsonRecord>>,
    }

    impl StructuredJsonExporter for CollectingJsonExporter {
        fn try_export(&self, record: &StructuredJsonRecord) -> OpenTelemetryExportOutcome {
            match self.records.lock() {
                Ok(mut records) => {
                    records.push(record.clone());
                    OpenTelemetryExportOutcome::Emitted
                }
                Err(_) => OpenTelemetryExportOutcome::Rejected,
            }
        }
    }

    impl CollectingJsonExporter {
        fn len(&self) -> Result<usize, std::io::Error> {
            self.records
                .lock()
                .map(|records| records.len())
                .map_err(|_| std::io::Error::other("collecting JSON exporter mutex poisoned"))
        }
    }

    fn sample_events() -> Result<(RequestId, Vec<GatewayEvent>), Box<dyn std::error::Error>> {
        let request_id = RequestId::try_new("request-01")?;
        let request = GatewayEvent::Request(RequestEvent::new(
            request_id.clone(),
            ClientKeyId::try_new("client-key-01")?,
            None,
            GatewayProtocol::OpenAiResponses,
            "requested-model".to_owned(),
            "public-model".to_owned(),
            Some("alias-model".to_owned()),
            true,
        ));
        let attempt = GatewayEvent::Attempt(AttemptEvent::new(
            request_id.clone(),
            1,
            RouteId::try_new("route-01")?,
            RouteCandidateId::try_new("candidate-01")?,
            CredentialId::try_new("credential-01")?,
            EndpointId::try_new("endpoint-01")?,
            UpstreamId::try_new("upstream-01")?,
            "internal-model".to_owned(),
            10,
            25,
            AttemptOutcome::Succeeded,
            AttemptRetryDecision::Completed,
        ));
        let usage = GatewayEvent::Usage(UsageEvent::from_usage(
            request_id.clone(),
            ResponseId::try_new("response-01")?,
            &Usage {
                input_tokens: Some(3),
                output_tokens: Some(5),
                ..Usage::default()
            },
        ));
        let health = GatewayEvent::Health(HealthEvent::new(
            HealthEventId::try_new("health-01")?,
            EndpointId::try_new("endpoint-01")?,
            Some(CredentialId::try_new("credential-01")?),
            Some("internal-model".to_owned()),
            30,
            HealthEventKind::CircuitRecovered,
        ));
        Ok((request_id, vec![request, attempt, usage, health]))
    }

    fn request_event(sequence: usize) -> Result<GatewayEvent, Box<dyn std::error::Error>> {
        Ok(GatewayEvent::Request(RequestEvent::new(
            RequestId::try_new(format!("p11-06-request-{sequence:04}"))?,
            ClientKeyId::try_new("p11-06-client-key")?,
            None,
            GatewayProtocol::OpenAiResponses,
            "p11-06-requested-model".to_owned(),
            "p11-06-public-model".to_owned(),
            None,
            false,
        )))
    }

    fn usage_event(
        request: &str,
        response: &str,
        input_tokens: u64,
    ) -> Result<GatewayEvent, Box<dyn std::error::Error>> {
        Ok(GatewayEvent::Usage(UsageEvent::from_usage(
            RequestId::try_new(request)?,
            ResponseId::try_new(response)?,
            &Usage {
                input_tokens: Some(input_tokens),
                output_tokens: Some(5),
                ..Usage::default()
            },
        )))
    }

    fn attempt_event(sequence: usize) -> Result<GatewayEvent, Box<dyn std::error::Error>> {
        Ok(GatewayEvent::Attempt(AttemptEvent::new(
            RequestId::try_new(format!("quarantine-request-{sequence:04}"))?,
            1,
            RouteId::try_new("route-01")?,
            RouteCandidateId::try_new("candidate-01")?,
            CredentialId::try_new("credential-01")?,
            EndpointId::try_new("endpoint-01")?,
            UpstreamId::try_new("upstream-01")?,
            "internal-model".to_owned(),
            10,
            25,
            AttemptOutcome::Succeeded,
            AttemptRetryDecision::Completed,
        )))
    }

    fn temporary_path(suffix: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "cpa-rust-gateway-p4-07-{}-{}-{suffix}",
            std::process::id(),
            sequence
        ))
    }

    #[test]
    fn batches_are_atomic_and_identical_replays_are_idempotent() -> TestResult {
        let (_request_id, events) = sample_events()?;
        let mut store = SqliteEventStore::open_in_memory()?;
        store.connection.execute_batch(
            "CREATE TRIGGER gateway_event_log_test_reject_attempt \
             BEFORE INSERT ON gateway_event_log WHEN NEW.event_type = 'attempt' \
             BEGIN SELECT RAISE(ABORT, 'test forced rollback'); END;",
        )?;

        assert!(store.append_batch(&events[..2]).is_err());
        assert!(store.list_events()?.is_empty());
        store
            .connection
            .execute_batch("DROP TRIGGER gateway_event_log_test_reject_attempt;")?;

        assert_eq!(store.append_batch(&events[..2])?, 2);
        assert_eq!(store.append_batch(&events[..2])?, 0);
        assert_eq!(store.list_events()?.len(), 2);

        let conflicting_request = GatewayEvent::Request(RequestEvent::new(
            RequestId::try_new("request-01")?,
            ClientKeyId::try_new("client-key-01")?,
            None,
            GatewayProtocol::OpenAiResponses,
            "different-requested-model".to_owned(),
            "public-model".to_owned(),
            Some("alias-model".to_owned()),
            true,
        ));
        assert!(matches!(
            store.append_batch(&[conflicting_request]),
            Err(crate::StoreError::ConflictingGatewayEventReplay)
        ));
        assert_eq!(store.list_events()?.len(), 2);
        Ok(())
    }

    #[test]
    fn file_reopen_restores_request_attempt_usage_and_health_and_quick_check() -> TestResult {
        let (request_id, events) = sample_events()?;
        let database_path = temporary_path("events.sqlite");
        {
            let mut store = SqliteEventStore::open(&database_path)?;
            assert_eq!(store.append_batch(&events)?, 4);
            store.quick_check()?;
        }

        let store = SqliteEventStore::open(&database_path)?;
        let request_events = store.events_for_request(&request_id)?;
        assert_eq!(request_events.len(), 3);
        assert_eq!(request_events[0].kind(), GatewayEventLogKind::Request);
        assert_eq!(request_events[1].kind(), GatewayEventLogKind::Attempt);
        assert_eq!(request_events[2].kind(), GatewayEventLogKind::Usage);
        let health_events = store.health_events()?;
        assert_eq!(health_events.len(), 1);
        assert_eq!(health_events[0].kind(), GatewayEventLogKind::Health);
        store.quick_check()?;
        drop(store);
        fs::remove_file(database_path)?;
        Ok(())
    }

    #[test]
    fn diagnostics_cannot_be_mistaken_for_durable_required_events() -> TestResult {
        let mut store = SqliteEventStore::open_in_memory()?;
        let diagnostic = GatewayEvent::Diagnostic(DiagnosticEvent::new(GatewayError::new(
            GatewayErrorCode::InternalError,
            gateway_core::ErrorScope::Internal,
        )));
        assert!(matches!(
            store.append_batch(&[diagnostic]),
            Err(crate::StoreError::DiagnosticEventNotPersistable)
        ));
        assert!(store.list_events()?.is_empty());
        Ok(())
    }

    #[test]
    fn full_required_queue_stays_non_blocking_before_writer_consumption() -> TestResult {
        let (_request_id, events) = sample_events()?;
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        let _writer = AsyncSqliteEventWriter::new(
            temporary_path("unused.sqlite"),
            receiver,
            EventWriterConfig::default(),
        );
        assert_eq!(
            queue.try_emit(events[0].clone()),
            gateway_core::EventEmission::Enqueued
        );
        assert_eq!(
            queue.try_emit(events[1].clone()),
            gateway_core::EventEmission::RequiredQueueFull
        );
        assert_eq!(queue.metrics().required_queue_full, 1);
        Ok(())
    }

    #[tokio::test]
    async fn writer_retains_failed_pending_batch_then_recovers_when_database_becomes_available()
    -> TestResult {
        let (_request_id, events) = sample_events()?;
        let parent = temporary_path("recovery");
        let database_path = parent.join("events.sqlite");
        let _ = fs::remove_dir_all(&parent);
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(2, 1)?)?;
        let telemetry_metrics = Arc::new(PrometheusMetrics::default());
        let json = Arc::new(CollectingJsonExporter::default());
        let telemetry = Arc::new(TelemetryPipeline::new(
            telemetry_metrics.clone(),
            json.clone(),
            Arc::new(NoopOpenTelemetryExporter),
        ));
        let writer = AsyncSqliteEventWriter::new(
            &database_path,
            receiver,
            EventWriterConfig::try_new(1, Duration::from_millis(5))?,
        )
        .with_telemetry_pipeline(telemetry);
        let metrics = writer.metrics_handle();
        assert_eq!(
            queue.try_emit(events[0].clone()),
            gateway_core::EventEmission::Enqueued
        );
        drop(queue);

        let writer = tokio::spawn(writer.run());
        let deadline = Instant::now() + Duration::from_secs(2);
        while metrics.snapshot().sqlite_write_failures == 0 {
            assert!(
                Instant::now() < deadline,
                "writer did not observe the unavailable database"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(metrics.snapshot().pending_required, 1);

        fs::create_dir_all(&parent)?;
        let reported = tokio::time::timeout(Duration::from_secs(2), writer).await??;
        assert_eq!(reported.required_events_committed, 1);
        assert_eq!(reported.rows_inserted, 1);
        assert_eq!(reported.pending_required, 0);
        assert_eq!(telemetry_metrics.snapshot().request_events, 1);
        assert_eq!(json.len()?, 1);
        let store = SqliteEventStore::open(&database_path)?;
        assert_eq!(store.list_events()?.len(), 1);
        drop(store);
        fs::remove_dir_all(parent)?;
        Ok(())
    }

    #[tokio::test]
    async fn crashed_pending_batch_requires_source_replay_then_restarts_cleanly() -> TestResult {
        let parent = temporary_path("p11-06-restart");
        let database_path = parent.join("events.sqlite");
        let _ = fs::remove_dir_all(&parent);
        let event = request_event(1)?;
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        let writer = AsyncSqliteEventWriter::new(
            &database_path,
            receiver,
            EventWriterConfig::try_new(1, Duration::from_millis(25))?,
        );
        let metrics = writer.metrics_handle();
        assert_eq!(
            queue.try_emit(event.clone()),
            gateway_core::EventEmission::Enqueued
        );
        drop(queue);

        let writer = tokio::spawn(writer.run());
        let deadline = Instant::now() + Duration::from_secs(2);
        while metrics.snapshot().sqlite_write_failures == 0 {
            assert!(
                Instant::now() < deadline,
                "writer did not observe the unavailable database before the injected crash"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(metrics.snapshot().required_events_committed, 0);
        assert_eq!(metrics.snapshot().pending_required, 1);
        writer.abort();
        assert!(
            writer.await.is_err(),
            "aborted writer unexpectedly joined cleanly"
        );

        fs::create_dir_all(&parent)?;
        let (replay_queue, replay_receiver) =
            BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        let replay_writer = AsyncSqliteEventWriter::new(
            &database_path,
            replay_receiver,
            EventWriterConfig::try_new(1, Duration::from_millis(5))?,
        );
        assert_eq!(
            replay_queue.try_emit(event),
            gateway_core::EventEmission::Enqueued
        );
        drop(replay_queue);
        let reported = tokio::time::timeout(Duration::from_secs(2), replay_writer.run()).await?;
        assert_eq!(reported.required_events_committed, 1);
        assert_eq!(reported.rows_inserted, 1);
        let store = SqliteEventStore::open(&database_path)?;
        assert_eq!(store.list_events()?.len(), 1);
        store.quick_check()?;
        drop(store);
        fs::remove_dir_all(parent)?;
        Ok(())
    }

    #[tokio::test]
    async fn sqlite_full_retains_one_finite_batch_until_capacity_returns() -> TestResult {
        const EVENT_COUNT: usize = 1_024;

        let database_path = temporary_path("p11-06-sqlite-full.sqlite");
        let committed_events = (0..EVENT_COUNT)
            .map(request_event)
            .collect::<Result<Vec<_>, _>>()?;
        let events = (EVENT_COUNT..EVENT_COUNT.saturating_mul(2))
            .map(request_event)
            .collect::<Result<Vec<_>, _>>()?;
        let page_count = {
            let mut store = SqliteEventStore::open(&database_path)?;
            assert_eq!(store.append_batch(&committed_events)?, EVENT_COUNT);
            store
                .connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
            let page_count: i64 = store
                .connection
                .query_row("PRAGMA page_count", [], |row| row.get(0))?;
            store
                .connection
                .execute_batch(&format!("PRAGMA max_page_count = {page_count};"))?;
            assert!(matches!(
                store.append_batch(&events),
                Err(crate::StoreError::Sqlite(rusqlite::Error::SqliteFailure(code, _)))
                    if code.extended_code == rusqlite::ffi::SQLITE_FULL
            ));
            page_count
        };

        let page_limit = Arc::new(std::sync::atomic::AtomicI64::new(page_count));
        let (queue, receiver) =
            BoundedEventQueue::try_new(EventQueueConfig::try_new(EVENT_COUNT, 1)?)?;
        let writer = AsyncSqliteEventWriter::new(
            &database_path,
            receiver,
            EventWriterConfig::try_new(EVENT_COUNT, Duration::from_millis(25))?,
        )
        .with_test_max_page_count(Arc::clone(&page_limit));
        let metrics = writer.metrics_handle();
        for event in events {
            assert_eq!(queue.try_emit(event), gateway_core::EventEmission::Enqueued);
        }
        drop(queue);

        let writer = tokio::spawn(writer.run());
        let deadline = Instant::now() + Duration::from_secs(3);
        while metrics.snapshot().sqlite_write_failures == 0 {
            assert!(
                Instant::now() < deadline,
                "max_page_count did not produce a SQLite full-write failure"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(metrics.snapshot().required_events_committed, 0);
        assert_eq!(metrics.snapshot().pending_required, EVENT_COUNT as u64);

        page_limit.store(page_count.saturating_add(1_024), Ordering::Relaxed);

        let reported = tokio::time::timeout(Duration::from_secs(5), writer).await??;
        assert_eq!(reported.required_events_committed, EVENT_COUNT as u64);
        assert_eq!(reported.rows_inserted, EVENT_COUNT as u64);
        assert_eq!(reported.pending_required, 0);
        let store = SqliteEventStore::open(&database_path)?;
        assert_eq!(store.list_events()?.len(), EVENT_COUNT.saturating_mul(2));
        store.quick_check()?;
        drop(store);
        fs::remove_file(database_path)?;
        Ok(())
    }

    #[tokio::test]
    async fn writer_fans_out_one_admitted_event_to_store_and_telemetry_without_a_second_receiver()
    -> TestResult {
        let (_request_id, events) = sample_events()?;
        let database_path = temporary_path("telemetry-fanout.sqlite");
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        let telemetry_metrics = Arc::new(PrometheusMetrics::default());
        let json = Arc::new(CollectingJsonExporter::default());
        let telemetry = Arc::new(TelemetryPipeline::new(
            telemetry_metrics.clone(),
            json.clone(),
            Arc::new(NoopOpenTelemetryExporter),
        ));
        let writer = AsyncSqliteEventWriter::new(
            &database_path,
            receiver,
            EventWriterConfig::try_new(1, Duration::from_millis(1))?,
        )
        .with_telemetry_pipeline(telemetry);

        assert_eq!(
            queue.try_emit(events[0].clone()),
            gateway_core::EventEmission::Enqueued
        );
        drop(queue);

        let reported = writer.run().await;
        assert_eq!(reported.required_events_committed, 1);
        assert_eq!(reported.rows_inserted, 1);
        assert_eq!(telemetry_metrics.snapshot().request_events, 1);
        assert_eq!(json.len()?, 1);

        let store = SqliteEventStore::open(&database_path)?;
        assert_eq!(store.list_events()?.len(), 1);
        drop(store);
        fs::remove_file(database_path)?;
        Ok(())
    }

    #[tokio::test]
    async fn writer_counts_diagnostics_without_persisting_them() -> TestResult {
        let database_path = temporary_path("diagnostic.sqlite");
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(1, 1)?)?;
        let telemetry_metrics = Arc::new(PrometheusMetrics::default());
        let json = Arc::new(CollectingJsonExporter::default());
        let telemetry = Arc::new(TelemetryPipeline::new(
            telemetry_metrics.clone(),
            json.clone(),
            Arc::new(NoopOpenTelemetryExporter),
        ));
        let writer = AsyncSqliteEventWriter::new(
            &database_path,
            receiver,
            EventWriterConfig::try_new(1, Duration::from_millis(1))?,
        )
        .with_telemetry_pipeline(telemetry);
        assert_eq!(
            queue.try_emit(GatewayEvent::Diagnostic(DiagnosticEvent::new(
                GatewayError::new(
                    GatewayErrorCode::InternalError,
                    gateway_core::ErrorScope::Internal,
                )
            ))),
            gateway_core::EventEmission::Enqueued
        );
        drop(queue);
        let reported = writer.run().await;
        assert_eq!(reported.diagnostics_not_persisted, 1);
        assert_eq!(reported.required_events_committed, 0);
        assert_eq!(telemetry_metrics.snapshot().diagnostic_events, 1);
        assert_eq!(json.len()?, 1);
        let store = SqliteEventStore::open(&database_path)?;
        assert!(store.list_events()?.is_empty());
        drop(store);
        fs::remove_file(database_path)?;
        Ok(())
    }

    #[tokio::test]
    async fn writer_quarantines_conflicting_replay_and_commits_healthy_batch_events() -> TestResult
    {
        let database_path = temporary_path("quarantine-batch.sqlite");
        let committed_usage = usage_event("request-a", "response-shared", 3)?;
        {
            let mut store = SqliteEventStore::open(&database_path)?;
            assert_eq!(
                store.append_batch(std::slice::from_ref(&committed_usage))?,
                1
            );
        }

        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(4, 1)?)?;
        let writer = AsyncSqliteEventWriter::new(
            &database_path,
            receiver,
            EventWriterConfig::try_new(4, Duration::from_millis(5))?,
        );
        let metrics = writer.metrics_handle();
        for event in [
            request_event(1)?,
            usage_event("request-b", "response-shared", 7)?,
            attempt_event(1)?,
        ] {
            assert_eq!(queue.try_emit(event), gateway_core::EventEmission::Enqueued);
        }

        let writer = tokio::spawn(writer.run());
        let deadline = Instant::now() + Duration::from_secs(2);
        while metrics.snapshot().required_events_quarantined == 0 {
            assert!(
                Instant::now() < deadline,
                "writer did not quarantine the conflicting replay"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(
            queue.try_emit(request_event(2)?),
            gateway_core::EventEmission::Enqueued
        );
        drop(queue);

        let reported = tokio::time::timeout(Duration::from_secs(2), writer).await??;
        assert_eq!(reported.required_events_committed, 3);
        assert_eq!(reported.rows_inserted, 3);
        assert_eq!(reported.required_events_quarantined, 1);
        assert_eq!(reported.sqlite_write_failures, 0);
        assert_eq!(reported.pending_required, 0);

        let store = SqliteEventStore::open(&database_path)?;
        let events = store.list_events()?;
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].event(), &committed_usage);
        assert_eq!(events[1].kind(), GatewayEventLogKind::Request);
        assert_eq!(events[2].kind(), GatewayEventLogKind::Attempt);
        drop(store);
        fs::remove_file(database_path)?;
        Ok(())
    }

    #[tokio::test]
    async fn writer_quarantines_poisoned_singleton_batch_and_keeps_consuming() -> TestResult {
        let database_path = temporary_path("quarantine-singleton.sqlite");
        {
            let mut store = SqliteEventStore::open(&database_path)?;
            assert_eq!(
                store.append_batch(&[usage_event("request-a", "response-shared", 3)?])?,
                1
            );
        }

        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(2, 1)?)?;
        let writer = AsyncSqliteEventWriter::new(
            &database_path,
            receiver,
            EventWriterConfig::try_new(1, Duration::from_millis(5))?,
        );
        let metrics = writer.metrics_handle();
        assert_eq!(
            queue.try_emit(usage_event("request-b", "response-shared", 7)?),
            gateway_core::EventEmission::Enqueued
        );

        let writer = tokio::spawn(writer.run());
        let deadline = Instant::now() + Duration::from_secs(2);
        while metrics.snapshot().required_events_quarantined == 0 {
            assert!(
                Instant::now() < deadline,
                "writer did not quarantine the poisoned singleton batch"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(
            queue.try_emit(request_event(1)?),
            gateway_core::EventEmission::Enqueued
        );
        drop(queue);

        let reported = tokio::time::timeout(Duration::from_secs(2), writer).await??;
        assert_eq!(reported.required_events_quarantined, 1);
        assert_eq!(reported.required_events_committed, 1);
        assert_eq!(reported.rows_inserted, 1);
        assert_eq!(reported.sqlite_write_failures, 0);
        assert_eq!(reported.pending_required, 0);
        let store = SqliteEventStore::open(&database_path)?;
        assert_eq!(store.list_events()?.len(), 2);
        drop(store);
        fs::remove_file(database_path)?;
        Ok(())
    }

    #[tokio::test]
    async fn an_oversized_upstream_identifier_is_quarantined_instead_of_wedging_the_writer()
    -> TestResult {
        // A Usage event id is the upstream-supplied response id, so its length is attacker
        // controlled. The durable schema bounds it; without the pre-insert check the resulting
        // CHECK violation is an ordinary SQLite error, which the writer would retry forever.
        let database_path = temporary_path("quarantine-oversized-identifier.sqlite");
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(2, 1)?)?;
        let writer = AsyncSqliteEventWriter::new(
            &database_path,
            receiver,
            EventWriterConfig::try_new(1, Duration::from_millis(5))?,
        );
        let metrics = writer.metrics_handle();
        let oversized = "r".repeat(MAX_EVENT_LOG_IDENTIFIER_BYTES + 1);
        assert_eq!(
            queue.try_emit(usage_event("request-oversized", &oversized, 3)?),
            gateway_core::EventEmission::Enqueued
        );

        let writer = tokio::spawn(writer.run());
        let deadline = Instant::now() + Duration::from_secs(2);
        while metrics.snapshot().required_events_quarantined == 0 {
            assert!(
                Instant::now() < deadline,
                "writer did not quarantine the oversized identifier"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        // The writer must keep consuming: a poisoned record may not block later Required events.
        assert_eq!(
            queue.try_emit(request_event(1)?),
            gateway_core::EventEmission::Enqueued
        );
        drop(queue);

        let reported = tokio::time::timeout(Duration::from_secs(2), writer).await??;
        assert_eq!(reported.required_events_quarantined, 1);
        assert_eq!(reported.required_events_committed, 1);
        assert_eq!(reported.sqlite_write_failures, 0);
        assert_eq!(reported.pending_required, 0);
        let store = SqliteEventStore::open(&database_path)?;
        assert_eq!(store.list_events()?.len(), 1);
        drop(store);
        fs::remove_file(database_path)?;
        Ok(())
    }

    #[tokio::test]
    async fn transient_failure_during_quarantine_fallback_preserves_healthy_events() -> TestResult {
        const PRE_FILL: usize = 64;

        let database_path = temporary_path("quarantine-transient.sqlite");
        let committed_events = (0..PRE_FILL)
            .map(request_event)
            .collect::<Result<Vec<_>, _>>()?;
        let page_count: i64 = {
            let mut store = SqliteEventStore::open(&database_path)?;
            assert_eq!(store.append_batch(&committed_events)?, PRE_FILL);
            assert_eq!(
                store.append_batch(&[usage_event("request-a", "response-shared", 3)?])?,
                1
            );
            store
                .connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
            store
                .connection
                .query_row("PRAGMA page_count", [], |row| row.get(0))?
        };

        let page_limit = Arc::new(std::sync::atomic::AtomicI64::new(page_count));
        let (queue, receiver) = BoundedEventQueue::try_new(EventQueueConfig::try_new(4, 1)?)?;
        let writer = AsyncSqliteEventWriter::new(
            &database_path,
            receiver,
            EventWriterConfig::try_new(4, Duration::from_millis(5))?,
        )
        .with_test_max_page_count(Arc::clone(&page_limit));
        let metrics = writer.metrics_handle();
        let large_request = GatewayEvent::Request(RequestEvent::new(
            RequestId::try_new("quarantine-large-request")?,
            ClientKeyId::try_new("client-key-01")?,
            None,
            GatewayProtocol::OpenAiResponses,
            "m".repeat(65_536),
            "public-model".to_owned(),
            None,
            false,
        ));
        for event in [
            usage_event("request-b", "response-shared", 7)?,
            large_request,
            attempt_event(1)?,
        ] {
            assert_eq!(queue.try_emit(event), gateway_core::EventEmission::Enqueued);
        }
        drop(queue);

        let writer = tokio::spawn(writer.run());
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let snapshot = metrics.snapshot();
            if snapshot.required_events_quarantined == 1
                && snapshot.sqlite_write_failures > 0
                && snapshot.pending_required == 2
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "fallback did not observe the injected transient failure"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(metrics.snapshot().required_events_committed, 0);

        page_limit.store(page_count.saturating_add(1_024), Ordering::Relaxed);
        let reported = tokio::time::timeout(Duration::from_secs(5), writer).await??;
        assert_eq!(reported.required_events_quarantined, 1);
        assert_eq!(reported.required_events_committed, 2);
        assert_eq!(reported.rows_inserted, 2);
        assert_eq!(reported.pending_required, 0);
        let store = SqliteEventStore::open(&database_path)?;
        assert_eq!(store.list_events()?.len(), PRE_FILL + 3);
        store.quick_check()?;
        drop(store);
        fs::remove_file(database_path)?;
        Ok(())
    }
}
