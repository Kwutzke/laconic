//! Inner doc comment for the module.

// SAFETY: the pointer is non-null because the caller checked it
// cbindgen:ignore
// grcov-excl-start
// coverage:ignore-start

/// Documents an exported function.
pub fn exported(a: i32) -> i32 {
    a + 1
}

/** A block doc comment. */
pub(crate) fn crate_visible() {}

pub(self) fn module_private() {}

fn private() {}

pub struct Config;

struct Internal;

pub enum Mode {
    On,
}

trait Hidden {}

const LIMIT: usize = 4;
