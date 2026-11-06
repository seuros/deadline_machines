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

    /// Push the expiry back by `ns`, for a budget that paused and should not
    /// pay for the time it spent waiting. Never stays never.
    #[must_use]
    pub const fn extend(&self, ns: u64) -> Self {
        match self.expires_at {
            Some(expiry) => Self::at(Instant(expiry.0.saturating_add(ns))),
            None => Self::never(),
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
        #[cfg(feature = "tokio")]
        notify: tokio::sync::Notify,
        #[cfg(feature = "tokio-util")]
        mirror: tokio_util::sync::CancellationToken,
    }

    impl Node {
        fn new(
            parent: Option<Arc<Self>>,
            #[cfg(feature = "tokio-util")] mirror: tokio_util::sync::CancellationToken,
        ) -> Arc<Self> {
            Arc::new(Self {
                cancelled: AtomicBool::new(false),
                parent,
                #[cfg(feature = "tokio")]
                notify: tokio::sync::Notify::new(),
                #[cfg(feature = "tokio-util")]
                mirror,
            })
        }

        fn is_cancelled(&self) -> bool {
            if self.cancelled.load(Ordering::Acquire) {
                return true;
            }
            #[cfg(feature = "tokio-util")]
            if self.mirror.is_cancelled() {
                return true;
            }
            self.parent
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
                node: Node::new(
                    None,
                    #[cfg(feature = "tokio-util")]
                    tokio_util::sync::CancellationToken::new(),
                ),
            }
        }

        #[must_use]
        pub fn child(&self) -> Self {
            Self {
                node: Node::new(
                    Some(Arc::clone(&self.node)),
                    #[cfg(feature = "tokio-util")]
                    self.node.mirror.child_token(),
                ),
            }
        }

        pub fn cancel(&self) {
            self.node.cancelled.store(true, Ordering::Release);
            #[cfg(feature = "tokio")]
            self.node.notify.notify_waiters();
            #[cfg(feature = "tokio-util")]
            self.node.mirror.cancel();
        }

        #[must_use]
        pub fn is_cancelled(&self) -> bool {
            self.node.is_cancelled()
        }

        /// Resolves once this token or any ancestor is cancelled. Cancel-safe:
        /// dropping the future and awaiting again loses nothing.
        #[cfg(feature = "tokio")]
        pub async fn cancelled(&self) {
            use core::future::{Future, poll_fn};
            use core::task::Poll;

            let mut waits = alloc::vec::Vec::new();
            let mut node = Some(&*self.node);
            while let Some(current) = node {
                waits.push(alloc::boxed::Box::pin(current.notify.notified()));
                node = current.parent.as_deref();
            }
            for wait in &mut waits {
                wait.as_mut().enable();
            }
            #[cfg(feature = "tokio-util")]
            let mut mirror = alloc::boxed::Box::pin(self.node.mirror.clone().cancelled_owned());
            if self.is_cancelled() {
                return;
            }
            poll_fn(|cx| {
                #[cfg(feature = "tokio-util")]
                if mirror.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(());
                }
                if waits
                    .iter_mut()
                    .any(|wait| wait.as_mut().poll(cx).is_ready())
                {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
            .await;
        }

        /// A root token that is cancelled whenever `token` is, so a scope can
        /// sit under an existing `tokio_util` cancellation tree.
        #[cfg(feature = "tokio-util")]
        #[must_use]
        pub fn linked_to(token: &tokio_util::sync::CancellationToken) -> Self {
            Self {
                node: Node::new(None, token.child_token()),
            }
        }

        /// A `tokio_util` token that is cancelled whenever this one (or an
        /// ancestor) is. Cancelling it cancels this token too.
        #[cfg(feature = "tokio-util")]
        #[must_use]
        pub fn to_cancellation_token(&self) -> tokio_util::sync::CancellationToken {
            self.node.mirror.clone()
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
        pub const fn with_token(deadline: Deadline, token: CancelToken) -> Self {
            Self { deadline, token }
        }

        #[must_use]
        pub const fn deadline(&self) -> Deadline {
            self.deadline
        }

        #[must_use]
        pub const fn token(&self) -> &CancelToken {
            &self.token
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

        /// The same scope with the tighter of its deadline and `other`.
        /// Shares cancellation with this one.
        #[must_use]
        pub fn tighten(&self, other: Deadline) -> Self {
            Self {
                deadline: self.deadline.tighten(other),
                token: self.token.clone(),
            }
        }

        /// The same scope with its expiry pushed back by `ns`. Shares
        /// cancellation with this one.
        #[must_use]
        pub fn extend(&self, ns: u64) -> Self {
            Self {
                deadline: self.deadline.extend(ns),
                token: self.token.clone(),
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
    use super::{Clock, Deadline, Scope, Stopped};
    use core::future::{Future, poll_fn};
    use core::pin::pin;
    use core::task::Poll;
    use std::time::Duration;
    use tokio::time::Instant as TokioInstant;

    /// Why a [`Scope`] stopped a future short.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ScopeError {
        /// The budget ran out before the future completed.
        Exceeded,
        /// The scope or an ancestor was cancelled.
        Cancelled,
        /// The deadline exceeded Tokio's representable instant range.
        OutOfRange,
    }

    impl From<Stopped> for ScopeError {
        fn from(stopped: Stopped) -> Self {
            match stopped {
                Stopped::Exceeded => Self::Exceeded,
                Stopped::Cancelled => Self::Cancelled,
            }
        }
    }

    impl From<TimeoutError> for ScopeError {
        fn from(error: TimeoutError) -> Self {
            match error {
                TimeoutError::Elapsed => Self::Exceeded,
                TimeoutError::OutOfRange => Self::OutOfRange,
            }
        }
    }

    impl core::fmt::Display for ScopeError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str(match self {
                Self::Exceeded => "deadline exceeded",
                Self::Cancelled => "deadline cancelled",
                Self::OutOfRange => "deadline out of range",
            })
        }
    }

    impl core::error::Error for ScopeError {}

    impl Scope {
        /// Resolves once this scope or any ancestor is cancelled. Cancel-safe.
        pub async fn cancelled(&self) {
            self.token().cancelled().await;
        }

        /// Run `future` under this scope's remaining budget and cancellation,
        /// in the time domain of `clock`.
        ///
        /// # Errors
        ///
        /// [`ScopeError::Cancelled`] when the scope is cancelled, which
        /// outranks [`ScopeError::Exceeded`] for a spent budget;
        /// [`ScopeError::OutOfRange`] if Tokio cannot represent the deadline.
        pub async fn tokio_timeout<F: Future>(
            &self,
            clock: &TokioClock,
            future: F,
        ) -> Result<F::Output, ScopeError> {
            let at = clock.to_tokio_instant(self.deadline())?;
            self.check(clock.now())?;
            let mut future = pin!(future);
            let mut cancelled = pin!(self.cancelled());
            let guarded = poll_fn(|cx| {
                if cancelled.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(Err(ScopeError::Cancelled));
                }
                future.as_mut().poll(cx).map(Ok)
            });
            match at {
                Some(at) => tokio::time::timeout_at(at, guarded)
                    .await
                    .map_err(|_| ScopeError::Exceeded)?,
                None => guarded.await,
            }
        }
    }

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
