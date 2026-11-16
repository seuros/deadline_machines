use super::*;

#[test]
fn cancellation_flows_down_but_not_up() {
    let root = CancelToken::root();
    let child = root.child();
    let grandchild = child.child();
    let sibling = root.child();

    assert_eq!(child.cancel(), Cancellation::Propagated);

    assert!(child.is_cancelled());
    assert!(grandchild.is_cancelled());
    assert!(!sibling.is_cancelled());
    assert!(!root.is_cancelled());

    root.cancel();
    assert!(sibling.is_cancelled());
    assert_eq!(
        CancelToken::default().child().cancel(),
        Cancellation::Propagated
    );
}

#[test]
fn a_repeat_or_an_inherited_cancellation_is_already_cancelled() {
    let root = CancelToken::root();
    let child = root.child();

    assert_eq!(root.cancel(), Cancellation::Propagated);
    assert_eq!(root.cancel(), Cancellation::AlreadyCancelled);
    let clone = root.clone();
    assert_eq!(clone.cancel(), Cancellation::AlreadyCancelled);
    // Born cancelled, and cancelling it changes nothing.
    assert_eq!(child.cancel(), Cancellation::AlreadyCancelled);
    assert!(!child.node.cancelled.load(Ordering::Acquire));
    assert_eq!(root.child().cancel(), Cancellation::AlreadyCancelled);
}

#[cfg(feature = "tokio")]
mod with_tokio {
    use super::*;
    use core::time::Duration;

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
}

#[cfg(feature = "tokio-util")]
mod with_tokio_util {
    use super::*;
    use core::time::Duration;
    use tokio_util::sync::CancellationToken;

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
        assert_eq!(child.cancel(), Cancellation::AlreadyCancelled);
    }
}
