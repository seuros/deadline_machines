use core::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use core::time::Duration;

use super::SUSPEND_TOLERANCE;

const UNSEEN: i64 = i64::MIN;

pub(super) struct Suspended {
    highest_skew: AtomicI64,
    total: AtomicU64,
}

impl Suspended {
    pub(super) const fn new() -> Self {
        Self {
            highest_skew: AtomicI64::new(UNSEEN),
            total: AtomicU64::new(0),
        }
    }

    pub(super) fn add_to(&self, uptime: Duration, realtime: Option<Duration>) -> Duration {
        if let Some(realtime) = realtime {
            let skew = skew(uptime, realtime);
            let previous = self.highest_skew.fetch_max(skew, Ordering::AcqRel);
            let asleep = asleep(previous, skew);
            if asleep > 0 {
                let mut total = self.total.load(Ordering::Acquire);
                while let Err(seen) = self.total.compare_exchange_weak(
                    total,
                    total.saturating_add(asleep),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    total = seen;
                }
            }
        }
        uptime.saturating_add(Duration::from_nanos(self.total.load(Ordering::Acquire)))
    }
}

fn skew(uptime: Duration, realtime: Duration) -> i64 {
    let nanos = |at: Duration| i128::try_from(at.as_nanos()).unwrap_or(i128::MAX);
    let skew = nanos(realtime).saturating_sub(nanos(uptime));
    i64::try_from(skew).unwrap_or(if skew < 0 { UNSEEN + 1 } else { i64::MAX })
}

fn asleep(previous: i64, skew: i64) -> u64 {
    if previous == UNSEEN {
        return 0;
    }
    let gain = i128::from(skew) - i128::from(previous);
    if gain > i128::try_from(SUSPEND_TOLERANCE.as_nanos()).unwrap_or(i128::MAX) {
        u64::try_from(gain).unwrap_or(u64::MAX)
    } else {
        0
    }
}

#[cfg(test)]
mod tests;
