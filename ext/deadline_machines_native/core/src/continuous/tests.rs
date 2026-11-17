use super::*;

#[test]
fn readings_never_go_backwards() {
    let clock = ContinuousClock::new();
    let mut last = clock.now();
    for _ in 0..10_000 {
        let now = clock.now();
        assert!(now >= last, "{now:?} went back from {last:?}");
        last = now;
    }
}

#[test]
fn a_fresh_clock_starts_near_its_epoch() {
    let clock = ContinuousClock::default();
    assert!(clock.now() < Duration::from_secs(1));
}

#[test]
fn copies_share_one_epoch() {
    let clock = ContinuousClock::new();
    let copy = clock;
    let first = clock.now();
    assert!(copy.now() >= first);
}

#[test]
fn the_clock_advances_with_real_time() {
    let clock = ContinuousClock::new();
    let before = clock.now();
    std::thread::sleep(Duration::from_millis(5));
    assert!(clock.now() >= before + Duration::from_millis(5));
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn boottime_is_never_behind_monotonic() {
    let monotonic = timespec(libc::CLOCK_MONOTONIC).expect("CLOCK_MONOTONIC reads");
    let boottime = read().expect("CLOCK_BOOTTIME reads");
    assert!(boottime >= monotonic, "{boottime:?} < {monotonic:?}");
}

#[cfg(target_vendor = "apple")]
#[test]
fn continuous_time_is_never_behind_uptime() {
    let uptime = timespec(libc::CLOCK_UPTIME_RAW).expect("CLOCK_UPTIME_RAW reads");
    let continuous = read().expect("CLOCK_MONOTONIC_RAW reads");
    assert!(continuous >= uptime, "{continuous:?} < {uptime:?}");
}

#[cfg(target_os = "freebsd")]
#[test]
fn the_estimate_is_never_behind_uptime() {
    let uptime = timespec(libc::CLOCK_MONOTONIC).expect("CLOCK_MONOTONIC reads");
    let continuous = read().expect("the estimate reads");
    assert!(continuous >= uptime, "{continuous:?} < {uptime:?}");
}
