use super::{Deadline, Instant};

#[test]
fn derive_keeps_the_tighter_bound() {
    let now = Instant(0);
    let parent = Deadline::after(now, 5_000);

    assert_eq!(parent.derive(now, 1_000).expires_at(), Some(Instant(1_000)));
    assert_eq!(parent.derive(now, 9_000).expires_at(), Some(Instant(5_000)));
    assert_eq!(
        Deadline::never().derive(now, 9_000).expires_at(),
        Some(Instant(9_000))
    );
}

#[test]
fn tighten_is_commutative_and_absorbs_never() {
    let a = Deadline::after(Instant(0), 10);
    let b = Deadline::after(Instant(0), 20);

    assert_eq!(a.tighten(b), b.tighten(a));
    assert_eq!(a.tighten(b), a);
    assert_eq!(a.tighten(Deadline::never()), a);
    assert_eq!(
        Deadline::never().tighten(Deadline::never()),
        Deadline::never()
    );
}

#[test]
fn remaining_saturates_instead_of_wrapping() {
    let deadline = Deadline::after(Instant(0), 100);

    assert_eq!(deadline.remaining_ns(Instant(40)), Some(60));
    assert_eq!(deadline.remaining_ns(Instant(400)), Some(0));
    assert!(deadline.is_expired(Instant(400)));
    assert!(!Deadline::never().is_expired(Instant(u64::MAX)));
}

#[test]
fn after_saturates_at_the_end_of_time() {
    assert_eq!(
        Deadline::after(Instant(u64::MAX), 10).expires_at(),
        Some(Instant(u64::MAX))
    );
}

#[test]
fn clamp_sleep_never_overruns_the_budget() {
    let deadline = Deadline::after(Instant(0), 100);

    assert_eq!(deadline.clamp_sleep_ns(Instant(0), 30), 30);
    assert_eq!(deadline.clamp_sleep_ns(Instant(80), 30), 20);
    assert_eq!(deadline.clamp_sleep_ns(Instant(200), 30), 0);
    assert_eq!(Deadline::never().clamp_sleep_ns(Instant(0), 30), 30);
}

#[cfg(feature = "alloc")]
mod with_alloc {
    use super::super::cancel::CancelToken;
    use super::super::{Deadline, Instant, Scope, Stopped};

    #[test]
    fn cancellation_flows_down_but_not_up() {
        let root = CancelToken::root();
        let child = root.child();
        let grandchild = child.child();
        let sibling = root.child();

        child.cancel();

        assert!(child.is_cancelled());
        assert!(grandchild.is_cancelled());
        assert!(!sibling.is_cancelled());
        assert!(!root.is_cancelled());

        root.cancel();
        assert!(sibling.is_cancelled());
    }

    #[test]
    fn a_scope_tightens_its_deadline_and_keeps_its_own_cancellation() {
        let now = Instant(0);
        let request = Scope::new(Deadline::after(now, 5_000));

        assert_eq!(
            request.derive(now, 1_000).deadline().expires_at(),
            Some(Instant(1_000))
        );
        assert_eq!(
            request.derive(now, 9_000).deadline().expires_at(),
            Some(Instant(5_000))
        );
        assert_eq!(request.child().deadline(), request.deadline());

        let child = request.child();
        child.cancel();
        assert!(child.is_cancelled());
        assert!(!request.is_cancelled());
    }

    #[test]
    fn cloning_a_scope_shares_cancellation_but_deriving_does_not() {
        let scope = Scope::never();
        let clone = scope.clone();
        let child = scope.child();

        clone.cancel();

        assert!(scope.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn check_reports_cancellation_ahead_of_expiry() {
        let scope = Scope::new(Deadline::after(Instant(0), 100));

        assert_eq!(scope.check(Instant(0)), Ok(()));
        assert_eq!(scope.check(Instant(500)), Err(Stopped::Exceeded));

        scope.cancel();
        assert_eq!(scope.check(Instant(500)), Err(Stopped::Cancelled));
    }

    #[test]
    fn a_scope_derived_after_cancellation_is_already_cancelled() {
        let scope = Scope::never();
        scope.cancel();

        assert!(scope.child().is_cancelled());
        assert!(scope.derive(Instant(0), 1_000).is_cancelled());
    }
}
