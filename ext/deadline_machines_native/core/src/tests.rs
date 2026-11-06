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
fn extend_pushes_the_expiry_back_and_keeps_never() {
    let deadline = Deadline::after(Instant(0), 100);

    assert_eq!(deadline.extend(50).expires_at(), Some(Instant(150)));
    assert_eq!(
        Deadline::at(Instant(u64::MAX)).extend(1).expires_at(),
        Some(Instant(u64::MAX))
    );
    assert_eq!(Deadline::never().extend(50), Deadline::never());
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
    fn tighten_and_extend_keep_the_same_cancellation() {
        let now = Instant(0);
        let scope = Scope::new(Deadline::after(now, 5_000));

        let tightened = scope.tighten(Deadline::after(now, 1_000));
        assert_eq!(tightened.deadline().expires_at(), Some(Instant(1_000)));
        assert_eq!(
            scope.tighten(Deadline::never()).deadline(),
            scope.deadline()
        );

        let extended = scope.extend(2_000);
        assert_eq!(extended.deadline().expires_at(), Some(Instant(7_000)));

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
        assert!(scope.derive(Instant(0), 1_000).is_cancelled());
    }
}

#[cfg(feature = "tokio")]
mod with_tokio {
    use super::super::cancel::CancelToken;
    use super::super::tokio_support::ScopeError;
    use super::super::{Clock, Deadline, Scope, tokio_support, tokio_support::TokioClock};
    use std::time::Duration;
    use tokio::time::{Instant, advance};

    #[tokio::test(start_paused = true)]
    async fn cancelled_resolves_when_an_ancestor_is_cancelled() {
        let root = CancelToken::root();
        let child = root.child().child();
        let waiter = tokio::spawn(async move { child.cancelled().await });

        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());

        root.cancel();
        assert!(
            tokio::time::timeout(Duration::from_secs(1), waiter)
                .await
                .is_ok()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_resolves_immediately_for_a_cancelled_token() {
        let token = CancelToken::root();
        token.cancel();

        assert!(
            tokio::time::timeout(Duration::from_secs(1), token.cancelled())
                .await
                .is_ok()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_survives_being_dropped_and_awaited_again() {
        let token = CancelToken::root();

        assert!(
            tokio::time::timeout(Duration::from_millis(10), token.cancelled())
                .await
                .is_err()
        );

        let second = {
            let token = token.clone();
            tokio::spawn(async move { token.cancelled().await })
        };
        tokio::task::yield_now().await;
        token.cancel();

        assert!(
            tokio::time::timeout(Duration::from_secs(1), second)
                .await
                .is_ok()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn scope_timeout_completes_within_budget() {
        let clock = TokioClock::new();
        let scope = Scope::new(tokio_support::deadline_after(
            &clock,
            Duration::from_secs(5),
        ));

        let result = scope
            .tokio_timeout(&clock, async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                7
            })
            .await;

        assert_eq!(result, Ok(7));
    }

    #[tokio::test(start_paused = true)]
    async fn scope_timeout_reports_an_exceeded_budget() {
        let clock = TokioClock::new();
        let scope = Scope::new(tokio_support::deadline_after(
            &clock,
            Duration::from_secs(1),
        ));

        let result = scope
            .tokio_timeout(&clock, async {
                tokio::time::sleep(Duration::from_secs(2)).await;
                7
            })
            .await;

        assert_eq!(result, Err(ScopeError::Exceeded));
        assert_eq!(clock.now_ns(), 1_000_000_000);
    }

    #[tokio::test(start_paused = true)]
    async fn scope_timeout_reports_cancellation_from_an_ancestor() {
        let clock = TokioClock::new();
        let parent = Scope::new(tokio_support::deadline_after(
            &clock,
            Duration::from_secs(5),
        ));
        let scope = parent.child();

        let canceller = {
            let parent = parent.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                parent.cancel();
            })
        };
        let result = scope
            .tokio_timeout(&clock, async {
                tokio::time::sleep(Duration::from_secs(2)).await;
                7
            })
            .await;

        assert_eq!(result, Err(ScopeError::Cancelled));
        assert!(canceller.await.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn scope_timeout_ranks_cancellation_ahead_of_expiry() {
        let clock = TokioClock::new();
        let scope = Scope::new(Deadline::at(clock.now()));
        scope.cancel();

        assert_eq!(
            scope.tokio_timeout(&clock, async { 7 }).await,
            Err(ScopeError::Cancelled)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn scope_timeout_without_a_deadline_only_watches_cancellation() {
        let clock = TokioClock::new();
        let scope = Scope::never();

        assert_eq!(scope.tokio_timeout(&clock, async { 7 }).await, Ok(7));
    }

    #[tokio::test(start_paused = true)]
    async fn tokio_clock_and_deadline_follow_paused_time() {
        let clock = TokioClock::new();
        let deadline = tokio_support::deadline_after(&clock, Duration::from_secs(5));
        let Ok(Some(at)) = clock.to_tokio_instant(deadline) else {
            panic!("finite deadline should convert to a Tokio instant")
        };

        advance(Duration::from_secs(2)).await;

        assert_eq!(clock.now_ns(), 2_000_000_000);
        assert_eq!(at.duration_since(Instant::now()), Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn timeout_uses_the_same_tokio_deadline() {
        let clock = TokioClock::new();
        let deadline = tokio_support::deadline_after(&clock, Duration::from_secs(1));

        let result = tokio_support::timeout(&clock, deadline, async {
            tokio::time::sleep(Duration::from_secs(2)).await;
            7
        })
        .await;

        assert_eq!(result, Err(tokio_support::TimeoutError::Elapsed));
    }

    #[tokio::test(start_paused = true)]
    async fn unbounded_deadline_does_not_start_a_timeout() {
        let clock = TokioClock::new();

        assert_eq!(
            tokio_support::timeout(&clock, super::super::Deadline::never(), async { 7 }).await,
            Ok(7)
        );
    }
}

#[cfg(feature = "tokio-util")]
mod with_tokio_util {
    use super::super::cancel::CancelToken;
    use super::super::{Deadline, Scope};
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    #[tokio::test(start_paused = true)]
    async fn a_linked_token_follows_the_tokio_util_tree() {
        let upstream = CancellationToken::new();
        let token = CancelToken::linked_to(&upstream.child_token());
        let scope = Scope::with_token(Deadline::never(), token.child());

        assert!(!scope.is_cancelled());
        let waiter = {
            let scope = scope.clone();
            tokio::spawn(async move { scope.cancelled().await })
        };
        tokio::task::yield_now().await;

        upstream.cancel();

        assert!(scope.is_cancelled());
        assert!(
            tokio::time::timeout(Duration::from_secs(1), waiter)
                .await
                .is_ok()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn cancelling_a_linked_token_does_not_reach_upstream() {
        let upstream = CancellationToken::new();
        let token = CancelToken::linked_to(&upstream);

        token.cancel();

        assert!(token.is_cancelled());
        assert!(!upstream.is_cancelled());
    }

    #[tokio::test(start_paused = true)]
    async fn an_exported_token_follows_this_tree() {
        let root = CancelToken::root();
        let child = root.child();
        let exported = child.to_cancellation_token();

        assert!(!exported.is_cancelled());
        root.cancel();

        assert!(exported.is_cancelled());
        assert!(
            tokio::time::timeout(Duration::from_secs(1), exported.cancelled())
                .await
                .is_ok()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn cancelling_an_exported_token_cancels_the_source() {
        let token = CancelToken::root();
        let child = token.child();

        child.to_cancellation_token().cancel();

        assert!(child.is_cancelled());
        assert!(!token.is_cancelled());
    }
}
