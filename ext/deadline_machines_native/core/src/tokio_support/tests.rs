use super::*;
use tokio::time::advance;

#[tokio::test(start_paused = true)]
async fn scope_timeout_completes_within_budget() {
    let clock = TokioClock::new();
    let scope = Scope::new(Deadline::after(clock.now(), Duration::from_secs(5)));

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
    let clock = TokioClock::default();
    let scope = Scope::new(Deadline::after(clock.now(), Duration::from_secs(1)));

    let result = scope
        .tokio_timeout(&clock, async {
            tokio::time::sleep(Duration::from_secs(2)).await;
            7
        })
        .await;

    assert_eq!(result, Err(Refusal::Exceeded));
    assert_eq!(clock.now(), Duration::from_secs(1));
}

#[tokio::test(start_paused = true)]
async fn scope_timeout_reports_cancellation_from_an_ancestor() {
    let clock = TokioClock::new();
    let parent = Scope::new(Deadline::after(clock.now(), Duration::from_secs(5)));
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

    assert_eq!(result, Err(Refusal::Cancelled));
    assert!(canceller.await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn scope_timeout_ranks_cancellation_ahead_of_expiry() {
    let clock = TokioClock::new();
    let scope = Scope::new(Deadline::at(clock.now()));
    assert_eq!(
        scope.tokio_timeout(&clock, async { 7 }).await,
        Err(Refusal::Exceeded)
    );

    scope.cancel();
    assert_eq!(
        scope.tokio_timeout(&clock, async { 7 }).await,
        Err(Refusal::Cancelled)
    );
}

#[tokio::test(start_paused = true)]
async fn scope_timeout_without_a_deadline_only_watches_cancellation() {
    let clock = TokioClock::new();

    assert_eq!(
        Scope::never().tokio_timeout(&clock, async { 7 }).await,
        Ok(7)
    );
}

#[tokio::test(start_paused = true)]
async fn tokio_clock_and_deadline_follow_paused_time() {
    let clock = TokioClock::new();
    let deadline = Deadline::after(clock.now(), Duration::from_secs(5));
    let Some(at) = clock.to_tokio_instant(deadline) else {
        panic!("finite deadline should convert to a Tokio instant")
    };

    advance(Duration::from_secs(2)).await;

    assert_eq!(clock.now(), Duration::from_secs(2));
    assert_eq!(
        at.duration_since(TokioInstant::now()),
        Duration::from_secs(3)
    );
}

#[tokio::test(start_paused = true)]
async fn timeout_uses_the_same_tokio_deadline() {
    let clock = TokioClock::new();
    let deadline = Deadline::after(clock.now(), Duration::from_secs(1));

    let result = timeout(&clock, deadline, async {
        tokio::time::sleep(Duration::from_secs(2)).await;
        7
    })
    .await;

    assert_eq!(result, Err(Refusal::Exceeded));
}

/// Unbounded, or past what Tokio can represent: no timer starts either way.
#[tokio::test(start_paused = true)]
async fn a_deadline_that_never_runs_out_starts_no_timer() {
    let clock = TokioClock::new();
    let past_tokio = Deadline::at(Duration::MAX);

    assert_eq!(clock.to_tokio_instant(Deadline::never()), None);
    assert_eq!(clock.to_tokio_instant(past_tokio), None);
    assert_eq!(timeout(&clock, Deadline::never(), async { 7 }).await, Ok(7));
    assert_eq!(timeout(&clock, past_tokio, async { 7 }).await, Ok(7));
}

#[cfg(feature = "tokio-util")]
#[tokio::test(start_paused = true)]
async fn a_linked_scope_follows_the_tokio_util_tree() {
    use crate::cancel::CancelToken;
    use tokio_util::sync::CancellationToken;

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
