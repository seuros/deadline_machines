//! Every reason a checkpoint refuses to admit more work.
use core::fmt;

/// Why a [`Deadline`](crate::Deadline) or [`Scope`](crate::Scope) stopped
/// admitting work.
///
/// A refusal is an answer about the instant supplied, not a recorded fact:
/// nothing in this crate remembers it, and [`Exceeded`](Self::Exceeded) at
/// one `now` may be answered `Ok` for an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The budget ran out at the instant supplied.
    Exceeded,
    /// This scope or an ancestor was cancelled. Outranks
    /// [`Exceeded`](Self::Exceeded) when both apply.
    Cancelled,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Exceeded => "deadline exceeded",
            Self::Cancelled => "deadline cancelled",
        })
    }
}

impl core::error::Error for Refusal {}
