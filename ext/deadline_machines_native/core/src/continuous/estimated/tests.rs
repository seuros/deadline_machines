use super::*;

const WALL: Duration = Duration::from_secs(1_700_000_000);

fn at(secs: u64, millis: u64) -> Duration {
    Duration::from_secs(secs) + Duration::from_millis(millis)
}

fn reading(suspended: &Suspended, uptime: Duration, gained: Duration) -> Duration {
    suspended.add_to(uptime, Some(WALL + uptime + gained))
}

#[test]
fn the_first_reading_is_plain_uptime() {
    let suspended = Suspended::new();
    assert_eq!(reading(&suspended, at(5, 0), at(3_600, 0)), at(5, 0));
}

#[test]
fn a_wall_clock_gain_past_the_tolerance_is_counted_as_sleep() {
    let suspended = Suspended::new();
    reading(&suspended, at(10, 0), Duration::ZERO);
    assert_eq!(reading(&suspended, at(20, 0), at(300, 0)), at(320, 0));
    assert_eq!(reading(&suspended, at(30, 0), at(300, 0)), at(330, 0));
}

#[test]
fn jitter_within_the_tolerance_is_ignored() {
    let suspended = Suspended::new();
    reading(&suspended, at(10, 0), Duration::ZERO);
    assert_eq!(reading(&suspended, at(20, 0), at(0, 500)), at(20, 0));
    assert_eq!(reading(&suspended, at(30, 0), SUSPEND_TOLERANCE), at(30, 0));
    assert_eq!(reading(&suspended, at(40, 0), at(1, 900)), at(40, 0));
}

#[test]
fn a_backward_step_is_ignored_and_never_runs_the_clock_back() {
    let suspended = Suspended::new();
    reading(&suspended, at(10, 0), at(600, 0));
    let stepped_back = suspended.add_to(at(20, 0), Some(WALL + at(20, 0)));
    assert_eq!(stepped_back, at(20, 0));
    assert_eq!(reading(&suspended, at(30, 0), at(600, 0)), at(30, 0));
}

#[test]
fn a_suspend_after_a_backward_step_is_undercounted_by_the_step() {
    let suspended = Suspended::new();
    reading(&suspended, at(10, 0), at(60, 0));
    reading(&suspended, at(20, 0), Duration::ZERO);
    assert_eq!(reading(&suspended, at(30, 0), at(360, 0)), at(330, 0));
}

#[test]
fn a_stale_reading_after_a_counted_sleep_is_not_counted_again() {
    let suspended = Suspended::new();
    reading(&suspended, at(10, 0), Duration::ZERO);
    assert_eq!(reading(&suspended, at(20, 0), at(300, 0)), at(320, 0));
    assert_eq!(reading(&suspended, at(15, 0), Duration::ZERO), at(315, 0));
    assert_eq!(reading(&suspended, at(30, 0), at(300, 0)), at(330, 0));
}

#[test]
fn a_missing_wall_clock_keeps_the_estimate_so_far() {
    let suspended = Suspended::new();
    reading(&suspended, at(10, 0), Duration::ZERO);
    reading(&suspended, at(20, 0), at(100, 0));
    assert_eq!(suspended.add_to(at(30, 0), None), at(130, 0));
}

#[test]
fn readings_through_a_mixed_history_never_go_backwards() {
    let suspended = Suspended::new();
    let history = [
        (at(1, 0), Duration::ZERO),
        (at(2, 0), at(0, 300)),
        (at(3, 0), at(900, 0)),
        (at(4, 0), at(100, 0)),
        (at(5, 0), at(101, 0)),
        (at(6, 0), at(1_500, 0)),
    ];
    let mut last = Duration::ZERO;
    for (uptime, gained) in history {
        let now = reading(&suspended, uptime, gained);
        assert!(now >= last, "{now:?} went back from {last:?}");
        last = now;
    }
    assert_eq!(last, at(6, 0) + at(1_499, 700));
}

#[test]
fn an_unrepresentable_skew_saturates_instead_of_wrapping() {
    assert_eq!(skew(Duration::MAX, Duration::ZERO), UNSEEN + 1);
    assert_eq!(skew(Duration::ZERO, Duration::MAX), i64::MAX);
    assert_eq!(asleep(UNSEEN + 1, i64::MAX), u64::MAX - 1);
}
