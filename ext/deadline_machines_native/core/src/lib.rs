#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

/// Compiles and runs the README's examples as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub const NS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(pub u64);

pub trait Clock {
    fn now_ns(&self) -> u64;

    fn now(&self) -> Instant {
        Instant(self.now_ns())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Deadline {
    expires_at: Option<Instant>,
}

impl Deadline {
    #[must_use]
    pub const fn never() -> Self {
        Self { expires_at: None }
    }

    #[must_use]
    pub const fn at(instant: Instant) -> Self {
        Self {
            expires_at: Some(instant),
        }
    }

    #[must_use]
    pub const fn after(now: Instant, timeout_ns: u64) -> Self {
        Self::at(Instant(now.0.saturating_add(timeout_ns)))
    }

    #[must_use]
    pub const fn expires_at(&self) -> Option<Instant> {
        self.expires_at
    }

    #[must_use]
    pub const fn remaining_ns(&self, now: Instant) -> Option<u64> {
        match self.expires_at {
            Some(expiry) => Some(expiry.0.saturating_sub(now.0)),
            None => None,
        }
    }

    #[must_use]
    pub const fn is_expired(&self, now: Instant) -> bool {
        matches!(self.remaining_ns(now), Some(0))
    }

    #[must_use]
    pub const fn derive(&self, now: Instant, timeout_ns: u64) -> Self {
        let candidate = Instant(now.0.saturating_add(timeout_ns));
        match self.expires_at {
            None => Self::at(candidate),
            Some(parent) if candidate.0 < parent.0 => Self::at(candidate),
            Some(parent) => Self::at(parent),
        }
    }

    #[must_use]
    pub const fn tighten(&self, other: Self) -> Self {
        match (self.expires_at, other.expires_at) {
            (None, any) | (any, None) => Self { expires_at: any },
            (Some(a), Some(b)) if a.0 < b.0 => Self::at(a),
            (Some(_), Some(b)) => Self::at(b),
        }
    }

    #[must_use]
    pub const fn clamp_sleep_ns(&self, now: Instant, requested_ns: u64) -> u64 {
        match self.remaining_ns(now) {
            Some(left) if left < requested_ns => left,
            _ => requested_ns,
        }
    }
}

#[cfg(feature = "alloc")]
pub mod cancel {
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicBool, Ordering};

    #[derive(Debug)]
    struct Node {
        cancelled: AtomicBool,
        parent: Option<Arc<Self>>,
    }

    impl Node {
        fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::Acquire)
                || self
                    .parent
                    .as_ref()
                    .is_some_and(|parent| parent.is_cancelled())
        }
    }

    #[derive(Debug, Clone)]
    pub struct CancelToken {
        node: Arc<Node>,
    }

    impl CancelToken {
        #[must_use]
        pub fn root() -> Self {
            Self {
                node: Arc::new(Node {
                    cancelled: AtomicBool::new(false),
                    parent: None,
                }),
            }
        }

        #[must_use]
        pub fn child(&self) -> Self {
            Self {
                node: Arc::new(Node {
                    cancelled: AtomicBool::new(false),
                    parent: Some(Arc::clone(&self.node)),
                }),
            }
        }

        pub fn cancel(&self) {
            self.node.cancelled.store(true, Ordering::Release);
        }

        #[must_use]
        pub fn is_cancelled(&self) -> bool {
            self.node.is_cancelled()
        }
    }

    impl Default for CancelToken {
        fn default() -> Self {
            Self::root()
        }
    }
}

/// Why a [`Scope`] stopped admitting work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// The budget ran out.
    Exceeded,
    /// Someone upstream gave up.
    Cancelled,
}

impl core::fmt::Display for Stopped {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Exceeded => "deadline exceeded",
            Self::Cancelled => "deadline cancelled",
        })
    }
}

impl core::error::Error for Stopped {}

#[cfg(feature = "alloc")]
mod scope {
    use super::{Deadline, Instant, Stopped, cancel::CancelToken};

    /// A budget and a cancellation handle as one value, which is what a retry
    /// policy or a circuit breaker actually needs to consult.
    ///
    /// ```
    /// use deadline_machines::{Deadline, Instant, Scope, Stopped};
    ///
    /// let now = Instant(0);
    /// let request = Scope::new(Deadline::after(now, 5_000));
    ///
    /// let attempt = request.derive(now, 9_000);
    /// assert_eq!(attempt.deadline(), request.deadline());
    ///
    /// request.cancel();
    /// assert_eq!(attempt.check(now), Err(Stopped::Cancelled));
    /// ```
    #[derive(Debug, Clone)]
    pub struct Scope {
        deadline: Deadline,
        token: CancelToken,
    }

    impl Scope {
        #[must_use]
        pub fn new(deadline: Deadline) -> Self {
            Self {
                deadline,
                token: CancelToken::root(),
            }
        }

        /// A scope with no time limit, cancellable only.
        #[must_use]
        pub fn never() -> Self {
            Self::new(Deadline::never())
        }

        #[must_use]
        pub const fn deadline(&self) -> Deadline {
            self.deadline
        }

        #[must_use]
        pub const fn remaining_ns(&self, now: Instant) -> Option<u64> {
            self.deadline.remaining_ns(now)
        }

        #[must_use]
        pub fn is_cancelled(&self) -> bool {
            self.token.is_cancelled()
        }

        #[must_use]
        pub const fn is_expired(&self, now: Instant) -> bool {
            self.deadline.is_expired(now)
        }

