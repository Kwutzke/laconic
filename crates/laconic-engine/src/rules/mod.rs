//! The rules. Each decides only whether it fires; the registry decides the tier and the fix shape,
//! per comment kind.

pub mod matching;
pub mod structural;
pub mod text;

pub use structural::{structural_block_rules, structural_subject_rules};
pub use text::text_rules;

use crate::rule::{BlockRule, SubjectRule};

/// Every rule laconic ships, in one call. `ignoreReason` and `deadIgnore` are not here: they are
/// evaluated at reconcile from surviving ignore directives, not dispatched over blocks.
pub fn all_block_rules() -> Vec<Box<dyn BlockRule>> {
    let mut rules = text_rules();
    rules.extend(structural_block_rules());
    rules
}

pub fn all_subject_rules() -> Vec<Box<dyn SubjectRule>> {
    structural_subject_rules()
}
