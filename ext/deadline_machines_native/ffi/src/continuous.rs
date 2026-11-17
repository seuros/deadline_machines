//! `DeadlineMachines::ContinuousClock`, the core's suspend-aware clock.
use std::sync::OnceLock;

use deadline_machines::continuous::ContinuousClock;
use magnus::{Error, RModule, function, prelude::*};

static CLOCK: OnceLock<ContinuousClock> = OnceLock::new();

fn now_ns() -> u64 {
    let now = CLOCK.get_or_init(ContinuousClock::new).now();
    u64::try_from(now.as_nanos()).unwrap_or(u64::MAX)
}

pub fn define(module: RModule) -> Result<(), Error> {
    CLOCK.get_or_init(ContinuousClock::new);
    let clock = module.define_module("ContinuousClock")?;
    clock.define_singleton_method("now_ns", function!(now_ns, 0))?;
    Ok(())
}
