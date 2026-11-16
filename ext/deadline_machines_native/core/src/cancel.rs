//! Cooperative cancellation that flows down a tree and never up.
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

/// What [`CancelToken::cancel`] did. Both answers are honoured: the token is
/// cancelled afterwards either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cancellation {
    /// This call cancelled the token and woke what waits on it.
    Propagated,
    /// The token, or an ancestor, was already cancelled; nothing changed.
    AlreadyCancelled,
}

#[derive(Debug)]
struct Node {
    cancelled: AtomicBool,
    parent: Option<Arc<Self>>,
    #[cfg(feature = "tokio")]
    notify: tokio::sync::Notify,
    #[cfg(feature = "tokio-util")]
    mirror: tokio_util::sync::CancellationToken,
}

impl Node {
    fn new(
        parent: Option<Arc<Self>>,
        #[cfg(feature = "tokio-util")] mirror: tokio_util::sync::CancellationToken,
    ) -> Arc<Self> {
        Arc::new(Self {
            cancelled: AtomicBool::new(false),
            parent,
            #[cfg(feature = "tokio")]
            notify: tokio::sync::Notify::new(),
            #[cfg(feature = "tokio-util")]
            mirror,
        })
    }

    fn is_cancelled(&self) -> bool {
        if self.cancelled.load(Ordering::Acquire) {
            return true;
        }
        #[cfg(feature = "tokio-util")]
        if self.mirror.is_cancelled() {
            return true;
        }
        self.parent
            .as_ref()
            .is_some_and(|parent| parent.is_cancelled())
    }
}

/// A node in a cancellation tree. Clones share the node; [`child`](Self::child)
/// makes a new one below it.
#[derive(Debug, Clone)]
pub struct CancelToken {
    node: Arc<Node>,
}

impl CancelToken {
    /// A token with no parent. Also the [`Default`].
    #[must_use]
    pub fn root() -> Self {
        Self {
            node: Node::new(
                None,
                #[cfg(feature = "tokio-util")]
                tokio_util::sync::CancellationToken::new(),
            ),
        }
    }

    /// A token cancelled whenever this one is, which can be cancelled on its
    /// own. A child of a cancelled token is born cancelled.
    #[must_use]
    pub fn child(&self) -> Self {
        Self {
            node: Node::new(
                Some(Arc::clone(&self.node)),
                #[cfg(feature = "tokio-util")]
                self.node.mirror.child_token(),
            ),
        }
    }

    /// Cancels this token and, through it, every descendant, now or later.
    /// A repeat, or a token whose ancestor is already cancelled, answers
    /// [`Cancellation::AlreadyCancelled`] and changes nothing.
    #[allow(
        clippy::must_use_candidate,
        reason = "cancelled for its effect; the outcome only says whether this call was first"
    )]
    pub fn cancel(&self) -> Cancellation {
        if self.is_cancelled() || self.node.cancelled.swap(true, Ordering::AcqRel) {
            return Cancellation::AlreadyCancelled;
        }
        #[cfg(feature = "tokio")]
        self.node.notify.notify_waiters();
        #[cfg(feature = "tokio-util")]
        self.node.mirror.cancel();
        Cancellation::Propagated
    }

    /// Whether this token or an ancestor was cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.node.is_cancelled()
    }

    /// Resolves once this token or any ancestor is cancelled. Cancel-safe:
    /// dropping the future and awaiting again loses nothing.
    #[cfg(feature = "tokio")]
    pub async fn cancelled(&self) {
        use core::future::{Future, poll_fn};
        use core::task::Poll;

        let mut waits = alloc::vec::Vec::new();
        let mut node = Some(&*self.node);
        while let Some(current) = node {
            waits.push(alloc::boxed::Box::pin(current.notify.notified()));
            node = current.parent.as_deref();
        }
        for wait in &mut waits {
            wait.as_mut().enable();
        }
        #[cfg(feature = "tokio-util")]
        let mut mirror = alloc::boxed::Box::pin(self.node.mirror.clone().cancelled_owned());
        if self.is_cancelled() {
            return;
        }
        poll_fn(|cx| {
            #[cfg(feature = "tokio-util")]
            if mirror.as_mut().poll(cx).is_ready() {
                return Poll::Ready(());
            }
            if waits
                .iter_mut()
                .any(|wait| wait.as_mut().poll(cx).is_ready())
            {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
    }

    /// A root token that is cancelled whenever `token` is, so a scope can
    /// sit under an existing `tokio_util` cancellation tree.
    #[cfg(feature = "tokio-util")]
    #[must_use]
    pub fn linked_to(token: &tokio_util::sync::CancellationToken) -> Self {
        Self {
            node: Node::new(None, token.child_token()),
        }
    }

    /// A `tokio_util` token that is cancelled whenever this one (or an
    /// ancestor) is. Cancelling it cancels this token too.
    #[cfg(feature = "tokio-util")]
    #[must_use]
    pub fn to_cancellation_token(&self) -> tokio_util::sync::CancellationToken {
        self.node.mirror.clone()
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::root()
    }
}

#[cfg(test)]
mod tests;
