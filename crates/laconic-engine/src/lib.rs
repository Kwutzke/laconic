//! The engine: the pipeline and nothing about any language.
//!
//! Every per-language fact reaches the engine through [`pack::Pack`]. Adding a language is adding
//! a pack, not touching the engine — charter constraint 4 — and what protects that is the pack
//! interface being enumerated and the trait being open, not a claim that languages are similar.

pub mod domain;
pub mod exclude;
pub mod pack;
pub mod pipeline;

pub use domain::{
    Attachment, Comment, CommentBlock, CommentKind, DeclaredSymbol, IgnoreDirective, Subject,
    Visibility,
};
pub use exclude::Config;
pub use pack::{BlankLinePolicy, DocComment, Pack};
pub use pipeline::{FileAnalysis, Skipped, analyse, resolve};
