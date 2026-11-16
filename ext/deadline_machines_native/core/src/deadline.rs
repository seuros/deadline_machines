//! The budget itself: an expiry instant, or none.
use core::time::Duration;

use crate::Refusal;

/// When a budget runs out, as an instant on the caller's monotonic clock, or
/// never.
///
/// Every instant is a [`Duration`] since an epoch the caller picks, and every
/// span is a `Duration`. Sums saturate at [`Duration::MAX`] instead of
/// wrapping, so a deadline past the end of time is simply very late.
///
/// ```
/// use core::time::Duration;
/// use deadline_machines::{Deadline, Refusal};
///
/// let now = Duration::ZERO;
/// let request = Deadline::after(now, Duration::from_secs(5));
///
/// // A child never outlives its parent.
/// assert_eq!(request.derive(now, Duration::from_secs(9)), request);
///
/// // A backoff that cannot overrun the budget.
/// let later = Duration::from_secs(4);
/// assert_eq!(request.clamp_sleep(later, Duration::from_secs(3)), Duration::from_secs(1));
/// assert_eq!(request.check(Duration::from_secs(5)), Err(Refusal::Exceeded));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Deadline {
    expires_at: Option<Duration>,
}

/// The earlier of two instants.
const fn earlier(a: Duration, b: Duration) -> Duration {
    let (a_secs, b_secs) = (a.as_secs(), b.as_secs());
    if a_secs < b_secs || (a_secs == b_secs && a.subsec_nanos() < b.subsec_nanos()) {
        a
    } else {
        b
    }
}

impl Deadline {
    /// A budget that never runs out. Also the [`Default`].
    #[must_use]
    pub const fn never() -> Self {
        Self { expires_at: None }
    }

    /// A budget that runs out at `instant`.
    #[must_use]
    pub const fn at(instant: Duration) -> Self {
        Self {
            expires_at: Some(instant),
        }
    }

    /// A budget that runs out `timeout` after `now`.
    #[must_use]
    pub const fn after(now: Duration, timeout: Duration) -> Self {
        Self::at(now.saturating_add(timeout))
    }

    /// The instant the budget runs out, `None` for [`never`](Self::never).
    #[must_use]
    pub const fn expires_at(&self) -> Option<Duration> {
        self.expires_at
    }

    /// What is left at `now`, zero once spent, `None` when there is no limit.
    #[must_use]
    pub const fn remaining(&self, now: Duration) -> Option<Duration> {
        match self.expires_at {
            Some(expiry) => Some(expiry.saturating_sub(now)),
            None => None,
        }
    }

    /// Whether nothing is left at `now`. The expiry instant itself is spent.
    #[must_use]
    pub const fn is_expired(&self, now: Duration) -> bool {
        match self.remaining(now) {
            Some(left) => left.is_zero(),
            None => false,
        }
    }

    /// The checkpoint without a cancellation tree.
    ///
    /// # Errors
    ///
    /// [`Refusal::Exceeded`] when the budget is spent at `now`.
    pub const fn check(&self, now: Duration) -> Result<(), Refusal> {
        if self.is_expired(now) {
            Err(Refusal::Exceeded)
        } else {
            Ok(())
        }
    }

    /// A child budget: the tighter of this one and `timeout` from `now`.
    #[must_use]
    pub const fn derive(&self, now: Duration, timeout: Duration) -> Self {
        self.tighten(Self::after(now, timeout))
    }

    /// The expiry pushed back by `by`, for a budget that paused and should
    /// not pay for the time it spent waiting. Never stays never.
    #[must_use]
    pub const fn extend(&self, by: Duration) -> Self {
        match self.expires_at {
            Some(expiry) => Self::at(expiry.saturating_add(by)),
            None => Self::never(),
        }
    }

    /// The tighter of this budget and `other`. Commutative; never is the
    /// identity.
    #[must_use]
    pub const fn tighten(&self, other: Self) -> Self {
        match (self.expires_at, other.expires_at) {
            (None, None) => Self::never(),
            (Some(only), None) | (None, Some(only)) => Self::at(only),
            (Some(a), Some(b)) => Self::at(earlier(a, b)),
        }
    }

    /// The longest sleep up to `requested` that still fits the budget at
    /// `now`. A retry policy calls this before each backoff so it cannot
    /// outlive its caller.
    #[must_use]
    pub const fn clamp_sleep(&self, now: Duration, requested: Duration) -> Duration {
        match self.remaining(now) {
            Some(left) => earlier(left, requested),
            None => requested,
        }
    }
}

#[cfg(test)]
mod tests;
