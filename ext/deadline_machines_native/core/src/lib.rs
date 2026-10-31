#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

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
        Self { expires_at: Some(instant) }
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
            None => requested_ns,
            Some(left) if left < requested_ns => left,
            Some(_) => requested_ns,
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
        parent: Option<Arc<Node>>,
    }

    impl Node {
        fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::Acquire)
                || self.parent.as_ref().is_some_and(|parent| parent.is_cancelled())
        }
    }

    #[derive(Debug, Clone)]
    pub struct CancelToken {
        node: Arc<Node>,
    }

    impl CancelToken {
        #[must_use]
        pub fn root() -> Self {
            Self { node: Arc::new(Node { cancelled: AtomicBool::new(false), parent: None }) }
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
            Self { origin: StdInstant::now() }
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

#[cfg(test)]
mod tests;
