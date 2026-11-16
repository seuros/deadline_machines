//! Thin binding to the `deadline-machines` budget arithmetic.
//!
//! `DeadlineMachines::Deadline` is a `Data` value defined in Ruby; every
//! operation on it lives here and goes through the core. The cancellation
//! tree, the ambient scope and the clock stay in Ruby: they hold Ruby
//! callbacks and fiber storage, which the core's `Arc` tree cannot.
mod init;

use core::time::Duration;

use deadline_machines::{Deadline, Refusal};
use magnus::{Error, RArray, RClass, RStruct, Ruby, TryConvert, Value, prelude::*};
use matryona_machines::ruby::{
    Class, Registry, Symbols, class_name, exact_fields_with, instant, nanoseconds,
};

matryona_machines::symbols! {
    /// A refusal as its Symbol: the snake case of the core variant.
    Refusals for Refusal as "refusal" {
        Exceeded => "exceeded",
        Cancelled => "cancelled",
    }
}

/// The gem's one `Data` class, resolved once at init and pinned.
static DEADLINE: Registry<Class> = Registry::new("deadline_machines");

fn class(ruby: &Ruby) -> Result<RClass, Error> {
    DEADLINE.get(ruby).map(|class| class.get(ruby))
}

/// `nil` for never, else an instant over the whole `Duration` range.
fn expiry(value: Value) -> Result<Option<Duration>, Error> {
    if value.is_nil() {
        Ok(None)
    } else {
        instant(value, "expires_at").map(Some)
    }
}

/// A `DeadlineMachines::Deadline`: exactly that class, its expiry read from
/// the struct (no reader runs) and checked again, since `Marshal.load` skips
/// `initialize`.
fn read(ruby: &Ruby, value: Value) -> Result<Deadline, Error> {
    let [expires_at] = exact_fields_with(ruby, value, class(ruby)?, || {
        format!(
            "expected a DeadlineMachines::Deadline, not {}",
            class_name(value)
        )
    })?;
    Ok(expiry(expires_at)?.map_or_else(Deadline::never, Deadline::at))
}

/// `__deadline_of`: the `Deadline` a `Scope` holds, read as every operation
/// reads one (exact class, the expiry from the struct, checked again) and
/// rebuilt from what was read, so neither a redefined `instance_of?` nor a
/// Marshal-edited value gets into a scope.
fn held(ruby: &Ruby, value: Value) -> Result<Value, Error> {
    build(ruby, read(ruby, value)?)
}

/// A frozen `Deadline` holding what the core answered. No Ruby code runs:
/// allocate, fill the member, freeze, which is all `Data#initialize` does
/// past the check, and the core's own answers need none.
fn build(ruby: &Ruby, deadline: Deadline) -> Result<Value, Error> {
    let instance = RStruct::try_convert(class(ruby)?.obj_alloc()?)?;
    let expires_at = deadline.expires_at().map_or_else(
        || ruby.qnil().as_value(),
        |at| nanoseconds(ruby, at).as_value(),
    );
    instance.aset(0, expires_at)?;
    instance.freeze();
    Ok(instance.as_value())
}

/// `__deadline`: what a `Deadline` may hold, `nil` or an instant.
fn validate(value: Value) -> Result<(), Error> {
    expiry(value).map(drop)
}

fn never(ruby: &Ruby) -> Result<Value, Error> {
    build(ruby, Deadline::never())
}

fn at(ruby: &Ruby, instant_at: Value) -> Result<Value, Error> {
    let at = instant(instant_at, "instant")?;
    build(ruby, Deadline::at(at))
}

fn after(ruby: &Ruby, now: Value, timeout: Value) -> Result<Value, Error> {
    let (now, timeout) = (instant(now, "now")?, instant(timeout, "timeout")?);
    build(ruby, Deadline::after(now, timeout))
}

fn remaining(ruby: &Ruby, this: Value, now: Value) -> Result<Value, Error> {
    let deadline = read(ruby, this)?;
    let left = deadline.remaining(instant(now, "now")?);
    Ok(left.map_or_else(
        || ruby.qnil().as_value(),
        |left| nanoseconds(ruby, left).as_value(),
    ))
}

fn is_expired(ruby: &Ruby, this: Value, now: Value) -> Result<bool, Error> {
    let deadline = read(ruby, this)?;
    Ok(deadline.is_expired(instant(now, "now")?))
}

fn check(ruby: &Ruby, this: Value, now: Value) -> Result<Value, Error> {
    let deadline = read(ruby, this)?;
    Ok(match deadline.check(instant(now, "now")?) {
        Ok(()) => ruby.qnil().as_value(),
        Err(refused) => Refusals::symbol(ruby, refused).as_value(),
    })
}

fn derive(ruby: &Ruby, this: Value, now: Value, timeout: Value) -> Result<Value, Error> {
    let deadline = read(ruby, this)?;
    let (now, timeout) = (instant(now, "now")?, instant(timeout, "timeout")?);
    build(ruby, deadline.derive(now, timeout))
}

fn extend_by(ruby: &Ruby, this: Value, by: Value) -> Result<Value, Error> {
    let deadline = read(ruby, this)?;
    build(ruby, deadline.extend(instant(by, "by")?))
}

fn tighten(ruby: &Ruby, this: Value, other: Value) -> Result<Value, Error> {
    let (deadline, other) = (read(ruby, this)?, read(ruby, other)?);
    build(ruby, deadline.tighten(other))
}

fn clamp_sleep(ruby: &Ruby, this: Value, now: Value, requested: Value) -> Result<Value, Error> {
    let deadline = read(ruby, this)?;
    let (now, requested) = (instant(now, "now")?, instant(requested, "requested")?);
    Ok(nanoseconds(ruby, deadline.clamp_sleep(now, requested)).as_value())
}

fn refusals(ruby: &Ruby) -> Result<RArray, Error> {
    let names = ruby.ary_new_capa(Refusals::ALL.len());
    for refused in Refusals::ALL {
        names.push(Refusals::symbol(ruby, *refused))?;
    }
    names.freeze();
    Ok(names)
}
