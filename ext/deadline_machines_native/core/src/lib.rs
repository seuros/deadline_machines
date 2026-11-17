//! Monotonic deadline arithmetic and cooperative cancellation trees.
//!
//! [`Deadline`] is `Copy`, allocation-free and reads no clock: the caller
//! passes `now` as a [`Duration`](core::time::Duration) since an epoch it
//! picks, and every span (timeout, extension, sleep) is a `Duration` too.
//! [`CancelToken`](cancel::CancelToken) and [`Scope`] share cancellation
//! through `Arc`, so they need the `alloc` feature. The `tokio` feature adds
//! a Tokio clock and timeout adapter, and the `continuous` feature a clock
//! that keeps counting through system suspend: the only places this crate
//! reads a clock.
#![no_std]
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(test)]
extern crate std;

/// Compiles and runs the README's examples as doctests.
#[cfg(all(doctest, feature = "tokio"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(feature = "alloc")]
pub mod cancel;
#[cfg(feature = "continuous")]
pub mod continuous;
mod deadline;
mod refusal;
#[cfg(feature = "alloc")]
mod scope;
#[cfg(feature = "tokio")]
pub mod tokio_support;

pub use deadline::Deadline;
pub use refusal::Refusal;
#[cfg(feature = "alloc")]
pub use scope::Scope;
