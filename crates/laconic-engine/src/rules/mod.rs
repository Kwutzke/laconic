//! The rules. Each decides only whether it fires; the registry decides the tier and the fix shape,
//! per comment kind.

pub mod matching;
pub mod text;

pub use text::text_rules;
