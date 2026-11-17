//! Private native entry point.
use magnus::{Error, IntoValue, RModule, Ruby, function, method, prelude::*};
use matryona_machines::ruby::Class;

use crate::{
    DEADLINE, after, at, check, clamp_sleep, derive, extend_by, held, is_expired, never, refusals,
    remaining, tighten, validate,
};

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module: RModule = ruby.define_module("DeadlineMachines")?;
    let deadline = Class::resolve(ruby, module, "Deadline")?;
    DEADLINE.set(ruby, deadline)?;
    module.const_set("REFUSALS", refusals(ruby)?.into_value_with(ruby))?;
    // The validation hook for the Data value; lib/ makes it private.
    module.define_singleton_method("__deadline", function!(validate, 1))?;
    module.define_singleton_method("__deadline_of", function!(held, 1))?;
    let class = deadline.get(ruby);
    class.define_singleton_method("never", function!(never, 0))?;
    class.define_singleton_method("at", function!(at, 1))?;
    class.define_singleton_method("after", function!(after, 2))?;
    class.define_method("remaining", method!(remaining, 1))?;
    class.define_method("expired?", method!(is_expired, 1))?;
    class.define_method("check", method!(check, 1))?;
    class.define_method("derive", method!(derive, 2))?;
    class.define_method("extend_by", method!(extend_by, 1))?;
    class.define_method("tighten", method!(tighten, 1))?;
    class.define_method("clamp_sleep", method!(clamp_sleep, 2))?;
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_vendor = "apple"
    ))]
    crate::continuous::define(module)?;
    Ok(())
}
