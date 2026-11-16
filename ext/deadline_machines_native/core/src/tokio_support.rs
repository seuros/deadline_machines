//! Tokio clock and timeout adapters.
//!
//! The one place this crate reads a clock: [`TokioClock`] follows Tokio's
//! time, paused test time included, so async applications use the same
//! [`Deadline`] for budget arithmetic and executor timeouts.
use core::future::{Future, poll_fn};
use core::pin::pin;
use core::task::Poll;
use core::time::Duration;

use tokio::time::Instant as TokioInstant;

use crate::{Deadline, Refusal, Scope};

/// A monotonic clock whose epoch is the moment it was built, in Tokio's time
/// domain.
#[derive(Debug, Clone, Copy)]
pub struct TokioClock {
    origin: TokioInstant,
}

impl TokioClock {
    /// Starts a time domain at Tokio's present. Also the [`Default`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: TokioInstant::now(),
        }
    }

    /// Tokio's present as a `Duration` since this clock's epoch: the `now`
    /// to hand [`Deadline`] and [`Scope`].
    #[must_use]
    pub fn now(&self) -> Duration {
        TokioInstant::now().saturating_duration_since(self.origin)
    }

    /// The Tokio instant at which `deadline`, built on this clock, runs out.
    ///
    /// `None` when it never does: an unbounded deadline, or one past the
    /// last instant Tokio can represent, which no timer would ever reach.
    #[must_use]
    pub fn to_tokio_instant(&self, deadline: Deadline) -> Option<TokioInstant> {
        deadline
            .expires_at()
            .and_then(|at| self.origin.checked_add(at))
    }
}

impl Default for TokioClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs `future` until it completes or `deadline`, built on `clock`, runs
/// out. A deadline that never runs out simply awaits the future.
///
/// # Errors
///
/// [`Refusal::Exceeded`] when the deadline passes first.
pub async fn timeout<F: Future>(
    clock: &TokioClock,
    deadline: Deadline,
    future: F,
) -> Result<F::Output, Refusal> {
    match clock.to_tokio_instant(deadline) {
        Some(at) => tokio::time::timeout_at(at, future)
            .await
            .map_err(|_| Refusal::Exceeded),
        None => Ok(future.await),
    }
}

impl Scope {
    /// Resolves once this scope or any ancestor is cancelled. Cancel-safe.
    pub async fn cancelled(&self) {
        self.token().cancelled().await;
    }

    /// Runs `future` under this scope's remaining budget and cancellation,
    /// in the time domain of `clock`.
    ///
    /// # Errors
    ///
    /// [`Refusal::Cancelled`] when the scope is cancelled, which outranks
    /// [`Refusal::Exceeded`] for a spent budget.
    pub async fn tokio_timeout<F: Future>(
        &self,
        clock: &TokioClock,
        future: F,
    ) -> Result<F::Output, Refusal> {
        self.check(clock.now())?;
        let mut future = pin!(future);
        let mut cancelled = pin!(self.cancelled());
        let guarded = poll_fn(|cx| {
            if cancelled.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Err(Refusal::Cancelled));
            }
            future.as_mut().poll(cx).map(Ok)
        });
        timeout(clock, self.deadline(), guarded).await?
    }
}

#[cfg(test)]
mod tests;
