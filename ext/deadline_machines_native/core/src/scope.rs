//! A budget and a cancellation handle as one value.
use core::time::Duration;

use crate::Deadline;
use crate::Refusal;
use crate::cancel::{CancelToken, Cancellation};

/// A budget and a cancellation handle as one value, which is what a retry
/// policy or a circuit breaker actually needs to consult.
///
/// Clones share cancellation; [`derive`](Self::derive) and
/// [`child`](Self::child) make a scope below this one.
///
/// ```
/// use core::time::Duration;
/// use deadline_machines::{Deadline, Refusal, Scope};
///
/// let now = Duration::ZERO;
/// let request = Scope::new(Deadline::after(now, Duration::from_secs(5)));
///
/// let attempt = request.derive(now, Duration::from_secs(9));
/// assert_eq!(attempt.deadline(), request.deadline());
///
/// request.cancel();
/// assert_eq!(attempt.check(now), Err(Refusal::Cancelled));
/// ```
#[derive(Debug, Clone)]
pub struct Scope {
    deadline: Deadline,
    token: CancelToken,
}

impl Scope {
    /// A scope under `deadline` with a fresh root token.
    #[must_use]
    pub fn new(deadline: Deadline) -> Self {
        Self::with_token(deadline, CancelToken::root())
    }

    /// A scope with no time limit, cancellable only.
    #[must_use]
    pub fn never() -> Self {
        Self::new(Deadline::never())
    }

    /// A scope under `deadline` that shares `token`.
    #[must_use]
    pub const fn with_token(deadline: Deadline, token: CancelToken) -> Self {
        Self { deadline, token }
    }

    /// The budget.
    #[must_use]
    pub const fn deadline(&self) -> Deadline {
        self.deadline
    }

    /// The cancellation handle.
    #[must_use]
    pub const fn token(&self) -> &CancelToken {
        &self.token
    }

    /// What is left at `now`; see [`Deadline::remaining`].
    #[must_use]
    pub const fn remaining(&self, now: Duration) -> Option<Duration> {
        self.deadline.remaining(now)
    }

    /// Whether this scope or an ancestor was cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Whether the budget is spent at `now`.
    #[must_use]
    pub const fn is_expired(&self, now: Duration) -> bool {
        self.deadline.is_expired(now)
    }

    /// Whether [`check`](Self::check) would refuse at `now`.
    #[must_use]
    pub fn is_done(&self, now: Duration) -> bool {
        self.is_cancelled() || self.is_expired(now)
    }

    /// Cancellation reaches every scope derived from this one, now or later.
    /// See [`CancelToken::cancel`].
    #[allow(
        clippy::must_use_candidate,
        reason = "cancelled for its effect; the outcome only says whether this call was first"
    )]
    pub fn cancel(&self) -> Cancellation {
        self.token.cancel()
    }

    /// The cooperative checkpoint.
    ///
    /// # Errors
    ///
    /// [`Refusal::Cancelled`] when this scope or an ancestor was cancelled,
    /// which outranks [`Refusal::Exceeded`] for a budget spent at `now`.
    pub fn check(&self, now: Duration) -> Result<(), Refusal> {
        if self.is_cancelled() {
            return Err(Refusal::Cancelled);
        }
        self.deadline.check(now)
    }

    /// A child scope whose deadline is the tighter of this one and `timeout`
    /// from `now`, cancellable on its own.
    #[must_use]
    pub fn derive(&self, now: Duration, timeout: Duration) -> Self {
        Self::with_token(self.deadline.derive(now, timeout), self.token.child())
    }

    /// A child scope with the same deadline, cancellable on its own.
    #[must_use]
    pub fn child(&self) -> Self {
        Self::with_token(self.deadline, self.token.child())
    }

    /// The same scope with the tighter of its deadline and `other`. Shares
    /// cancellation with this one.
    #[must_use]
    pub fn tighten(&self, other: Deadline) -> Self {
        Self::with_token(self.deadline.tighten(other), self.token.clone())
    }

    /// The same scope with its expiry pushed back by `by`. Shares
    /// cancellation with this one.
    #[must_use]
    pub fn extend(&self, by: Duration) -> Self {
        Self::with_token(self.deadline.extend(by), self.token.clone())
    }

    /// The longest sleep up to `requested` that still fits the budget at
    /// `now`; see [`Deadline::clamp_sleep`].
    #[must_use]
    pub const fn clamp_sleep(&self, now: Duration, requested: Duration) -> Duration {
        self.deadline.clamp_sleep(now, requested)
    }
}

#[cfg(test)]
mod tests;
