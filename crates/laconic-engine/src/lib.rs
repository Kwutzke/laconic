//! The engine: the pipeline and nothing about any language.
//!
//! Every per-language fact reaches the engine through [`pack::Pack`]. Adding a language is adding
//! a pack, not touching the engine — charter constraint 4 — and what protects that is the pack
//! interface being enumerated and the trait being open, not a claim that languages are similar.

pub mod config;
pub mod dispatch;
pub mod domain;
pub mod exclude;
pub mod fix;
pub mod pack;
pub mod pipeline;
pub mod registry;
pub mod report;
pub mod rule;
pub mod rules;
pub mod run;

pub use config::{ConfigError, ConfigFile, Resolved, Thresholds, defaults_toml, discover};
pub use dispatch::{Rules, dispatch, is_autofixable};
pub use domain::{
    Attachment, Comment, CommentBlock, CommentKind, DeclaredSymbol, IgnoreDirective, Subject,
    Visibility,
};
pub use exclude::Config;
pub use fix::fix;
pub use pack::{BlankLinePolicy, DocComment, Pack};
pub use pipeline::{FileAnalysis, Skipped, analyse, resolve};
pub use registry::{Dispatch, Disposition, ErrorPolicy, FixShape, Registry, RuleEntry, Tier};
pub use report::{
    EXIT_CLEAN, EXIT_GATE, EXIT_USAGE, Finding, Report, UnreadableFile, WithheldNote,
};
pub use rule::{BlockContext, BlockRule, RuleHit, SubjectContext, SubjectRule};
pub use rules::structural::{
    ABSOLUTE_DOC_LINES, DENSITY_MAX_RATIO, DENSITY_MIN_COMMENT_LINES, DOC_LINES_PER_MEMBER,
};
pub use rules::{
    all_block_rules, all_subject_rules, structural_block_rules, structural_subject_rules,
    text_rules,
};
pub use run::{Run, languages};
