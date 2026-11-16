use super::*;

const fn ns(nanos: u64) -> Duration {
    Duration::from_nanos(nanos)
}

#[test]
fn a_scope_tightens_its_deadline_and_keeps_its_own_cancellation() {
    let request = Scope::new(Deadline::after(ns(0), ns(5_000)));

    assert_eq!(
        request.derive(ns(0), ns(1_000)).deadline().expires_at(),
        Some(ns(1_000))
    );
    assert_eq!(
        request.derive(ns(0), ns(9_000)).deadline(),
        request.deadline()
    );
    assert_eq!(request.child().deadline(), request.deadline());
    assert_eq!(request.remaining(ns(1_000)), Some(ns(4_000)));
    assert_eq!(request.clamp_sleep(ns(4_000), ns(3_000)), ns(1_000));

    let child = request.child();
    assert_eq!(child.cancel(), Cancellation::Propagated);
    assert!(child.is_cancelled());
    assert!(!request.is_cancelled());
}

#[test]
fn cloning_a_scope_shares_cancellation_but_deriving_does_not() {
    let scope = Scope::never();
    let clone = scope.clone();
    let child = scope.child();

    child.cancel();
    assert!(!scope.is_cancelled());

    clone.cancel();
    assert!(scope.is_cancelled());
    assert_eq!(scope.cancel(), Cancellation::AlreadyCancelled);
}

#[test]
fn check_reports_cancellation_ahead_of_expiry_and_changes_nothing() {
    let scope = Scope::new(Deadline::after(ns(0), ns(100)));

    assert_eq!(scope.check(ns(0)), Ok(()));
    assert!(!scope.is_done(ns(99)));
    assert_eq!(scope.check(ns(500)), Err(Refusal::Exceeded));
    assert!(scope.is_done(ns(500)));
    // Refused at 500, admitted at 50: a refusal is about its instant only.
    assert_eq!(scope.check(ns(50)), Ok(()));

    scope.cancel();
    assert_eq!(scope.check(ns(500)), Err(Refusal::Cancelled));
    assert_eq!(scope.check(ns(0)), Err(Refusal::Cancelled));
}

#[test]
fn tighten_and_extend_keep_the_same_cancellation() {
    let scope = Scope::new(Deadline::after(ns(0), ns(5_000)));

    let tightened = scope.tighten(Deadline::after(ns(0), ns(1_000)));
    assert_eq!(tightened.deadline().expires_at(), Some(ns(1_000)));
    assert_eq!(
        scope.tighten(Deadline::never()).deadline(),
        scope.deadline()
    );

    let extended = scope.extend(ns(2_000));
    assert_eq!(extended.deadline().expires_at(), Some(ns(7_000)));

    tightened.cancel();
    assert!(scope.is_cancelled());
    assert!(extended.is_cancelled());
}

#[test]
fn with_token_shares_an_existing_token() {
    let token = CancelToken::root();
    let scope = Scope::with_token(Deadline::never(), token.child());

    token.cancel();
    assert!(scope.is_cancelled());
    assert!(scope.token().is_cancelled());
}

#[test]
fn a_scope_derived_after_cancellation_is_already_cancelled() {
    let scope = Scope::never();
    scope.cancel();

    assert!(scope.child().is_cancelled());
    assert!(scope.derive(ns(0), ns(1_000)).is_cancelled());
}
