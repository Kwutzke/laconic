//! Inner doc comment for the module.

// a line comment

/* a block comment */

/**/

/*******/

////////

/// Documents an exported function.
pub fn exported(a: i32) -> i32 {
    #[allow(clippy::let_and_return)]
    let x = a + 1; // a trailing comment
    println!("{x}");

    // a detached comment

    x
}

/** A block doc comment. */
pub(crate) fn crate_visible() {}

fn private() {}

pub(self) fn module_private() {}

pub struct Config;

struct Internal;
