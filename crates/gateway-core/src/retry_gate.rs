//! Transport-neutral cancellation and transparent-retry boundary port.

use std::{future::Future, pin::Pin};

/// A boxed, sendable wait operation for a downstream cancellation request.
pub type TransparentRetryGateFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Reads the downstream boundary that determines whether an upstream retry is transparent.
///
/// Implementations must report actual client-visible semantic delivery, not a decoded or queued
/// upstream event. The cancellation future lets a request coordinator drop an in-flight upstream
/// future promptly, while the two synchronous methods make the preflight and post-failure decision
/// deterministic without exposing transport-specific types.
pub trait TransparentRetryGate: Send + Sync {
    /// Returns whether the downstream request has already been cancelled.
    fn is_cancelled(&self) -> bool;

    /// Returns whether another upstream attempt remains invisible to the client.
    fn allows_transparent_retry(&self) -> bool;

    /// Resolves when the downstream request is cancelled.
    fn cancelled(&self) -> TransparentRetryGateFuture<'_>;
}