        #[must_use]
        pub fn is_done(&self, now: Instant) -> bool {
            self.is_cancelled() || self.is_expired(now)
        }

        /// Cancellation reaches every scope derived from this one, now or later.
        pub fn cancel(&self) {
            self.token.cancel();
        }

        /// The cooperative checkpoint.
        ///
        /// # Errors
        ///
        /// [`Stopped::Cancelled`] when this scope or an ancestor was cancelled,
        /// which outranks [`Stopped::Exceeded`] for a spent budget. Matches the
        /// precedence on the Ruby side.
        pub fn check(&self, now: Instant) -> Result<(), Stopped> {
            if self.is_cancelled() {
                return Err(Stopped::Cancelled);
            }
            if self.is_expired(now) {
                return Err(Stopped::Exceeded);
            }
            Ok(())
        }

        /// A child scope whose deadline is the tighter of this one and
        /// `timeout_ns` from `now`.
        #[must_use]
        pub fn derive(&self, now: Instant, timeout_ns: u64) -> Self {
            Self {
                deadline: self.deadline.derive(now, timeout_ns),
                token: self.token.child(),
            }
        }

        /// A child scope with the same deadline, cancellable on its own.
        #[must_use]
        pub fn child(&self) -> Self {
            Self {
                deadline: self.deadline,
                token: self.token.child(),
            }
        }

        /// The longest sleep that still fits the budget. A retry policy calls
        /// this before each backoff so it cannot outlive its caller.
        #[must_use]
        pub const fn clamp_sleep_ns(&self, now: Instant, requested_ns: u64) -> u64 {
            self.deadline.clamp_sleep_ns(now, requested_ns)
        }
    }
}

#[cfg(feature = "alloc")]
pub use scope::Scope;

#[cfg(feature = "std")]
mod system_clock {
    use super::Clock;
    use std::time::Instant as StdInstant;

    #[derive(Debug, Clone, Copy)]
    pub struct SystemClock {
        origin: StdInstant,
    }

    impl SystemClock {
        #[must_use]
        pub fn new() -> Self {
            Self {
                origin: StdInstant::now(),
            }
        }
    }

    impl Default for SystemClock {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Clock for SystemClock {
        fn now_ns(&self) -> u64 {
            u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
        }
    }
}

#[cfg(feature = "std")]
pub use system_clock::SystemClock;

/// Tokio clock and timeout adapters.
///
/// This feature keeps Tokio out of the core crate while allowing async
/// applications to use the same deadline value for budget arithmetic and
/// executor timeouts.
#[cfg(feature = "tokio")]
pub mod tokio_support {
    use super::{Clock, Deadline};
    use core::future::Future;
    use std::time::Duration;
    use tokio::time::Instant as TokioInstant;

    /// Why a Tokio deadline could not be enforced.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TimeoutError {
        /// The deadline exceeded Tokio's representable instant range.
        OutOfRange,
        /// The deadline elapsed before the future completed.
        Elapsed,
    }

    /// A clock whose time domain follows Tokio, including paused test time.
    #[derive(Debug, Clone, Copy)]
    pub struct TokioClock {
        origin: TokioInstant,
    }

    impl TokioClock {
        /// Start a Tokio-relative monotonic time domain.
        #[must_use]
        pub fn new() -> Self {
            Self {
                origin: TokioInstant::now(),
            }
        }

        /// Convert a deadline created from this clock into Tokio's time domain.
        ///
        /// `None` means the deadline is unbounded. A deadline must be converted
        /// with the same clock that was used to create it.
        ///
        /// # Errors
        ///
        /// Returns [`TimeoutError::OutOfRange`] if Tokio cannot represent the
        /// corresponding instant.
        pub fn to_tokio_instant(
            &self,
            deadline: Deadline,
        ) -> Result<Option<TokioInstant>, TimeoutError> {
            deadline
                .expires_at()
                .map(|at| {
                    self.origin
                        .checked_add(Duration::from_nanos(at.0))
                        .ok_or(TimeoutError::OutOfRange)
                })
                .transpose()
        }
    }

    impl Default for TokioClock {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Clock for TokioClock {
        fn now_ns(&self) -> u64 {
            u64::try_from(
                TokioInstant::now()
                    .saturating_duration_since(self.origin)
                    .as_nanos(),
            )
            .unwrap_or(u64::MAX)
        }
    }

    /// Enforce a deadline around a future using Tokio's clock and timer.
    ///
    /// Use the same [`TokioClock`] to construct `deadline` and enforce it.
    /// An unbounded deadline simply awaits the future.
    ///
    /// # Errors
    ///
    /// Returns [`TimeoutError::Elapsed`] when the deadline expires, or
    /// [`TimeoutError::OutOfRange`] if Tokio cannot represent the deadline.
    pub async fn timeout<F: Future>(
        clock: &TokioClock,
        deadline: Deadline,
        future: F,
    ) -> Result<F::Output, TimeoutError> {
        match clock.to_tokio_instant(deadline)? {
            Some(at) => tokio::time::timeout_at(at, future)
                .await
                .map_err(|_| TimeoutError::Elapsed),
            None => Ok(future.await),
        }
    }

    /// Create a deadline relative to this clock.
    #[must_use]
    pub fn deadline_after(clock: &TokioClock, timeout: Duration) -> Deadline {
        let timeout_ns = u64::try_from(timeout.as_nanos()).unwrap_or(u64::MAX);
        Deadline::after(clock.now(), timeout_ns)
    }
}

#[cfg(test)]
mod tests;
