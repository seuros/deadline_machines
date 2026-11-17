//! A monotonic clock that keeps counting while the machine sleeps.
//!
//! Tokio's and std's `Instant` stop during system suspend, so a span measured
//! across a laptop lid closing comes back short by the whole nap: a condition
//! that held through the sleep looks like it held for only seconds, and a
//! deadline outlives the time that really passed. [`ContinuousClock`] reads a
//! clock the kernel documents as advancing through suspend where one exists:
//!
//! - Linux and Android: `CLOCK_BOOTTIME`, "identical to `CLOCK_MONOTONIC`,
//!   except it also includes any time that the system is suspended"
//!   (`clock_gettime(2)`, Linux 2.6.39+).
//! - Apple platforms: `CLOCK_MONOTONIC_RAW`, the `mach_continuous_time` clock.
//!   Darwin's `clock_gettime(3)` defines `CLOCK_UPTIME_RAW` as advancing "in
//!   the same manner as `CLOCK_MONOTONIC_RAW`, but that does not increment
//!   while the system is asleep"; `mach_absolute_time` is that uptime clock.
//! - FreeBSD: no clock counts suspend, so it is **estimated from the wall
//!   clock**. Measured with a real S3 on FreeBSD 16.0-CURRENT amd64 (`zzz`,
//!   one sample per second): across the suspend `CLOCK_REALTIME` gained
//!   65.3 s while `CLOCK_MONOTONIC` (`CLOCK_BOOTTIME` is an alias) and
//!   `CLOCK_UPTIME` gained 6.8 s each. `ContinuousClock` reads
//!   `CLOCK_MONOTONIC` and adds every gain of the wall clock over it larger
//!   than [`SUSPEND_TOLERANCE`] (2 s). A forward wall-clock step past that
//!   tolerance reads as sleep; a backward step is never subtracted, so the
//!   clock never runs back, but it hides that much of the next suspend. The
//!   estimate is shared by every `ContinuousClock` in the process.
//!   (`<sys/_clock_id.h>` makes `CLOCK_BOOTTIME` a Linux-compat name for
//!   `CLOCK_MONOTONIC`; libc 0.2.191 maps it to `CLOCK_UPTIME` instead, so
//!   this names `CLOCK_MONOTONIC` directly.)
//!
//! Every other target fails to compile with this feature on.
//!
//! Suspend itself is not exercised by the tests; no CI machine sleeps. They
//! check that readings never go backwards, that the clock is never behind the
//! uptime clock it extends, and drive the FreeBSD estimate with injected
//! readings.
use core::mem::MaybeUninit;
use core::time::Duration;

#[cfg(any(target_os = "freebsd", test))]
mod estimated;

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_vendor = "apple"
)))]
compile_error!(
    "deadline-machines `continuous`: no known clock on this target counts suspend; \
     supported targets are Linux, Android, FreeBSD and Apple platforms"
);

/// The wall-clock gain FreeBSD must see before it counts suspend.
///
/// The wall clock must gain this much more than the uptime clock between two
/// readings. It absorbs the RTC's one-second resolution on resume and small
/// NTP steps; a shorter suspend goes uncounted. Linux and Apple platforms
/// read a kernel clock instead.
pub const SUSPEND_TOLERANCE: Duration = Duration::from_secs(2);

/// A monotonic clock whose epoch is the moment it was built, and which keeps
/// counting through system suspend.
///
/// Shaped like [`TokioClock`](crate::tokio_support::TokioClock), so either
/// can supply the `now` handed to [`Deadline`](crate::Deadline) and
/// [`Scope`](crate::Scope). It reads the kernel, so it does not follow
/// Tokio's paused test time.
///
/// The clock read cannot fail on the supported targets: both clocks predate
/// the oldest kernels Rust supports. Were it to fail, `now` answers the epoch,
/// `Duration::ZERO`, and debug builds assert.
#[derive(Debug, Clone, Copy)]
pub struct ContinuousClock {
    origin: Duration,
}

impl ContinuousClock {
    /// Starts a time domain at the present. Also the [`Default`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: read().unwrap_or(Duration::ZERO),
        }
    }

    /// The present as a `Duration` since this clock's epoch, time spent
    /// suspended included: the `now` to hand [`Deadline`](crate::Deadline)
    /// and [`Scope`](crate::Scope).
    #[must_use]
    pub fn now(&self) -> Duration {
        read().map_or(Duration::ZERO, |at| at.saturating_sub(self.origin))
    }
}

impl Default for ContinuousClock {
    fn default() -> Self {
        Self::new()
    }
}

fn read() -> Option<Duration> {
    let time = sample();
    debug_assert!(time.is_some(), "the suspend-aware clock is always readable");
    time
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn sample() -> Option<Duration> {
    timespec(libc::CLOCK_BOOTTIME)
}

#[cfg(target_vendor = "apple")]
fn sample() -> Option<Duration> {
    timespec(libc::CLOCK_MONOTONIC_RAW)
}

#[cfg(target_os = "freebsd")]
fn sample() -> Option<Duration> {
    static SUSPENDED: estimated::Suspended = estimated::Suspended::new();
    let uptime = timespec(libc::CLOCK_MONOTONIC)?;
    Some(SUSPENDED.add_to(uptime, timespec(libc::CLOCK_REALTIME)))
}

#[allow(unsafe_code)]
fn timespec(clock: libc::clockid_t) -> Option<Duration> {
    let mut time = MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: `time` is a writable `timespec` that outlives the call, and
    // `clock_gettime` writes only through that pointer.
    if unsafe { libc::clock_gettime(clock, time.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: `clock_gettime` returned 0, so it filled in the whole `timespec`.
    let time = unsafe { time.assume_init() };
    let secs = u64::try_from(time.tv_sec).ok()?;
    let nanos = u32::try_from(time.tv_nsec)
        .ok()
        .filter(|nanos| *nanos < 1_000_000_000)?;
    Some(Duration::new(secs, nanos))
}

#[cfg(test)]
mod tests;
