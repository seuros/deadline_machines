use super::*;

const fn ns(nanos: u64) -> Duration {
    Duration::from_nanos(nanos)
}

#[test]
fn derive_keeps_the_tighter_bound() {
    let parent = Deadline::after(ns(0), ns(5_000));

    assert_eq!(
        parent.derive(ns(0), ns(1_000)).expires_at(),
        Some(ns(1_000))
    );
    assert_eq!(parent.derive(ns(0), ns(9_000)), parent);
    assert_eq!(
        Deadline::never().derive(ns(0), ns(9_000)),
        Deadline::at(ns(9_000))
    );
}

#[test]
fn tighten_is_commutative_and_absorbs_never() {
    let a = Deadline::after(ns(0), ns(10));
    let b = Deadline::after(ns(0), ns(20));
    // Same seconds, different subsecond nanos, and the reverse.
    let c = Deadline::at(Duration::new(1, 5));
    let d = Deadline::at(Duration::new(2, 0));

    assert_eq!(a.tighten(b), b.tighten(a));
    assert_eq!(a.tighten(b), a);
    assert_eq!(c.tighten(Deadline::at(Duration::new(1, 7))), c);
    assert_eq!(d.tighten(c), c);
    assert_eq!(a.tighten(Deadline::never()), a);
    assert_eq!(Deadline::never().tighten(a), a);
    assert_eq!(
        Deadline::never().tighten(Deadline::never()),
        Deadline::never()
    );
    assert_eq!(Deadline::default(), Deadline::never());
}

#[test]
fn remaining_saturates_instead_of_wrapping() {
    let deadline = Deadline::after(ns(0), ns(100));

    assert_eq!(deadline.remaining(ns(40)), Some(ns(60)));
    assert_eq!(deadline.remaining(ns(400)), Some(Duration::ZERO));
    assert!(deadline.is_expired(ns(100)));
    assert!(!deadline.is_expired(ns(99)));
    assert_eq!(Deadline::never().remaining(Duration::MAX), None);
    assert!(!Deadline::never().is_expired(Duration::MAX));
}

#[test]
fn check_refuses_only_a_spent_budget_and_judges_each_instant_alone() {
    let deadline = Deadline::after(ns(0), ns(100));
    let before = deadline;

    assert_eq!(deadline.check(ns(99)), Ok(()));
    assert_eq!(deadline.check(ns(100)), Err(Refusal::Exceeded));
    // A refusal records nothing: an earlier instant is admitted again.
    assert_eq!(deadline.check(ns(15)), Ok(()));
    assert_eq!(deadline, before);
    assert_eq!(Deadline::never().check(Duration::MAX), Ok(()));
}

#[test]
fn sums_saturate_at_the_end_of_time() {
    assert_eq!(
        Deadline::after(Duration::MAX, ns(10)).expires_at(),
        Some(Duration::MAX)
    );
    assert_eq!(
        Deadline::at(Duration::MAX).extend(ns(1)).expires_at(),
        Some(Duration::MAX)
    );
}

#[test]
fn extend_pushes_the_expiry_back_and_keeps_never() {
    let deadline = Deadline::after(ns(0), ns(100));

    assert_eq!(deadline.extend(ns(50)).expires_at(), Some(ns(150)));
    assert_eq!(Deadline::never().extend(ns(50)), Deadline::never());
}

#[test]
fn clamp_sleep_never_overruns_the_budget() {
    let deadline = Deadline::after(ns(0), ns(100));

    assert_eq!(deadline.clamp_sleep(ns(0), ns(30)), ns(30));
    assert_eq!(deadline.clamp_sleep(ns(80), ns(30)), ns(20));
    assert_eq!(deadline.clamp_sleep(ns(200), ns(30)), Duration::ZERO);
    assert_eq!(Deadline::never().clamp_sleep(ns(0), ns(30)), ns(30));
}
